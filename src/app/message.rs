use crate::core::{
    config::ConfigValue,
    package::{InstalledPackage, Package, PackageDetail, Update},
};

use super::{AppTheme, View};

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    Install(Package),
    Remove(Package),
    Update(Package),
    UpdateAll,
}

#[derive(Debug, Clone)]
pub enum Message {
    NavigateTo(View),

    Browse(BrowseMessage),
    Installed(InstalledMessage),
    Updates(UpdatesMessage),
    Settings(SettingsMessage),
    Repositories(RepositoriesMessage),

    ConfirmAction,
    CancelAction,

    ProgressTick,
}

#[derive(Debug, Clone)]
pub enum BrowseMessage {
    SearchQueryChanged(String),
    SearchSubmit,
    SearchResults(Result<Vec<Package>, String>),
    SourceFilterToggle(String),
    SelectPackage(String),
    PackageDetailLoaded(Result<Box<PackageDetail>, String>),
    InstallPackage(Package),
    InstallComplete(Result<(), String>),
    DismissInstallError,
    CloseDetail,
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
    BrowseAdapterField(String),
    BrowseAdapterFieldResult(String, String),
    RevertAdapterField(String),
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
