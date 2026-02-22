use serde::de::DeserializeOwned;
use wasmtime::{AsContextMut, Instance, Memory, Store, StoreContextMut};

use super::abi;
use super::host::HostState;

/// Decode a fat i64 return value into (ptr, len).
/// High 32 bits = ptr, low 32 bits = len.
pub fn decode_fat_ptr(val: i64) -> (u32, u32) {
    let ptr = (val >> 32) as u32;
    let len = (val & 0xFFFF_FFFF) as u32;
    (ptr, len)
}

/// Read a UTF-8 string from WASM linear memory at the given offset and length.
pub fn read_string(
    memory: &Memory,
    store: impl wasmtime::AsContext<Data = HostState>,
    ptr: u32,
    len: u32,
) -> Result<String, String> {
    let data = memory.data(&store);
    let start = ptr as usize;
    let end = start.checked_add(len as usize).ok_or("Memory overflow")?;

    if end > data.len() {
        return Err(format!(
            "Out of bounds read: offset={start}, len={len}, memory_size={}",
            data.len()
        ));
    }

    String::from_utf8(data[start..end].to_vec())
        .map_err(|e| format!("Invalid UTF-8 from WASM memory: {e}"))
}

/// Write a string into WASM linear memory by calling the plugin's `allocate` export.
/// Returns (ptr, len) of the written data.
pub fn write_string(
    instance: &Instance,
    store: &mut Store<HostState>,
    s: &str,
) -> Result<(u32, u32), String> {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;

    let allocate = instance
        .get_typed_func::<u32, u32>(store.as_context_mut(), abi::EXPORT_ALLOCATE)
        .map_err(|e| format!("Plugin missing '{}' export: {e}", abi::EXPORT_ALLOCATE))?;

    let ptr = allocate
        .call(store.as_context_mut(), len)
        .map_err(|e| format!("allocate({len}) failed: {e}"))?;

    let memory = get_memory(instance, store.as_context_mut())?;
    let data = memory.data_mut(store.as_context_mut());

    let start = ptr as usize;
    let end = start
        .checked_add(len as usize)
        .ok_or("Memory overflow during write")?;

    if end > data.len() {
        return Err(format!(
            "Out of bounds write: offset={start}, len={len}, memory_size={}",
            data.len()
        ));
    }

    data[start..end].copy_from_slice(bytes);
    Ok((ptr, len))
}

/// Call the plugin's `deallocate` export to free memory.
pub fn call_deallocate(
    instance: &Instance,
    store: &mut Store<HostState>,
    ptr: u32,
    len: u32,
) -> Result<(), String> {
    let deallocate = instance
        .get_typed_func::<(u32, u32), ()>(store.as_context_mut(), abi::EXPORT_DEALLOCATE)
        .map_err(|e| format!("Plugin missing '{}' export: {e}", abi::EXPORT_DEALLOCATE))?;

    deallocate
        .call(store.as_context_mut(), (ptr, len))
        .map_err(|e| format!("deallocate({ptr}, {len}) failed: {e}"))?;

    Ok(())
}

/// Read a fat-pointer i64 result, extract the JSON string, and deserialize it.
pub fn read_result_json<T: DeserializeOwned>(
    memory: &Memory,
    store: impl wasmtime::AsContext<Data = HostState>,
    fat_ptr: i64,
) -> Result<T, String> {
    let (ptr, len) = decode_fat_ptr(fat_ptr);

    if ptr == 0 && len == 0 {
        return Err("Plugin returned null pointer".into());
    }

    let json_str = read_string(memory, store, ptr, len)?;
    serde_json::from_str(&json_str).map_err(|e| format!("Failed to deserialize JSON result: {e}"))
}

/// Get the "memory" export from a WASM instance.
pub fn get_memory(
    instance: &Instance,
    mut store: impl AsContextMut<Data = HostState>,
) -> Result<Memory, String> {
    instance
        .get_memory(store.as_context_mut(), abi::EXPORT_MEMORY)
        .ok_or_else(|| format!("Plugin missing '{}' export", abi::EXPORT_MEMORY))
}

/// Write a string into WASM memory via the `allocate` export, using a Caller context.
/// Used from within host functions where we have a Caller instead of &mut Store.
pub fn write_string_caller(
    caller: &mut StoreContextMut<'_, HostState>,
    memory: &Memory,
    allocate: &wasmtime::Func,
    s: &str,
) -> Result<(u32, u32), String> {
    let bytes = s.as_bytes();
    let len = bytes.len() as u32;

    let mut results = [wasmtime::Val::I32(0)];
    allocate
        .call(
            &mut *caller,
            &[wasmtime::Val::I32(len as i32)],
            &mut results,
        )
        .map_err(|e| format!("allocate({len}) failed: {e}"))?;

    let ptr = match results[0] {
        wasmtime::Val::I32(v) => v as u32,
        _ => return Err("allocate returned unexpected type".into()),
    };

    let data = memory.data_mut(&mut *caller);
    let start = ptr as usize;
    let end = start
        .checked_add(len as usize)
        .ok_or("Memory overflow during write")?;

    if end > data.len() {
        return Err(format!(
            "Out of bounds write: offset={start}, len={len}, memory_size={}",
            data.len()
        ));
    }

    data[start..end].copy_from_slice(bytes);
    Ok((ptr, len))
}
