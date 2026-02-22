use std::path::PathBuf;
use std::sync::Arc;

use wasmtime::{AsContextMut, Engine, Linker, Module, Store};

use crate::core::adapter::{
    Adapter, AdapterError, AdapterInfo, HealthStatus, ProgressSender, Result,
};
use crate::core::capabilities::Capabilities;
use crate::core::config::{AdapterConfig, ConfigSchema};
use crate::core::package::{InstallResult, InstalledPackage, Package, PackageDetail, Update};
use crate::core::privilege::PackageMode;
use crate::core::profile::Profile;
use crate::core::repository::Repository;

use super::abi;
use super::host::HostState;
use super::host_functions;
use super::manifest::PluginManifest;
use super::memory;

const FUEL_LIMIT: u64 = 10_000_000;

#[derive(serde::Serialize)]
struct PackagesWithMode {
    packages: Vec<Package>,
    mode: String,
}

#[derive(serde::Serialize)]
struct ModeInput {
    mode: String,
}

fn mode_str(mode: PackageMode) -> String {
    match mode {
        PackageMode::User => "user".into(),
        PackageMode::System => "system".into(),
    }
}

pub struct WasmAdapter {
    engine: Arc<Engine>,
    module: Arc<Module>,
    linker: Arc<Linker<HostState>>,
    host_state: HostState,
    info: AdapterInfo,
    capabilities: Capabilities,
}

impl WasmAdapter {
    pub fn load(plugin_dir: PathBuf) -> Result<Self> {
        let manifest_path = plugin_dir.join("manifest.toml");
        let wasm_path = plugin_dir.join("plugin.wasm");

        let manifest: PluginManifest =
            super::manifest::load_manifest(&manifest_path).map_err(AdapterError::PluginError)?;

        let wasm_bytes = std::fs::read(&wasm_path).map_err(|e| {
            AdapterError::PluginError(format!("Failed to read {}: {e}", wasm_path.display()))
        })?;

        let mut config = wasmtime::Config::new();
        config.consume_fuel(true);
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)
            .map_err(|e| AdapterError::PluginError(format!("Failed to create WASM engine: {e}")))?;

        let module = Module::new(&engine, &wasm_bytes).map_err(|e| {
            AdapterError::PluginError(format!("Failed to compile WASM module: {e}"))
        })?;

        let mut linker = Linker::new(&engine);
        host_functions::register_host_functions(&mut linker).map_err(AdapterError::PluginError)?;

        let host_state = HostState::new(manifest.adapter.id.clone(), manifest.permissions.clone());

        // Ensure plugin data directory exists
        std::fs::create_dir_all(&host_state.data_dir).map_err(|e| {
            AdapterError::PluginError(format!(
                "Failed to create plugin data dir {}: {e}",
                host_state.data_dir.display()
            ))
        })?;

        // Create a temporary store to initialize and query the plugin
        let mut store = Store::new(&engine, host_state.clone());
        store
            .set_fuel(FUEL_LIMIT)
            .map_err(|e| AdapterError::PluginError(format!("Failed to set fuel: {e}")))?;

        let instance = linker.instantiate(&mut store, &module).map_err(|e| {
            AdapterError::PluginError(format!("Failed to instantiate WASM module: {e}"))
        })?;

        // Call adapter_init()
        if let Ok(init_fn) =
            instance.get_typed_func::<(), ()>(store.as_context_mut(), abi::EXPORT_INIT)
        {
            init_fn
                .call(&mut store, ())
                .map_err(|e| AdapterError::PluginError(format!("adapter_init failed: {e}")))?;
        }

        // Call adapter_info() -> i64 (fat ptr to JSON)
        let info_fn = instance
            .get_typed_func::<(), i64>(store.as_context_mut(), abi::EXPORT_INFO)
            .map_err(|e| {
                AdapterError::PluginError(format!("Plugin missing adapter_info export: {e}"))
            })?;

        let info_result = info_fn
            .call(&mut store, ())
            .map_err(|e| AdapterError::PluginError(format!("adapter_info failed: {e}")))?;

        let wasm_memory = memory::get_memory(&instance, store.as_context_mut())
            .map_err(AdapterError::PluginError)?;

        #[derive(serde::Deserialize)]
        struct PluginInfo {
            #[serde(default)]
            id: Option<String>,
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            version: Option<String>,
            #[serde(default)]
            description: Option<String>,
            #[serde(default)]
            icon: Option<String>,
        }

        let plugin_info: PluginInfo = memory::read_result_json(&wasm_memory, &store, info_result)
            .map_err(AdapterError::PluginError)?;

        // Call adapter_capabilities() -> i64 (fat ptr to JSON)
        let capabilities = if let Ok(cap_fn) =
            instance.get_typed_func::<(), i64>(store.as_context_mut(), abi::EXPORT_CAPABILITIES)
        {
            let cap_result = cap_fn.call(&mut store, ()).map_err(|e| {
                AdapterError::PluginError(format!("adapter_capabilities failed: {e}"))
            })?;

            memory::read_result_json::<Capabilities>(&wasm_memory, &store, cap_result)
                .unwrap_or(manifest.capabilities)
        } else {
            manifest.capabilities
        };

        let adapter_info = AdapterInfo {
            id: plugin_info
                .id
                .unwrap_or_else(|| manifest.adapter.id.clone()),
            name: plugin_info
                .name
                .unwrap_or_else(|| manifest.adapter.name.clone()),
            version: plugin_info
                .version
                .unwrap_or_else(|| manifest.adapter.version.clone()),
            capabilities,
            enabled: true,
            is_builtin: false,
            plugin_path: Some(plugin_dir),
            description: plugin_info
                .description
                .unwrap_or_else(|| manifest.adapter.description.clone()),
            icon: plugin_info.icon,
        };

        let engine = Arc::new(engine);
        let module = Arc::new(module);
        let linker = Arc::new(linker);

        Ok(Self {
            engine,
            module,
            linker,
            host_state,
            info: adapter_info,
            capabilities,
        })
    }

    /// Create a fresh Store + Instance for a single operation.
    fn fresh_instance(
        engine: &Engine,
        module: &Module,
        linker: &Linker<HostState>,
        host_state: HostState,
    ) -> std::result::Result<(Store<HostState>, wasmtime::Instance), AdapterError> {
        let mut store = Store::new(engine, host_state);
        store
            .set_fuel(FUEL_LIMIT)
            .map_err(|e| AdapterError::PluginError(format!("Failed to set fuel: {e}")))?;

        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|e| AdapterError::PluginError(format!("Failed to instantiate: {e}")))?;

        Ok((store, instance))
    }

    /// Call an export that takes no arguments and returns JSON via fat pointer.
    async fn call_no_args<T: serde::de::DeserializeOwned + Send + 'static>(
        &self,
        export_name: &'static str,
    ) -> Result<T> {
        let engine = self.engine.clone();
        let module = self.module.clone();
        let linker = self.linker.clone();
        let host_state = self.host_state.clone();

        tokio::task::spawn_blocking(move || {
            let (mut store, instance) =
                Self::fresh_instance(&engine, &module, &linker, host_state)?;

            let func = instance
                .get_typed_func::<(), i64>(store.as_context_mut(), export_name)
                .map_err(|e| {
                    AdapterError::PluginError(format!("Missing export '{export_name}': {e}"))
                })?;

            let result = func
                .call(&mut store, ())
                .map_err(|e| AdapterError::PluginError(format!("{export_name} failed: {e}")))?;

            let mem = memory::get_memory(&instance, store.as_context_mut())
                .map_err(AdapterError::PluginError)?;

            memory::read_result_json(&mem, &store, result).map_err(AdapterError::PluginError)
        })
        .await
        .map_err(|e| AdapterError::Other(format!("Task join error: {e}")))?
    }

    /// Call an export that takes a single JSON string argument and returns JSON.
    async fn call_with_json<
        I: serde::Serialize + Send + 'static,
        O: serde::de::DeserializeOwned + Send + 'static,
    >(
        &self,
        export_name: &'static str,
        input: I,
    ) -> Result<O> {
        let engine = self.engine.clone();
        let module = self.module.clone();
        let linker = self.linker.clone();
        let host_state = self.host_state.clone();

        tokio::task::spawn_blocking(move || {
            let (mut store, instance) =
                Self::fresh_instance(&engine, &module, &linker, host_state)?;

            let input_json = serde_json::to_string(&input).map_err(|e| {
                AdapterError::PluginError(format!("Failed to serialize input: {e}"))
            })?;

            let (ptr, len) = memory::write_string(&instance, &mut store, &input_json)
                .map_err(AdapterError::PluginError)?;

            let func = instance
                .get_typed_func::<(i32, i32), i64>(store.as_context_mut(), export_name)
                .map_err(|e| {
                    AdapterError::PluginError(format!("Missing export '{export_name}': {e}"))
                })?;

            let result = func
                .call(&mut store, (ptr as i32, len as i32))
                .map_err(|e| AdapterError::PluginError(format!("{export_name} failed: {e}")))?;

            let mem = memory::get_memory(&instance, store.as_context_mut())
                .map_err(AdapterError::PluginError)?;

            memory::read_result_json(&mem, &store, result).map_err(AdapterError::PluginError)
        })
        .await
        .map_err(|e| AdapterError::Other(format!("Task join error: {e}")))?
    }
}

#[async_trait::async_trait]
impl Adapter for WasmAdapter {
    fn info(&self) -> &AdapterInfo {
        &self.info
    }

    fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        mode: PackageMode,
    ) -> Result<Vec<Package>> {
        #[derive(serde::Serialize)]
        struct SearchInput {
            query: String,
            limit: Option<usize>,
            mode: String,
        }

        let input = SearchInput {
            query: query.to_string(),
            limit,
            mode: mode_str(mode),
        };

        self.call_with_json(abi::EXPORT_SEARCH, input).await
    }

    async fn package_detail(&self, _package_id: &str) -> Result<PackageDetail> {
        Err(AdapterError::NotSupported)
    }

    async fn install(
        &self,
        packages: &[Package],
        _progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        let input = PackagesWithMode {
            packages: packages.to_vec(),
            mode: mode_str(mode),
        };
        self.call_with_json(abi::EXPORT_INSTALL, input).await
    }

    async fn remove(
        &self,
        packages: &[Package],
        _progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<()> {
        let input = PackagesWithMode {
            packages: packages.to_vec(),
            mode: mode_str(mode),
        };
        let _: serde_json::Value = self.call_with_json(abi::EXPORT_REMOVE, input).await?;
        Ok(())
    }

    async fn update(
        &self,
        packages: &[Package],
        _progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        let input = PackagesWithMode {
            packages: packages.to_vec(),
            mode: mode_str(mode),
        };
        self.call_with_json(abi::EXPORT_UPDATE, input).await
    }

    async fn list_installed(&self, mode: PackageMode) -> Result<Vec<InstalledPackage>> {
        let input = ModeInput {
            mode: mode_str(mode),
        };
        self.call_with_json(abi::EXPORT_LIST_INSTALLED, input).await
    }

    async fn list_updates(&self, mode: PackageMode) -> Result<Vec<Update>> {
        let input = ModeInput {
            mode: mode_str(mode),
        };
        self.call_with_json(abi::EXPORT_LIST_UPDATES, input).await
    }

    async fn sync(&self, _progress: Option<ProgressSender>) -> Result<()> {
        let _: serde_json::Value = self.call_no_args(abi::EXPORT_SYNC).await?;
        Ok(())
    }

    async fn list_repositories(&self) -> Result<Vec<Repository>> {
        self.call_no_args(abi::EXPORT_LIST_REPOS).await
    }

    async fn get_config(&self) -> Result<AdapterConfig> {
        self.call_no_args(abi::EXPORT_GET_CONFIG).await
    }

    async fn set_config(&self, config: &AdapterConfig) -> Result<()> {
        let config = config.clone();
        let _: serde_json::Value = self.call_with_json(abi::EXPORT_SET_CONFIG, config).await?;
        Ok(())
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        self.call_no_args(abi::EXPORT_HEALTH_CHECK).await
    }

    async fn list_profiles(&self) -> Result<Vec<Profile>> {
        Err(AdapterError::NotSupported)
    }

    async fn active_profile(&self) -> Result<Profile> {
        Err(AdapterError::NotSupported)
    }

    async fn switch_profile(&self, _profile_id: &str) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    async fn add_repository(&self, _repo: &Repository) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    async fn remove_repository(&self, _repo_name: &str) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    fn config_schema(&self) -> Option<ConfigSchema> {
        None
    }

    fn initial_config(&self) -> Option<AdapterConfig> {
        None
    }
}
