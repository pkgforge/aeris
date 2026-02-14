use serde::{Deserialize, Serialize};

use super::adapter::AdapterId;

pub type PackageId = String;

impl Package {
    /// Extract (repo_name, pkg_id) from the package id format "repo_name.pkg_id"
    pub fn soar_query_parts(&self) -> Option<(&str, &str)> {
        self.id.split_once('.')
    }

    /// Build a soar query string "name#pkg_id:repo_name"
    pub fn soar_query(&self) -> Option<String> {
        let (repo_name, pkg_id) = self.soar_query_parts()?;
        Some(format!("{}#{}:{}", self.name, pkg_id, repo_name))
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub version_req: Option<String>,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVariant {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDetail {
    pub package: Package,
    pub dependencies: Vec<Dependency>,
    pub screenshots: Vec<String>,
    pub readme: Option<String>,
    pub maintainers: Vec<String>,
    pub build_date: Option<String>,
    pub download_url: Option<String>,
    pub variants: Vec<PackageVariant>,
    pub snapshots: Vec<Snapshot>,
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
