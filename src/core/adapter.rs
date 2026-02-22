use std::path::PathBuf;

use super::{
    capabilities::Capabilities,
    config::{AdapterConfig, ConfigSchema},
    package::{InstallResult, InstalledPackage, Package, PackageDetail, Update},
    privilege::PackageMode,
    profile::Profile,
    repository::Repository,
};

pub type AdapterId = String;
pub type ProgressSender = tokio::sync::mpsc::UnboundedSender<ProgressEvent>;

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Download {
        adapter_id: AdapterId,
        package_name: String,
        current_bytes: u64,
        total_bytes: u64,
    },
    Phase {
        adapter_id: AdapterId,
        package_name: String,
        phase: String,
        progress_percent: f32,
    },
    Status {
        adapter_id: AdapterId,
        message: String,
    },
    Completed {
        adapter_id: AdapterId,
        package_name: String,
    },
    Failed {
        adapter_id: AdapterId,
        package_name: String,
        error: String,
    },
    BatchProgress {
        adapter_id: AdapterId,
        completed: u32,
        total: u32,
        failed: u32,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Adapter not found: {0}")]
    NotFound(String),
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Operation not supported")]
    NotSupported,
    #[error("Plugin error: {0}")]
    PluginError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AdapterError>;

#[derive(Debug, Clone)]
pub struct AdapterInfo {
    pub id: AdapterId,
    pub name: String,
    pub version: String,
    pub capabilities: Capabilities,
    pub enabled: bool,
    pub is_builtin: bool,
    pub plugin_path: Option<PathBuf>,
    pub description: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: String,
    pub package_count: Option<u64>,
    pub repo_count: Option<u32>,
    pub cache_size: Option<u64>,
}

#[async_trait::async_trait]
pub trait Adapter: Send + Sync {
    fn info(&self) -> &AdapterInfo;
    fn capabilities(&self) -> &Capabilities;

    async fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        mode: PackageMode,
    ) -> Result<Vec<Package>>;

    async fn package_detail(&self, _package_id: &str) -> Result<PackageDetail> {
        Err(AdapterError::NotSupported)
    }

    async fn install(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>>;

    async fn remove(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<()>;

    async fn update(
        &self,
        packages: &[Package],
        progress: Option<ProgressSender>,
        mode: PackageMode,
    ) -> Result<Vec<InstallResult>>;

    async fn list_installed(&self, mode: PackageMode) -> Result<Vec<InstalledPackage>>;

    async fn list_updates(&self, mode: PackageMode) -> Result<Vec<Update>>;

    async fn sync(&self, _progress: Option<ProgressSender>) -> Result<()> {
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

    async fn list_repositories(&self) -> Result<Vec<Repository>> {
        Err(AdapterError::NotSupported)
    }

    async fn add_repository(&self, _repo: &Repository) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    async fn remove_repository(&self, _repo_name: &str) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    async fn set_repo_enabled(
        &self,
        _name: &str,
        _enabled: bool,
        _mode: PackageMode,
    ) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    fn config_schema(&self) -> Option<ConfigSchema> {
        None
    }

    fn initial_config(&self) -> Option<AdapterConfig> {
        None
    }

    async fn get_config(&self) -> Result<AdapterConfig> {
        Err(AdapterError::NotSupported)
    }

    async fn set_config(&self, _config: &AdapterConfig) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    async fn set_config_for_mode(&self, config: &AdapterConfig, _mode: PackageMode) -> Result<()> {
        self.set_config(config).await
    }

    async fn run_package(&self, _package: &Package, _args: &[String]) -> Result<()> {
        Err(AdapterError::NotSupported)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::default())
    }
}
