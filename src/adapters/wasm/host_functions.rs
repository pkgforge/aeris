use std::process::Command;

use serde::{Deserialize, Serialize};
use wasmtime::{AsContext, AsContextMut, Caller, Linker};

use super::abi;
use super::host::HostState;
use super::memory;

#[derive(Deserialize)]
struct ExecRequest {
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

#[derive(Serialize)]
struct ExecResponse {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Serialize)]
struct HttpResponse {
    status: u16,
    body: String,
}

#[derive(Serialize)]
struct FsReadResponse {
    content: String,
}

pub fn register_host_functions(linker: &mut Linker<HostState>) -> Result<(), String> {
    register_host_log(linker)?;
    register_host_exec(linker)?;
    register_host_fs_read(linker)?;
    register_host_fs_write(linker)?;
    register_host_fs_exists(linker)?;
    register_host_http_get(linker)?;
    register_host_report_progress(linker)?;
    Ok(())
}

fn register_host_log(linker: &mut Linker<HostState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::IMPORT_NAMESPACE,
            abi::IMPORT_LOG,
            |mut caller: Caller<'_, HostState>, level: i32, ptr: i32, len: i32| {
                let memory = match caller.get_export(abi::EXPORT_MEMORY) {
                    Some(wasmtime::Extern::Memory(m)) => m,
                    _ => return,
                };

                let msg =
                    match memory::read_string(&memory, caller.as_context(), ptr as u32, len as u32)
                    {
                        Ok(s) => s,
                        Err(_) => return,
                    };

                let adapter_id = caller.data().adapter_id.clone();
                match level {
                    abi::LOG_ERROR => log::error!("[plugin:{adapter_id}] {msg}"),
                    abi::LOG_WARN => log::warn!("[plugin:{adapter_id}] {msg}"),
                    abi::LOG_INFO => log::info!("[plugin:{adapter_id}] {msg}"),
                    abi::LOG_DEBUG => log::debug!("[plugin:{adapter_id}] {msg}"),
                    _ => log::debug!("[plugin:{adapter_id}] {msg}"),
                }
            },
        )
        .map_err(|e| format!("Failed to register {}: {e}", abi::IMPORT_LOG))?;
    Ok(())
}

fn register_host_exec(linker: &mut Linker<HostState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::IMPORT_NAMESPACE,
            abi::IMPORT_EXEC,
            |mut caller: Caller<'_, HostState>, cmd_ptr: i32, cmd_len: i32| -> i64 {
                let result = exec_impl(&mut caller, cmd_ptr as u32, cmd_len as u32);
                match result {
                    Ok(fat) => fat,
                    Err(e) => {
                        log::error!("[plugin:{}] host_exec error: {e}", caller.data().adapter_id);
                        0i64
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to register {}: {e}", abi::IMPORT_EXEC))?;
    Ok(())
}

fn exec_impl(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> Result<i64, String> {
    let memory = caller
        .get_export(abi::EXPORT_MEMORY)
        .and_then(|e| e.into_memory())
        .ok_or("missing memory export")?;

    let json_str = memory::read_string(&memory, caller.as_context(), ptr, len)?;
    let req: ExecRequest =
        serde_json::from_str(&json_str).map_err(|e| format!("Invalid exec request JSON: {e}"))?;

    let resolved_cmd = caller.data().validate_command(&req.command)?;

    let output = Command::new(&resolved_cmd)
        .args(&req.args)
        .output()
        .map_err(|e| format!("Failed to execute {resolved_cmd}: {e}"))?;

    let response = ExecResponse {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };

    let response_json = serde_json::to_string(&response)
        .map_err(|e| format!("Failed to serialize response: {e}"))?;

    let allocate = caller
        .get_export(abi::EXPORT_ALLOCATE)
        .and_then(|e| e.into_func())
        .ok_or("missing allocate export")?;

    let (w_ptr, w_len) = memory::write_string_caller(
        &mut caller.as_context_mut(),
        &memory,
        &allocate,
        &response_json,
    )?;

    Ok(((w_ptr as i64) << 32) | (w_len as i64))
}

fn register_host_fs_read(linker: &mut Linker<HostState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::IMPORT_NAMESPACE,
            abi::IMPORT_FS_READ,
            |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i64 {
                match fs_read_impl(&mut caller, path_ptr as u32, path_len as u32) {
                    Ok(fat) => fat,
                    Err(e) => {
                        log::error!(
                            "[plugin:{}] host_fs_read error: {e}",
                            caller.data().adapter_id
                        );
                        0i64
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to register {}: {e}", abi::IMPORT_FS_READ))?;
    Ok(())
}

fn fs_read_impl(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> Result<i64, String> {
    let memory = caller
        .get_export(abi::EXPORT_MEMORY)
        .and_then(|e| e.into_memory())
        .ok_or("missing memory export")?;

    let path_str = memory::read_string(&memory, caller.as_context(), ptr, len)?;
    let validated = caller.data().validate_path(&path_str)?;

    let content =
        std::fs::read_to_string(&validated).map_err(|e| format!("Failed to read file: {e}"))?;

    let response = FsReadResponse { content };
    let response_json =
        serde_json::to_string(&response).map_err(|e| format!("Failed to serialize: {e}"))?;

    let allocate = caller
        .get_export(abi::EXPORT_ALLOCATE)
        .and_then(|e| e.into_func())
        .ok_or("missing allocate export")?;

    let (w_ptr, w_len) = memory::write_string_caller(
        &mut caller.as_context_mut(),
        &memory,
        &allocate,
        &response_json,
    )?;

    Ok(((w_ptr as i64) << 32) | (w_len as i64))
}

fn register_host_fs_write(linker: &mut Linker<HostState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::IMPORT_NAMESPACE,
            abi::IMPORT_FS_WRITE,
            |mut caller: Caller<'_, HostState>,
             path_ptr: i32,
             path_len: i32,
             data_ptr: i32,
             data_len: i32|
             -> i32 {
                match fs_write_impl(
                    &mut caller,
                    path_ptr as u32,
                    path_len as u32,
                    data_ptr as u32,
                    data_len as u32,
                ) {
                    Ok(()) => 0,
                    Err(e) => {
                        log::error!(
                            "[plugin:{}] host_fs_write error: {e}",
                            caller.data().adapter_id
                        );
                        -1
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to register {}: {e}", abi::IMPORT_FS_WRITE))?;
    Ok(())
}

fn fs_write_impl(
    caller: &mut Caller<'_, HostState>,
    path_ptr: u32,
    path_len: u32,
    data_ptr: u32,
    data_len: u32,
) -> Result<(), String> {
    let memory = caller
        .get_export(abi::EXPORT_MEMORY)
        .and_then(|e| e.into_memory())
        .ok_or("missing memory export")?;

    let path_str = memory::read_string(&memory, caller.as_context(), path_ptr, path_len)?;
    let validated = caller.data().validate_path(&path_str)?;

    let data = memory::read_string(&memory, caller.as_context(), data_ptr, data_len)?;

    if let Some(parent) = validated.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }

    std::fs::write(&validated, data.as_bytes()).map_err(|e| format!("Failed to write file: {e}"))
}

fn register_host_fs_exists(linker: &mut Linker<HostState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::IMPORT_NAMESPACE,
            abi::IMPORT_FS_EXISTS,
            |mut caller: Caller<'_, HostState>, path_ptr: i32, path_len: i32| -> i32 {
                let memory = match caller.get_export(abi::EXPORT_MEMORY) {
                    Some(wasmtime::Extern::Memory(m)) => m,
                    _ => return -1,
                };

                let path_str = match memory::read_string(
                    &memory,
                    caller.as_context(),
                    path_ptr as u32,
                    path_len as u32,
                ) {
                    Ok(s) => s,
                    Err(_) => return -1,
                };

                match caller.data().validate_path(&path_str) {
                    Ok(validated) => {
                        if validated.exists() {
                            1
                        } else {
                            0
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[plugin:{}] host_fs_exists error: {e}",
                            caller.data().adapter_id
                        );
                        -1
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to register {}: {e}", abi::IMPORT_FS_EXISTS))?;
    Ok(())
}

fn register_host_http_get(linker: &mut Linker<HostState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::IMPORT_NAMESPACE,
            abi::IMPORT_HTTP_GET,
            |mut caller: Caller<'_, HostState>, url_ptr: i32, url_len: i32| -> i64 {
                match http_get_impl(&mut caller, url_ptr as u32, url_len as u32) {
                    Ok(fat) => fat,
                    Err(e) => {
                        log::error!(
                            "[plugin:{}] host_http_get error: {e}",
                            caller.data().adapter_id
                        );
                        0i64
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to register {}: {e}", abi::IMPORT_HTTP_GET))?;
    Ok(())
}

fn http_get_impl(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> Result<i64, String> {
    if !caller.data().has_network_permission() {
        return Err(format!(
            "Network access denied for plugin '{}'",
            caller.data().adapter_id
        ));
    }

    let memory = caller
        .get_export(abi::EXPORT_MEMORY)
        .and_then(|e| e.into_memory())
        .ok_or("missing memory export")?;

    let url = memory::read_string(&memory, caller.as_context(), ptr, len)?;

    let resp = ureq::get(&url)
        .call()
        .map_err(|e| format!("HTTP GET failed: {e}"))?;

    let status = resp.status();
    let body = resp
        .into_body()
        .read_to_string()
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    let response = HttpResponse {
        status: status.as_u16(),
        body,
    };
    let response_json =
        serde_json::to_string(&response).map_err(|e| format!("Failed to serialize: {e}"))?;

    let allocate = caller
        .get_export(abi::EXPORT_ALLOCATE)
        .and_then(|e| e.into_func())
        .ok_or("missing allocate export")?;

    let (w_ptr, w_len) = memory::write_string_caller(
        &mut caller.as_context_mut(),
        &memory,
        &allocate,
        &response_json,
    )?;

    Ok(((w_ptr as i64) << 32) | (w_len as i64))
}

fn register_host_report_progress(linker: &mut Linker<HostState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::IMPORT_NAMESPACE,
            abi::IMPORT_REPORT_PROGRESS,
            |mut caller: Caller<'_, HostState>, json_ptr: i32, json_len: i32| {
                let memory = match caller.get_export(abi::EXPORT_MEMORY) {
                    Some(wasmtime::Extern::Memory(m)) => m,
                    _ => return,
                };

                let json_str = match memory::read_string(
                    &memory,
                    caller.as_context(),
                    json_ptr as u32,
                    json_len as u32,
                ) {
                    Ok(s) => s,
                    Err(_) => return,
                };

                // Log the progress event; actual channel forwarding is a follow-up
                let adapter_id = caller.data().adapter_id.clone();
                log::debug!("[plugin:{adapter_id}] progress: {json_str}");
            },
        )
        .map_err(|e| format!("Failed to register {}: {e}", abi::IMPORT_REPORT_PROGRESS))?;
    Ok(())
}
