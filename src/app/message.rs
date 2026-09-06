use crate::core::{package::Package, privilege::PackageMode};

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Install(Package, PackageMode),
    Update(Package, PackageMode),
    UpdateAll(PackageMode),
    /// Update everything one manager holds, for a manager that cannot be
    /// pointed at a single package. Carries its id and name.
    UpdateEverythingIn {
        adapter_id: String,
        adapter_name: String,
        mode: PackageMode,
    },
    /// Install the current selection in the Browse view.
    BatchInstall {
        count: usize,
    },
    /// Update the current selection in the Updates view.
    BatchUpdate {
        count: usize,
    },
    /// Remove a specific installed entry, disambiguated by unique_key.
    RemoveInstalled {
        pkg: Package,
        unique_key: String,
        mode: PackageMode,
    },
    /// Remove the current selection in the Installed view.
    BatchRemoveInstalled {
        count: usize,
    },
    /// Apply the soar declarative manifest.
    ApplyManifest {
        prune: bool,
        remove_names: Vec<String>,
    },
    /// Remove a declared package entry from the manifest file.
    RemoveManifestEntry {
        name: String,
    },
    /// Replace the manifest's packages table with the current installed set.
    ImportInstalledManifest,
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
