use serde::{Deserialize, Serialize};

use super::adapter::AdapterId;

pub type PackageId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: PackageId,
    pub name: String,
    pub version: String,
    pub adapter_id: AdapterId,
    pub description: Option<String>,
    pub size: Option<u64>,
    pub homepage: Option<String>,
    pub license: Option<String>,
    pub installed: bool,
    pub update_available: bool,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub icon_url: Option<String>,
}

/// What a manager knows about one package beyond what a listing carries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDetail {
    pub package: Package,
    /// What the package is built as, in the manager's own words.
    pub pkg_type: Option<String>,
    /// Where the package is built from.
    pub source: Option<String>,
    pub build_date: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    pub package: Package,
    pub installed_at: String,
    pub install_size: u64,
    pub install_path: Option<String>,
    pub pinned: bool,
    pub auto_installed: bool,
    pub is_healthy: bool,
    pub profile: Option<String>,
}

impl InstalledPackage {
    /// Unique key for selection/tracking, distinguishing different installs of the same package.
    pub fn unique_key(&self) -> String {
        format!("{}@{}", self.package.id, self.package.version)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Update {
    pub package: Package,
    pub current_version: String,
    pub new_version: String,
    pub download_size: Option<u64>,
    pub is_security: bool,
    pub changelog_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub package_name: String,
    pub package_id: String,
    pub version: String,
    pub success: bool,
    pub error: Option<String>,
}
