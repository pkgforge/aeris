use crate::core::{package::Package, privilege::PackageMode};

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Install(Package, PackageMode),
    Remove(Package, PackageMode),
    Update(Package, PackageMode),
    UpdateAll(PackageMode),
    BatchInstall(Vec<Package>, PackageMode),
    BatchRemove(Vec<Package>, PackageMode),
    BatchUpdate(Vec<Package>, PackageMode),
}

#[derive(Debug, Clone, Default)]
pub struct RepoInfo {
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub desktop_integration: bool,
    pub has_pubkey: bool,
    pub signature_verification: bool,
    pub sync_interval: Option<String>,
}
