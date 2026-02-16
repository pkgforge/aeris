pub mod message;

use std::sync::Arc;

use iced::{
    Color, Element, Length, Subscription, Task,
    widget::{
        button, center, column, container, mouse_area, opaque, progress_bar, row, rule, space,
        stack, text,
    },
};
use soar_events::{InstallStage, RemoveStage, SoarEvent, VerifyStage};

use crate::{
    adapters::soar::SoarAdapter,
    config::AerisConfig,
    core::{
        adapter::Adapter,
        config::{AdapterConfig, ConfigFieldType, ConfigValue},
        privilege::{PackageMode, PrivilegeManager},
    },
    views,
};

pub use message::Message;

pub const APP_NAME: &str = "Aeris";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl AppTheme {
    pub const ALL: [AppTheme; 3] = [AppTheme::System, AppTheme::Light, AppTheme::Dark];
}

impl std::fmt::Display for AppTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppTheme::System => write!(f, "System"),
            AppTheme::Light => write!(f, "Light"),
            AppTheme::Dark => write!(f, "Dark"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Dashboard,
    Browse,
    Installed,
    Updates,
    Repositories,
    AdapterInfo,
    Settings,
}

impl View {
    pub const ALL: [View; 4] = [
        View::Dashboard,
        View::Browse,
        View::Installed,
        View::Updates,
    ];
}

impl std::fmt::Display for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            View::Dashboard => write!(f, "Dashboard"),
            View::Browse => write!(f, "Browse"),
            View::Installed => write!(f, "Installed"),
            View::Updates => write!(f, "Updates"),
            View::Repositories => write!(f, "Repositories"),
            View::AdapterInfo => write!(f, "Adapter"),
            View::Settings => write!(f, "Settings"),
        }
    }
}

pub enum OperationType {
    Install,
    Remove,
    Update,
    UpdateAll,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OperationType::Install => write!(f, "Installing"),
            OperationType::Remove => write!(f, "Removing"),
            OperationType::Update => write!(f, "Updating"),
            OperationType::UpdateAll => write!(f, "Updating all"),
        }
    }
}

pub enum OperationStatus {
    Starting,
    Downloading { current: u64, total: u64 },
    Verifying(String),
    Installing(String),
    Removing(String),
    Completed,
    Failed(String),
}

impl OperationStatus {
    pub fn label(&self) -> String {
        match self {
            OperationStatus::Starting => "Starting...".into(),
            OperationStatus::Downloading { current, total } => {
                if *total > 0 {
                    let pct = (*current as f64 / *total as f64 * 100.0) as u64;
                    let current_mb = *current as f64 / 1_048_576.0;
                    let total_mb = *total as f64 / 1_048_576.0;
                    format!("Downloading {pct}% ({current_mb:.1} / {total_mb:.1} MB)")
                } else {
                    "Downloading...".into()
                }
            }
            OperationStatus::Verifying(stage) => format!("Verifying ({stage})..."),
            OperationStatus::Installing(phase) => format!("Installing ({phase})..."),
            OperationStatus::Removing(phase) => format!("Removing ({phase})..."),
            OperationStatus::Completed => "Completed".into(),
            OperationStatus::Failed(e) => format!("Failed: {e}"),
        }
    }

    pub fn progress(&self) -> Option<f32> {
        match self {
            OperationStatus::Downloading { current, total } if *total > 0 => {
                Some(*current as f32 / *total as f32)
            }
            _ => None,
        }
    }
}

pub struct ActiveOperation {
    pub operation_type: OperationType,
    pub package_name: String,
    pub status: OperationStatus,
}

pub struct App {
    selected_theme: AppTheme,
    current_view: View,
    browse: views::browse::BrowseState,
    installed: views::installed::InstalledState,
    updates: views::updates::UpdatesState,
    settings: views::settings::SettingsState,
    repositories: views::repositories::RepositoriesState,
    aeris_config: AerisConfig,
    adapter: Arc<SoarAdapter>,
    confirm_dialog: Option<message::ConfirmAction>,
    event_receiver: std::sync::mpsc::Receiver<SoarEvent>,
    active_operation: Option<ActiveOperation>,
    selected_install_mode: PackageMode,
    current_mode: PackageMode,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let aeris_config = AerisConfig::load();
        soar_config::config::init().expect("Failed to load soar config");
        let soar_config = soar_config::config::get_config();

        let selected_theme = aeris_config.theme();
        let startup_view = aeris_config.startup_view();

        let (adapter, event_receiver) =
            SoarAdapter::new(soar_config).expect("Failed to initialize Soar adapter");
        let adapter = Arc::new(adapter);

        let settings = views::settings::SettingsState::load(&aeris_config, adapter.as_ref());

        let default_mode = settings
            .adapter_config
            .values
            .get("default_package_mode")
            .and_then(|v| match v {
                ConfigValue::String(s) => Some(if s == "system" {
                    PackageMode::System
                } else {
                    PackageMode::User
                }),
                _ => Some(PackageMode::User),
            })
            .unwrap_or(PackageMode::User);

        let load_adapter = adapter.clone();
        let init_task = Task::perform(
            async move {
                load_adapter
                    .list_installed(default_mode)
                    .await
                    .map_err(|e| e.to_string())
            },
            |result| Message::Installed(message::InstalledMessage::PackagesLoaded(result)),
        );

        (
            Self {
                selected_theme,
                current_view: startup_view,
                browse: views::browse::BrowseState::default(),
                installed: views::installed::InstalledState {
                    loading: true,
                    ..Default::default()
                },
                updates: views::updates::UpdatesState::default(),
                settings,
                repositories: views::repositories::RepositoriesState::default(),
                aeris_config,
                adapter,
                confirm_dialog: None,
                event_receiver,
                active_operation: None,
                selected_install_mode: default_mode,
                current_mode: default_mode,
            },
            init_task,
        )
    }

    pub fn title(&self) -> String {
        format!("{APP_NAME} - {}", self.current_view)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NavigateTo(view) => {
                self.current_view = view;
                return match view {
                    View::Installed if !self.installed.loaded => self.load_installed(),
                    View::Repositories if !self.repositories.loaded => self.load_repositories(),
                    _ => Task::none(),
                };
            }
            Message::Browse(msg) => return self.update_browse(msg),
            Message::Installed(msg) => return self.update_installed(msg),
            Message::Updates(msg) => return self.update_updates(msg),
            Message::Settings(msg) => return self.update_settings(msg),
            Message::Repositories(msg) => return self.update_repositories(msg),
            Message::CancelAction => {
                self.confirm_dialog = None;
            }
            Message::ConfirmAction => {
                if let Some(action) = self.confirm_dialog.take() {
                    return self.execute_confirmed(action);
                }
            }
            Message::ProgressTick => {
                while let Ok(event) = self.event_receiver.try_recv() {
                    self.handle_soar_event(event);
                }
            }
        }
        Task::none()
    }

    fn update_browse(&mut self, msg: message::BrowseMessage) -> Task<Message> {
        match msg {
            message::BrowseMessage::SearchQueryChanged(query) => {
                self.browse.search_query = query;
            }
            message::BrowseMessage::SearchSubmit => {
                if self.browse.search_query.trim().is_empty() {
                    return Task::none();
                }
                self.browse.loading = true;
                let query = self.browse.search_query.clone();
                let adapter = self.adapter.clone();
                return Task::perform(
                    async move {
                        adapter
                            .search(&query, None)
                            .await
                            .map_err(|e| e.to_string())
                    },
                    |result| Message::Browse(message::BrowseMessage::SearchResults(result)),
                );
            }
            message::BrowseMessage::SearchResults(result) => {
                self.browse.loading = false;
                self.browse.has_searched = true;
                self.browse.result_version += 1;
                match result {
                    Ok(packages) => {
                        self.browse.error = None;
                        self.browse.search_results = packages;
                    }
                    Err(e) => {
                        log::error!("Search failed: {e}");
                        self.browse.error = Some(e);
                        self.browse.search_results.clear();
                    }
                }
            }
            message::BrowseMessage::SelectPackage(id) => {
                self.browse.selected_package = self
                    .browse
                    .search_results
                    .iter()
                    .find(|p| p.id == id)
                    .cloned();
            }
            message::BrowseMessage::CloseDetail => {
                self.browse.selected_package = None;
            }
            message::BrowseMessage::InstallPackage(pkg) => {
                let mode = self.selected_install_mode;
                self.confirm_dialog = Some(message::ConfirmAction::Install(pkg, mode));
            }
            message::BrowseMessage::InstallPackageWithMode(pkg, mode) => {
                self.confirm_dialog = Some(message::ConfirmAction::Install(pkg, mode));
            }
            message::BrowseMessage::InstallModeChanged(mode) => {
                self.selected_install_mode = mode;
                if let Some(message::ConfirmAction::Install(pkg, _)) = &self.confirm_dialog {
                    self.confirm_dialog = Some(message::ConfirmAction::Install(pkg.clone(), mode));
                }
            }
            message::BrowseMessage::InstallComplete(result) => {
                self.active_operation = None;
                let pkg_id = self.browse.installing.take();
                self.browse.result_version += 1;
                match result {
                    Ok(()) => {
                        log::info!("Package installed successfully");
                        if let Some(ref id) = pkg_id {
                            self.set_browse_installed(id, true);
                        }
                        return self.load_installed();
                    }
                    Err(e) => {
                        log::error!("Install failed: {e}");
                        self.browse.install_error = Some(e);
                    }
                }
            }
            message::BrowseMessage::DismissInstallError => {
                self.browse.install_error = None;
                self.browse.result_version += 1;
            }
            _ => {}
        }
        Task::none()
    }

    fn load_installed(&mut self) -> Task<Message> {
        self.installed.loading = true;
        let adapter = self.adapter.clone();
        let mode = self.current_mode;
        Task::perform(
            async move { adapter.list_installed(mode).await.map_err(|e| e.to_string()) },
            |result| Message::Installed(message::InstalledMessage::PackagesLoaded(result)),
        )
    }

    fn set_browse_installed(&mut self, pkg_id: &str, installed: bool) {
        if let Some(pkg) = self
            .browse
            .search_results
            .iter_mut()
            .find(|p| p.id == pkg_id)
        {
            pkg.installed = installed;
            self.browse.result_version += 1;
        }
    }

    fn set_browse_update_available(&mut self, pkg_id: &str, available: bool) {
        if let Some(pkg) = self
            .browse
            .search_results
            .iter_mut()
            .find(|p| p.id == pkg_id)
        {
            pkg.update_available = available;
            self.browse.result_version += 1;
        }
    }

    fn set_browse_update_available_all(&mut self, available: bool) {
        let mut changed = false;
        for pkg in &mut self.browse.search_results {
            if pkg.update_available != available {
                pkg.update_available = available;
                changed = true;
            }
        }
        if changed {
            self.browse.result_version += 1;
        }
    }

    fn update_installed(&mut self, msg: message::InstalledMessage) -> Task<Message> {
        match msg {
            message::InstalledMessage::Refresh => {
                return self.load_installed();
            }
            message::InstalledMessage::PackagesLoaded(result) => {
                self.installed.loading = false;
                self.installed.loaded = true;
                self.installed.result_version += 1;
                match result {
                    Ok(packages) => {
                        self.installed.error = None;
                        self.installed.packages = packages;
                    }
                    Err(e) => {
                        log::error!("Failed to load installed packages: {e}");
                        self.installed.error = Some(e);
                        self.installed.packages.clear();
                    }
                }
            }
            message::InstalledMessage::RemovePackage(pkg) => {
                self.confirm_dialog = Some(message::ConfirmAction::Remove(pkg));
            }
            message::InstalledMessage::RemoveComplete(result) => {
                self.active_operation = None;
                let pkg_id = self.installed.removing.take();
                match result {
                    Ok(()) => {
                        log::info!("Package removed successfully");
                        if let Some(ref id) = pkg_id {
                            self.set_browse_installed(id, false);
                        }
                        return self.load_installed();
                    }
                    Err(e) => {
                        log::error!("Remove failed: {e}");
                        self.installed.error = Some(e);
                        self.installed.result_version += 1;
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn update_updates(&mut self, msg: message::UpdatesMessage) -> Task<Message> {
        match msg {
            message::UpdatesMessage::CheckUpdates => {
                self.updates.loading = true;
                let adapter = self.adapter.clone();
                let mode = self.current_mode;
                return Task::perform(
                    async move { adapter.list_updates(mode).await.map_err(|e| e.to_string()) },
                    |result| Message::Updates(message::UpdatesMessage::UpdatesLoaded(result)),
                );
            }
            message::UpdatesMessage::UpdatesLoaded(result) => {
                self.updates.loading = false;
                self.updates.checked = true;
                self.updates.result_version += 1;
                match result {
                    Ok(updates) => {
                        self.updates.error = None;
                        self.updates.updates = updates;
                    }
                    Err(e) => {
                        log::error!("Failed to check updates: {e}");
                        self.updates.error = Some(e);
                        self.updates.updates.clear();
                    }
                }
            }
            message::UpdatesMessage::UpdatePackage(pkg) => {
                self.confirm_dialog = Some(message::ConfirmAction::Update(pkg));
            }
            message::UpdatesMessage::UpdateComplete(result) => {
                self.active_operation = None;
                let pkg_id = self.updates.updating.take();
                match result {
                    Ok(()) => {
                        log::info!("Package updated successfully");
                        match pkg_id.as_deref() {
                            Some("__all__") => self.set_browse_update_available_all(false),
                            Some(id) => self.set_browse_update_available(id, false),
                            None => {}
                        }
                        let check = self.update_updates(message::UpdatesMessage::CheckUpdates);
                        let reload = self.load_installed();
                        return Task::batch([check, reload]);
                    }
                    Err(e) => {
                        log::error!("Update failed: {e}");
                        self.updates.error = Some(e);
                        self.updates.result_version += 1;
                    }
                }
            }
            message::UpdatesMessage::UpdateAll => {
                if self.updates.updates.is_empty() || self.updates.updating.is_some() {
                    return Task::none();
                }
                self.confirm_dialog = Some(message::ConfirmAction::UpdateAll);
            }
        }
        Task::none()
    }

    fn update_settings(&mut self, msg: message::SettingsMessage) -> Task<Message> {
        match msg {
            message::SettingsMessage::ThemeChanged(theme) => {
                self.settings.selected_theme = theme;
                self.selected_theme = theme;
                self.settings.aeris_dirty = true;
                self.settings.aeris_save_success = false;
            }
            message::SettingsMessage::StartupViewChanged(view) => {
                self.settings.startup_view = view;
                self.settings.aeris_dirty = true;
                self.settings.aeris_save_success = false;
            }
            message::SettingsMessage::NotificationsToggled(v) => {
                self.settings.notifications = v;
                self.settings.aeris_dirty = true;
                self.settings.aeris_save_success = false;
            }
            message::SettingsMessage::SaveAeris => {
                self.settings.saving = true;
                self.settings.aeris_save_error = None;
                self.settings.aeris_save_success = false;

                let theme_str = match self.settings.selected_theme {
                    AppTheme::System => "system",
                    AppTheme::Light => "light",
                    AppTheme::Dark => "dark",
                }
                .to_string();

                let view_str = match self.settings.startup_view {
                    View::Dashboard => "dashboard",
                    View::Browse => "browse",
                    View::Installed => "installed",
                    View::Updates => "updates",
                    _ => "dashboard",
                }
                .to_string();

                let notifications = self.settings.notifications;

                self.aeris_config.theme = Some(theme_str);
                self.aeris_config.startup_view = Some(view_str);
                self.aeris_config.notifications = Some(notifications);

                let config = self.aeris_config.clone();
                return Task::perform(async move { config.save() }, |result| {
                    Message::Settings(message::SettingsMessage::SaveAerisResult(result))
                });
            }
            message::SettingsMessage::SaveAerisResult(result) => {
                self.settings.saving = false;
                match result {
                    Ok(()) => {
                        self.settings.aeris_dirty = false;
                        self.settings.aeris_save_success = true;
                    }
                    Err(e) => {
                        self.settings.aeris_save_error = Some(e);
                    }
                }
            }
            message::SettingsMessage::AdapterFieldChanged(key, value) => {
                self.settings.adapter_config.values.insert(key, value);
                self.settings.adapter_dirty = true;
                self.settings.adapter_save_success = false;
            }
            message::SettingsMessage::AdapterAerisFieldChanged(key, value) => {
                self.settings.adapter_settings.insert(key, value);
                self.settings.adapter_dirty = true;
                self.settings.adapter_save_success = false;
            }
            message::SettingsMessage::BrowseAdapterField(key) => {
                return Task::perform(
                    async move {
                        let handle = rfd::AsyncFileDialog::new().pick_folder().await;
                        handle.map(|h| (key, h.path().to_string_lossy().to_string()))
                    },
                    |result| match result {
                        Some((key, path)) => Message::Settings(
                            message::SettingsMessage::BrowseAdapterFieldResult(key, path),
                        ),
                        None => {
                            Message::Settings(message::SettingsMessage::BrowseAdapterFieldResult(
                                String::new(),
                                String::new(),
                            ))
                        }
                    },
                );
            }
            message::SettingsMessage::BrowseAdapterFieldResult(key, path) => {
                if !key.is_empty() && !path.is_empty() {
                    self.settings
                        .adapter_config
                        .values
                        .insert(key, ConfigValue::String(path));
                    self.settings.adapter_dirty = true;
                    self.settings.adapter_save_success = false;
                }
            }
            message::SettingsMessage::BrowseExecutableField(key) => {
                return Task::perform(
                    async move {
                        let handle = rfd::AsyncFileDialog::new()
                            .add_filter("Executable", &["sh", "bash", ""])
                            .pick_file()
                            .await;
                        handle.map(|h| (key, h.path().to_string_lossy().to_string()))
                    },
                    |result| match result {
                        Some((key, path)) => Message::Settings(
                            message::SettingsMessage::BrowseExecutableFieldResult(key, path),
                        ),
                        None => Message::Settings(
                            message::SettingsMessage::BrowseExecutableFieldResult(
                                String::new(),
                                String::new(),
                            ),
                        ),
                    },
                );
            }
            message::SettingsMessage::BrowseExecutableFieldResult(key, path) => {
                if !key.is_empty() && !path.is_empty() {
                    self.settings.adapter_settings.insert(key, path);
                    self.settings.adapter_dirty = true;
                    self.settings.adapter_save_success = false;
                }
            }
            message::SettingsMessage::RevertAdapterField(key) => {
                if let Some(original) = self.settings.adapter_config_original.values.get(&key) {
                    self.settings
                        .adapter_config
                        .values
                        .insert(key, original.clone());
                } else {
                    self.settings.adapter_config.values.remove(&key);
                }
                let has_changes =
                    self.settings.adapter_config.values.iter().any(|(k, v)| {
                        self.settings.adapter_config_original.values.get(k) != Some(v)
                    });
                self.settings.adapter_dirty = has_changes;
            }
            message::SettingsMessage::RevertAdapterAerisField(key) => {
                self.settings.adapter_settings.remove(&key);
                self.settings.adapter_dirty = true;
            }
            message::SettingsMessage::SaveAdapter => {
                self.settings.saving = true;
                self.settings.adapter_save_error = None;
                self.settings.adapter_save_success = false;

                let mut aeris_config = self.aeris_config.clone();
                let mut save_adapter_config = false;
                let mut adapter_config_to_save = self.settings.adapter_config.clone();

                if let Some(ref schema) = self.settings.adapter_schema {
                    for field in &schema.fields {
                        if field.aeris_managed {
                            if let Some(value) = self.settings.adapter_settings.get(&field.key) {
                                aeris_config.set_adapter_setting(
                                    &schema.adapter_id,
                                    &field.key,
                                    value,
                                );
                            }
                            adapter_config_to_save.values.remove(&field.key);
                        }
                    }
                    save_adapter_config = adapter_config_to_save.values.iter().any(|(k, v)| {
                        self.settings.adapter_config_original.values.get(k) != Some(v)
                    });
                }

                if let Err(e) = aeris_config.save() {
                    self.settings.adapter_save_error = Some(e);
                    self.settings.saving = false;
                    return Task::none();
                }
                self.aeris_config = aeris_config;

                if save_adapter_config {
                    let adapter = self.adapter.clone();
                    return Task::perform(
                        async move {
                            adapter
                                .set_config(&adapter_config_to_save)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |result| {
                            Message::Settings(message::SettingsMessage::SaveAdapterResult(result))
                        },
                    );
                } else {
                    self.settings.saving = false;
                    self.settings.adapter_dirty = false;
                    self.settings.adapter_save_success = true;
                }
            }
            message::SettingsMessage::SaveAdapterResult(result) => {
                self.settings.saving = false;
                match result {
                    Ok(()) => {
                        self.settings.adapter_dirty = false;
                        self.settings.adapter_save_success = true;
                    }
                    Err(e) => {
                        self.settings.adapter_save_error = Some(e);
                    }
                }
            }
        }
        Task::none()
    }

    fn prepare_adapter_config_for_save(&self) -> AdapterConfig {
        let schema = self.settings.adapter_schema.as_ref();
        let mut save = self.settings.adapter_config.clone();

        if let Some(schema) = schema {
            for field in &schema.fields {
                if matches!(field.field_type, ConfigFieldType::Number) {
                    if let Some(ConfigValue::String(s)) = save.values.get(&field.key) {
                        if let Ok(n) = s.trim().parse::<i64>() {
                            save.values
                                .insert(field.key.clone(), ConfigValue::Integer(n));
                        }
                    }
                }
            }
        }

        save
    }

    fn load_repositories(&mut self) -> Task<Message> {
        self.repositories.loading = true;
        Task::perform(
            async move {
                let config = soar_config::config::get_config();
                let repos: Vec<message::RepoInfo> = config
                    .repositories
                    .iter()
                    .map(|r| message::RepoInfo {
                        name: r.name.clone(),
                        url: r.url.clone(),
                        enabled: r.enabled.unwrap_or(true),
                        desktop_integration: r.desktop_integration.unwrap_or(false),
                        has_pubkey: r.pubkey.is_some(),
                        signature_verification: r.signature_verification.unwrap_or(false),
                        sync_interval: r.sync_interval.clone(),
                    })
                    .collect();
                Ok(repos)
            },
            |result| Message::Repositories(message::RepositoriesMessage::Loaded(result)),
        )
    }

    fn update_repositories(&mut self, msg: message::RepositoriesMessage) -> Task<Message> {
        match msg {
            message::RepositoriesMessage::Refresh => {
                return self.load_repositories();
            }
            message::RepositoriesMessage::Loaded(result) => {
                self.repositories.loading = false;
                self.repositories.loaded = true;
                self.repositories.result_version += 1;
                match result {
                    Ok(repos) => {
                        self.repositories.error = None;
                        self.repositories.repositories = repos;
                    }
                    Err(e) => {
                        log::error!("Failed to load repositories: {e}");
                        self.repositories.error = Some(e);
                    }
                }
            }
            message::RepositoriesMessage::SyncRepo(_name) => {
                // Soar syncs all repos at once; treat single-repo sync the same
                self.repositories.syncing = Some("__all__".into());
                self.repositories.sync_error = None;
                self.repositories.result_version += 1;
                let adapter = self.adapter.clone();
                return Task::perform(
                    async move { adapter.sync(None).await.map_err(|e| e.to_string()) },
                    |result| {
                        Message::Repositories(message::RepositoriesMessage::SyncComplete(result))
                    },
                );
            }
            message::RepositoriesMessage::SyncAll => {
                self.repositories.syncing = Some("__all__".into());
                self.repositories.sync_error = None;
                self.repositories.result_version += 1;
                let adapter = self.adapter.clone();
                return Task::perform(
                    async move { adapter.sync(None).await.map_err(|e| e.to_string()) },
                    |result| {
                        Message::Repositories(message::RepositoriesMessage::SyncComplete(result))
                    },
                );
            }
            message::RepositoriesMessage::SyncComplete(result) => {
                self.repositories.syncing = None;
                self.repositories.result_version += 1;
                match result {
                    Ok(()) => {
                        log::info!("Repository sync completed");
                    }
                    Err(e) => {
                        log::error!("Sync failed: {e}");
                        self.repositories.sync_error = Some(e);
                    }
                }
            }
            message::RepositoriesMessage::ToggleEnabled(name, enabled) => {
                let repo_name = name.clone();
                return Task::perform(
                    async move { crate::config::save_repo_enabled(&repo_name, enabled) },
                    |result| {
                        Message::Repositories(message::RepositoriesMessage::ToggleResult(result))
                    },
                );
            }
            message::RepositoriesMessage::ToggleResult(result) => match result {
                Ok(()) => return self.load_repositories(),
                Err(e) => self.repositories.sync_error = Some(e),
            },
        }
        Task::none()
    }

    fn execute_confirmed(&mut self, action: message::ConfirmAction) -> Task<Message> {
        match action {
            message::ConfirmAction::Install(ref pkg, mode) => {
                if let Some(query) = pkg.soar_query() {
                    self.active_operation = Some(ActiveOperation {
                        operation_type: OperationType::Install,
                        package_name: pkg.name.clone(),
                        status: OperationStatus::Starting,
                    });
                    self.browse.installing = Some(pkg.id.clone());
                    self.browse.result_version += 1;

                    if mode == PackageMode::System {
                        let _ = PrivilegeManager::detect_elevator();
                    }

                    let adapter = self.adapter.clone();
                    let settings = self.settings.adapter_settings.clone();
                    return Task::perform(
                        async move {
                            adapter
                                .install_package(&query, mode, &settings)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |result| Message::Browse(message::BrowseMessage::InstallComplete(result)),
                    );
                }
            }
            message::ConfirmAction::Remove(ref pkg) => {
                if let Some(query) = pkg.soar_query() {
                    self.active_operation = Some(ActiveOperation {
                        operation_type: OperationType::Remove,
                        package_name: pkg.name.clone(),
                        status: OperationStatus::Starting,
                    });
                    self.installed.removing = Some(pkg.id.clone());
                    self.installed.result_version += 1;
                    let adapter = self.adapter.clone();
                    return Task::perform(
                        async move {
                            adapter
                                .remove_package(&query)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |result| {
                            Message::Installed(message::InstalledMessage::RemoveComplete(result))
                        },
                    );
                }
            }
            message::ConfirmAction::Update(ref pkg) => {
                if let Some(query) = pkg.soar_query() {
                    self.active_operation = Some(ActiveOperation {
                        operation_type: OperationType::Update,
                        package_name: pkg.name.clone(),
                        status: OperationStatus::Starting,
                    });
                    self.updates.updating = Some(pkg.id.clone());
                    self.updates.result_version += 1;
                    let adapter = self.adapter.clone();
                    return Task::perform(
                        async move {
                            adapter
                                .update_package(&query)
                                .await
                                .map_err(|e| e.to_string())
                        },
                        |result| Message::Updates(message::UpdatesMessage::UpdateComplete(result)),
                    );
                }
            }
            message::ConfirmAction::UpdateAll => {
                self.active_operation = Some(ActiveOperation {
                    operation_type: OperationType::UpdateAll,
                    package_name: "all packages".into(),
                    status: OperationStatus::Starting,
                });
                self.updates.updating = Some("__all__".into());
                self.updates.result_version += 1;
                let adapter = self.adapter.clone();
                return Task::perform(
                    async move { adapter.update_all().await.map_err(|e| e.to_string()) },
                    |result| Message::Updates(message::UpdatesMessage::UpdateComplete(result)),
                );
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.sidebar_view();
        let content = match self.current_view {
            View::Dashboard => {
                let stats = views::dashboard::DashboardStats {
                    installed_count: self.installed.packages.len(),
                    repo_count: self.adapter.repo_count(),
                };
                views::dashboard::view(&stats)
            }
            View::Browse => views::browse::view(&self.browse),
            View::Installed => views::installed::view(&self.installed),
            View::Updates => views::updates::view(&self.updates),
            View::Settings => views::settings::view(&self.settings),
            View::Repositories => views::repositories::view(&self.repositories),
            View::AdapterInfo => views::adapter_info::view(self.adapter.info()),
        };

        let main: Element<'_, Message> = if let Some(ref op) = self.active_operation {
            column![content, self.progress_bar_view(op)]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            content
        };

        let base = row![sidebar, main];

        if let Some(ref action) = self.confirm_dialog {
            modal(
                base,
                self.confirm_dialog_view(action),
                Message::CancelAction,
            )
        } else if let Some(ref pkg) = self.browse.selected_package {
            modal(
                base,
                views::browse::package_detail_view(pkg),
                Message::Browse(message::BrowseMessage::CloseDetail),
            )
        } else {
            base.into()
        }
    }

    fn confirm_dialog_view(&self, action: &message::ConfirmAction) -> Element<'_, Message> {
        let is_destructive = matches!(action, message::ConfirmAction::Remove(_));
        let is_install = matches!(action, message::ConfirmAction::Install(..));

        let has_system = self.adapter.info().capabilities.supports_system_packages;
        static MODES: [PackageMode; 2] = [PackageMode::User, PackageMode::System];

        let (title, description, mode_section): (_, _, Element<'_, Message>) = match action {
            message::ConfirmAction::Install(pkg, mode) => {
                let current_mode = *mode;
                let has_multiple_modes = has_system;
                let is_system = current_mode == PackageMode::System;

                let mode_selector: Element<'_, Message> = if has_multiple_modes {
                    column![
                        row![
                            text("Install mode:").size(13),
                            iced::widget::pick_list(&MODES[..], Some(current_mode), |m| {
                                Message::Browse(message::BrowseMessage::InstallModeChanged(m))
                            },)
                            .width(120),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                        if is_system {
                            text("Requires administrator privileges").size(11)
                        } else {
                            text("").size(11)
                        }
                    ]
                    .spacing(4)
                    .into()
                } else {
                    column![].into()
                };

                (
                    "Install Package",
                    format!("{} {}", pkg.name, pkg.version),
                    mode_selector,
                )
            }
            message::ConfirmAction::Remove(pkg) => (
                "Remove Package",
                format!("{} {}", pkg.name, pkg.version),
                column![].into(),
            ),
            message::ConfirmAction::Update(pkg) => (
                "Update Package",
                format!("{} {}", pkg.name, pkg.version),
                column![].into(),
            ),
            message::ConfirmAction::UpdateAll => (
                "Update All",
                "All packages with available updates will be updated.".to_string(),
                column![].into(),
            ),
        };

        let cancel_btn = button(text("Cancel").size(14))
            .on_press(Message::CancelAction)
            .style(button::secondary)
            .padding([8, 16]);

        let confirm_btn = button(text("Confirm").size(14))
            .on_press(Message::ConfirmAction)
            .padding([8, 16]);

        let confirm_btn = if is_destructive {
            confirm_btn.style(button::danger)
        } else {
            confirm_btn.style(button::primary)
        };

        let mut content = column![text(title).size(18), text(description).size(14),].spacing(12);

        if is_install {
            content = content.push(mode_section);
        }

        content = content.push(row![cancel_btn, confirm_btn].spacing(8));

        container(
            content
                .spacing(16)
                .padding(24)
                .align_x(iced::Alignment::Center),
        )
        .style(container::rounded_box)
        .width(360)
        .into()
    }

    fn sidebar_view(&self) -> Element<'_, Message> {
        let nav_items = [
            (View::Dashboard, "Dashboard"),
            (View::Browse, "Browse"),
            (View::Installed, "Installed"),
            (View::Updates, "Updates"),
            (View::Repositories, "Repositories"),
            (View::AdapterInfo, "Adapter"),
        ];

        let mut nav = column![].spacing(4).padding(8);

        for (view, label) in nav_items {
            let is_active = self.current_view == view;
            let btn = button(text(label).size(14).width(Length::Fill).center())
                .on_press(Message::NavigateTo(view))
                .width(Length::Fill)
                .padding([8, 12]);

            let btn = if is_active {
                btn.style(button::primary)
            } else {
                btn.style(button::text)
            };

            nav = nav.push(btn);
        }

        // Settings button at the bottom
        let is_settings_active = self.current_view == View::Settings;
        let settings_btn = button(text("Settings").size(14).width(Length::Fill).center())
            .on_press(Message::NavigateTo(View::Settings))
            .width(Length::Fill)
            .padding([8, 12]);

        let settings_btn = if is_settings_active {
            settings_btn.style(button::primary)
        } else {
            settings_btn.style(button::text)
        };

        let settings_section = column![].spacing(4).padding(8).push(settings_btn);

        container(
            column![
                text(APP_NAME).size(20).center().width(Length::Fill),
                rule::horizontal(1),
                nav,
                space(),
                rule::horizontal(1),
                settings_section,
            ]
            .spacing(8)
            .height(Length::Fill),
        )
        .width(180)
        .height(Length::Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.active_operation.is_some() {
            iced::time::every(std::time::Duration::from_millis(50)).map(|_| Message::ProgressTick)
        } else {
            Subscription::none()
        }
    }

    fn handle_soar_event(&mut self, event: SoarEvent) {
        let op = match self.active_operation.as_mut() {
            Some(op) => op,
            None => return,
        };

        match event {
            SoarEvent::DownloadStarting { total, .. } => {
                op.status = OperationStatus::Downloading { current: 0, total };
            }
            SoarEvent::DownloadResuming { current, total, .. }
            | SoarEvent::DownloadProgress { current, total, .. } => {
                op.status = OperationStatus::Downloading { current, total };
            }
            SoarEvent::DownloadComplete { .. } => {
                op.status = OperationStatus::Downloading {
                    current: 1,
                    total: 1,
                };
            }
            SoarEvent::Verifying { stage, .. } => {
                let label = match stage {
                    VerifyStage::Checksum => "checksum",
                    VerifyStage::Signature => "signature",
                    VerifyStage::Passed => "passed",
                    VerifyStage::Failed(ref e) => {
                        op.status = OperationStatus::Failed(format!("Verification failed: {e}"));
                        return;
                    }
                };
                op.status = OperationStatus::Verifying(label.into());
            }
            SoarEvent::Installing { stage, .. } => {
                let label = match stage {
                    InstallStage::Extracting => "extracting",
                    InstallStage::ExtractingNested => "extracting nested",
                    InstallStage::LinkingBinaries => "linking binaries",
                    InstallStage::DesktopIntegration => "desktop integration",
                    InstallStage::SetupPortable => "setting up portable",
                    InstallStage::RecordingDatabase => "recording to database",
                    InstallStage::RunningHook(ref h) => {
                        op.status = OperationStatus::Installing(format!("hook: {h}"));
                        return;
                    }
                    InstallStage::Complete => "complete",
                };
                op.status = OperationStatus::Installing(label.into());
            }
            SoarEvent::Removing { stage, .. } => {
                let label = match stage {
                    RemoveStage::RunningHook(ref h) => {
                        op.status = OperationStatus::Removing(format!("hook: {h}"));
                        return;
                    }
                    RemoveStage::UnlinkingBinaries => "unlinking binaries",
                    RemoveStage::UnlinkingDesktop => "unlinking desktop",
                    RemoveStage::UnlinkingIcons => "unlinking icons",
                    RemoveStage::RemovingDirectory => "removing directory",
                    RemoveStage::CleaningDatabase => "cleaning database",
                    RemoveStage::Complete { .. } => "complete",
                };
                op.status = OperationStatus::Removing(label.into());
            }
            SoarEvent::OperationComplete { .. } => {
                op.status = OperationStatus::Completed;
            }
            SoarEvent::OperationFailed { error, .. } => {
                op.status = OperationStatus::Failed(error);
            }
            _ => {}
        }
    }

    fn progress_bar_view(&self, op: &ActiveOperation) -> Element<'_, Message> {
        let label = text(format!("{} {}", op.operation_type, op.package_name)).size(13);
        let status = text(op.status.label()).size(12);

        let mut content = column![label, status].spacing(4).padding([8, 16]);

        if let Some(progress) = op.status.progress() {
            content = content.push(progress_bar(0.0..=1.0, progress));
        }

        container(content)
            .width(Length::Fill)
            .style(|theme: &iced::Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(palette.background.weak.color.into()),
                    border: iced::Border {
                        width: 1.0,
                        color: palette.background.strong.color,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            })
            .into()
    }

    pub fn theme(&self) -> Option<iced::Theme> {
        match self.selected_theme {
            AppTheme::System => None,
            AppTheme::Light => Some(iced::Theme::Light),
            AppTheme::Dark => Some(iced::Theme::Dark),
        }
    }
}

fn modal<'a>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
    on_blur: Message,
) -> Element<'a, Message> {
    stack![
        base.into(),
        opaque(
            mouse_area(center(opaque(content)).style(|_theme| {
                container::Style {
                    background: Some(
                        Color {
                            a: 0.8,
                            ..Color::BLACK
                        }
                        .into(),
                    ),
                    ..container::Style::default()
                }
            }))
            .on_press(on_blur)
        )
    ]
    .into()
}
