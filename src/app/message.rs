use crate::core::{
    config::ConfigValue,
    package::{InstalledPackage, Package, PackageDetail, Update},
    privilege::PackageMode,
};

use super::{AppTheme, View};

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Install(Package, PackageMode),
    Remove(Package, PackageMode),
    Update(Package, PackageMode),
    UpdateAll(PackageMode),
}

#[derive(Debug, Clone)]
pub enum Message {
    NavigateTo(View),
    PackageModeChanged(PackageMode),

    Browse(BrowseMessage),
    Installed(InstalledMessage),
    Updates(UpdatesMessage),
    Settings(SettingsMessage),
    Repositories(RepositoriesMessage),

    ToggleSidebar,

    ConfirmAction,
    CancelAction,

    ProgressTick,
}

#[derive(Debug, Clone)]
pub enum BrowseMessage {
    SearchQueryChanged(String),
    SearchSubmit,
    SearchSubmitDebounced(u64),
    SearchResults(Result<Vec<Package>, String>),
    SourceFilterToggle(String),
    SelectPackage(String),
    PackageDetailLoaded(Result<Box<PackageDetail>, String>),
    InstallPackage(Package),
    InstallPackageWithMode(Package, PackageMode),
    InstallComplete(Result<(), String>),
    DismissInstallError,
    CloseDetail,
    InstallModeChanged(PackageMode),
}

#[derive(Debug, Clone)]
pub enum InstalledMessage {
    Refresh,
    PackagesLoaded(Result<Vec<InstalledPackage>, String>),
    FilterChanged(String),
    SourceFilterChanged(Option<String>),
    RemovePackage(Package),
    RemoveComplete(Result<(), String>),
}

#[derive(Debug, Clone)]
pub enum UpdatesMessage {
    CheckUpdates,
    UpdatesLoaded(Result<Vec<Update>, String>),
    UpdatePackage(Package),
    UpdateComplete(Result<(), String>),
    UpdateAll,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    ThemeChanged(AppTheme),
    StartupViewChanged(View),
    NotificationsToggled(bool),
    SaveAeris,
    SaveAerisResult(Result<(), String>),
    AdapterFieldChanged(String, ConfigValue),
    AdapterAerisFieldChanged(String, String),
    BrowseAdapterField(String),
    BrowseAdapterFieldResult(String, String),
    BrowseExecutableField(String),
    BrowseExecutableFieldResult(String, String),
    RevertAdapterField(String),
    RevertAdapterAerisField(String),
    SaveAdapter,
    SaveAdapterResult(Result<(), String>),
}

#[derive(Debug, Clone)]
pub enum RepositoriesMessage {
    Refresh,
    Loaded(Result<Vec<RepoInfo>, String>),
    SyncRepo(String),
    SyncAll,
    SyncComplete(Result<(), String>),
    ToggleEnabled(String, bool),
    ToggleResult(Result<(), String>),
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
