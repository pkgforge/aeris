use std::sync::Arc;

use soar_config::config::Config;
use soar_events::{ChannelSink, EventSinkHandle, SoarEvent};
use soar_operations::{
    InstallOptions, RemoveResolveResult, ResolveResult, SoarContext, install, remove, update,
};

use crate::core::{
    adapter::{Adapter, AdapterError, AdapterInfo, ProgressSender, Result},
    capabilities::Capabilities,
    config::{AdapterConfig, ConfigSchema},
    package::{InstallResult, InstalledPackage, Package, PackageDetail, Update},
    profile::Profile,
    repository::Repository,
};

pub struct SoarAdapter {
    ctx: SoarContext,
    info: AdapterInfo,
}

impl SoarAdapter {
    pub fn repo_count(&self) -> usize {
        self.ctx.config().repositories.len()
    }

    pub async fn install_package(&self, query: &str) -> Result<()> {
        let options = InstallOptions::default();
        let results = install::resolve_packages(&self.ctx, &[query.to_string()], &options)
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

        let report = install::perform_installation(&self.ctx, targets, &options)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    pub async fn remove_package(&self, query: &str) -> Result<()> {
        let results = remove::resolve_removals(&self.ctx, &[query.to_string()], false)
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

        let report = remove::perform_removal(&self.ctx, to_remove)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    pub async fn update_package(&self, query: &str) -> Result<()> {
        let updates = update::check_updates(&self.ctx, Some(&[query.to_string()]))
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if updates.is_empty() {
            return Err(AdapterError::Other("No updates available".into()));
        }

        let report = update::perform_update(&self.ctx, updates, false)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    pub async fn update_all(&self) -> Result<()> {
        let updates = update::check_updates(&self.ctx, None)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if updates.is_empty() {
            return Ok(());
        }

        let report = update::perform_update(&self.ctx, updates, false)
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))?;

        if let Some(failed) = report.failed.first() {
            return Err(AdapterError::Other(failed.error.clone()));
        }

        Ok(())
    }

    pub fn new(config: Config) -> Result<(Self, std::sync::mpsc::Receiver<SoarEvent>)> {
        let (sink, receiver) = ChannelSink::new();
        let events: EventSinkHandle = Arc::new(sink);
        let ctx = SoarContext::new(config, events);

        Ok((
            Self {
                ctx,
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
        let result = soar_operations::search::search_packages(&self.ctx, query, false, limit)
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

    async fn remove(&self, _packages: &[Package], _progress: Option<ProgressSender>) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    async fn update(
        &self,
        _packages: &[Package],
        _progress: Option<ProgressSender>,
    ) -> Result<Vec<InstallResult>> {
        Err(AdapterError::NotSupported)
    }

    async fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        let result = soar_operations::list::list_installed(&self.ctx, None)
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

    async fn list_updates(&self) -> Result<Vec<Update>> {
        let updates = update::check_updates(&self.ctx, None)
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
        self.ctx
            .sync()
            .await
            .map_err(|e| AdapterError::Other(e.to_string()))
    }

    async fn list_repositories(&self) -> Result<Vec<Repository>> {
        Ok(self
            .ctx
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
        None
    }

    async fn get_config(&self) -> Result<AdapterConfig> {
        Err(AdapterError::NotSupported)
    }

    async fn set_config(&self, _config: &AdapterConfig) -> Result<()> {
        Err(AdapterError::NotSupported)
    }
}
