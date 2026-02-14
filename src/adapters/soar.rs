use std::sync::Arc;

use soar_config::config::Config;
use soar_events::{EventSinkHandle, NullSink};
use soar_operations::SoarContext;

use crate::core::{
    adapter::{Adapter, AdapterError, AdapterInfo, ProgressSender, Result},
    capabilities::Capabilities,
    config::{AdapterConfig, ConfigSchema},
    package::{InstalledPackage, InstallResult, Package, PackageDetail, Update},
    profile::Profile,
    repository::Repository,
};

pub struct SoarAdapter {
    ctx: SoarContext,
    info: AdapterInfo,
}

impl SoarAdapter {
    pub fn new(config: Config) -> Result<Self> {
        let events: EventSinkHandle = Arc::new(NullSink);
        let ctx = SoarContext::new(config, events);

        Ok(Self {
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
        })
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

    async fn remove(
        &self,
        _packages: &[Package],
        _progress: Option<ProgressSender>,
    ) -> Result<()> {
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
                    profile: Some(pkg.profile.clone()),
                }
            })
            .collect())
    }

    async fn list_updates(&self) -> Result<Vec<Update>> {
        Err(AdapterError::NotSupported)
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
