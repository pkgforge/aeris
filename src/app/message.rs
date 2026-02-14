use crate::core::{
    adapter::AdapterInfo,
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
    ThemeChanged(AppTheme),

    Browse(BrowseMessage),
    Installed(InstalledMessage),
    Updates(UpdatesMessage),
    Adapters(AdaptersMessage),

    ConfirmAction,
    CancelAction,
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
pub enum AdaptersMessage {
    Refresh,
    AdaptersLoaded(Vec<AdapterInfo>),
    ToggleAdapter(String),
    SyncAdapter(String),
    SyncAll,
    SyncComplete(String, Result<(), String>),
}
