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
    Manifest,
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
            View::Manifest => write!(f, "Manifest"),
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
    pub(crate) manifest_state: views::manifest::ManifestState,

    // Text input entities
    pub(crate) search_input: Entity<crate::components::TextInput>,

    /// Focus handle so the root div can receive app-level key actions
    /// (Escape, Enter) when no other element is focused.
    focus_handle: FocusHandle,
    /// Set when an overlay opens whose TextInput should be focused on the
    /// next render. Cleared after applying focus.
    pending_settings_edit_focus: bool,

    /// Receives a notification each time the watched manifest file changes on
    /// disk. Drained on the same timer as adapter progress events.
    manifest_watcher_rx: Option<std::sync::mpsc::Receiver<()>>,
    /// Kept alive so the underlying inotify/fsevent handles stay registered.
    /// Boxed as `Any` because notify's watcher type does not appear in any
    /// other field's signature here.
    _manifest_watcher: Option<Box<dyn std::any::Any + Send>>,
    /// Earliest instant at which a queued external reload may run. Lets us
    /// coalesce bursts of events from a single atomic save.
    manifest_reload_due: Option<Instant>,
}

/// Wait at least this long after a notify event before reloading. Coalesces
/// the burst that an atomic rename produces and keeps us from racing partial
/// writes from external editors.
const MANIFEST_RELOAD_COALESCE_MS: u64 = 200;

/// Spawn a notify watcher on the manifest's parent directory and return a
/// receiver that fires whenever the manifest path is touched. The watcher
/// itself is returned boxed so the caller can keep it alive without naming
/// notify types across the field type.
fn spawn_manifest_watcher(
    adapter: &Arc<SoarAdapter>,
) -> (
    Option<std::sync::mpsc::Receiver<()>>,
    Option<Box<dyn std::any::Any + Send>>,
) {
    use notify::{EventKind, RecursiveMode, Watcher};

    let path = adapter.manifest_path();
    let parent = match path.parent() {
        Some(p) => p.to_path_buf(),
        None => return (None, None),
    };
    if let Err(e) = std::fs::create_dir_all(&parent) {
        log::warn!("manifest watcher: cannot create parent dir: {e}");
        return (None, None);
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let target = path;

    let watcher_result =
        notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
            Ok(event) => {
                let interesting = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                );
                if !interesting {
                    return;
                }
                if event.paths.iter().any(|p| p == &target) {
                    let _ = tx.send(());
                }
            }
            Err(e) => log::warn!("manifest watcher error: {e}"),
        });

    let mut watcher = match watcher_result {
        Ok(w) => w,
        Err(e) => {
            log::warn!("manifest watcher: failed to create: {e}");
            return (None, None);
        }
    };
    if let Err(e) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
        log::warn!("manifest watcher: failed to watch {}: {e}", parent.display());
        return (None, None);
    }

    (Some(rx), Some(Box::new(watcher) as Box<dyn std::any::Any + Send>))
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

        let (manifest_watcher_rx, manifest_watcher) = spawn_manifest_watcher(&adapter);

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
            manifest_state: views::manifest::ManifestState::default(),
            search_input,
            focus_handle: cx.focus_handle(),
            pending_settings_edit_focus: false,
            manifest_watcher_rx,
            _manifest_watcher: manifest_watcher,
            manifest_reload_due: None,
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

    pub fn load_manifest_diff(&mut self, cx: &mut Context<Self>) {
        use crate::adapters::soar::ManifestLoadError;
        use views::manifest::ManifestStatus;

        self.manifest_state.path = Some(self.adapter.manifest_path());
        self.manifest_state.status = ManifestStatus::Loading;

        let adapter = self.adapter.clone();
        let mode = self.current_mode;
        let prune = self.manifest_state.prune;

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result =
                    crate::tokio_spawn(async move { adapter.manifest_diff(mode, prune).await })
                        .await;

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.manifest_state.status = match result {
                            Ok(Ok(diff)) => ManifestStatus::Loaded(diff),
                            Ok(Err(ManifestLoadError::FileMissing)) => ManifestStatus::FileMissing,
                            Ok(Err(ManifestLoadError::Parse(e))) => ManifestStatus::ParseError(e),
                            Ok(Err(ManifestLoadError::Other(e))) => ManifestStatus::Failed(e),
                            Err(e) => ManifestStatus::Failed(format!("{e}")),
                        };
                        if let Some(name) = app.manifest_state.selected_entry.clone() {
                            match app.adapter.read_manifest_entry(&name) {
                                Ok(Some(snap)) => {
                                    app.manifest_state.selected_snapshot = Some(snap);
                                }
                                _ => {
                                    app.manifest_state.selected_entry = None;
                                    app.manifest_state.selected_snapshot = None;
                                }
                            }
                        }
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn request_manifest_apply(&mut self, cx: &mut Context<Self>) {
        use views::manifest::ManifestStatus;
        let prune = self.manifest_state.prune;
        let remove_names: Vec<String> = match &self.manifest_state.status {
            ManifestStatus::Loaded(diff) if prune => {
                diff.to_remove.iter().map(|e| e.name.clone()).collect()
            }
            _ => Vec::new(),
        };
        if remove_names.is_empty() {
            self.apply_manifest(prune, cx);
        } else {
            self.confirm_dialog = Some(ConfirmAction::ApplyManifest {
                prune,
                remove_names,
            });
            cx.notify();
        }
    }

    pub fn apply_manifest(&mut self, prune: bool, cx: &mut Context<Self>) {
        use crate::adapters::soar::ManifestLoadError;
        use views::manifest::ManifestStatus;

        let adapter_id = self.adapter.info().id.clone();
        let seed_keys: Vec<String> = match &self.manifest_state.status {
            ManifestStatus::Loaded(diff) => {
                let mut keys: Vec<String> = diff
                    .to_install
                    .iter()
                    .chain(diff.to_update.iter())
                    .filter_map(|e| e.pkg_id.clone())
                    .map(|pid| crate::core::adapter::progress_key(&adapter_id, &pid))
                    .collect();
                if prune {
                    keys.extend(
                        diff.to_remove
                            .iter()
                            .filter_map(|e| e.pkg_id.clone())
                            .map(|pid| crate::core::adapter::progress_key(&adapter_id, &pid)),
                    );
                }
                keys
            }
            _ => Vec::new(),
        };
        for key in seed_keys {
            self.record_progress(key, OperationStatus::Starting);
        }

        self.manifest_state.applying = true;
        self.manifest_state.apply_error = None;
        self.manifest_state.last_report = None;

        let adapter = self.adapter.clone();
        let mode = self.current_mode;

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result = crate::tokio_spawn(async move {
                    adapter.apply_manifest(mode, prune, false).await
                })
                .await;

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.manifest_state.applying = false;
                        match result {
                            Ok(Ok(report)) => {
                                app.manifest_state.last_report = Some(report);
                                let msg = format!(
                                    "Manifest applied: {} installed, {} updated, {} removed",
                                    report.installed, report.updated, report.removed
                                );
                                if report.failed > 0 {
                                    app.add_toast(
                                        ToastLevel::Error,
                                        format!("{msg}, {} failed", report.failed),
                                    );
                                } else {
                                    app.add_toast(ToastLevel::Success, msg);
                                }
                            }
                            Ok(Err(err)) => {
                                let msg = match err {
                                    ManifestLoadError::FileMissing => {
                                        "Manifest file is missing".to_string()
                                    }
                                    ManifestLoadError::Parse(e) | ManifestLoadError::Other(e) => e,
                                };
                                app.manifest_state.apply_error = Some(msg.clone());
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Manifest apply failed: {msg}"),
                                );
                            }
                            Err(e) => {
                                let msg = format!("{e}");
                                app.manifest_state.apply_error = Some(msg.clone());
                                app.add_toast(
                                    ToastLevel::Error,
                                    format!("Manifest apply failed: {msg}"),
                                );
                            }
                        }
                        app.installed_state.loaded = false;
                        app.updates_state.checked = false;
                        app.load_manifest_diff(cx);
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn open_manifest_add(&mut self, cx: &mut Context<Self>) {
        use views::manifest::{
            build_manifest_edit_modal, ManifestEditKind, ManifestEntrySnapshot,
        };
        let snap = ManifestEntrySnapshot {
            version: "*".to_string(),
            ..Default::default()
        };
        self.manifest_state.edit = Some(build_manifest_edit_modal(
            ManifestEditKind::Add,
            &snap,
            cx,
        ));
        self.manifest_state.pending_edit_focus = true;
        cx.notify();
    }

    pub fn open_manifest_edit(&mut self, name: String, cx: &mut Context<Self>) {
        use views::manifest::{
            build_manifest_edit_modal, ManifestEditKind, ManifestEntrySnapshot,
        };
        let snap = match self.adapter.read_manifest_entry(&name) {
            Ok(Some(s)) => s,
            Ok(None) => ManifestEntrySnapshot {
                name: name.clone(),
                version: "*".to_string(),
                ..Default::default()
            },
            Err(e) => {
                self.add_toast(ToastLevel::Error, format!("Read failed: {e}"));
                return;
            }
        };
        self.manifest_state.edit = Some(build_manifest_edit_modal(
            ManifestEditKind::Edit(name),
            &snap,
            cx,
        ));
        self.manifest_state.pending_edit_focus = true;
        cx.notify();
    }

    pub fn close_manifest_edit(&mut self, cx: &mut Context<Self>) {
        self.manifest_state.edit = None;
        cx.notify();
    }

    pub fn apply_manifest_edit(&mut self, cx: &mut Context<Self>) {
        use views::manifest::{ManifestEditKind, ManifestEntrySnapshot};
        let edit = match self.manifest_state.edit.take() {
            Some(e) => e,
            None => return,
        };
        let build_commands_raw = edit.build_commands_input.read(cx).content().to_string();
        let build_commands_joined = build_commands_raw
            .split('\n')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("; ");

        let mut snap = ManifestEntrySnapshot {
            name: edit.name_input.read(cx).content().trim().to_string(),
            version: edit.version_input.read(cx).content().trim().to_string(),
            pkg_id: edit.pkg_id_input.read(cx).content().trim().to_string(),
            repo: edit.repo_input.read(cx).content().trim().to_string(),
            url: edit.url_input.read(cx).content().trim().to_string(),
            github: edit.github_input.read(cx).content().trim().to_string(),
            gitlab: edit.gitlab_input.read(cx).content().trim().to_string(),
            asset_pattern: edit.asset_pattern_input.read(cx).content().trim().to_string(),
            tag_pattern: edit.tag_pattern_input.read(cx).content().trim().to_string(),
            include_prerelease: edit.include_prerelease,
            build_commands: build_commands_joined,
            build_dependencies: edit
                .build_dependencies_input
                .read(cx)
                .content()
                .trim()
                .to_string(),
            install_patterns: edit
                .install_patterns_input
                .read(cx)
                .content()
                .trim()
                .to_string(),
            profile: edit.profile_input.read(cx).content().trim().to_string(),
            pinned: edit.pinned,
            binary_only: edit.binary_only,
        };

        if snap.name.is_empty() {
            self.add_toast(ToastLevel::Error, "Package name cannot be empty".into());
            self.manifest_state.edit = Some(edit);
            return;
        }
        if let ManifestEditKind::Edit(ref original) = edit.kind {
            if original != &snap.name {
                // The name was changed on an existing entry. Remove the old key so
                // we do not leave both.
                if let Err(e) = self.adapter.write_manifest_remove(original) {
                    self.manifest_state.save_error = Some(e.clone());
                    self.add_toast(ToastLevel::Error, format!("Manifest save failed: {e}"));
                    return;
                }
            }
        }

        // Normalize: "*" version becomes empty so Simple form is chosen when
        // nothing else differentiates the entry.
        if snap.version == "*" {
            snap.version = String::new();
        }

        match self.adapter.write_manifest_entry(&snap) {
            Ok(()) => {
                self.manifest_state.save_error = None;
                self.load_manifest_diff(cx);
            }
            Err(e) => {
                self.manifest_state.save_error = Some(e.clone());
                self.add_toast(ToastLevel::Error, format!("Manifest save failed: {e}"));
            }
        }
    }

    pub fn remove_manifest_entry(&mut self, name: String, cx: &mut Context<Self>) {
        match self.adapter.write_manifest_remove(&name) {
            Ok(()) => {
                self.manifest_state.save_error = None;
                self.add_toast(ToastLevel::Info, format!("Removed {name} from manifest"));
                self.load_manifest_diff(cx);
            }
            Err(e) => {
                self.manifest_state.save_error = Some(e.clone());
                self.add_toast(ToastLevel::Error, format!("Manifest save failed: {e}"));
            }
        }
    }

    pub fn import_installed_into_manifest(&mut self, cx: &mut Context<Self>) {
        let entries: Vec<(String, String)> = self
            .installed_state
            .packages
            .iter()
            .filter(|p| p.package.adapter_id == "soar")
            .map(|p| (p.package.name.clone(), p.package.version.clone()))
            .collect();
        if entries.is_empty() {
            self.add_toast(
                ToastLevel::Info,
                "No installed soar packages to import".into(),
            );
            return;
        }
        let count = entries.len();
        match self.adapter.write_manifest_replace_packages(&entries) {
            Ok(()) => {
                self.manifest_state.save_error = None;
                self.add_toast(
                    ToastLevel::Success,
                    format!("Imported {count} packages into manifest"),
                );
                self.load_manifest_diff(cx);
            }
            Err(e) => {
                self.manifest_state.save_error = Some(e.clone());
                self.add_toast(ToastLevel::Error, format!("Manifest save failed: {e}"));
            }
        }
    }

    pub fn select_manifest_entry(&mut self, name: String, cx: &mut Context<Self>) {
        let snap = match self.adapter.read_manifest_entry(&name) {
            Ok(s) => s,
            Err(_) => None,
        };
        self.manifest_state.selected_entry = Some(name);
        self.manifest_state.selected_snapshot = snap;
        cx.notify();
    }

    pub fn clear_manifest_selection(&mut self, cx: &mut Context<Self>) {
        self.manifest_state.selected_entry = None;
        self.manifest_state.selected_snapshot = None;
        cx.notify();
    }

    pub fn create_empty_manifest(&mut self, cx: &mut Context<Self>) {
        match self.adapter.write_manifest_replace_packages(&[]) {
            Ok(()) => {
                self.manifest_state.save_error = None;
                self.add_toast(ToastLevel::Success, "Created an empty manifest".into());
                self.load_manifest_diff(cx);
            }
            Err(e) => {
                self.manifest_state.save_error = Some(e.clone());
                self.add_toast(ToastLevel::Error, format!("Manifest save failed: {e}"));
            }
        }
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
        for pkg in &packages {
            let key = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
            self.updates_state
                .package_progress
                .insert(key, OperationStatus::Starting);
        }
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
        self.pending_settings_edit_focus = true;
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
        if self.manifest_state.edit.is_some() {
            self.close_manifest_edit(cx);
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

    fn record_progress(&mut self, key: String, status: OperationStatus) {
        self.browse_state
            .package_progress
            .insert(key.clone(), status.clone());
        self.installed_state
            .package_progress
            .insert(key.clone(), status.clone());
        self.updates_state.package_progress.insert(key, status);
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
                    self.record_progress(
                        key,
                        OperationStatus::Downloading {
                            current: current_bytes,
                            total: total_bytes,
                        },
                    );
                }
                ProgressEvent::Phase {
                    adapter_id,
                    package_id,
                    phase,
                    ..
                } => {
                    let key = progress_key(&adapter_id, &package_id);
                    self.record_progress(key, OperationStatus::Installing(phase));
                }
                ProgressEvent::Completed {
                    adapter_id,
                    package_id,
                } => {
                    let key = progress_key(&adapter_id, &package_id);
                    self.record_progress(key, OperationStatus::Completed);
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
                    self.record_progress(key, OperationStatus::Failed(error));
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
                        self.record_progress(
                            key,
                            OperationStatus::Downloading { current: 0, total },
                        );
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
                        self.record_progress(
                            key,
                            OperationStatus::Downloading { current, total },
                        );
                    }
                }
                SoarEvent::DownloadComplete { pkg_id, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        self.record_progress(
                            key,
                            OperationStatus::Installing("Download complete".into()),
                        );
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
                        self.record_progress(key, OperationStatus::Verifying(label.into()));
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
                        self.record_progress(key, OperationStatus::Installing(label));
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
                        self.record_progress(key, OperationStatus::Removing(label));
                    }
                }
                SoarEvent::OperationComplete { pkg_id, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        self.record_progress(key, OperationStatus::Completed);
                    }
                }
                SoarEvent::OperationFailed { pkg_id, error, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        self.record_progress(key, OperationStatus::Failed(error));
                    }
                }
                SoarEvent::DownloadRetry { pkg_id, .. }
                | SoarEvent::DownloadAborted { pkg_id, .. } => {
                    if let Some(key) = self.soar_progress_key(&pkg_id) {
                        self.record_progress(
                            key,
                            OperationStatus::Failed("Download failed".into()),
                        );
                    }
                }
                _ => {}
            }
        }

        if had_events {
            cx.notify();
        }

        self.process_manifest_watch(cx);
    }

    /// Drain pending notify events for the manifest file and trigger a
    /// debounced reload when the change originated outside the app.
    fn process_manifest_watch(&mut self, cx: &mut Context<Self>) {
        let mut saw_event = false;
        if let Some(rx) = &self.manifest_watcher_rx {
            while rx.try_recv().is_ok() {
                saw_event = true;
            }
        }

        if saw_event && !self.adapter.is_recent_self_write() {
            self.manifest_reload_due =
                Some(Instant::now() + Duration::from_millis(MANIFEST_RELOAD_COALESCE_MS));
        }

        if let Some(due) = self.manifest_reload_due {
            if Instant::now() >= due {
                self.manifest_reload_due = None;
                // Avoid wiping out a modal that the user is actively editing.
                if self.manifest_state.edit.is_none() {
                    self.load_manifest_diff(cx);
                } else {
                    // Try again shortly so the reload still happens after
                    // the modal closes.
                    self.manifest_reload_due =
                        Some(Instant::now() + Duration::from_millis(MANIFEST_RELOAD_COALESCE_MS));
                }
            }
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

impl Focusable for App {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for App {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme::current_theme(self.selected_theme);

        // Cleanup expired toasts
        self.cleanup_toasts();
        self.reap_running();

        // Ensure the root receives app-level key actions when nothing else
        // is focused (e.g. on first render after window open).
        if !self.focus_handle.contains_focused(window, cx)
            && !window.focused(cx).is_some_and(|f| f.is_focused(window))
        {
            window.focus(&self.focus_handle);
        }

        // Focus settings edit input on first render after open.
        if self.pending_settings_edit_focus {
            if let Some(ref edit) = self.settings_state.edit {
                let handle = edit.input.focus_handle(cx);
                window.focus(&handle);
                self.pending_settings_edit_focus = false;
            }
        }

        if self.manifest_state.pending_edit_focus {
            if let Some(ref edit) = self.manifest_state.edit {
                use views::manifest::ManifestEditKind;
                let handle = match &edit.kind {
                    ManifestEditKind::Add => edit.name_input.focus_handle(cx),
                    ManifestEditKind::Edit(_) => edit.version_input.focus_handle(cx),
                };
                window.focus(&handle);
                self.manifest_state.pending_edit_focus = false;
            }
        }

        let mut root = div()
            .id("app-root")
            .key_context("App")
            .track_focus(&self.focus_handle)
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
                    .min_h_0()
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
                ConfirmAction::ApplyManifest { remove_names, .. } => {
                    if remove_names.is_empty() {
                        format!("Apply manifest?{}", mode_suffix(&self.current_mode))
                    } else {
                        let preview = remove_names
                            .iter()
                            .take(5)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ");
                        let suffix = if remove_names.len() > 5 {
                            format!(" and {} more", remove_names.len() - 5)
                        } else {
                            String::new()
                        };
                        format!(
                            "Apply manifest with prune?{} This will remove: {preview}{suffix}.",
                            mode_suffix(&self.current_mode)
                        )
                    }
                }
                ConfirmAction::RemoveManifestEntry { name } => {
                    format!("Remove {name} from manifest?")
                }
                ConfirmAction::ImportInstalledManifest => {
                    "Replace the manifest with your currently installed packages?".to_string()
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
                .w(px(640.0))
                .child(
                    div()
                        .text_size(px(styles::font_size::HEADING))
                        .child(format!("Edit {}", edit.label)),
                );

            match edit.field_type.clone() {
                ConfigFieldType::Select(options) => {
                    let cancel_select = cx.listener(|app, _: &ClickEvent, _window, cx| {
                        app.close_settings_edit(cx);
                    });
                    body = body.child(
                        div()
                            .text_size(px(styles::font_size::CAPTION))
                            .text_color(text_muted)
                            .child("Select a value, or press Escape to cancel."),
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
                    body = body.child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .child(
                                div()
                                    .id("settings-edit-select-cancel")
                                    .px(px(styles::spacing::LG))
                                    .py(px(styles::spacing::XS))
                                    .rounded(px(styles::radius::MD))
                                    .bg(surface)
                                    .border_1()
                                    .border_color(border)
                                    .cursor_pointer()
                                    .hover(move |s| s.bg(hover))
                                    .on_click(cancel_select)
                                    .child("Cancel"),
                            ),
                    );
                }
                _ => {
                    let mut input_row = div()
                        .flex()
                        .flex_row()
                        .gap(px(styles::spacing::SM))
                        .items_center()
                        .w_full()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .px(px(styles::spacing::MD))
                                .py(px(10.0))
                                .rounded(px(styles::radius::MD))
                                .bg(surface)
                                .border_1()
                                .border_color(border)
                                .child(edit.input.clone()),
                        );

                    // Add a Browse… button for file/dir-like fields.
                    let needs_browse = matches!(
                        edit.field_type,
                        ConfigFieldType::ExecutablePath | ConfigFieldType::PathList
                    );
                    if needs_browse {
                        let pick_dir = matches!(edit.field_type, ConfigFieldType::PathList);
                        let input_handle = edit.input.clone();
                        let browse = cx.listener(move |_app, _: &ClickEvent, _window, cx| {
                            let dialog = rfd::FileDialog::new();
                            let chosen = if pick_dir {
                                dialog.pick_folder()
                            } else {
                                dialog.pick_file()
                            };
                            if let Some(path) = chosen {
                                let s = path.to_string_lossy().to_string();
                                input_handle.update(cx, |ti, cx| {
                                    ti.set_content(s, cx);
                                });
                            }
                            cx.notify();
                        });
                        input_row = input_row.child(
                            div()
                                .id("settings-edit-browse")
                                .px(px(styles::spacing::MD))
                                .py(px(styles::spacing::XS))
                                .rounded(px(styles::radius::MD))
                                .bg(surface)
                                .border_1()
                                .border_color(border)
                                .cursor_pointer()
                                .text_size(px(styles::font_size::SMALL))
                                .on_click(browse)
                                .child(if pick_dir {
                                    "Pick folder…"
                                } else {
                                    "Pick file…"
                                }),
                        );
                    }
                    body = body.child(input_row);
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

        if self.manifest_state.edit.is_some() {
            root = root.child(self.render_manifest_edit_modal(&theme, cx));
        }

        root
    }
}

impl App {
    fn render_manifest_edit_modal(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        use views::manifest::ManifestEditKind;
        let edit = self
            .manifest_state
            .edit
            .as_ref()
            .expect("called only when edit is Some");

        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;
        let text_muted = theme.text_muted;

        let title = match &edit.kind {
            ManifestEditKind::Add => "Add package to manifest".to_string(),
            ManifestEditKind::Edit(name) => format!("Edit {name}"),
        };
        let name_editable = matches!(edit.kind, ManifestEditKind::Add);

        let cancel = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.close_manifest_edit(cx);
        });
        let save = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.apply_manifest_edit(cx);
        });
        let toggle_prerelease = cx.listener(|app, _: &ClickEvent, _window, cx| {
            if let Some(ref mut e) = app.manifest_state.edit {
                e.include_prerelease = !e.include_prerelease;
                cx.notify();
            }
        });
        let toggle_pinned = cx.listener(|app, _: &ClickEvent, _window, cx| {
            if let Some(ref mut e) = app.manifest_state.edit {
                e.pinned = !e.pinned;
                cx.notify();
            }
        });
        let toggle_binary_only = cx.listener(|app, _: &ClickEvent, _window, cx| {
            if let Some(ref mut e) = app.manifest_state.edit {
                e.binary_only = !e.binary_only;
                cx.notify();
            }
        });

        let include_prerelease = edit.include_prerelease;
        let pinned = edit.pinned;
        let binary_only = edit.binary_only;

        let name_input = edit.name_input.clone();
        let version_input = edit.version_input.clone();
        let pkg_id_input = edit.pkg_id_input.clone();
        let repo_input = edit.repo_input.clone();
        let url_input = edit.url_input.clone();
        let github_input = edit.github_input.clone();
        let gitlab_input = edit.gitlab_input.clone();
        let asset_pattern_input = edit.asset_pattern_input.clone();
        let tag_pattern_input = edit.tag_pattern_input.clone();
        let build_commands_input = edit.build_commands_input.clone();
        let build_dependencies_input = edit.build_dependencies_input.clone();
        let install_patterns_input = edit.install_patterns_input.clone();
        let profile_input = edit.profile_input.clone();

        let field_input = |entity: Entity<crate::components::TextInput>, editable: bool| -> Div {
            div()
                .min_w_0()
                .overflow_hidden()
                .px(px(styles::spacing::MD))
                .py(px(10.0))
                .rounded(px(styles::radius::MD))
                .bg(if editable { surface } else { theme.hover })
                .border_1()
                .border_color(border)
                .child(entity)
        };

        let field_label = |label: &str, hint: Option<&str>| -> Div {
            let mut col = div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::XXXS))
                .child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text_muted)
                        .child(label.to_string()),
                );
            if let Some(h) = hint {
                col = col.child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .text_color(text_muted)
                        .child(h.to_string()),
                );
            }
            col
        };

        let section = |title: &str, rows: Vec<Div>| -> Div {
            let mut col = div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::SM))
                .child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text_muted)
                        .child(title.to_uppercase()),
                );
            for row in rows {
                col = col.child(row);
            }
            col
        };

        let toggle_row = |label: &str,
                          description: &str,
                          on: bool,
                          id: &str,
                          listener: Box<
            dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
        >|
         -> Div {
            let track_on = primary;
            let track_off = border;
            let track = if on { track_on } else { track_off };
            let thumb = if on {
                div().ml_auto().w(px(16.0)).h(px(16.0)).rounded_full().bg(gpui::white())
            } else {
                div().w(px(16.0)).h(px(16.0)).rounded_full().bg(gpui::white())
            };
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap(px(styles::spacing::MD))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(styles::spacing::XXXS))
                        .child(
                            div()
                                .text_size(px(styles::font_size::BODY))
                                .font_weight(FontWeight::MEDIUM)
                                .child(label.to_string()),
                        )
                        .child(
                            div()
                                .text_size(px(styles::font_size::CAPTION))
                                .text_color(text_muted)
                                .child(description.to_string()),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(id.to_string()))
                        .w(px(34.0))
                        .h(px(20.0))
                        .p(px(2.0))
                        .rounded_full()
                        .bg(track)
                        .cursor_pointer()
                        .flex()
                        .flex_row()
                        .items_center()
                        .on_click(listener)
                        .child(thumb),
                )
        };

        let labeled_row = |label: &str,
                           hint: Option<&str>,
                           entity: Entity<crate::components::TextInput>,
                           editable: bool|
         -> Div {
            div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::XXS))
                .child(field_label(label, hint))
                .child(field_input(entity, editable))
        };

        let basic = section(
            "Basic",
            vec![
                labeled_row("Name", None, name_input, name_editable),
                labeled_row(
                    "Version",
                    Some("Use * for latest, or a specific version like 1.2.3"),
                    version_input,
                    true,
                ),
            ],
        );

        let identity = section(
            "Identity",
            vec![
                labeled_row(
                    "Package ID",
                    Some("Optional. Disambiguates packages that share a name across repos."),
                    pkg_id_input,
                    true,
                ),
                labeled_row(
                    "Repository",
                    Some("Optional. Restricts the lookup to a specific repository."),
                    repo_input,
                    true,
                ),
            ],
        );

        let source = section(
            "External source",
            vec![
                labeled_row(
                    "Download URL",
                    Some("Direct URL to an installable asset."),
                    url_input,
                    true,
                ),
                labeled_row(
                    "GitHub repository",
                    Some("owner/repo to fetch a release from."),
                    github_input,
                    true,
                ),
                labeled_row(
                    "GitLab repository",
                    Some("owner/repo to fetch a release from."),
                    gitlab_input,
                    true,
                ),
                labeled_row(
                    "Asset pattern",
                    Some("Glob to match the release asset, e.g. *linux*.AppImage."),
                    asset_pattern_input,
                    true,
                ),
                labeled_row(
                    "Tag pattern",
                    Some("Glob to filter releases, e.g. v*-stable."),
                    tag_pattern_input,
                    true,
                ),
                toggle_row(
                    "Include prereleases",
                    "Pull pre-release tags from github or gitlab when matching.",
                    include_prerelease,
                    "manifest-toggle-prerelease",
                    Box::new(toggle_prerelease),
                ),
            ],
        );

        let build = section(
            "Build",
            vec![
                labeled_row(
                    "Commands",
                    Some("Shell commands separated by semicolons. Env: $INSTALL_DIR, $PKG_NAME, $PKG_VERSION, $NPROC."),
                    build_commands_input,
                    true,
                ),
                labeled_row(
                    "Dependencies",
                    Some("Comma-separated build dependencies expected on PATH."),
                    build_dependencies_input,
                    true,
                ),
            ],
        );

        let options = section(
            "Options",
            vec![
                labeled_row(
                    "Install patterns",
                    Some("Comma-separated glob patterns of files to keep."),
                    install_patterns_input,
                    true,
                ),
                labeled_row(
                    "Profile",
                    Some("Override the default profile this package installs into."),
                    profile_input,
                    true,
                ),
                toggle_row(
                    "Pinned",
                    "Skip automatic updates for this package.",
                    pinned,
                    "manifest-toggle-pinned",
                    Box::new(toggle_pinned),
                ),
                toggle_row(
                    "Binary only",
                    "Install just the binaries, no desktop or icon files.",
                    binary_only,
                    "manifest-toggle-binary-only",
                    Box::new(toggle_binary_only),
                ),
            ],
        );

        let inner = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::LG))
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child("Press Escape to cancel."),
            )
            .child(basic)
            .child(identity)
            .child(source)
            .child(build)
            .child(options);

        let footer = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .justify_end()
            .pt(px(styles::spacing::MD))
            .border_t_1()
            .border_color(border)
            .child(
                div()
                    .id("manifest-edit-cancel")
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
                    .id("manifest-edit-save")
                    .px(px(styles::spacing::LG))
                    .py(px(styles::spacing::XS))
                    .rounded(px(styles::radius::MD))
                    .bg(primary)
                    .text_color(gpui::white())
                    .cursor_pointer()
                    .on_click(save)
                    .child("Save"),
            );

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
                    .w(px(640.0))
                    .max_h(px(720.0))
                    .rounded(px(styles::radius::LG))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("manifest-edit-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p(px(styles::spacing::XXL))
                            .child(inner),
                    )
                    .child(
                        div()
                            .px(px(styles::spacing::XXL))
                            .pb(px(styles::spacing::LG))
                            .child(footer),
                    ),
            )
    }

    fn render_sidebar(&mut self, theme: &theme::Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current_view;
        let mut nav_items: Vec<(View, &str)> = vec![
            (View::Dashboard, "Dashboard"),
            (View::Browse, "Browse"),
            (View::Installed, "Installed"),
            (View::Updates, "Updates"),
            (View::AdapterInfo, "Adapters"),
        ];
        if self
            .adapter_manager
            .list_adapters()
            .iter()
            .any(|info| info.capabilities.supports_declarative)
        {
            nav_items.push((View::Manifest, "Manifest"));
        }
        nav_items.push((View::Settings, "Settings"));

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
        let wrapper = div().flex_1().min_h_0().flex().flex_col();

        match self.current_view {
            View::Dashboard => wrapper.child(self.render_dashboard(theme, cx)),
            View::Browse => wrapper.child(self.render_browse(theme, cx)),
            View::Installed => wrapper.child(self.render_installed(theme, cx)),
            View::Updates => wrapper.child(self.render_updates(theme, cx)),
            View::AdapterInfo => wrapper.child(self.render_adapter_info(theme, cx)),
            View::Manifest => wrapper.child(self.render_manifest(theme, cx)),
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
            ConfirmAction::UpdateAll(_mode) => {
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
            ConfirmAction::ApplyManifest { prune, .. } => {
                self.apply_manifest(prune, cx);
            }
            ConfirmAction::RemoveManifestEntry { name } => {
                self.remove_manifest_entry(name, cx);
            }
            ConfirmAction::ImportInstalledManifest => {
                self.import_installed_into_manifest(cx);
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
