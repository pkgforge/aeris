use crate::core::{
    config::ConfigValue,
    package::{InstalledPackage, Package, PackageDetail, Update},
    privilege::PackageMode,
    registry::PluginEntry,
};

use super::{AppTheme, View};

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

#[derive(Debug, Clone)]
pub enum Message {
    NavigateTo(View),
    PackageModeChanged(PackageMode),

    Browse(BrowseMessage),
    Installed(InstalledMessage),
    Updates(UpdatesMessage),
    Settings(SettingsMessage),
    Repositories(RepositoriesMessage),
    Adapter(AdapterMessage),

    ToggleSidebar,

    ConfirmAction,
    CancelAction,

    DismissToast(u64),
    CancelQueuedOperation(u64),
    ProcessNextQueued,
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
    ToggleSelect(String),
    SelectAll,
    ClearSelection,
    InstallSelected,
}

#[derive(Debug, Clone)]
pub enum InstalledMessage {
    Refresh,
    PackagesLoaded(Result<Vec<InstalledPackage>, String>),
    FilterChanged(String),
    SourceFilterChanged(Option<String>),
    RemovePackage(Package),
    RemoveComplete(Result<(), String>),
    UpdatePackage(Package),
    ToggleSelect(String),
    SelectAll,
    ClearSelection,
    RemoveSelected,
}

#[derive(Debug, Clone)]
pub enum UpdatesMessage {
    CheckUpdates,
    UpdatesLoaded {
        result: Result<Vec<Update>, String>,
        /// Adapters (id, name) that don't support listing available updates.
        no_update_listing: Vec<(String, String)>,
    },
    UpdatePackage(Package),
    UpdateComplete(Result<(), String>),
    UpdateAll,
    /// Trigger update for all installed packages from a specific adapter (by id).
    UpdateAdapterAll(String),
    ToggleSelect(String),
    SelectAll,
    ClearSelection,
    UpdateSelected,
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

#[derive(Debug, Clone)]
pub enum AdapterMessage {
    ToggleAdapter(String, bool),
    ToggleSaved(Result<(), String>),
    FetchRegistry,
    RegistryFetched(Result<Vec<PluginEntry>, String>),
    InstallPlugin(PluginEntry),
    PluginInstalled(Result<String, String>),
    RemovePlugin(String),
    PluginRemoved(Result<String, String>),
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
