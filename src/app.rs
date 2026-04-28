pub mod message;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::*;
use soar_events::SoarEvent;

use crate::{
    adapters::soar::SoarAdapter,
    config::AerisConfig,
    core::{
        adapter::Adapter, adapter_manager::AdapterManager, privilege::PackageMode,
        registry::PluginEntry,
    },
    styles, theme, views,
};

pub use message::{ConfirmAction, RepoInfo};

actions!(app, [Escape, Confirm]);

pub fn bind_app_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("escape", Escape, None),
        KeyBinding::new("enter", Confirm, None),
    ]);
}

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
            View::AdapterInfo => write!(f, "Adapters"),
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Success,
    Error,
    Info,
}

pub struct Toast {
    pub id: u64,
    pub level: ToastLevel,
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct RunPicker {
    pub package_name: String,
    /// Unique key (id@version) used to track running processes per package.
    pub package_key: String,
    /// Absolute paths of executable candidates inside the install dir.
    pub binaries: Vec<std::path::PathBuf>,
}

/// A binary the user launched via Run. Tracked so we can offer a Stop button
/// and reap exited processes periodically.
pub struct RunningProcess {
    pub id: u64,
    pub label: String,
    pub child: std::process::Child,
}

/// Find user-runnable binaries for a package: symlinks in `bin_path` whose
/// canonicalized target lives inside `install_path`. Avoids launching
/// internal helpers/libraries inside the install dir that happen to be
/// marked executable but aren't meant to be invoked directly.
pub(crate) fn list_package_binaries(
    install_path: &std::path::Path,
    bin_path: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let canonical_install = match std::fs::canonicalize(install_path) {
        Ok(p) => p,
        Err(_) => return out,
    };
    let read = match std::fs::read_dir(bin_path) {
        Ok(r) => r,
        Err(_) => return out,
    };
    for entry in read.flatten() {
        let path = entry.path();
        let symlink_meta = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !symlink_meta.file_type().is_symlink() {
            continue;
        }
        let canonical_target = match std::fs::canonicalize(&path) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if canonical_target.starts_with(&canonical_install) {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// Best-effort lookup of soar's bin directory for the active profile.
pub(crate) fn soar_active_bin_path() -> Option<std::path::PathBuf> {
    let config = soar_config::config::get_config();
    let active = &config.default_profile;
    let profile = config.profile.get(active)?;
    Some(std::path::PathBuf::from(&profile.root_path).join("bin"))
}

#[derive(Default)]
pub struct AdapterViewState {
    pub registry_plugins: Vec<PluginEntry>,
    pub registry_loading: bool,
    pub registry_error: Option<String>,
    pub installing_plugin: Option<String>,
    pub removing_plugin: Option<String>,
    pub repos_by_adapter: HashMap<String, Vec<RepoInfo>>,
    pub repos_loading: HashMap<String, bool>,
    pub repos_loaded: HashMap<String, bool>,
    pub repos_error: HashMap<String, String>,
    pub repos_version: u64,
    pub syncing: Option<String>,
    pub sync_error: Option<String>,
    pub profiles_by_adapter: HashMap<String, Vec<crate::core::profile::Profile>>,
    pub profiles_loading: HashMap<String, bool>,
    pub profiles_error: HashMap<String, String>,
    pub switching_profile: Option<String>,
}

pub struct App {
    pub(crate) selected_theme: AppTheme,
    pub(crate) current_view: View,
    sidebar_expanded: bool,
    pub(crate) aeris_config: AerisConfig,
    pub(crate) adapter: Arc<SoarAdapter>,
    pub(crate) adapter_manager: AdapterManager,
    pub(crate) adapter_view: AdapterViewState,
    pub(crate) confirm_dialog: Option<ConfirmAction>,
    pub(crate) run_picker: Option<RunPicker>,
    /// Running processes launched via Run, keyed by package unique_key.
    pub(crate) running_processes: HashMap<String, Vec<RunningProcess>>,
    next_run_id: u64,
    event_receiver: std::sync::mpsc::Receiver<SoarEvent>,
    active_operation: Option<ActiveOperation>,
    package_progress: HashMap<String, OperationStatus>,
    next_operation_id: u64,
    toasts: Vec<Toast>,
    next_toast_id: u64,
    /// Latest BatchProgress event from any adapter: (adapter_id, completed, total, failed).
    batch_progress: Option<(String, u32, u32, u32)>,
    progress_sender: crate::core::adapter::ProgressSender,
    progress_receiver: tokio::sync::mpsc::UnboundedReceiver<crate::core::adapter::ProgressEvent>,
    pub(crate) selected_install_mode: PackageMode,
    pub(crate) current_mode: PackageMode,

    // View states
    pub(crate) browse_state: views::browse::BrowseState,
    pub(crate) installed_state: views::installed::InstalledState,
    pub(crate) updates_state: views::updates::UpdatesState,
    pub(crate) settings_state: views::settings::SettingsState,

    // Text input entities
    pub(crate) search_input: Entity<crate::components::TextInput>,
}

impl App {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let aeris_config = AerisConfig::load();
        soar_config::config::init().expect("Failed to load soar config");
        let soar_config = soar_config::config::get_config();

        let selected_theme = aeris_config.theme();
        let startup_view = aeris_config.startup_view();

        let (adapter, event_receiver) =
            SoarAdapter::new(soar_config).expect("Failed to initialize Soar adapter");
        let adapter = Arc::new(adapter);

        let mut adapter_manager = AdapterManager::new();
        adapter_manager.register(adapter.clone() as Arc<dyn Adapter>);

        for result in crate::adapters::wasm::load_all_plugins() {
            match result {
                Ok(wasm_adapter) => {
                    log::info!("Loaded plugin: {}", wasm_adapter.info().id);
                    adapter_manager.register(Arc::new(wasm_adapter));
                }
                Err(e) => log::warn!("Failed to load plugin: {e}"),
            }
        }

        let disabled: std::collections::HashSet<String> =
            aeris_config.disabled_adapters.iter().cloned().collect();
        adapter_manager.set_disabled(disabled);

        let default_mode = PackageMode::User;
        let (progress_sender, progress_receiver) = tokio::sync::mpsc::unbounded_channel();

        let settings_state = views::settings::SettingsState::load(&aeris_config, adapter.as_ref());

        let search_input = cx.new(|cx| crate::components::TextInput::new(cx, "Search packages..."));

        // Poll for progress events periodically
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let should_continue = cx
                        .update(|cx| {
                            this.update(cx, |app, cx| {
                                app.drain_progress(cx);
                            })
                            .is_ok()
                        })
                        .is_ok();
                    if !should_continue {
                        break;
                    }
                }
            },
        )
        .detach();

        Self {
            selected_theme,
            current_view: startup_view,
            sidebar_expanded: false,
            aeris_config,
            adapter,
            adapter_manager,
            adapter_view: AdapterViewState::default(),
            confirm_dialog: None,
            run_picker: None,
            running_processes: HashMap::new(),
            next_run_id: 1,
            event_receiver,
            active_operation: None,
            package_progress: HashMap::new(),
            next_operation_id: 1,
            toasts: Vec::new(),
            next_toast_id: 1,
            batch_progress: None,
            progress_sender,
            progress_receiver,
            selected_install_mode: default_mode,
            current_mode: default_mode,
            browse_state: views::browse::BrowseState::default(),
            installed_state: views::installed::InstalledState::default(),
            updates_state: views::updates::UpdatesState::default(),
            settings_state,
            search_input,
        }
    }

    fn navigate_to(&mut self, view: View, _cx: &mut Context<Self>) {
        self.current_view = view;
    }

    pub fn perform_search(&mut self, cx: &mut Context<Self>) {
        let query = self.browse_state.search_query.clone();
        if query.is_empty() {
            return;
        }

        self.browse_state.loading = true;
        self.browse_state.error = None;
        self.browse_state.search_debounce_version += 1;
        let version = self.browse_state.search_debounce_version;

        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .filter(|a| self.adapter_manager.is_enabled(&a.info().id))
            .collect();
        let mode = self.current_mode;

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let results = crate::tokio_spawn(async move {
                    let mut results = Vec::new();
                    for adapter in &manager_adapters {
                        if adapter.capabilities().can_search {
                            match adapter.search(&query, None, mode).await {
                                Ok(pkgs) => results.extend(pkgs),
                                Err(e) => {
                                    log::warn!("Search failed for {}: {e}", adapter.info().id)
                                }
                            }
                        }
                    }
                    results
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        if app.browse_state.search_debounce_version == version {
                            app.browse_state.search_results = results;
                            app.browse_state.loading = false;
                            app.browse_state.has_searched = true;
                            app.browse_state.result_version += 1;
                            cx.notify();
                        }
                    })
                });
            },
        )
        .detach();
    }

    // ---- Business logic stubs ----

    pub fn load_installed(&mut self, cx: &mut Context<Self>) {
        self.installed_state.loading = true;
        self.installed_state.error = None;

        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();
        let mode = self.current_mode;

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let (all_packages, updatable_adapters) = crate::tokio_spawn(async move {
                    let mut all_packages = Vec::new();
                    let mut updatable_adapters = std::collections::HashSet::new();

                    for adapter in &manager_adapters {
                        match adapter.list_installed(mode).await {
                            Ok(pkgs) => all_packages.extend(pkgs),
                            Err(e) => log::warn!("List installed failed: {e}"),
                        }
                        let caps = adapter.capabilities();
                        if caps.can_update && !caps.can_list_updates {
                            updatable_adapters.insert(adapter.info().id.clone());
                        }
                    }
                    (all_packages, updatable_adapters)
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.installed_state.packages = all_packages;
                        app.installed_state.loading = false;
                        app.installed_state.loaded = true;
                        app.installed_state.result_version += 1;
                        app.installed_state.updatable_adapters = updatable_adapters;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn check_updates(&mut self, cx: &mut Context<Self>) {
        self.updates_state.loading = true;
        self.updates_state.error = None;

        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();
        let mode = self.current_mode;

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let (all_updates, no_update_listing) = crate::tokio_spawn(async move {
                    let mut all_updates = Vec::new();
                    let mut no_update_listing = Vec::new();

                    for adapter in &manager_adapters {
                        let caps = adapter.capabilities();
                        if caps.can_list_updates {
                            match adapter.list_updates(mode).await {
                                Ok(updates) => all_updates.extend(updates),
                                Err(e) => log::warn!("Check updates failed: {e}"),
                            }
                        } else if caps.can_update {
                            no_update_listing
                                .push((adapter.info().id.clone(), adapter.info().name.clone()));
                        }
                    }
                    (all_updates, no_update_listing)
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.updates_state.updates = all_updates;
                        app.updates_state.loading = false;
                        app.updates_state.checked = true;
                        app.updates_state.no_update_listing = no_update_listing;
                        app.updates_state.result_version += 1;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn update_all(&mut self, cx: &mut Context<Self>) {
        if self.updates_state.updates.is_empty() {
            return;
        }
        self.updates_state.updating = Some("__all__".to_string());
        let packages: Vec<_> = self
            .updates_state
            .updates
            .iter()
            .map(|u| u.package.clone())
            .collect();
        let mode = self.current_mode;
        let progress_sender = self.progress_sender.clone();
        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        let count = packages.len();
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let errors = crate::tokio_spawn(async move {
                    let mut by_adapter: HashMap<String, Vec<crate::core::package::Package>> =
                        HashMap::new();
                    for pkg in &packages {
                        by_adapter
                            .entry(pkg.adapter_id.clone())
                            .or_default()
                            .push(pkg.clone());
                    }

                    let mut errors: Vec<String> = Vec::new();
                    for (adapter_id, pkgs) in by_adapter {
                        if let Some(adapter) =
                            manager_adapters.iter().find(|a| a.info().id == adapter_id)
                        {
                            match adapter
                                .update(&pkgs, Some(progress_sender.clone()), mode)
                                .await
                            {
                                Ok(_) => log::info!("Updated packages for {adapter_id}"),
                                Err(e) => {
                                    log::error!("Update failed for {adapter_id}: {e}");
                                    errors.push(format!("{e}"));
                                }
                            }
                        }
                    }
                    errors
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.updates_state.updating = None;
                        app.updates_state.updates.clear();
                        app.updates_state.result_version += 1;
                        if errors.is_empty() {
                            app.add_toast(ToastLevel::Success, format!("Updated {count} packages"));
                        } else {
                            for err in &errors {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Failed to update: {err}"),
                                );
                            }
                        }
                        app.installed_state.loaded = false;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn update_selected(&mut self, cx: &mut Context<Self>) {
        if self.updates_state.selected.is_empty() {
            return;
        }
        self.updates_state.updating = Some("__batch__".to_string());
        let selected = self.updates_state.selected.clone();
        let packages: Vec<_> = self
            .updates_state
            .updates
            .iter()
            .filter(|u| selected.contains(&u.package.id))
            .map(|u| u.package.clone())
            .collect();
        let mode = self.current_mode;
        let progress_sender = self.progress_sender.clone();

        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        let count = packages.len();
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let errors = crate::tokio_spawn(async move {
                    let mut by_adapter: HashMap<String, Vec<crate::core::package::Package>> =
                        HashMap::new();
                    for pkg in &packages {
                        by_adapter
                            .entry(pkg.adapter_id.clone())
                            .or_default()
                            .push(pkg.clone());
                    }

                    let mut errors: Vec<String> = Vec::new();
                    for (adapter_id, pkgs) in by_adapter {
                        if let Some(adapter) =
                            manager_adapters.iter().find(|a| a.info().id == adapter_id)
                        {
                            match adapter
                                .update(&pkgs, Some(progress_sender.clone()), mode)
                                .await
                            {
                                Ok(_) => log::info!("Updated selected packages for {adapter_id}"),
                                Err(e) => {
                                    log::error!("Update selected failed for {adapter_id}: {e}");
                                    errors.push(format!("{e}"));
                                }
                            }
                        }
                    }
                    errors
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.updates_state.updating = None;
                        app.updates_state.selected.clear();
                        app.updates_state.result_version += 1;
                        if errors.is_empty() {
                            app.add_toast(ToastLevel::Success, format!("Updated {count} packages"));
                        } else {
                            for err in &errors {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Failed to update: {err}"),
                                );
                            }
                        }
                        app.installed_state.loaded = false;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn install_selected_browse(&mut self, cx: &mut Context<Self>) {
        if self.browse_state.selected.is_empty() {
            return;
        }
        self.browse_state.installing = Some("__batch__".to_string());
        let selected = self.browse_state.selected.clone();
        let packages: Vec<_> = self
            .browse_state
            .search_results
            .iter()
            .filter(|p| selected.contains(&p.id) && !p.installed)
            .cloned()
            .collect();
        let pkg_ids: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
        let progress_keys: Vec<String> = packages
            .iter()
            .map(|p| crate::core::adapter::progress_key(&p.adapter_id, &p.id))
            .collect();
        for key in &progress_keys {
            self.browse_state
                .package_progress
                .insert(key.clone(), OperationStatus::Starting);
        }
        let mode = self.current_mode;
        let progress_sender = self.progress_sender.clone();

        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                crate::tokio_spawn(async move {
                    let mut by_adapter: HashMap<String, Vec<crate::core::package::Package>> =
                        HashMap::new();
                    for pkg in &packages {
                        by_adapter
                            .entry(pkg.adapter_id.clone())
                            .or_default()
                            .push(pkg.clone());
                    }

                    for (adapter_id, pkgs) in by_adapter {
                        if let Some(adapter) =
                            manager_adapters.iter().find(|a| a.info().id == adapter_id)
                        {
                            match adapter
                                .install(&pkgs, Some(progress_sender.clone()), mode)
                                .await
                            {
                                Ok(_) => log::info!("Installed selected packages for {adapter_id}"),
                                Err(e) => {
                                    log::error!("Install selected failed for {adapter_id}: {e}")
                                }
                            }
                        }
                    }
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.browse_state.installing = None;
                        app.browse_state.selected.clear();
                        // Mark installed in search results
                        for p in &mut app.browse_state.search_results {
                            if pkg_ids.contains(&p.id) {
                                p.installed = true;
                            }
                        }
                        for key in &progress_keys {
                            app.browse_state.package_progress.remove(key);
                        }
                        app.browse_state.result_version += 1;
                        app.installed_state.loaded = false;
                        app.add_toast(
                            ToastLevel::Success,
                            format!("Installed {} packages", pkg_ids.len()),
                        );
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn remove_selected_installed(&mut self, cx: &mut Context<Self>) {
        if self.installed_state.selected.is_empty() {
            return;
        }
        self.installed_state.removing = Some("__batch__".to_string());
        let selected = self.installed_state.selected.clone();
        let packages: Vec<_> = self
            .installed_state
            .packages
            .iter()
            .filter(|p| selected.contains(&p.unique_key()))
            .map(|p| p.package.clone())
            .collect();
        let progress_keys: Vec<String> = packages
            .iter()
            .map(|p| crate::core::adapter::progress_key(&p.adapter_id, &p.id))
            .collect();
        for key in &progress_keys {
            self.installed_state
                .package_progress
                .insert(key.clone(), OperationStatus::Starting);
        }
        let count = packages.len();
        let mode = self.current_mode;
        let progress_sender = self.progress_sender.clone();

        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let errors = crate::tokio_spawn(async move {
                    let mut by_adapter: HashMap<String, Vec<crate::core::package::Package>> =
                        HashMap::new();
                    for pkg in &packages {
                        by_adapter
                            .entry(pkg.adapter_id.clone())
                            .or_default()
                            .push(pkg.clone());
                    }

                    let mut errors: Vec<String> = Vec::new();
                    for (adapter_id, pkgs) in by_adapter {
                        if let Some(adapter) =
                            manager_adapters.iter().find(|a| a.info().id == adapter_id)
                        {
                            match adapter
                                .remove(&pkgs, Some(progress_sender.clone()), mode)
                                .await
                            {
                                Ok(_) => log::info!("Removed selected packages for {adapter_id}"),
                                Err(e) => {
                                    log::error!("Remove selected failed for {adapter_id}: {e}");
                                    errors.push(format!("{e}"));
                                }
                            }
                        }
                    }
                    errors
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.installed_state.removing = None;
                        app.installed_state.selected.clear();
                        for key in &progress_keys {
                            app.installed_state.package_progress.remove(key);
                        }
                        app.installed_state.result_version += 1;
                        if errors.is_empty() {
                            app.add_toast(ToastLevel::Success, format!("Removed {count} packages"));
                        } else {
                            for err in &errors {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Failed to remove: {err}"),
                                );
                            }
                        }
                        app.load_installed(cx);
                    })
                });
            },
        )
        .detach();
    }

    pub fn sync_all_repos(&mut self, cx: &mut Context<Self>) {
        if self.adapter_view.syncing.is_some() {
            return;
        }
        self.adapter_view.syncing = Some("__all__".to_string());
        self.adapter_view.sync_error = None;

        let progress_sender = self.progress_sender.clone();
        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let errors = crate::tokio_spawn(async move {
                    let mut errors: Vec<(String, String)> = Vec::new();
                    for adapter in &manager_adapters {
                        if adapter.capabilities().can_sync {
                            match adapter.sync(Some(progress_sender.clone())).await {
                                Ok(_) => log::info!("Synced {}", adapter.info().id),
                                Err(e) => {
                                    log::warn!("Sync failed for {}: {e}", adapter.info().id);
                                    errors.push((adapter.info().id.clone(), format!("{e}")));
                                }
                            }
                        }
                    }
                    errors
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.adapter_view.syncing = None;
                        app.adapter_view.repos_version += 1;
                        if errors.is_empty() {
                            app.add_toast(ToastLevel::Success, "Repositories synced".into());
                            app.adapter_view.sync_error = None;
                        } else {
                            for (adapter_id, err) in &errors {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Sync failed for {adapter_id}: {err}"),
                                );
                            }
                            app.adapter_view.sync_error = Some(
                                errors
                                    .iter()
                                    .map(|(id, e)| format!("{id}: {e}"))
                                    .collect::<Vec<_>>()
                                    .join("; "),
                            );
                        }
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn load_repos(&mut self, cx: &mut Context<Self>) {
        let adapters: Vec<(String, Arc<dyn Adapter>)> = self
            .adapter_manager
            .list_adapters_with_status()
            .iter()
            .filter(|(info, enabled)| *enabled && info.capabilities.can_list_repos)
            .filter_map(|(info, _)| {
                self.adapter_manager
                    .get_adapter(&info.id)
                    .map(|a| (info.id.clone(), a))
            })
            .collect();

        for (id, _) in &adapters {
            self.adapter_view.repos_loading.insert(id.clone(), true);
            self.adapter_view.repos_error.remove(id);
        }

        for (adapter_id, adapter) in adapters {
            cx.spawn(
                async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    let repos = crate::tokio_spawn(async move {
                        match adapter.list_repositories().await {
                            Ok(repos) => Ok(repos
                                .into_iter()
                                .map(|r| RepoInfo {
                                    name: r.name,
                                    url: r.url,
                                    enabled: r.enabled,
                                    desktop_integration: false,
                                    has_pubkey: false,
                                    signature_verification: false,
                                    sync_interval: None,
                                })
                                .collect::<Vec<_>>()),
                            Err(e) => Err(format!("{e}")),
                        }
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("{e}")));

                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            match repos {
                                Ok(repos) => {
                                    app.adapter_view
                                        .repos_by_adapter
                                        .insert(adapter_id.clone(), repos);
                                }
                                Err(e) => {
                                    app.adapter_view.repos_error.insert(adapter_id.clone(), e);
                                }
                            }
                            app.adapter_view
                                .repos_loading
                                .insert(adapter_id.clone(), false);
                            app.adapter_view
                                .repos_loaded
                                .insert(adapter_id.clone(), true);
                            app.adapter_view.repos_version += 1;
                            cx.notify();
                        })
                    });
                },
            )
            .detach();
        }
    }

    pub fn fetch_registry(&mut self, cx: &mut Context<Self>) {
        self.adapter_view.registry_loading = true;
        self.adapter_view.registry_error = None;

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result = crate::core::registry::fetch_registry(None);

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        match result {
                            Ok(registry) => {
                                app.adapter_view.registry_plugins = registry.plugins;
                            }
                            Err(e) => {
                                app.adapter_view.registry_error = Some(e);
                            }
                        }
                        app.adapter_view.registry_loading = false;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn save_aeris_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_state.saving = true;
        self.settings_state.aeris_save_error = None;
        self.settings_state.aeris_save_success = false;

        // Apply settings
        self.aeris_config.theme = Some(match self.settings_state.selected_theme {
            AppTheme::System => "system".to_string(),
            AppTheme::Light => "light".to_string(),
            AppTheme::Dark => "dark".to_string(),
        });
        self.aeris_config.startup_view = Some(match self.settings_state.startup_view {
            View::Dashboard => "dashboard".to_string(),
            View::Browse => "browse".to_string(),
            View::Installed => "installed".to_string(),
            View::Updates => "updates".to_string(),
            _ => "dashboard".to_string(),
        });
        self.aeris_config.notifications = Some(self.settings_state.notifications);

        self.selected_theme = self.settings_state.selected_theme;

        match self.aeris_config.save() {
            Ok(_) => {
                self.settings_state.aeris_save_success = true;
                self.settings_state.aeris_dirty = false;
            }
            Err(e) => {
                self.settings_state.aeris_save_error = Some(e);
            }
        }
        self.settings_state.saving = false;
        cx.notify();
    }

    pub fn save_adapter_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_state.saving = true;
        self.settings_state.adapter_save_error = None;
        self.settings_state.adapter_save_success = false;

        let config = self.settings_state.adapter_config.clone();
        let adapter = self.adapter.clone();
        let mode = self.current_mode;

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result =
                    crate::tokio_spawn(
                        async move { adapter.set_config_for_mode(&config, mode).await },
                    )
                    .await
                    .unwrap_or_else(|e| {
                        Err(crate::core::adapter::AdapterError::Other(format!("{e}")))
                    });

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        match result {
                            Ok(_) => {
                                app.settings_state.adapter_save_success = true;
                                app.settings_state.adapter_dirty = false;
                                app.settings_state.adapter_config_original =
                                    app.settings_state.adapter_config.clone();
                            }
                            Err(e) => {
                                app.settings_state.adapter_save_error = Some(format!("{e}"));
                            }
                        }
                        app.settings_state.saving = false;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn toggle_adapter_config(&mut self, key: &str, cx: &mut Context<Self>) {
        use crate::core::config::ConfigValue;
        let current = self
            .settings_state
            .adapter_config
            .values
            .get(key)
            .and_then(|v| match v {
                ConfigValue::Bool(b) => Some(*b),
                _ => None,
            })
            .unwrap_or(false);
        self.settings_state
            .adapter_config
            .values
            .insert(key.to_string(), ConfigValue::Bool(!current));
        self.settings_state.adapter_dirty =
            self.settings_state.adapter_config != self.settings_state.adapter_config_original;
        cx.notify();
    }

    pub fn load_profiles(&mut self, adapter_id: &str, cx: &mut Context<Self>) {
        let adapter = match self.adapter_manager.get_adapter(adapter_id) {
            Some(a) => a,
            None => return,
        };
        if !adapter.capabilities().has_profiles {
            return;
        }
        let aid = adapter_id.to_string();
        self.adapter_view.profiles_loading.insert(aid.clone(), true);
        self.adapter_view.profiles_error.remove(&aid);
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result = crate::tokio_spawn(async move { adapter.list_profiles().await })
                    .await
                    .unwrap_or_else(|e| {
                        Err(crate::core::adapter::AdapterError::Other(format!("{e}")))
                    });
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.adapter_view.profiles_loading.insert(aid.clone(), false);
                        match result {
                            Ok(profiles) => {
                                app.adapter_view
                                    .profiles_by_adapter
                                    .insert(aid.clone(), profiles);
                            }
                            Err(e) => {
                                app.adapter_view
                                    .profiles_error
                                    .insert(aid.clone(), format!("{e}"));
                            }
                        }
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn switch_to_profile(
        &mut self,
        adapter_id: &str,
        profile_id: &str,
        cx: &mut Context<Self>,
    ) {
        let adapter = match self.adapter_manager.get_adapter(adapter_id) {
            Some(a) => a,
            None => return,
        };
        if !adapter.capabilities().has_profiles {
            return;
        }
        self.adapter_view.switching_profile = Some(profile_id.to_string());
        let aid = adapter_id.to_string();
        let pid = profile_id.to_string();
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result =
                    crate::tokio_spawn(
                        async move { adapter.switch_profile(&pid).await.map(|_| pid) },
                    )
                    .await
                    .unwrap_or_else(|e| {
                        Err(crate::core::adapter::AdapterError::Other(format!("{e}")))
                    });
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.adapter_view.switching_profile = None;
                        match result {
                            Ok(switched_to) => {
                                app.add_toast(
                                    ToastLevel::Success,
                                    format!("Switched to profile {switched_to}"),
                                );
                                app.load_profiles(&aid, cx);
                                // Profile change affects installed packages location
                                app.installed_state.loaded = false;
                            }
                            Err(e) => {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Failed to switch profile: {e}"),
                                );
                            }
                        }
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn revert_adapter_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_state.adapter_config = self.settings_state.adapter_config_original.clone();
        self.settings_state.adapter_dirty = false;
        self.settings_state.adapter_save_error = None;
        self.settings_state.adapter_save_success = false;
        cx.notify();
    }

    pub fn open_settings_edit(
        &mut self,
        key: &str,
        label: &str,
        field_type: crate::core::config::ConfigFieldType,
        cx: &mut Context<Self>,
    ) {
        use crate::core::config::ConfigValue;
        let initial = self
            .settings_state
            .adapter_config
            .values
            .get(key)
            .map(|v| match v {
                ConfigValue::String(s) => s.clone(),
                ConfigValue::Integer(n) => n.to_string(),
                ConfigValue::Bool(b) => b.to_string(),
                ConfigValue::StringList(list) => list.join(", "),
            })
            .unwrap_or_default();
        let placeholder = label.to_string();
        let input = cx.new(|cx| {
            let mut ti = crate::components::TextInput::new(cx, placeholder);
            ti.set_content(initial, cx);
            ti
        });
        self.settings_state.edit = Some(crate::views::settings::SettingsEdit {
            key: key.to_string(),
            label: label.to_string(),
            field_type,
            input,
        });
        cx.notify();
    }

    pub fn close_settings_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_state.edit = None;
        cx.notify();
    }

    pub fn apply_settings_edit(&mut self, raw: String, cx: &mut Context<Self>) {
        use crate::core::config::{ConfigFieldType, ConfigValue};
        let edit = match self.settings_state.edit.take() {
            Some(e) => e,
            None => return,
        };
        let new_value = match edit.field_type {
            ConfigFieldType::Number => match raw.trim().parse::<i64>() {
                Ok(n) => ConfigValue::Integer(n),
                Err(_) => {
                    self.add_toast(ToastLevel::Error, format!("'{raw}' is not a valid number"));
                    self.settings_state.edit = Some(edit);
                    return;
                }
            },
            ConfigFieldType::Toggle => return,
            _ => ConfigValue::String(raw),
        };
        self.settings_state
            .adapter_config
            .values
            .insert(edit.key, new_value);
        self.settings_state.adapter_dirty =
            self.settings_state.adapter_config != self.settings_state.adapter_config_original;
        cx.notify();
    }

    /// Run an installed package by enumerating executables in its install_path.
    /// 0 → error toast; 1 → spawn directly; many → open a RunPicker overlay.
    pub fn run_installed(
        &mut self,
        installed: crate::core::package::InstalledPackage,
        cx: &mut Context<Self>,
    ) {
        let install_path = match installed.install_path.as_deref() {
            Some(p) => std::path::PathBuf::from(p),
            None => {
                self.add_toast(
                    ToastLevel::Error,
                    format!("No install path for {}", installed.package.name),
                );
                return;
            }
        };

        let adapter = match self
            .adapter_manager
            .get_adapter(&installed.package.adapter_id)
        {
            Some(a) => a,
            None => return,
        };
        if !adapter.capabilities().can_run {
            self.add_toast(
                ToastLevel::Error,
                format!(
                    "{} does not support running packages",
                    installed.package.adapter_id
                ),
            );
            return;
        }

        let bin_path = match soar_active_bin_path() {
            Some(p) => p,
            None => {
                self.add_toast(
                    ToastLevel::Error,
                    "Could not locate active profile bin directory".into(),
                );
                return;
            }
        };
        let binaries = list_package_binaries(&install_path, &bin_path);
        let package_key = installed.unique_key();
        match binaries.len() {
            0 => self.add_toast(
                ToastLevel::Error,
                format!(
                    "No binaries from {} are exposed in {}",
                    install_path.display(),
                    bin_path.display()
                ),
            ),
            1 => {
                let path = binaries.into_iter().next().unwrap();
                self.spawn_binary(&path, &package_key);
            }
            _ => {
                self.run_picker = Some(RunPicker {
                    package_name: installed.package.name.clone(),
                    binaries,
                    package_key,
                });
                cx.notify();
            }
        }
    }

    pub(crate) fn spawn_binary(&mut self, path: &std::path::Path, package_key: &str) {
        let label = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("binary")
            .to_string();
        match std::process::Command::new(path).spawn() {
            Ok(child) => {
                let id = self.next_run_id;
                self.next_run_id = self.next_run_id.wrapping_add(1);
                self.running_processes
                    .entry(package_key.to_string())
                    .or_default()
                    .push(RunningProcess {
                        id,
                        label: label.clone(),
                        child,
                    });
                self.add_toast(ToastLevel::Info, format!("Launched {label}"));
            }
            Err(e) => self.add_toast(
                ToastLevel::Error,
                format!("Failed to run {}: {e}", path.display()),
            ),
        }
    }

    /// Stop all running processes belonging to the given package.
    pub fn stop_running(&mut self, package_key: &str, cx: &mut Context<Self>) {
        let mut killed = 0;
        if let Some(procs) = self.running_processes.get_mut(package_key) {
            for proc in procs.iter_mut() {
                if proc.child.kill().is_ok() {
                    killed += 1;
                }
            }
        }
        self.running_processes.remove(package_key);
        if killed > 0 {
            self.add_toast(ToastLevel::Info, format!("Stopped {killed} process(es)"));
        }
        cx.notify();
    }

    /// Reap exited child processes so the running_processes map stays accurate.
    fn reap_running(&mut self) {
        let mut empty_keys = Vec::new();
        for (key, procs) in self.running_processes.iter_mut() {
            procs.retain_mut(|p| match p.child.try_wait() {
                Ok(Some(_)) => false, // exited
                Ok(None) => true,     // still running
                Err(_) => false,      // unknown — drop
            });
            if procs.is_empty() {
                empty_keys.push(key.clone());
            }
        }
        for k in empty_keys {
            self.running_processes.remove(&k);
        }
    }

    pub fn load_package_detail(
        &mut self,
        pkg: crate::core::package::Package,
        cx: &mut Context<Self>,
    ) {
        let adapter = match self.adapter_manager.get_adapter(&pkg.adapter_id) {
            Some(a) => a,
            None => return,
        };
        if !adapter.capabilities().has_package_detail {
            self.browse_state.selected_detail = None;
            self.browse_state.detail_loading = false;
            self.browse_state.detail_error = None;
            return;
        }

        self.browse_state.detail_request_id = self.browse_state.detail_request_id.wrapping_add(1);
        let request_id = self.browse_state.detail_request_id;
        self.browse_state.selected_detail = None;
        self.browse_state.detail_loading = true;
        self.browse_state.detail_error = None;

        let pkg_id = pkg.id.clone();
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result =
                    crate::tokio_spawn(async move { adapter.package_detail(&pkg_id).await })
                        .await
                        .unwrap_or_else(|e| {
                            Err(crate::core::adapter::AdapterError::Other(format!("{e}")))
                        });

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        // Discard result if a newer request superseded this one
                        if request_id != app.browse_state.detail_request_id {
                            return;
                        }
                        app.browse_state.detail_loading = false;
                        match result {
                            Ok(detail) => {
                                app.browse_state.selected_detail = Some(detail);
                            }
                            Err(e) => {
                                app.browse_state.detail_error = Some(format!("{e}"));
                            }
                        }
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    /// Handle the Escape key. Closes the topmost overlay or clears selection.
    pub(crate) fn handle_escape(&mut self, cx: &mut Context<Self>) {
        if self.settings_state.edit.is_some() {
            self.close_settings_edit(cx);
            return;
        }
        if self.run_picker.is_some() {
            self.run_picker = None;
            cx.notify();
            return;
        }
        if self.confirm_dialog.is_some() {
            self.confirm_dialog = None;
            cx.notify();
            return;
        }
        if !self.browse_state.selected.is_empty() {
            self.browse_state.selected.clear();
            cx.notify();
            return;
        }
        if !self.installed_state.selected.is_empty() {
            self.installed_state.selected.clear();
            cx.notify();
            return;
        }
        if self.browse_state.selected_package.is_some() {
            self.browse_state.selected_package = None;
            self.browse_state.selected_detail = None;
            cx.notify();
        }
    }

    /// Handle Enter to confirm the active dialog.
    pub(crate) fn handle_confirm(&mut self, cx: &mut Context<Self>) {
        if let Some(action) = self.confirm_dialog.take() {
            self.execute_confirmed_action(action, cx);
        }
    }

    pub(crate) fn add_toast(&mut self, level: ToastLevel, message: String) {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toasts.push(Toast {
            id,
            level,
            message,
            created_at: Instant::now(),
            duration: Duration::from_secs(5),
        });
    }

    fn cleanup_toasts(&mut self) {
        self.toasts.retain(|t| t.created_at.elapsed() < t.duration);
    }

    fn drain_progress(&mut self, cx: &mut Context<Self>) {
        use crate::core::adapter::{ProgressEvent, progress_key};
        use soar_events::{InstallStage, RemoveStage, SoarEvent, VerifyStage};

        let mut had_events = false;

        // Drain ProgressEvent channel (from WASM adapters)
        while let Ok(event) = self.progress_receiver.try_recv() {
            had_events = true;
            match event {
                ProgressEvent::Download {
                    adapter_id,
                    package_id,
                    current_bytes,
                    total_bytes,
                } => {
                    let key = progress_key(&adapter_id, &package_id);
                    let status = OperationStatus::Downloading {
                        current: current_bytes,
                        total: total_bytes,
                    };
                    self.browse_state
                        .package_progress
                        .insert(key.clone(), status.clone());
                    self.installed_state.package_progress.insert(key, status);
                }
                ProgressEvent::Phase {
                    adapter_id,
                    package_id,
                    phase,
                    ..
                } => {
                    let key = progress_key(&adapter_id, &package_id);
                    let status = OperationStatus::Installing(phase);
                    self.browse_state
                        .package_progress
                        .insert(key.clone(), status.clone());
                    self.installed_state.package_progress.insert(key, status);
                }
                ProgressEvent::Completed {
                    adapter_id,
                    package_id,
                } => {
                    let key = progress_key(&adapter_id, &package_id);
                    self.browse_state
                        .package_progress
                        .insert(key.clone(), OperationStatus::Completed);
                    self.installed_state
                        .package_progress
                        .insert(key, OperationStatus::Completed);
                    for pkg in &mut self.browse_state.search_results {
                        if pkg.id == package_id && pkg.adapter_id == adapter_id {
                            pkg.installed = true;
                        }
                    }
                }
                ProgressEvent::Failed {
                    adapter_id,
                    package_id,
                    error,
                } => {
                    let key = progress_key(&adapter_id, &package_id);
                    let status = OperationStatus::Failed(error);
                    self.browse_state
                        .package_progress
                        .insert(key.clone(), status.clone());
                    self.installed_state.package_progress.insert(key, status);
                }
                ProgressEvent::Status { message, .. } => {
                    log::info!("Progress status: {message}");
                }
                ProgressEvent::BatchProgress {
                    adapter_id,
                    completed,
                    total,
                    failed,
                } => {
                    if completed >= total && total > 0 {
                        self.batch_progress = None;
                    } else {
                        self.batch_progress = Some((adapter_id, completed, total, failed));
                    }
                }
            }
        }

        // Drain SoarEvent channel (from soar adapter)
        while let Ok(event) = self.event_receiver.try_recv() {
            had_events = true;
            match event {
                SoarEvent::DownloadStarting { pkg_id, total, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let status = OperationStatus::Downloading { current: 0, total };
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                SoarEvent::DownloadProgress {
                    pkg_id,
                    current,
                    total,
                    ..
                }
                | SoarEvent::DownloadResuming {
                    pkg_id,
                    current,
                    total,
                    ..
                } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let status = OperationStatus::Downloading { current, total };
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                SoarEvent::DownloadComplete { pkg_id, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let status = OperationStatus::Installing("Download complete".into());
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                SoarEvent::Verifying { pkg_id, stage, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let label = match stage {
                            VerifyStage::Checksum => "Verifying checksum",
                            VerifyStage::Signature => "Verifying signature",
                            VerifyStage::Passed => "Verification passed",
                            VerifyStage::Failed(_) => "Verification failed",
                        };
                        let status = OperationStatus::Verifying(label.into());
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                SoarEvent::Installing { pkg_id, stage, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let label = match &stage {
                            InstallStage::Extracting => "Extracting".to_string(),
                            InstallStage::ExtractingNested => "Extracting nested".to_string(),
                            InstallStage::LinkingBinaries => "Linking binaries".to_string(),
                            InstallStage::DesktopIntegration => "Desktop integration".to_string(),
                            InstallStage::SetupPortable => "Setting up portable".to_string(),
                            InstallStage::RecordingDatabase => "Recording to database".to_string(),
                            InstallStage::RunningHook(h) => format!("Running hook: {h}"),
                            InstallStage::Complete => "Complete".to_string(),
                        };
                        let status = OperationStatus::Installing(label);
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                SoarEvent::Removing { pkg_id, stage, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let label = match &stage {
                            RemoveStage::RunningHook(h) => format!("Running hook: {h}"),
                            RemoveStage::UnlinkingBinaries => "Unlinking binaries".to_string(),
                            RemoveStage::UnlinkingDesktop => "Unlinking desktop files".to_string(),
                            RemoveStage::UnlinkingIcons => "Unlinking icons".to_string(),
                            RemoveStage::RemovingDirectory => "Removing directory".to_string(),
                            RemoveStage::CleaningDatabase => "Cleaning database".to_string(),
                            RemoveStage::Complete { .. } => "Complete".to_string(),
                        };
                        let status = OperationStatus::Removing(label);
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                SoarEvent::OperationComplete { pkg_id, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), OperationStatus::Completed);
                        self.installed_state
                            .package_progress
                            .insert(key, OperationStatus::Completed);
                    }
                }
                SoarEvent::OperationFailed { pkg_id, error, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let status = OperationStatus::Failed(error);
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                SoarEvent::DownloadRetry { pkg_id, .. }
                | SoarEvent::DownloadAborted { pkg_id, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        let status = OperationStatus::Failed("Download failed".into());
                        self.browse_state
                            .package_progress
                            .insert(key.clone(), status.clone());
                        self.installed_state.package_progress.insert(key, status);
                    }
                }
                _ => {}
            }
        }

        if had_events {
            cx.notify();
        }
    }

    /// Find the progress key for a soar package by matching its pkg_id suffix
    /// against browse/installed search results.
    fn soar_progress_key(&self, soar_pkg_id: &str) -> Option<String> {
        use crate::core::adapter::progress_key;
        // Browse results: aeris id = "{repo_name}.{pkg_id}", try matching by suffix
        for pkg in &self.browse_state.search_results {
            if pkg.adapter_id == "soar" && pkg.id.ends_with(soar_pkg_id) {
                return Some(progress_key("soar", &pkg.id));
            }
        }
        // Installed packages
        for pkg in &self.installed_state.packages {
            if pkg.package.adapter_id == "soar" && pkg.package.id.ends_with(soar_pkg_id) {
                return Some(progress_key("soar", &pkg.package.id));
            }
        }
        // Fallback: use soar_pkg_id directly
        Some(progress_key("soar", soar_pkg_id))
    }
}

impl Render for App {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme::current_theme(self.selected_theme);

        // Cleanup expired toasts
        self.cleanup_toasts();
        self.reap_running();

        let mut root = div()
            .id("app-root")
            .key_context("App")
            .on_action(cx.listener(|app, _: &Escape, _window, cx| {
                app.handle_escape(cx);
            }))
            .on_action(cx.listener(|app, _: &Confirm, _window, cx| {
                app.handle_confirm(cx);
            }))
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.bg)
            .text_color(theme.text)
            .font_family("system-ui")
            .child(self.render_sidebar(&theme, cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .child(self.render_header(&theme, cx))
                    .child(self.render_content(&theme, cx)),
            );

        // Toast overlay
        if !self.toasts.is_empty() {
            let toast_elements: Vec<Div> = self
                .toasts
                .iter()
                .map(|toast| {
                    let (bg, border_color) = match toast.level {
                        ToastLevel::Success => {
                            (theme.success.opacity(0.15), theme.success.opacity(0.3))
                        }
                        ToastLevel::Error => {
                            (theme.danger.opacity(0.15), theme.danger.opacity(0.3))
                        }
                        ToastLevel::Info => {
                            (theme.primary.opacity(0.15), theme.primary.opacity(0.3))
                        }
                    };
                    div()
                        .px(px(styles::spacing::LG))
                        .py(px(styles::spacing::SM))
                        .rounded(px(styles::radius::MD))
                        .bg(bg)
                        .border_1()
                        .border_color(border_color)
                        .text_size(px(styles::font_size::SMALL))
                        .child(toast.message.clone())
                })
                .collect();

            root = root.child(
                div()
                    .absolute()
                    .bottom(px(styles::spacing::XL))
                    .right(px(styles::spacing::XL))
                    .flex()
                    .flex_col()
                    .gap(px(styles::spacing::SM))
                    .children(toast_elements),
            );
        }

        // Confirm dialog overlay
        if let Some(ref action) = self.confirm_dialog.clone() {
            let mode_suffix = |mode: &PackageMode| match mode {
                PackageMode::User => " (User)",
                PackageMode::System => " (System)",
            };
            let message = match action {
                ConfirmAction::Install(pkg, mode) => {
                    format!("Install {}?{}", pkg.name, mode_suffix(mode))
                }
                ConfirmAction::Remove(pkg, mode) => {
                    format!("Remove {}?{}", pkg.name, mode_suffix(mode))
                }
                ConfirmAction::Update(pkg, mode) => {
                    format!("Update {}?{}", pkg.name, mode_suffix(mode))
                }
                ConfirmAction::UpdateAll(mode) => {
                    format!("Update all packages?{}", mode_suffix(mode))
                }
                ConfirmAction::BatchInstall(pkgs, mode) => {
                    format!("Install {} packages?{}", pkgs.len(), mode_suffix(mode))
                }
                ConfirmAction::BatchRemove(pkgs, mode) => {
                    format!("Remove {} packages?{}", pkgs.len(), mode_suffix(mode))
                }
                ConfirmAction::BatchUpdate(pkgs, mode) => {
                    format!("Update {} packages?{}", pkgs.len(), mode_suffix(mode))
                }
                ConfirmAction::RemoveInstalled { pkg, mode, .. } => {
                    format!("Remove {}?{}", pkg.name, mode_suffix(mode))
                }
                ConfirmAction::BatchRemoveInstalled { count } => {
                    format!(
                        "Remove {count} packages?{}",
                        mode_suffix(&self.current_mode)
                    )
                }
            };

            let confirm_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                if let Some(action) = app.confirm_dialog.take() {
                    app.execute_confirmed_action(action, cx);
                }
            });
            let cancel_listener = cx.listener(|app, _: &ClickEvent, _window, _cx| {
                app.confirm_dialog = None;
            });

            let surface = theme.surface;
            let border = theme.border;
            let primary = theme.primary;
            let hover = theme.hover;

            root = root.child(
                div()
                    .absolute()
                    .size_full()
                    .occlude()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 0.0,
                        a: 0.5,
                    })
                    .child(
                        div()
                            .p(px(styles::spacing::XXL))
                            .rounded(px(styles::radius::LG))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .flex()
                            .flex_col()
                            .gap(px(styles::spacing::LG))
                            .child(
                                div()
                                    .text_size(px(styles::font_size::HEADING))
                                    .child(message),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(styles::spacing::SM))
                                    .justify_end()
                                    .child(
                                        div()
                                            .id("confirm-cancel")
                                            .px(px(styles::spacing::LG))
                                            .py(px(styles::spacing::XS))
                                            .rounded(px(styles::radius::MD))
                                            .bg(surface)
                                            .border_1()
                                            .border_color(border)
                                            .cursor_pointer()
                                            .hover(move |s| s.bg(hover))
                                            .on_click(cancel_listener)
                                            .child("Cancel"),
                                    )
                                    .child(
                                        div()
                                            .id("confirm-ok")
                                            .px(px(styles::spacing::LG))
                                            .py(px(styles::spacing::XS))
                                            .rounded(px(styles::radius::MD))
                                            .bg(primary)
                                            .text_color(gpui::white())
                                            .cursor_pointer()
                                            .on_click(confirm_listener)
                                            .child("Confirm"),
                                    ),
                            ),
                    ),
            );
        }

        // Run picker overlay (multi-binary packages)
        if let Some(picker) = self.run_picker.clone() {
            let surface = theme.surface;
            let border = theme.border;
            let primary = theme.primary;
            let hover = theme.hover;
            let text_muted = theme.text_muted;

            let cancel_picker = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.run_picker = None;
                cx.notify();
            });

            let mut binary_buttons = div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::XS))
                .w_full();

            for (idx, path) in picker.binaries.iter().enumerate() {
                let path_clone = path.clone();
                let key_clone = picker.package_key.clone();
                let listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                    app.spawn_binary(&path_clone, &key_clone);
                    app.run_picker = None;
                    cx.notify();
                });
                let label = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                binary_buttons = binary_buttons.child(
                    div()
                        .id(SharedString::from(format!("run-pick-{idx}")))
                        .px(px(styles::spacing::MD))
                        .py(px(styles::spacing::SM))
                        .rounded(px(styles::radius::MD))
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .on_click(listener)
                        .child(label),
                );
            }

            root =
                root.child(
                    div()
                        .absolute()
                        .size_full()
                        .occlude()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(Hsla {
                            h: 0.0,
                            s: 0.0,
                            l: 0.0,
                            a: 0.5,
                        })
                        .child(
                            div()
                                .p(px(styles::spacing::XXL))
                                .rounded(px(styles::radius::LG))
                                .bg(surface)
                                .border_1()
                                .border_color(border)
                                .flex()
                                .flex_col()
                                .gap(px(styles::spacing::LG))
                                .min_w(px(360.0))
                                .child(div().text_size(px(styles::font_size::HEADING)).child(
                                    format!("Run {} — choose a binary", picker.package_name),
                                ))
                                .child(
                                    div()
                                        .text_size(px(styles::font_size::CAPTION))
                                        .text_color(text_muted)
                                        .child(format!(
                                            "{} executables found",
                                            picker.binaries.len()
                                        )),
                                )
                                .child(binary_buttons)
                                .child(
                                    div().flex().flex_row().justify_end().child(
                                        div()
                                            .id("run-picker-cancel")
                                            .px(px(styles::spacing::LG))
                                            .py(px(styles::spacing::XS))
                                            .rounded(px(styles::radius::MD))
                                            .bg(primary)
                                            .text_color(gpui::white())
                                            .cursor_pointer()
                                            .on_click(cancel_picker)
                                            .child("Cancel"),
                                    ),
                                ),
                        ),
                );
        }

        // Settings edit modal (text/number/select fields)
        if let Some(ref edit) = self.settings_state.edit {
            use crate::core::config::ConfigFieldType;
            let surface = theme.surface;
            let border = theme.border;
            let primary = theme.primary;
            let hover = theme.hover;
            let text_muted = theme.text_muted;

            let cancel = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.close_settings_edit(cx);
            });

            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::SM))
                .min_w(px(380.0))
                .child(
                    div()
                        .text_size(px(styles::font_size::HEADING))
                        .child(format!("Edit {}", edit.label)),
                );

            match edit.field_type.clone() {
                ConfigFieldType::Select(options) => {
                    body = body.child(
                        div()
                            .text_size(px(styles::font_size::CAPTION))
                            .text_color(text_muted)
                            .child("Select a value"),
                    );
                    let mut list = div().flex().flex_col().gap(px(styles::spacing::XS));
                    for (idx, opt) in options.iter().enumerate() {
                        let opt_clone = opt.clone();
                        let listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                            app.apply_settings_edit(opt_clone.clone(), cx);
                        });
                        list = list.child(
                            div()
                                .id(SharedString::from(format!("set-edit-{idx}")))
                                .px(px(styles::spacing::MD))
                                .py(px(styles::spacing::SM))
                                .rounded(px(styles::radius::MD))
                                .bg(surface)
                                .border_1()
                                .border_color(border)
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .on_click(listener)
                                .child(opt.clone()),
                        );
                    }
                    body = body.child(list);
                }
                _ => {
                    body = body.child(
                        div()
                            .px(px(styles::spacing::MD))
                            .py(px(10.0))
                            .rounded(px(styles::radius::MD))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .child(edit.input.clone()),
                    );
                    let input_handle = edit.input.clone();
                    let save = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                        let value = input_handle.read(cx).content().to_string();
                        app.apply_settings_edit(value, cx);
                    });
                    body = body.child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(styles::spacing::SM))
                            .justify_end()
                            .child(
                                div()
                                    .id("settings-edit-cancel")
                                    .px(px(styles::spacing::LG))
                                    .py(px(styles::spacing::XS))
                                    .rounded(px(styles::radius::MD))
                                    .bg(surface)
                                    .border_1()
                                    .border_color(border)
                                    .cursor_pointer()
                                    .hover(move |s| s.bg(hover))
                                    .on_click(cancel)
                                    .child("Cancel"),
                            )
                            .child(
                                div()
                                    .id("settings-edit-save")
                                    .px(px(styles::spacing::LG))
                                    .py(px(styles::spacing::XS))
                                    .rounded(px(styles::radius::MD))
                                    .bg(primary)
                                    .text_color(gpui::white())
                                    .cursor_pointer()
                                    .on_click(save)
                                    .child("Save"),
                            ),
                    );
                }
            }

            root = root.child(
                div()
                    .absolute()
                    .size_full()
                    .occlude()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 0.0,
                        a: 0.5,
                    })
                    .child(
                        div()
                            .p(px(styles::spacing::XXL))
                            .rounded(px(styles::radius::LG))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .child(body),
                    ),
            );
        }

        root
    }
}

impl App {
    fn render_sidebar(&mut self, theme: &theme::Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current_view;
        let nav_items: [(View, &str); 6] = [
            (View::Dashboard, "Dashboard"),
            (View::Browse, "Browse"),
            (View::Installed, "Installed"),
            (View::Updates, "Updates"),
            (View::AdapterInfo, "Adapters"),
            (View::Settings, "Settings"),
        ];

        let nav_listeners: Vec<_> = nav_items
            .iter()
            .map(|(view, _)| {
                let view = *view;
                cx.listener(move |app, _: &ClickEvent, _window, cx| {
                    app.current_view = view;
                    if view == View::AdapterInfo {
                        let any_loaded = app.adapter_view.repos_loaded.values().any(|v| *v);
                        if !any_loaded {
                            app.load_repos(cx);
                        }
                    }
                })
            })
            .collect();

        let hover_color = theme.hover;
        let primary = theme.primary;
        let text_color = theme.text;

        div()
            .w(px(200.0))
            .flex()
            .flex_col()
            .bg(theme.surface)
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .px(px(styles::spacing::LG))
                    .py(px(styles::spacing::XL))
                    .child(
                        div()
                            .text_size(px(styles::font_size::TITLE))
                            .font_weight(FontWeight::BOLD)
                            .child(APP_NAME),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(styles::spacing::XXS))
                    .px(px(styles::spacing::SM))
                    .children(nav_items.into_iter().zip(nav_listeners).map(
                        move |((view, label), listener)| {
                            let is_active = current == view;
                            let bg = if is_active {
                                primary
                            } else {
                                transparent_black()
                            };
                            let text = if is_active { gpui::white() } else { text_color };

                            div()
                                .id(SharedString::from(format!("nav-{label}")))
                                .px(px(styles::spacing::MD))
                                .py(px(styles::spacing::SM))
                                .rounded(px(styles::radius::MD))
                                .bg(bg)
                                .text_color(text)
                                .cursor_pointer()
                                .hover(move |s| if is_active { s } else { s.bg(hover_color) })
                                .on_click(listener)
                                .child(label)
                        },
                    )),
            )
    }

    fn render_header(&mut self, theme: &theme::Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let mode_label = match self.current_mode {
            PackageMode::User => "User",
            PackageMode::System => "System",
        };

        // Active operation indicator
        let op_indicator = if let Some(ref op) = self.active_operation {
            Some(
                div()
                    .px(px(styles::spacing::MD))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::FULL))
                    .bg(theme.warning.opacity(0.2))
                    .text_size(px(styles::font_size::CAPTION))
                    .child(format!("{}: {}", op.operation_type, op.status.label())),
            )
        } else {
            None
        };

        let mut header = div()
            .w_full()
            .px(px(styles::spacing::XXL))
            .py(px(styles::spacing::MD))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{}", self.current_view)),
            );

        if let Some(indicator) = op_indicator {
            header = header.child(indicator);
        }

        if let Some((adapter_id, completed, total, failed)) = self.batch_progress.clone() {
            let label = if failed > 0 {
                format!("{adapter_id} batch: {completed}/{total} ({failed} failed)")
            } else {
                format!("{adapter_id} batch: {completed}/{total}")
            };
            header = header.child(
                div()
                    .px(px(styles::spacing::MD))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::FULL))
                    .bg(theme.primary.opacity(0.2))
                    .text_size(px(styles::font_size::CAPTION))
                    .child(label),
            );
        }

        let toggle_mode = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.toggle_mode(cx);
        });

        header.child(
            div()
                .id("mode-toggle")
                .px(px(styles::spacing::MD))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::FULL))
                .bg(theme.primary)
                .text_color(gpui::white())
                .text_size(px(styles::font_size::CAPTION))
                .cursor_pointer()
                .on_click(toggle_mode)
                .child(mode_label),
        )
    }

    pub(crate) fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        self.current_mode = match self.current_mode {
            PackageMode::User => PackageMode::System,
            PackageMode::System => PackageMode::User,
        };
        // Invalidate per-view caches so they reload for the new mode
        self.installed_state.loaded = false;
        self.installed_state.packages.clear();
        self.updates_state.checked = false;
        self.updates_state.updates.clear();
        self.updates_state.no_update_listing.clear();
        cx.notify();
    }

    fn render_content(&mut self, theme: &theme::Theme, cx: &mut Context<Self>) -> Div {
        let wrapper = div().flex_1();

        match self.current_view {
            View::Dashboard => wrapper.child(self.render_dashboard(theme, cx)),
            View::Browse => wrapper.child(self.render_browse(theme, cx)),
            View::Installed => wrapper.child(self.render_installed(theme, cx)),
            View::Updates => wrapper.child(self.render_updates(theme, cx)),
            View::AdapterInfo => wrapper.child(self.render_adapter_info(theme, cx)),
            View::Settings => wrapper.child(self.render_settings(theme, cx)),
        }
    }

    fn execute_confirmed_action(&mut self, action: ConfirmAction, cx: &mut Context<Self>) {
        match action {
            ConfirmAction::Install(pkg, mode) => {
                self.install_package(pkg, mode, cx);
            }
            ConfirmAction::Remove(pkg, mode) => {
                self.remove_package(pkg, mode, cx);
            }
            ConfirmAction::Update(pkg, mode) => {
                self.update_package(pkg, mode, cx);
            }
            ConfirmAction::UpdateAll(mode) => {
                self.update_all(cx);
            }
            ConfirmAction::BatchInstall(pkgs, mode) => {
                self.batch_install(pkgs, mode, cx);
            }
            ConfirmAction::BatchRemove(pkgs, mode) => {
                self.batch_remove(pkgs, mode, cx);
            }
            ConfirmAction::BatchUpdate(pkgs, mode) => {
                self.batch_update(pkgs, mode, cx);
            }
            ConfirmAction::RemoveInstalled {
                pkg,
                unique_key,
                mode,
            } => {
                self.remove_installed_package(pkg, unique_key, mode, cx);
            }
            ConfirmAction::BatchRemoveInstalled { .. } => {
                self.remove_selected_installed(cx);
            }
        }
    }

    pub(crate) fn install_package(
        &mut self,
        pkg: crate::core::package::Package,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        let pkg_id = pkg.id.clone();
        let pkg_name = pkg.name.clone();
        let progress_key = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
        self.browse_state.installing = Some(pkg_id.clone());
        self.browse_state
            .package_progress
            .insert(progress_key.clone(), OperationStatus::Starting);
        let progress_sender = self.progress_sender.clone();
        let adapter = self.adapter_manager.get_adapter(&pkg.adapter_id);

        if let Some(adapter) = adapter {
            cx.spawn(
                async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    let result = crate::tokio_spawn(async move {
                        adapter.install(&[pkg], Some(progress_sender), mode).await
                    })
                    .await;

                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            app.browse_state.installing = None;
                            match result {
                                Ok(Ok(_)) => {
                                    // Mark as installed in search results
                                    for p in &mut app.browse_state.search_results {
                                        if p.id == pkg_id {
                                            p.installed = true;
                                        }
                                    }
                                    app.browse_state.package_progress.remove(&progress_key);
                                    app.add_toast(
                                        ToastLevel::Success,
                                        format!("Installed {pkg_name}"),
                                    );
                                    // Refresh installed list
                                    app.installed_state.loaded = false;
                                }
                                Ok(Err(e)) => {
                                    app.browse_state.package_progress.insert(
                                        progress_key.clone(),
                                        OperationStatus::Failed(format!("{e}")),
                                    );
                                    app.add_toast(
                                        ToastLevel::Error,
                                        format!("Failed to install {pkg_name}: {e}"),
                                    );
                                }
                                Err(e) => {
                                    app.browse_state.package_progress.insert(
                                        progress_key.clone(),
                                        OperationStatus::Failed(format!("{e}")),
                                    );
                                }
                            }
                            app.browse_state.result_version += 1;
                            cx.notify();
                        })
                    });
                },
            )
            .detach();
        }
    }

    pub(crate) fn remove_package(
        &mut self,
        pkg: crate::core::package::Package,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        self.remove_installed_package(pkg.clone(), pkg.id.clone(), mode, cx);
    }

    /// Remove from installed view — uses unique_key so duplicate package names
    /// don't cause the wrong card to show "Removing…".
    pub(crate) fn remove_installed_package(
        &mut self,
        pkg: crate::core::package::Package,
        unique_key: String,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        let pkg_name = pkg.name.clone();
        let progress_key = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
        self.installed_state.removing = Some(unique_key);
        self.installed_state
            .package_progress
            .insert(progress_key.clone(), OperationStatus::Starting);
        let progress_sender = self.progress_sender.clone();
        let adapter = self.adapter_manager.get_adapter(&pkg.adapter_id);

        if let Some(adapter) = adapter {
            cx.spawn(
                async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    let result = crate::tokio_spawn(async move {
                        adapter.remove(&[pkg], Some(progress_sender), mode).await
                    })
                    .await;

                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            app.installed_state.removing = None;
                            app.installed_state.package_progress.remove(&progress_key);
                            match result {
                                Ok(Ok(_)) => {
                                    app.add_toast(
                                        ToastLevel::Success,
                                        format!("Removed {pkg_name}"),
                                    );
                                }
                                Ok(Err(e)) => {
                                    app.add_toast(
                                        ToastLevel::Error,
                                        format!("Failed to remove {pkg_name}: {e}"),
                                    );
                                }
                                Err(e) => {
                                    app.add_toast(
                                        ToastLevel::Error,
                                        format!("Failed to remove {pkg_name}: {e}"),
                                    );
                                }
                            }
                            app.installed_state.result_version += 1;
                            app.load_installed(cx);
                        })
                    });
                },
            )
            .detach();
        }
    }

    fn update_package(
        &mut self,
        pkg: crate::core::package::Package,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        let pkg_name = pkg.name.clone();
        self.updates_state.updating = Some(pkg.id.clone());
        let progress_sender = self.progress_sender.clone();
        let adapter = self.adapter_manager.get_adapter(&pkg.adapter_id);

        if let Some(adapter) = adapter {
            cx.spawn(
                async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    let result = crate::tokio_spawn(async move {
                        adapter.update(&[pkg], Some(progress_sender), mode).await
                    })
                    .await;

                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            app.updates_state.updating = None;
                            app.updates_state.result_version += 1;
                            match result {
                                Ok(Ok(_)) => {
                                    app.add_toast(
                                        ToastLevel::Success,
                                        format!("Updated {pkg_name}"),
                                    );
                                }
                                Ok(Err(e)) => {
                                    app.add_toast(
                                        ToastLevel::Error,
                                        format!("Failed to update {pkg_name}: {e}"),
                                    );
                                }
                                Err(e) => {
                                    app.add_toast(
                                        ToastLevel::Error,
                                        format!("Failed to update {pkg_name}: {e}"),
                                    );
                                }
                            }
                            app.installed_state.loaded = false;
                            cx.notify();
                        })
                    });
                },
            )
            .detach();
        }
    }

    fn batch_install(
        &mut self,
        pkgs: Vec<crate::core::package::Package>,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        let count = pkgs.len();
        self.browse_state.installing = Some("__batch__".to_string());
        let progress_sender = self.progress_sender.clone();
        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let errors = crate::tokio_spawn(async move {
                    let mut by_adapter: HashMap<String, Vec<crate::core::package::Package>> =
                        HashMap::new();
                    for pkg in pkgs {
                        by_adapter
                            .entry(pkg.adapter_id.clone())
                            .or_default()
                            .push(pkg);
                    }

                    let mut errors: Vec<String> = Vec::new();
                    for (adapter_id, pkgs) in by_adapter {
                        if let Some(adapter) =
                            manager_adapters.iter().find(|a| a.info().id == adapter_id)
                        {
                            match adapter
                                .install(&pkgs, Some(progress_sender.clone()), mode)
                                .await
                            {
                                Ok(_) => log::info!("Batch install completed for {adapter_id}"),
                                Err(e) => {
                                    log::error!("Batch install failed for {adapter_id}: {e}");
                                    errors.push(format!("{e}"));
                                }
                            }
                        }
                    }
                    errors
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.browse_state.installing = None;
                        app.browse_state.result_version += 1;
                        if errors.is_empty() {
                            app.add_toast(
                                ToastLevel::Success,
                                format!("Installed {count} packages"),
                            );
                        } else {
                            for err in &errors {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Failed to install: {err}"),
                                );
                            }
                        }
                        app.installed_state.loaded = false;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    fn batch_remove(
        &mut self,
        pkgs: Vec<crate::core::package::Package>,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        self.installed_state.removing = Some("__batch__".to_string());
        let progress_sender = self.progress_sender.clone();
        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        let count = pkgs.len();
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let errors = crate::tokio_spawn(async move {
                    let mut by_adapter: HashMap<String, Vec<crate::core::package::Package>> =
                        HashMap::new();
                    for pkg in pkgs {
                        by_adapter
                            .entry(pkg.adapter_id.clone())
                            .or_default()
                            .push(pkg);
                    }

                    let mut errors: Vec<String> = Vec::new();
                    for (adapter_id, pkgs) in by_adapter {
                        if let Some(adapter) =
                            manager_adapters.iter().find(|a| a.info().id == adapter_id)
                        {
                            match adapter
                                .remove(&pkgs, Some(progress_sender.clone()), mode)
                                .await
                            {
                                Ok(_) => log::info!("Batch remove completed for {adapter_id}"),
                                Err(e) => {
                                    log::error!("Batch remove failed for {adapter_id}: {e}");
                                    errors.push(format!("{e}"));
                                }
                            }
                        }
                    }
                    errors
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.installed_state.removing = None;
                        app.installed_state.result_version += 1;
                        if errors.is_empty() {
                            app.add_toast(ToastLevel::Success, format!("Removed {count} packages"));
                        } else {
                            for err in &errors {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Failed to remove: {err}"),
                                );
                            }
                        }
                        app.load_installed(cx);
                    })
                });
            },
        )
        .detach();
    }

    fn batch_update(
        &mut self,
        pkgs: Vec<crate::core::package::Package>,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        self.updates_state.updating = Some("__batch__".to_string());
        let progress_sender = self.progress_sender.clone();
        let manager_adapters: Vec<Arc<dyn Adapter>> = self
            .adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .collect();

        let count = pkgs.len();
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let errors = crate::tokio_spawn(async move {
                    let mut by_adapter: HashMap<String, Vec<crate::core::package::Package>> =
                        HashMap::new();
                    for pkg in pkgs {
                        by_adapter
                            .entry(pkg.adapter_id.clone())
                            .or_default()
                            .push(pkg);
                    }

                    let mut errors: Vec<String> = Vec::new();
                    for (adapter_id, pkgs) in by_adapter {
                        if let Some(adapter) =
                            manager_adapters.iter().find(|a| a.info().id == adapter_id)
                        {
                            match adapter
                                .update(&pkgs, Some(progress_sender.clone()), mode)
                                .await
                            {
                                Ok(_) => log::info!("Batch update completed for {adapter_id}"),
                                Err(e) => {
                                    log::error!("Batch update failed for {adapter_id}: {e}");
                                    errors.push(format!("{e}"));
                                }
                            }
                        }
                    }
                    errors
                })
                .await
                .unwrap_or_default();

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.updates_state.updating = None;
                        app.updates_state.result_version += 1;
                        if errors.is_empty() {
                            app.add_toast(ToastLevel::Success, format!("Updated {count} packages"));
                        } else {
                            for err in &errors {
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Failed to update: {err}"),
                                );
                            }
                        }
                        app.installed_state.loaded = false;
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }
}
