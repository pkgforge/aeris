pub mod message;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::*;

use crate::{
    adapters::command::{self, CommandAdapter},
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

/// The manager aeris ships knowing about.
pub const SOAR_ID: &str = "soar";
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
    Downloading {
        current: u64,
        total: u64,
    },
    /// What the manager says it is doing, in its own words.
    Installing(String),
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
            OperationStatus::Installing(phase) => format!("Installing ({phase})..."),
            OperationStatus::Completed => "Completed".into(),
            OperationStatus::Failed(e) => format!("Failed: {e}"),
        }
    }

    /// Whether the work this describes is over, one way or the other.
    ///
    /// A record left behind by a finished operation says nothing about what
    /// is happening now, and reading it as though it did leaves the last
    /// thing that happened on screen forever.
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            OperationStatus::Completed | OperationStatus::Failed(_)
        )
    }

    /// The same thing said in as few characters as it takes, for somewhere
    /// too narrow to spell it out.
    pub fn short_label(&self) -> String {
        match self {
            OperationStatus::Downloading { current, total } if *total > 0 => {
                let percent = (*current as f64 / *total as f64 * 100.0) as u64;
                format!("Downloading {percent}%")
            }
            OperationStatus::Installing(phase) => phase.clone(),
            OperationStatus::Failed(_) => "Failed".into(),
            other => other.label(),
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

/// Order results by how well they answer the query rather than by which
/// manager happened to be asked first.
///
/// An exact name comes first, then a name starting with what was typed, then
/// one merely containing it, and last anything that matched on its
/// description alone. Within a tier the shorter name wins, since a longer one
/// is usually a variant of it, and the name settles the rest so the order
/// does not shift between searches.
pub(crate) fn rank_results(results: &mut [crate::core::package::Package], query: &str) {
    let query = query.trim().to_lowercase();

    results.sort_by_cached_key(|pkg| {
        let name = pkg.name.to_lowercase();
        let (tier, score) = if name == query {
            (0u8, 0)
        } else if name.starts_with(&query) {
            (1, 0)
        } else if name.contains(&query) {
            (2, 0)
        } else if let Some(score) = subsequence_score(&name, &query) {
            (3, score)
        } else {
            (4, 0)
        };

        (
            tier,
            std::cmp::Reverse(score),
            name.len(),
            name,
            pkg.adapter_id.clone(),
        )
    });
}

/// How well a name answers a query typed as an abbreviation, or nothing when
/// the query is not in there at all.
///
/// The letters have to appear in order. A letter scores most at the very
/// start, well at the start of a word within the name, and little in the
/// middle of one; running on unbroken from the last letter is worth as much
/// again. The name's length is then taken off, so of two names holding the
/// query the tighter one wins.
///
/// Letters are taken as they come rather than by looking for the best
/// arrangement. That can score an awkward name slightly low, which costs an
/// ordering rather than a result.
fn subsequence_score(name: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return None;
    }

    let name: Vec<char> = name.chars().collect();
    let mut score = 0u32;
    let mut from = 0usize;
    let mut previous: Option<usize> = None;

    for wanted in query.chars() {
        let at = name[from..].iter().position(|c| *c == wanted)? + from;

        score += if at == 0 {
            16
        } else if matches!(name[at - 1], '-' | '_' | '.' | ' ' | '/' | '+') {
            8
        } else {
            1
        };

        if previous.is_some_and(|last| at == last + 1) {
            score += 8;
        }

        previous = Some(at);
        from = at + 1;
    }

    Some(score.saturating_sub(name.len().min(32) as u32))
}

/// A sentence accounting for the managers this scope leaves out, or nothing
/// when it leaves none out.
///
/// Without it a manager that only installs one way looks like it lost its
/// packages when the scope is switched.
pub(crate) fn scope_note(names: &[String], mode: PackageMode) -> Option<String> {
    let elsewhere = match mode {
        PackageMode::User => "System",
        PackageMode::System => "User",
    };

    match names {
        [] => None,
        [one] => Some(format!(
            "{one} only works in {elsewhere} mode, so its packages are listed there."
        )),
        [rest @ .., last] => Some(format!(
            "{} and {last} only work in {elsewhere} mode, so their packages are listed there.",
            rest.join(", ")
        )),
    }
}

/// Where the manager links the commands it installs.
pub(crate) fn active_bin_path(paths: &HashMap<String, String>) -> Option<std::path::PathBuf> {
    paths.get("bin").map(std::path::PathBuf::from)
}

#[derive(Default)]
pub struct AdapterViewState {
    pub registry_plugins: Vec<PluginEntry>,
    pub registry_loading: bool,
    pub registry_error: Option<String>,
    /// When the listing on show was read, so the page can say how old it is.
    pub registry_read_at: Option<std::time::SystemTime>,
    /// Set once the page has looked, so a listing that comes back empty or
    /// fails is not requested again on every frame.
    pub registry_considered: bool,
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
    /// The manager aeris drives, absent when none could be reached.
    pub(crate) adapter: Option<Arc<dyn Adapter>>,
    /// Why soar could not be used, for the adapters page to explain.
    pub(crate) soar_problem: Option<String>,
    /// Where the manager keeps its files, read once at startup. Aeris edits
    /// some of them directly rather than through a command for every field.
    pub(crate) paths: HashMap<String, String>,
    /// When aeris last wrote the declarative file itself, so the watcher can
    /// tell its own writes from someone else's.
    last_self_write: std::cell::Cell<Option<Instant>>,
    pub(crate) adapter_manager: AdapterManager,
    pub(crate) adapter_view: AdapterViewState,
    pub(crate) confirm_dialog: Option<ConfirmAction>,
    pub(crate) run_picker: Option<RunPicker>,
    /// Running processes launched via Run, keyed by package unique_key.
    pub(crate) running_processes: HashMap<String, Vec<RunningProcess>>,
    next_run_id: u64,
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

/// Turn a stage name as a manager reports it into one worth showing.
fn readable(stage: &str) -> String {
    let mut words = stage.replace('_', " ");
    if let Some(first) = words.get_mut(..1) {
        first.make_ascii_uppercase();
    }

    words
}

fn missing_manifest() -> String {
    "soar did not say where its packages file is".to_string()
}

/// Window during which a change arriving after one of our own writes is
/// treated as ours and ignored by the watcher.
const SELF_WRITE_DEBOUNCE: Duration = Duration::from_millis(750);

/// Wait at least this long after a notify event before reloading. Coalesces
/// the burst that an atomic rename produces and keeps us from racing partial
/// writes from external editors.
const MANIFEST_RELOAD_COALESCE_MS: u64 = 200;

/// Spawn a notify watcher on the manifest's parent directory and return a
/// receiver that fires whenever the manifest path is touched. The watcher
/// itself is returned boxed so the caller can keep it alive without naming
/// notify types across the field type.
fn spawn_manifest_watcher(
    path: Option<&std::path::Path>,
) -> (
    Option<std::sync::mpsc::Receiver<()>>,
    Option<Box<dyn std::any::Any + Send>>,
) {
    use notify::{EventKind, RecursiveMode, Watcher};

    let Some(path) = path else {
        return (None, None);
    };
    let path = path.to_path_buf();
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
        log::warn!(
            "manifest watcher: failed to watch {}: {e}",
            parent.display()
        );
        return (None, None);
    }

    (
        Some(rx),
        Some(Box::new(watcher) as Box<dyn std::any::Any + Send>),
    )
}

/// Register an adapter unless one already answers to its id.
///
/// Ids decide where settings are kept, so the one registered first keeps the
/// name rather than being replaced by whatever was discovered later.
fn register_new(manager: &mut AdapterManager, adapter: Arc<dyn Adapter>) {
    let id = adapter.info().id.clone();

    if manager.get_adapter(&id).is_some() {
        log::warn!("Ignoring a second adapter named {id}");
        return;
    }

    manager.register(adapter);
}

impl App {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let aeris_config = AerisConfig::load();

        let selected_theme = aeris_config.theme();
        let startup_view = aeris_config.startup_view();

        let mut adapter_manager = AdapterManager::new();
        let mut soar_problem = None;
        let mut paths = HashMap::new();

        // Turned off means left alone: no looking for it, and nothing said
        // about not finding it.
        let adapter: Option<Arc<dyn Adapter>> = if aeris_config.is_adapter_disabled(SOAR_ID) {
            None
        } else {
            // Soar describes itself, so aeris drives whichever one is
            // installed rather than the one it was built against.
            match CommandAdapter::from_command(SOAR_ID, command::DESCRIBE_ARGS)
                .map(CommandAdapter::as_builtin)
            {
                Ok(soar) => {
                    log::info!("Driving soar {}", soar.info().version);
                    paths = soar.file_paths().unwrap_or_else(|e| {
                        log::warn!("soar did not say where its files are: {e}");
                        HashMap::new()
                    });

                    let adapter: Arc<dyn Adapter> = Arc::new(soar);
                    adapter_manager.register(adapter.clone());
                    Some(adapter)
                }
                // Said on the adapters page instead of thrown as a message,
                // so it can be read once and turned off rather than met at
                // every start.
                Err(e) => {
                    log::warn!("Soar is unavailable: {e}");
                    soar_problem = Some(match &e {
                        crate::core::adapter::AdapterError::PluginError(said) => said.clone(),
                        other => other.to_string(),
                    });
                    None
                }
            }
        };

        for result in crate::adapters::command::load_all() {
            match result {
                Ok(manifest_adapter) => {
                    log::info!("Loaded adapter: {}", manifest_adapter.info().id);
                    register_new(&mut adapter_manager, Arc::new(manifest_adapter));
                }
                Err(e) => log::warn!("Failed to load adapter: {e}"),
            }
        }

        let disabled: std::collections::HashSet<String> =
            aeris_config.disabled_adapters.iter().cloned().collect();
        adapter_manager.set_disabled(disabled);

        // Whichever scope the adapters actually work in. A manager that only
        // ever acts system wide would otherwise have its packages counted and
        // labelled as the user's.
        let default_mode = if adapter_manager
            .list_adapters()
            .iter()
            .any(|info| info.capabilities.supports_user_packages)
        {
            PackageMode::User
        } else if adapter_manager
            .list_adapters()
            .iter()
            .any(|info| info.capabilities.supports_system_packages)
        {
            PackageMode::System
        } else {
            PackageMode::User
        };
        let (progress_sender, progress_receiver) = tokio::sync::mpsc::unbounded_channel();

        let settings_state = match adapter.as_ref() {
            Some(adapter) => views::settings::SettingsState::load(&aeris_config, adapter.as_ref()),
            None => views::settings::SettingsState::default(),
        };

        let search_input = cx.new(|cx| crate::components::TextInput::new(cx, "Search packages..."));

        let (manifest_watcher_rx, manifest_watcher) =
            spawn_manifest_watcher(paths.get("packages_config").map(std::path::Path::new));

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
            soar_problem,
            paths,
            last_self_write: std::cell::Cell::new(None),
            adapter_manager,
            adapter_view: AdapterViewState::default(),
            confirm_dialog: None,
            run_picker: None,
            running_processes: HashMap::new(),
            next_run_id: 1,
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

        let mode = self.current_mode;
        let manager_adapters = self.adapters_for(mode);

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
                    rank_results(&mut results, &query);
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

    /// The adapters that should answer for a scope.
    ///
    /// Enabled, and able to work that way at all: a manager that only
    /// installs system wide has no user packages, and asking anyway would
    /// answer with system ones listed under the user's name.
    fn adapters_for(&self, mode: PackageMode) -> Vec<Arc<dyn Adapter>> {
        self.adapter_manager
            .list_adapters()
            .iter()
            .filter_map(|info| self.adapter_manager.get_adapter(&info.id))
            .filter(|a| {
                self.adapter_manager.is_enabled(&a.info().id) && a.capabilities().works_in(mode)
            })
            .collect()
    }

    pub fn load_installed(&mut self, cx: &mut Context<Self>) {
        self.installed_state.loading = true;
        self.installed_state.error = None;

        let mode = self.current_mode;
        let manager_adapters = self.adapters_for(mode);

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

    /// Where the declarative package file lives, for a manager that has one.
    fn manifest_path(&self) -> Option<std::path::PathBuf> {
        self.paths
            .get("packages_config")
            .map(std::path::PathBuf::from)
    }

    /// Remember that the next change to the file is ours, so the watcher does
    /// not read it back as though someone else had edited it.
    fn mark_self_write(&self) {
        self.last_self_write.set(Some(Instant::now()));
    }

    fn is_recent_self_write(&self) -> bool {
        self.last_self_write
            .get()
            .is_some_and(|at| at.elapsed() < SELF_WRITE_DEBOUNCE)
    }

    fn read_manifest_entry(
        &self,
        name: &str,
    ) -> std::result::Result<Option<views::manifest::ManifestEntrySnapshot>, String> {
        let Some(path) = self.manifest_path() else {
            return Ok(None);
        };
        crate::manifest_file::read_entry(&path, name)
    }

    fn write_manifest_entry(
        &self,
        snapshot: &views::manifest::ManifestEntrySnapshot,
    ) -> std::result::Result<(), String> {
        let path = self.manifest_path().ok_or_else(missing_manifest)?;
        self.mark_self_write();
        crate::manifest_file::write_entry(&path, snapshot)
    }

    fn write_manifest_remove(&self, name: &str) -> std::result::Result<(), String> {
        let path = self.manifest_path().ok_or_else(missing_manifest)?;
        self.mark_self_write();
        crate::manifest_file::remove_entry(&path, name)
    }

    fn write_manifest_replace_packages(
        &self,
        entries: &[(String, String)],
    ) -> std::result::Result<(), String> {
        let path = self.manifest_path().ok_or_else(missing_manifest)?;
        self.mark_self_write();
        crate::manifest_file::replace_packages(&path, entries)
    }

    pub fn load_manifest_diff(&mut self, cx: &mut Context<Self>) {
        use crate::manifest_file::ManifestLoadError;
        use views::manifest::ManifestStatus;

        let path = self.manifest_path();
        self.manifest_state.path = path.clone();
        self.manifest_state.status = ManifestStatus::Loading;

        let Some(adapter) = self.adapter.clone() else {
            self.manifest_state.status = ManifestStatus::Failed("No package manager".into());
            return;
        };

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result = crate::tokio_spawn(async move {
                    match path.as_deref() {
                        Some(path) => crate::manifest_file::check_readable(path)?,
                        None => return Err(ManifestLoadError::FileMissing),
                    }

                    adapter
                        .declarative_diff()
                        .await
                        .map_err(|e| ManifestLoadError::Other(e.to_string()))
                })
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
                            match app.read_manifest_entry(&name) {
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
        use crate::manifest_file::ManifestLoadError;
        use views::manifest::ManifestStatus;

        let Some(adapter_id) = self.adapter.as_ref().map(|a| a.info().id.clone()) else {
            return;
        };
        let seed_keys: Vec<String> = match &self.manifest_state.status {
            ManifestStatus::Loaded(diff) => {
                let mut keys: Vec<String> = diff
                    .to_install
                    .iter()
                    .chain(diff.to_update.iter())
                    .map(|e| crate::core::adapter::package_key(&adapter_id, &e.name))
                    .collect();
                if prune {
                    keys.extend(
                        diff.to_remove
                            .iter()
                            .map(|e| crate::core::adapter::package_key(&adapter_id, &e.name)),
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

        let Some(adapter) = self.adapter.clone() else {
            return;
        };
        let progress = self.progress_sender.clone();

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result = crate::tokio_spawn(async move {
                    adapter
                        .declarative_apply(prune, Some(progress))
                        .await
                        .map_err(|e| ManifestLoadError::Other(e.to_string()))
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
        use views::manifest::{ManifestEditKind, ManifestEntrySnapshot, build_manifest_edit_modal};
        let snap = ManifestEntrySnapshot {
            version: "*".to_string(),
            ..Default::default()
        };
        self.manifest_state.edit =
            Some(build_manifest_edit_modal(ManifestEditKind::Add, &snap, cx));
        self.manifest_state.pending_edit_focus = true;
        cx.notify();
    }

    pub fn open_manifest_edit(&mut self, name: String, cx: &mut Context<Self>) {
        use views::manifest::{ManifestEditKind, ManifestEntrySnapshot, build_manifest_edit_modal};
        let snap = match self.read_manifest_entry(&name) {
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
            repo: edit.repo_input.read(cx).content().trim().to_string(),
            url: edit.url_input.read(cx).content().trim().to_string(),
            github: edit.github_input.read(cx).content().trim().to_string(),
            gitlab: edit.gitlab_input.read(cx).content().trim().to_string(),
            asset_pattern: edit
                .asset_pattern_input
                .read(cx)
                .content()
                .trim()
                .to_string(),
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
                if let Err(e) = self.write_manifest_remove(original) {
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

        match self.write_manifest_entry(&snap) {
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
        match self.write_manifest_remove(&name) {
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
        match self.write_manifest_replace_packages(&entries) {
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
        let snap = match self.read_manifest_entry(&name) {
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
        match self.write_manifest_replace_packages(&[]) {
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

        let mode = self.current_mode;
        let manager_adapters = self.adapters_for(mode);

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
            .filter(|u| {
                selected.contains(&crate::core::adapter::package_key(
                    &u.package.adapter_id,
                    &u.package.id,
                ))
            })
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
            .filter(|p| {
                selected.contains(&crate::core::adapter::package_key(&p.adapter_id, &p.id))
                    && !p.installed
            })
            .cloned()
            .collect();
        let package_ids: Vec<String> = packages.iter().map(|p| p.id.clone()).collect();
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
                            if package_ids.contains(&p.id) {
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
                            format!("Installed {} packages", package_ids.len()),
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

    /// Adapters whose manifest is on disk but which cannot be used, with the
    /// command each is missing.
    ///
    /// These exist as far as the person who added them is concerned, so the
    /// page has to account for them rather than leave them nowhere.
    pub fn unusable_adapters(&self) -> Vec<(crate::core::adapter::AdapterInfo, String)> {
        let mut unusable = Vec::new();

        // Soar has a place on this page whether or not it can run, so there
        // is always something to turn back on.
        if self.adapter_manager.get_adapter(SOAR_ID).is_none() {
            // Already a whole sentence, since detection is what produced it.
            let reason = self
                .soar_problem
                .clone()
                .unwrap_or_else(|| "soar is not installed".to_string());

            unusable.push((
                crate::core::adapter::AdapterInfo {
                    id: SOAR_ID.to_string(),
                    name: "Soar".to_string(),
                    version: String::new(),
                    capabilities: Default::default(),
                    enabled: false,
                    is_builtin: true,
                    plugin_path: None,
                    description: "Fast package manager for static binaries".to_string(),
                    icon: None,
                },
                reason,
            ));
        }

        unusable.extend(
            crate::adapters::command::manifest::discover()
                .into_iter()
                .filter(|(_, manifest)| self.adapter_manager.get_adapter(&manifest.id).is_none())
                .map(|(path, manifest)| {
                    let capabilities =
                        crate::adapters::command::adapter::capabilities_from(&manifest);
                    // Nothing is run from here, so the command being absent is
                    // the reason worth assuming and the one worth saying.
                    let reason = format!("{} is not installed", manifest.detect.command);

                    (
                        crate::core::adapter::AdapterInfo {
                            id: manifest.id,
                            name: manifest.name,
                            version: manifest.version,
                            capabilities,
                            enabled: false,
                            is_builtin: false,
                            plugin_path: Some(path),
                            description: manifest.description,
                            icon: manifest.icon,
                        },
                        reason,
                    )
                }),
        );

        unusable
    }

    /// Try again to use an adapter whose command was missing, now that it
    /// might not be.
    pub fn retry_adapter(&mut self, id: String, cx: &mut Context<Self>) {
        let Some((path, manifest)) = crate::adapters::command::manifest::discover()
            .into_iter()
            .find(|(_, manifest)| manifest.id == id)
        else {
            return;
        };

        let name = manifest.name.clone();
        match CommandAdapter::new(manifest, Some(path)) {
            Ok(adapter) => {
                register_new(&mut self.adapter_manager, Arc::new(adapter));
                self.add_toast(ToastLevel::Success, format!("{name} is ready"));
            }
            Err(e) => {
                use crate::core::adapter::AdapterError;

                let reason = match &e {
                    AdapterError::PluginError(said) => said.clone(),
                    other => other.to_string(),
                };
                self.add_toast(
                    ToastLevel::Error,
                    format!("{name} still cannot run: {reason}"),
                );
            }
        }

        cx.notify();
    }

    /// Forget an adapter that was added but never worked.
    pub fn forget_adapter(&mut self, id: String, cx: &mut Context<Self>) {
        match crate::core::registry::remove_plugin(&id) {
            Ok(()) => self.add_toast(ToastLevel::Success, format!("Removed {id}")),
            Err(e) => self.add_toast(ToastLevel::Error, format!("Could not remove {id}: {e}")),
        }

        cx.notify();
    }

    /// Show whatever was read last, and read again if that was long enough
    /// ago. Called when the adapters page is opened.
    pub fn consider_registry(&mut self, cx: &mut Context<Self>) {
        if self.adapter_view.registry_considered {
            return;
        }
        self.adapter_view.registry_considered = true;

        if let Some((registry, read_at)) = crate::core::registry::cached_registry() {
            self.adapter_view.registry_plugins = registry.plugins;
            self.adapter_view.registry_read_at = Some(read_at);
        }

        // `never` leaves reading it again to whoever asks.
        let Some(within) = self.aeris_config.registry_sync_interval() else {
            return;
        };

        if crate::core::registry::cache_is_stale(within) {
            self.fetch_registry(cx);
        }
    }

    pub fn fetch_registry(&mut self, cx: &mut Context<Self>) {
        self.adapter_view.registry_loading = true;
        self.adapter_view.registry_error = None;

        let registry_url = self.aeris_config.registry_url.clone();

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result = crate::core::registry::fetch_registry(registry_url.as_deref());

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        match result {
                            Ok(registry) => {
                                app.adapter_view.registry_plugins = registry.plugins;
                                app.adapter_view.registry_read_at =
                                    Some(std::time::SystemTime::now());
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

    /// Fetch an adapter's manifest from the registry and start using it.
    ///
    /// The manifest is kept whether or not the manager it describes is
    /// installed, so installing the manager later is all it takes.
    pub fn install_plugin(&mut self, entry: PluginEntry, cx: &mut Context<Self>) {
        let id = entry.id.clone();
        let name = entry.name.clone();
        self.adapter_view.installing_plugin = Some(id.clone());

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let fetched = crate::core::registry::download_plugin(&entry).and_then(|path| {
                    let manifest = crate::adapters::command::manifest::load(&path)?;
                    Ok((path, manifest))
                });

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.adapter_view.installing_plugin = None;

                        match fetched {
                            Ok((path, manifest)) => {
                                match CommandAdapter::new(manifest, Some(path)) {
                                    Ok(adapter) => {
                                        register_new(&mut app.adapter_manager, Arc::new(adapter));
                                        app.add_toast(ToastLevel::Success, format!("Added {name}"));
                                    }
                                    // The manifest is sound but the manager it
                                    // describes is missing or too old. It is
                                    // kept either way, so installing that is
                                    // all it takes.
                                    Err(e) => {
                                        use crate::core::adapter::AdapterError;

                                        let reason = match &e {
                                            AdapterError::PluginError(said) => said.clone(),
                                            other => other.to_string(),
                                        };
                                        app.add_toast(
                                            ToastLevel::Error,
                                            format!("Added {name}, but {reason}"),
                                        )
                                    }
                                }
                            }
                            Err(e) => app
                                .add_toast(ToastLevel::Error, format!("Could not add {name}: {e}")),
                        }

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
        // An empty URL falls back to the default, so it is stored as nothing
        // rather than as an empty string.
        self.aeris_config.registry_url = if self.settings_state.registry_url.trim().is_empty() {
            None
        } else {
            Some(self.settings_state.registry_url.trim().to_string())
        };

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

    /// Fetch the registry from the URL currently in the box and report back
    /// how many adapters it offers, or why it could not be read. The value is
    /// tested as-is rather than waiting for a save, and a blank URL means the
    /// default, so a source can be tried before it is committed.
    pub fn test_registry(&mut self, cx: &mut Context<Self>) {
        self.settings_state.registry_testing = true;
        self.settings_state.registry_test_error = None;
        self.settings_state.registry_test_count = None;

        let url = self.settings_state.registry_url.trim().to_string();
        let url = if url.is_empty() { None } else { Some(url) };

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let result = crate::core::registry::fetch_registry(url.as_deref());
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.settings_state.registry_testing = false;
                        match result {
                            Ok(registry) => {
                                app.settings_state.registry_test_count =
                                    Some(registry.plugins.len());
                            }
                            Err(e) => {
                                app.settings_state.registry_test_error = Some(e);
                            }
                        }
                        cx.notify();
                    })
                });
            },
        )
        .detach();
    }

    pub fn save_adapter_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_state.saving = true;
        self.settings_state.adapter_save_error = None;
        self.settings_state.adapter_save_success = false;

        let config = self.settings_state.adapter_config.clone();
        let Some(adapter) = self.adapter.clone() else {
            return;
        };
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
        let as_bool = |v: Option<&ConfigValue>| match v {
            Some(ConfigValue::Bool(b)) => Some(*b),
            _ => None,
        };
        // Flip relative to the value in effect — an override, or else what the
        // manager currently has — rather than the empty edit state.
        let effective = as_bool(self.settings_state.adapter_config.values.get(key))
            .or_else(|| as_bool(self.settings_state.current_config.values.get(key)))
            .unwrap_or(false);
        let next = !effective;

        // Landing back on what the manager already has drops the override, so a
        // no-op is not stored as a change.
        if as_bool(self.settings_state.current_config.values.get(key)) == Some(next) {
            self.settings_state.adapter_config.values.remove(key);
        } else {
            self.settings_state
                .adapter_config
                .values
                .insert(key.to_string(), ConfigValue::Bool(next));
        }
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
        // A value that is actually set — an override typed this session or
        // read from the manager's config file — fills the input so it can be
        // edited; only a pure default (nothing concrete) starts empty.
        let value = self
            .settings_state
            .adapter_config
            .values
            .get(key)
            .or_else(|| self.settings_state.current_config.values.get(key));
        let initial = value
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
            scope: crate::views::settings::SettingsEditScope::Adapter,
            key: key.to_string(),
            label: label.to_string(),
            field_type,
            input,
        });
        self.pending_settings_edit_focus = true;
        cx.notify();
    }

    /// Open the shared settings editor for the registry URL, an Aeris-level
    /// value rather than a field on the active adapter.
    pub fn open_registry_url_edit(&mut self, cx: &mut Context<Self>) {
        let initial = self.settings_state.registry_url.clone();
        let input = cx.new(|cx| {
            let mut ti = crate::components::TextInput::new(cx, "URL or local path");
            ti.set_content(initial, cx);
            ti
        });
        self.settings_state.edit = Some(crate::views::settings::SettingsEdit {
            scope: crate::views::settings::SettingsEditScope::RegistryUrl,
            key: "registry_url".to_string(),
            label: "Registry URL".to_string(),
            field_type: crate::core::config::ConfigFieldType::Text,
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
        use crate::views::settings::SettingsEditScope;
        let edit = match self.settings_state.edit.take() {
            Some(e) => e,
            None => return,
        };

        // The registry URL is an Aeris-level string rather than an adapter
        // field, so it is written back to the Aeris settings rather than to
        // the adapter config. An empty value means "use the default".
        if edit.scope == SettingsEditScope::RegistryUrl {
            let trimmed = raw.trim().to_string();
            let changed = self.settings_state.registry_url != trimmed;
            self.settings_state.registry_url = trimmed;
            if changed {
                self.settings_state.aeris_dirty = true;
            }
            cx.notify();
            return;
        }

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
        // An empty value, or one that matches what the manager already has on
        // disk, means "no override": the key is dropped rather than stored, so
        // clearing the box or saving an unchanged value leaves the field as it is.
        let is_empty_text = matches!(&new_value, ConfigValue::String(s) if s.trim().is_empty());
        let matches_current = self
            .settings_state
            .current_config
            .values
            .get(&edit.key)
            .is_some_and(|current| current == &new_value);
        if is_empty_text || matches_current {
            self.settings_state.adapter_config.values.remove(&edit.key);
        } else {
            self.settings_state
                .adapter_config
                .values
                .insert(edit.key, new_value);
        }
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

        let package_key = installed.unique_key();
        let package_name = installed.package.name.clone();
        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                // Where a manager links what it installs is its own business,
                // so the package's own adapter is asked rather than assuming
                // every package went where the built-in one puts things.
                let paths = crate::tokio_spawn(async move { adapter.paths().await })
                    .await
                    .unwrap_or_else(|e| {
                        Err(crate::core::adapter::AdapterError::Other(format!("{e}")))
                    });

                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        let Some(bin_path) = paths.as_ref().ok().and_then(active_bin_path) else {
                            app.add_toast(
                                ToastLevel::Error,
                                format!(
                                    "{package_name} cannot be run: its manager did not say where it links commands"
                                ),
                            );
                            return;
                        };

                        let binaries = list_package_binaries(&install_path, &bin_path);
                        match binaries.len() {
                            0 => app.add_toast(
                                ToastLevel::Error,
                                format!(
                                    "No binaries from {} are exposed in {}",
                                    install_path.display(),
                                    bin_path.display()
                                ),
                            ),
                            1 => {
                                let path = binaries.into_iter().next().unwrap();
                                app.spawn_binary(&path, &package_key);
                            }
                            _ => {
                                app.run_picker = Some(RunPicker {
                                    package_name,
                                    binaries,
                                    package_key,
                                });
                            }
                        }
                        cx.notify();
                    })
                });
            },
        )
        .detach();
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
        // Saying so beats an empty space, which reads as something that
        // failed rather than something never offered.
        if !adapter.capabilities().has_package_detail {
            self.browse_state.selected_detail = None;
            self.browse_state.detail_loading = false;
            self.browse_state.detail_error = Some(format!(
                "{} does not report more than this",
                adapter.info().name
            ));
            return;
        }

        self.browse_state.detail_request_id = self.browse_state.detail_request_id.wrapping_add(1);
        let request_id = self.browse_state.detail_request_id;
        self.browse_state.selected_detail = None;
        self.browse_state.detail_loading = true;
        self.browse_state.detail_error = None;

        let package_id = pkg.id.clone();
        log::debug!("loading details for {package_id}");

        cx.spawn(
            async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let asked_for = package_id.clone();
                let result =
                    crate::tokio_spawn(async move { adapter.package_detail(&package_id).await })
                        .await
                        .unwrap_or_else(|e| {
                            Err(crate::core::adapter::AdapterError::Other(format!("{e}")))
                        });

                if let Err(ref e) = result {
                    log::warn!("could not load details for {asked_for}: {e}");
                }

                let updated = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        // Discard result if a newer request superseded this one
                        if request_id != app.browse_state.detail_request_id {
                            return;
                        }
                        app.browse_state.detail_loading = false;
                        match result {
                            Ok(detail) => {
                                log::debug!(
                                    "showing details for {asked_for}: type={:?} source={:?}",
                                    detail.pkg_type,
                                    detail.source
                                );
                                app.browse_state.selected_detail = Some(detail);
                            }
                            Err(e) => {
                                app.browse_state.detail_error = Some(format!("{e}"));
                            }
                        }
                        cx.notify();
                    })
                });

                if let Err(e) = updated {
                    log::warn!("could not show details for {asked_for}: {e}");
                }
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

        let mut had_events = false;

        // Drain the events adapters report as they work
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
                    self.record_progress(key, OperationStatus::Installing(readable(&phase)));
                }
                ProgressEvent::Completed {
                    adapter_id,
                    package_id,
                } => {
                    let key = progress_key(&adapter_id, &package_id);
                    self.record_progress(key, OperationStatus::Completed);
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

        if saw_event && !self.is_recent_self_write() {
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
                    .min_w_0()
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
                        div().flex().flex_row().justify_end().child(
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
    fn render_manifest_edit_modal(&mut self, theme: &theme::Theme, cx: &mut Context<Self>) -> Div {
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
            let mut col = div().flex().flex_col().gap(px(styles::spacing::SM)).child(
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

        let toggle_row =
            |label: &str,
             description: &str,
             on: bool,
             id: &str,
             listener: Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static>|
             -> Div {
                let track_on = primary;
                let track_off = border;
                let track = if on { track_on } else { track_off };
                let thumb = if on {
                    div()
                        .ml_auto()
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded_full()
                        .bg(gpui::white())
                } else {
                    div()
                        .w(px(16.0))
                        .h(px(16.0))
                        .rounded_full()
                        .bg(gpui::white())
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
            vec![labeled_row(
                "Repository",
                Some("Optional. Restricts the lookup to a specific repository."),
                repo_input,
                true,
            )],
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
                    Some(
                        "Shell commands separated by semicolons. Env: $INSTALL_DIR, $PKG_NAME, $PKG_VERSION, $NPROC.",
                    ),
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

        // Offered only where the manager says it can act system wide, which
        // needs privileges nothing here knows how to ask for otherwise.
        // Worth offering only when there is more than one way to work.
        if !(self.any_adapter_works_in(PackageMode::User)
            && self.any_adapter_works_in(PackageMode::System))
        {
            return header;
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

    /// Whether any adapter in use works in the given scope.
    pub(crate) fn any_adapter_works_in(&self, mode: PackageMode) -> bool {
        self.adapter_manager.list_adapters().iter().any(|info| {
            self.adapter_manager.is_enabled(&info.id)
                && match mode {
                    PackageMode::User => info.capabilities.supports_user_packages,
                    PackageMode::System => info.capabilities.supports_system_packages,
                }
        })
    }

    pub(crate) fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        let wanted = match self.current_mode {
            PackageMode::User => PackageMode::System,
            PackageMode::System => PackageMode::User,
        };

        // Nothing works that way, so there is nothing to switch to.
        if !self.any_adapter_works_in(wanted) {
            return;
        }

        self.current_mode = wanted;
        // Invalidate per-view caches so they reload for the new mode
        self.installed_state.loaded = false;
        self.installed_state.packages.clear();
        self.updates_state.checked = false;
        self.updates_state.updates.clear();
        self.updates_state.no_update_listing.clear();
        cx.notify();
    }

    fn render_content(&mut self, theme: &theme::Theme, cx: &mut Context<Self>) -> Div {
        let wrapper = div().flex_1().min_h_0().min_w_0().flex().flex_col();

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

    /// Record what a package's state became, everywhere browse shows it.
    ///
    /// The operation that ran is what knows this. A completion event only
    /// says the work finished, not which way it left the package.
    fn mark_installed(&mut self, adapter_id: &str, package_id: &str, installed: bool) {
        for package in &mut self.browse_state.search_results {
            if package.id == package_id && package.adapter_id == adapter_id {
                package.installed = installed;
            }
        }

        if let Some(selected) = &mut self.browse_state.selected_package
            && selected.id == package_id
            && selected.adapter_id == adapter_id
        {
            selected.installed = installed;
        }

        let key = crate::core::adapter::progress_key(adapter_id, package_id);
        self.browse_state.package_progress.remove(&key);
    }

    pub(crate) fn install_package(
        &mut self,
        pkg: crate::core::package::Package,
        mode: PackageMode,
        cx: &mut Context<Self>,
    ) {
        let package_id = pkg.id.clone();
        let pkg_name = pkg.name.clone();
        let adapter_id = pkg.adapter_id.clone();
        let progress_key = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
        self.browse_state.installing = Some(package_id.clone());
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
                                    app.mark_installed(&adapter_id, &package_id, true);
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
        let package_id = pkg.id.clone();
        let adapter_id = pkg.adapter_id.clone();
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
                                    app.mark_installed(&adapter_id, &package_id, false);
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

#[cfg(test)]
mod tests {
    // Deliberately not glob importing the parent: it pulls in gpui's prelude,
    // which shadows the test attribute.
    use super::readable;

    #[test]
    fn results_are_ordered_by_how_well_they_answer_the_query() {
        use super::rank_results;
        use crate::core::package::Package;

        let pkg = |name: &str, adapter: &str| Package {
            id: name.to_string(),
            name: name.to_string(),
            version: "1.0".into(),
            adapter_id: adapter.to_string(),
            description: None,
            size: None,
            homepage: None,
            license: None,
            installed: false,
            update_available: false,
            category: None,
            tags: Vec::new(),
            icon_url: None,
        };

        // As they arrive: one manager's answers, then the next one's.
        let mut results = vec![
            pkg("ripgrep-all", "soar"),
            pkg("fd", "soar"),
            pkg("the-fd-thing", "soar"),
            pkg("fd", "pacstall"),
            pkg("fd-find", "pacstall"),
            pkg("unrelated", "am"),
        ];

        rank_results(&mut results, "fd");

        let order: Vec<(&str, &str)> = results
            .iter()
            .map(|p| (p.name.as_str(), p.adapter_id.as_str()))
            .collect();

        assert_eq!(
            order,
            vec![
                // Exact, and the two managers holding it sit together.
                ("fd", "pacstall"),
                ("fd", "soar"),
                // Starts with what was typed.
                ("fd-find", "pacstall"),
                // Merely contains it.
                ("the-fd-thing", "soar"),
                // Matched on something other than the name, where the same
                // shorter-first rule applies for want of anything better.
                ("unrelated", "am"),
                ("ripgrep-all", "soar"),
            ]
        );
    }

    #[test]
    fn a_name_answers_an_abbreviation_when_its_letters_are_in_order() {
        use super::subsequence_score;

        // The letters are there in order, so this is the whole point.
        assert!(subsequence_score("ripgrep", "rgrep").is_some());
        // And here they are not: there is no second `g`.
        assert!(subsequence_score("grep", "rgrep").is_none());
        assert!(subsequence_score("ripgrep", "").is_none());
        assert!(subsequence_score("fd", "ripgrep").is_none());

        // Of two names holding the query, the tighter one scores higher.
        let tight = subsequence_score("ripgrep", "rgrep").unwrap();
        let loose = subsequence_score("ripgrep-all", "rgrep").unwrap();
        assert!(tight > loose, "{tight} should beat {loose}");

        // Starting a word beats landing in the middle of one.
        let boundary = subsequence_score("fd-find", "ff").unwrap();
        let middle = subsequence_score("fdxfind", "ff").unwrap();
        assert!(boundary > middle, "{boundary} should beat {middle}");
    }

    #[test]
    fn an_abbreviation_outranks_a_name_that_does_not_hold_it() {
        use super::rank_results;
        use crate::core::package::Package;

        let pkg = |name: &str| Package {
            id: name.to_string(),
            name: name.to_string(),
            version: "1.0".into(),
            adapter_id: "soar".into(),
            description: None,
            size: None,
            homepage: None,
            license: None,
            installed: false,
            update_available: false,
            category: None,
            tags: Vec::new(),
            icon_url: None,
        };

        let mut results = vec![pkg("unrelated"), pkg("ripgrep-all"), pkg("ripgrep")];
        rank_results(&mut results, "rgrep");

        let order: Vec<&str> = results.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(order, vec!["ripgrep", "ripgrep-all", "unrelated"]);
    }

    #[test]
    fn a_scope_accounts_for_the_managers_it_leaves_out() {
        use super::scope_note;
        use crate::core::privilege::PackageMode;

        assert_eq!(scope_note(&[], PackageMode::User), None);

        assert_eq!(
            scope_note(&["Pacstall".to_string()], PackageMode::User).as_deref(),
            Some("Pacstall only works in System mode, so its packages are listed there.")
        );

        assert_eq!(
            scope_note(
                &["Pacstall".to_string(), "Apt".to_string()],
                PackageMode::User
            )
            .as_deref(),
            Some("Pacstall and Apt only work in System mode, so their packages are listed there.")
        );

        // The other way round names the other scope, since what is left out
        // there is a manager that only works for one person.
        assert_eq!(
            scope_note(&["AppMan".to_string()], PackageMode::System).as_deref(),
            Some("AppMan only works in User mode, so its packages are listed there.")
        );
    }

    #[test]
    fn work_that_ended_is_not_work_in_flight() {
        use super::OperationStatus;

        assert!(OperationStatus::Completed.is_finished());
        assert!(OperationStatus::Failed("nope".into()).is_finished());
        assert!(!OperationStatus::Starting.is_finished());
        assert!(
            !OperationStatus::Downloading {
                current: 1,
                total: 2
            }
            .is_finished()
        );
        assert!(!OperationStatus::Installing("extracting".into()).is_finished());
    }

    #[test]
    fn a_status_says_less_where_there_is_less_room() {
        use super::OperationStatus;

        let downloading = OperationStatus::Downloading {
            current: 82_100_000,
            total: 341_300_000,
        };
        assert_eq!(downloading.short_label(), "Downloading 24%");
        assert!(downloading.label().contains("MB"));
    }

    #[test]
    fn a_stage_is_shown_the_way_a_person_would_write_it() {
        assert_eq!(readable("linking_binaries"), "Linking binaries");
        assert_eq!(readable("extracting"), "Extracting");
        assert_eq!(readable(""), "");
    }
}
