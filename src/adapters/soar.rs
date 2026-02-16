use std::sync::Arc;

use soar_config::config::Config;
use soar_events::{ChannelSink, EventSinkHandle, SoarEvent};
use soar_operations::{
    InstallOptions, RemoveResolveResult, ResolveResult, SoarContext, install, remove, update,
};

use crate::core::{
    adapter::{Adapter, AdapterError, AdapterInfo, ProgressSender, Result},
    capabilities::Capabilities,
    config::{AdapterConfig, ConfigField, ConfigFieldType, ConfigSchema, ConfigValue},
    package::{InstallResult, InstalledPackage, Package, PackageDetail, Update},
    privilege::PackageMode,
    profile::Profile,
    repository::Repository,
};

pub struct SoarAdapter {
    user_ctx: SoarContext,
    has_system: bool,
    info: AdapterInfo,
}

impl SoarAdapter {
    pub fn repo_count(&self) -> usize {
        self.user_ctx.config().repositories.len()
    }

    pub fn supports_system(&self) -> bool {
        self.has_system
    }

    pub async fn install_package(
        &self,
        query: &str,
        mode: PackageMode,
        settings: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        if mode == PackageMode::System {
            let executable_path = settings.get("executable_path").map(|s| s.as_str());
            return self.install_system_package(query, executable_path).await;
        }

        let options = InstallOptions::default();
        let results = install::resolve_packages(&self.user_ctx, &[query.to_string()], &options)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        let mut targets = Vec::new();
        for result in results {
            match result {
                ResolveResult::Resolved(t) => targets.extend(t),
                ResolveResult::AlreadyInstalled { pkg_name, .. } => {
                    return Err(AdapterError::Other(format!(
                        "{pkg_name} is already installed"
                    )));
                }
                ResolveResult::NotFound(q) => {
                    return Err(AdapterError::Other(format!("Package not found: {q}")));
                }
                ResolveResult::Ambiguous(amb) => {
                    return Err(AdapterError::Other(format!(
                        "Ambiguous package query: {}",
                        amb.query
                    )));
                }
            }
        }

        if targets.is_empty() {
            return Err(AdapterError::Other("No packages to install".into()));
        }

        let report = install::perform_installation(&self.user_ctx, targets, &options)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    async fn install_system_package(
        &self,
        query: &str,
        configured_path: Option<&str>,
    ) -> Result<()> {
        let soar_path = configured_path
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .unwrap_or_else(Self::find_executable_path);

        let output = crate::core::privilege::run_elevated(
            PackageMode::System,
            &soar_path,
            &["install", "--system", query],
        )
        .map_err(|e| AdapterError::Other(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::Other(format!(
                "Installation failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn find_executable_path() -> String {
        let candidates = [
            "/usr/local/bin/soar",
            "/usr/bin/soar",
            &format!(
                "{}/.cargo/bin/soar",
                std::env::var("HOME").unwrap_or_default()
            ),
            &format!(
                "{}/.local/bin/soar",
                std::env::var("HOME").unwrap_or_default()
            ),
        ];

        for path in &candidates {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }

        "soar".to_string()
    }

    async fn list_installed_system(&self) -> Result<Vec<InstalledPackage>> {
        Ok(vec![])
    }

    async fn remove_system_package(&self, packages: &[Package]) -> Result<()> {
        let soar_path = Self::find_executable_path();
        let pkg_names: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
        let mut args = vec!["remove", "--system"];
        args.extend(pkg_names.iter().map(|s| s.as_str()));

        let output = crate::core::privilege::run_elevated(
            PackageMode::System,
            &soar_path,
            &args,
        )
        .map_err(|e| AdapterError::Other(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::Other(format!("Remove failed: {}", stderr)));
        }

        Ok(())
    }

    pub async fn remove_package(&self, query: &str) -> Result<()> {
        let results = remove::resolve_removals(&self.user_ctx, &[query.to_string()], false)
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        let mut to_remove = Vec::new();
        for result in results {
            match result {
                RemoveResolveResult::Resolved(pkgs) => to_remove.extend(pkgs),
                RemoveResolveResult::NotInstalled(q) => {
                    return Err(AdapterError::Other(format!("Package not installed: {q}")));
                }
                RemoveResolveResult::Ambiguous { query, .. } => {
                    return Err(AdapterError::Other(format!(
                        "Ambiguous package query: {query}"
                    )));
                }
            }
        }

        if to_remove.is_empty() {
            return Err(AdapterError::Other("No packages to remove".into()));
        }

        let report = remove::perform_removal(&self.user_ctx, to_remove)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    pub async fn update_package(&self, query: &str) -> Result<()> {
        let updates = update::check_updates(&self.user_ctx, Some(&[query.to_string()]))
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if updates.is_empty() {
            return Err(AdapterError::Other("No updates available".into()));
        }

        let report = update::perform_update(&self.user_ctx, updates, false, false)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    pub async fn update_all(&self) -> Result<()> {
        let updates = update::check_updates(&self.user_ctx, None)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if updates.is_empty() {
            return Ok(());
        }

        let report = update::perform_update(&self.user_ctx, updates, false, false)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    fn build_config_schema(&self) -> ConfigSchema {
        let profile_keys: Vec<String> = self.user_ctx.config().profile.keys().cloned().collect();

        ConfigSchema {
            adapter_id: "soar".into(),
            fields: vec![
                ConfigField {
                    key: "executable_path".into(),
                    label: "Soar binary path".into(),
                    description: Some(
                        "Path to soar binary for system operations (auto-detected if empty)".into(),
                    ),
                    field_type: ConfigFieldType::ExecutablePath,
                    aeris_managed: true,
                    ..Default::default()
                },
                ConfigField {
                    key: "parallel".into(),
                    label: "Parallel downloads".into(),
                    field_type: ConfigFieldType::Toggle,
                    default: Some(ConfigValue::Bool(true)),
                    ..Default::default()
                },
                ConfigField {
                    key: "parallel_limit".into(),
                    label: "Parallel limit".into(),
                    field_type: ConfigFieldType::Number,
                    default: Some(ConfigValue::Integer(4)),
                    ..Default::default()
                },
                ConfigField {
                    key: "search_limit".into(),
                    label: "Search result limit".into(),
                    field_type: ConfigFieldType::Number,
                    default: Some(ConfigValue::Integer(20)),
                    ..Default::default()
                },
                ConfigField {
                    key: "signature_verification".into(),
                    label: "Signature verification".into(),
                    field_type: ConfigFieldType::Toggle,
                    default: Some(ConfigValue::Bool(true)),
                    ..Default::default()
                },
                ConfigField {
                    key: "desktop_integration".into(),
                    label: "Desktop integration".into(),
                    field_type: ConfigFieldType::Toggle,
                    default: Some(ConfigValue::Bool(false)),
                    ..Default::default()
                },
                ConfigField {
                    key: "bin_path".into(),
                    label: "Bin path".into(),
                    field_type: ConfigFieldType::PathList,
                    section: Some("Paths".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "cache_path".into(),
                    label: "Cache path".into(),
                    field_type: ConfigFieldType::PathList,
                    section: Some("Paths".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "db_path".into(),
                    label: "DB path".into(),
                    field_type: ConfigFieldType::PathList,
                    section: Some("Paths".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "desktop_path".into(),
                    label: "Desktop path".into(),
                    field_type: ConfigFieldType::PathList,
                    section: Some("Paths".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "repositories_path".into(),
                    label: "Repos path".into(),
                    field_type: ConfigFieldType::PathList,
                    section: Some("Paths".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "portable_dirs".into(),
                    label: "Portable dirs".into(),
                    field_type: ConfigFieldType::PathList,
                    section: Some("Paths".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "ghcr_concurrency".into(),
                    label: "GHCR concurrency".into(),
                    field_type: ConfigFieldType::Number,
                    default: Some(ConfigValue::Integer(8)),
                    section: Some("Advanced".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "sync_interval".into(),
                    label: "Sync interval".into(),
                    field_type: ConfigFieldType::Text,
                    section: Some("Advanced".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "default_profile".into(),
                    label: "Default profile".into(),
                    field_type: ConfigFieldType::Select(profile_keys),
                    default: Some(ConfigValue::String("default".into())),
                    section: Some("Advanced".into()),
                    ..Default::default()
                },
                ConfigField {
                    key: "default_package_mode".into(),
                    label: "Default package mode".into(),
                    description: Some("Default mode for package operations".into()),
                    field_type: ConfigFieldType::Select(if self.has_system {
                        vec!["user".into(), "system".into()]
                    } else {
                        vec!["user".into()]
                    }),
                    default: Some(ConfigValue::String("user".into())),
                    section: Some("Package Mode".into()),
                    ..Default::default()
                },
            ],
        }
    }

    fn build_initial_config(&self) -> AdapterConfig {
        use std::collections::HashMap;

        let cfg = self.user_ctx.config();
        let mut values = HashMap::new();

        values.insert(
            "parallel".into(),
            ConfigValue::Bool(cfg.parallel.unwrap_or(true)),
        );
        values.insert(
            "parallel_limit".into(),
            ConfigValue::String(cfg.parallel_limit.unwrap_or(4).to_string()),
        );
        values.insert(
            "search_limit".into(),
            ConfigValue::String(cfg.search_limit.unwrap_or(20).to_string()),
        );
        values.insert(
            "signature_verification".into(),
            ConfigValue::Bool(cfg.signature_verification.unwrap_or(true)),
        );
        values.insert(
            "desktop_integration".into(),
            ConfigValue::Bool(cfg.desktop_integration.unwrap_or(false)),
        );
        values.insert(
            "bin_path".into(),
            ConfigValue::String(cfg.bin_path.clone().unwrap_or_default()),
        );
        values.insert(
            "cache_path".into(),
            ConfigValue::String(cfg.cache_path.clone().unwrap_or_default()),
        );
        values.insert(
            "db_path".into(),
            ConfigValue::String(cfg.db_path.clone().unwrap_or_default()),
        );
        values.insert(
            "desktop_path".into(),
            ConfigValue::String(cfg.desktop_path.clone().unwrap_or_default()),
        );
        values.insert(
            "repositories_path".into(),
            ConfigValue::String(cfg.repositories_path.clone().unwrap_or_default()),
        );
        values.insert(
            "portable_dirs".into(),
            ConfigValue::String(cfg.portable_dirs.clone().unwrap_or_default()),
        );
        values.insert(
            "ghcr_concurrency".into(),
            ConfigValue::String(cfg.ghcr_concurrency.unwrap_or(8).to_string()),
        );
        values.insert(
            "sync_interval".into(),
            ConfigValue::String(cfg.sync_interval.clone().unwrap_or_default()),
        );
        values.insert(
            "default_profile".into(),
            ConfigValue::String(cfg.default_profile.clone()),
        );
        values.insert(
            "default_package_mode".into(),
            ConfigValue::String("user".into()),
        );

        AdapterConfig { values }
    }

    pub fn new(config: Config) -> Result<(Self, std::sync::mpsc::Receiver<SoarEvent>)> {
        let (sink, receiver) = ChannelSink::new();
        let events: EventSinkHandle = Arc::new(sink);
        let user_ctx = SoarContext::new(config, events);

        let has_system = Self::can_run_system();

        Ok((
            Self {
                user_ctx,
                has_system,
                info: AdapterInfo {
                    id: "soar".into(),
                    name: "Soar".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    capabilities: Capabilities {
                        can_search: true,
                        can_install: true,
                        can_remove: true,
                        can_update: true,
                        can_list: true,
                        can_sync: true,
                        can_run: true,
                        can_add_repo: true,
                        can_remove_repo: true,
                        can_list_repos: true,
                        has_profiles: false,
                        has_size_info: true,
                        has_package_detail: true,
                        supports_verification: true,
                        supports_portable: true,
                        supports_hooks: true,
                        supports_build_from_source: true,
                        supports_batch_install: true,
                        supports_user_packages: true,
                        supports_system_packages: has_system,
                        ..Capabilities::default()
                    },
                    enabled: true,
                    is_builtin: true,
                    plugin_path: None,
                    description: "Native package manager for portable packages".into(),
                    icon: None,
                },
            },
            receiver,
        ))
    }

    fn can_run_system() -> bool {
        crate::core::privilege::PrivilegeManager::detect_elevator().is_some()
    }
}

fn soar_pkg_to_aeris(pkg: &soar_core::database::models::Package, installed: bool) -> Package {
    Package {
        id: format!("{}.{}", pkg.repo_name, pkg.pkg_id),
        name: pkg.pkg_name.clone(),
        version: pkg.version.clone(),
        adapter_id: "soar".into(),
        description: Some(pkg.description.clone()),
        size: pkg.size,
        homepage: pkg.homepages.as_ref().and_then(|h| h.first().cloned()),
        license: pkg.licenses.as_ref().and_then(|l| l.first().cloned()),
        installed,
        update_available: false,
        category: pkg.categories.as_ref().and_then(|c| c.first().cloned()),
        tags: pkg.tags.clone().unwrap_or_default(),
        icon_url: pkg.icon.clone(),
    }
}

#[async_trait::async_trait]
impl Adapter for SoarAdapter {
    fn info(&self) -> &AdapterInfo {
        &self.info
    }

    fn capabilities(&self) -> &Capabilities {
        &self.info.capabilities
    }

    async fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<Package>> {
        let result = soar_operations::search::search_packages(&self.user_ctx, query, false, limit)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        Ok(result
            .packages
            .iter()
            .map(|entry| soar_pkg_to_aeris(&entry.package, entry.installed))
            .collect())
    }

    async fn install(
        &self,
        _packages: &[Package],
        _progress: Option<ProgressSender>,
    ) -> Result<Vec<InstallResult>> {
        Err(AdapterError::NotSupported)
    }

    async fn remove(&self, packages: &[Package], _progress: Option<ProgressSender>, mode: PackageMode) -> Result<()> {
        if mode == PackageMode::System {
            return self.remove_system_package(packages).await;
        }
        
        let pkg_ids: Vec<String> = packages.iter().map(|p| p.name.clone()).collect();
        let results = remove::resolve_removals(&self.user_ctx, &pkg_ids, false)
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        let mut to_remove = Vec::new();
        for result in results {
            match result {
                RemoveResolveResult::Resolved(pkgs) => to_remove.extend(pkgs),
                RemoveResolveResult::NotInstalled(q) => {
                    return Err(AdapterError::Other(format!("Package not installed: {}", q)));
                }
                RemoveResolveResult::Ambiguous { query, candidates } => {
                    return Err(AdapterError::Other(format!(
                        "Ambiguous package: {} ({} candidates)",
                        query,
                        candidates.len()
                    )));
                }
            }
        }

        if to_remove.is_empty() {
            return Err(AdapterError::Other("No packages to remove".into()));
        }

        remove::perform_removal(&self.user_ctx, to_remove)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        Ok(())
    }

    async fn update(
        &self,
        _packages: &[Package],
        _progress: Option<ProgressSender>,
        _mode: PackageMode,
    ) -> Result<Vec<InstallResult>> {
        Err(AdapterError::NotSupported)
    }

    async fn list_installed(&self, mode: PackageMode) -> Result<Vec<InstalledPackage>> {
        if mode == PackageMode::System {
            return self.list_installed_system().await;
        }
        
        let result = soar_operations::list::list_installed(&self.user_ctx, None)
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        Ok(result
            .packages
            .iter()
            .map(|entry| {
                let pkg = &entry.package;
                InstalledPackage {
                    package: Package {
                        id: format!("{}.{}", pkg.repo_name, pkg.pkg_id),
                        name: pkg.pkg_name.clone(),
                        version: pkg.version.clone(),
                        adapter_id: "soar".into(),
                        description: None,
                        size: Some(pkg.size),
                        homepage: None,
                        license: None,
                        installed: true,
                        update_available: false,
                        category: None,
                        tags: vec![],
                        icon_url: None,
                    },
                    installed_at: pkg.installed_date.clone(),
                    install_size: entry.disk_size,
                    install_path: Some(pkg.installed_path.clone()),
                    pinned: pkg.pinned,
                    auto_installed: false,
                    is_healthy: entry.is_healthy,
                    profile: Some(pkg.profile.clone()),
                }
            })
            .collect())
    }

    async fn list_updates(&self, mode: PackageMode) -> Result<Vec<Update>> {
        if mode == PackageMode::System {
            return Ok(vec![]);
        }
        
        let updates = update::check_updates(&self.user_ctx, None)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        Ok(updates
            .iter()
            .map(|u| Update {
                package: Package {
                    id: format!("{}.{}", u.repo_name, u.pkg_id),
                    name: u.pkg_name.clone(),
                    version: u.current_version.clone(),
                    adapter_id: "soar".into(),
                    description: None,
                    size: None,
                    homepage: None,
                    license: None,
                    installed: true,
                    update_available: true,
                    category: None,
                    tags: vec![],
                    icon_url: None,
                },
                current_version: u.current_version.clone(),
                new_version: u.new_version.clone(),
                download_size: None,
                is_security: false,
                changelog_url: None,
            })
            .collect())
    }

    async fn sync(&self, _progress: Option<ProgressSender>) -> Result<()> {
        self.user_ctx
            .sync()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))
    }

    async fn list_repositories(&self) -> Result<Vec<Repository>> {
        Ok(self
            .user_ctx
            .config()
            .repositories
            .iter()
            .map(|r| Repository {
                name: r.name.clone(),
                url: r.url.clone(),
                enabled: true,
                description: None,
            })
            .collect())
    }

    async fn package_detail(&self, _package_id: &str) -> Result<PackageDetail> {
        Err(AdapterError::NotSupported)
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

    fn config_schema(&self) -> Option<ConfigSchema> {
        Some(self.build_config_schema())
    }

    fn initial_config(&self) -> Option<AdapterConfig> {
        Some(self.build_initial_config())
    }

    async fn get_config(&self) -> Result<AdapterConfig> {
        self.initial_config().ok_or(AdapterError::NotSupported)
    }

    async fn set_config(&self, config: &AdapterConfig) -> Result<()> {
        use toml_edit::DocumentMut;

        let config_path = soar_config::config::CONFIG_PATH
            .read()
            .unwrap()
            .to_path_buf();

        let mut doc = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| s.parse::<DocumentMut>().ok())
            .unwrap_or_default();

        for (key, value) in &config.values {
            match value {
                ConfigValue::Bool(v) => {
                    doc[key.as_str()] = toml_edit::value(*v);
                }
                ConfigValue::Integer(v) => {
                    doc[key.as_str()] = toml_edit::value(*v);
                }
                ConfigValue::String(s) => {
                    if s.trim().is_empty() {
                        doc.remove(key.as_str());
                    } else {
                        doc[key.as_str()] = toml_edit::value(s.as_str());
                    }
                }
                ConfigValue::StringList(list) => {
                    let mut arr = toml_edit::Array::new();
                    for item in list {
                        arr.push(item.as_str());
                    }
                    doc[key.as_str()] = toml_edit::value(arr);
                }
            }
        }

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AdapterError::Other(e.to_string()))?;
        }
        std::fs::write(&config_path, doc.to_string())
            .map_err(|e| AdapterError::Other(e.to_string()))
    }
}
