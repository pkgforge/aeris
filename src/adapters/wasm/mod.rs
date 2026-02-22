pub mod abi;
pub mod adapter;
pub mod host;
pub mod host_functions;
pub mod manifest;
pub mod memory;

pub use adapter::WasmAdapter;
pub use manifest::{PluginManifest, discover_plugins};

use crate::core::adapter::AdapterError;

pub fn load_all_plugins() -> Vec<Result<WasmAdapter, AdapterError>> {
    discover_plugins()
        .into_iter()
        .map(|(path, _manifest)| WasmAdapter::load(path))
        .collect()
}
