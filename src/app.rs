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

use crate::{adapters::soar::SoarAdapter, core::adapter::Adapter, views};

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
}

impl std::fmt::Display for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            View::Dashboard => write!(f, "Dashboard"),
            View::Browse => write!(f, "Browse"),
            View::Installed => write!(f, "Installed"),
            View::Updates => write!(f, "Updates"),
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
    adapter: Arc<SoarAdapter>,
    confirm_dialog: Option<message::ConfirmAction>,
    event_receiver: std::sync::mpsc::Receiver<SoarEvent>,
    active_operation: Option<ActiveOperation>,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        let config = soar_config::config::get_config();
        let (adapter, event_receiver) =
            SoarAdapter::new(config).expect("Failed to initialize Soar adapter");
        let adapter = Arc::new(adapter);

        let load_adapter = adapter.clone();
        let init_task = Task::perform(
            async move {
                load_adapter
                    .list_installed()
                    .await
                    .map_err(|e| e.to_string())
            },
            |result| Message::Installed(message::InstalledMessage::PackagesLoaded(result)),
        );

        (
            Self {
                selected_theme: AppTheme::default(),
                current_view: View::default(),
                browse: views::browse::BrowseState::default(),
                installed: views::installed::InstalledState {
                    loading: true,
                    ..Default::default()
                },
                updates: views::updates::UpdatesState::default(),
                adapter,
                confirm_dialog: None,
                event_receiver,
                active_operation: None,
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
                    _ => Task::none(),
                };
            }
            Message::ThemeChanged(theme) => {
                self.selected_theme = theme;
            }
            Message::Browse(msg) => return self.update_browse(msg),
            Message::Installed(msg) => return self.update_installed(msg),
            Message::Updates(msg) => return self.update_updates(msg),
            Message::Adapters(_msg) => {}
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
            message::BrowseMessage::InstallPackage(pkg) => {
                self.confirm_dialog = Some(message::ConfirmAction::Install(pkg));
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
                        self.browse.error = Some(e);
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn load_installed(&mut self) -> Task<Message> {
        self.installed.loading = true;
        let adapter = self.adapter.clone();
        Task::perform(
            async move { adapter.list_installed().await.map_err(|e| e.to_string()) },
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
                return Task::perform(
                    async move { adapter.list_updates().await.map_err(|e| e.to_string()) },
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

    fn execute_confirmed(&mut self, action: message::ConfirmAction) -> Task<Message> {
        match action {
            message::ConfirmAction::Install(ref pkg) => {
                if let Some(query) = pkg.soar_query() {
                    self.active_operation = Some(ActiveOperation {
                        operation_type: OperationType::Install,
                        package_name: pkg.name.clone(),
                        status: OperationStatus::Starting,
                    });
                    self.browse.installing = Some(pkg.id.clone());
                    self.browse.result_version += 1;
                    let adapter = self.adapter.clone();
                    return Task::perform(
                        async move {
                            adapter
                                .install_package(&query)
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
        } else {
            base.into()
        }
    }

    fn confirm_dialog_view(&self, action: &message::ConfirmAction) -> Element<'_, Message> {
        let (title, description) = match action {
            message::ConfirmAction::Install(pkg) => {
                ("Install Package", format!("{} {}", pkg.name, pkg.version))
            }
            message::ConfirmAction::Remove(pkg) => {
                ("Remove Package", format!("{} {}", pkg.name, pkg.version))
            }
            message::ConfirmAction::Update(pkg) => {
                ("Update Package", format!("{} {}", pkg.name, pkg.version))
            }
            message::ConfirmAction::UpdateAll => (
                "Update All",
                "All packages with available updates will be updated.".to_string(),
            ),
        };

        let is_destructive = matches!(action, message::ConfirmAction::Remove(_));

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

        container(
            column![
                text(title).size(18),
                text(description).size(14),
                row![cancel_btn, confirm_btn].spacing(8),
            ]
            .spacing(16)
            .padding(24)
            .align_x(iced::Alignment::Center),
        )
        .style(container::rounded_box)
        .width(320)
        .into()
    }

    fn sidebar_view(&self) -> Element<'_, Message> {
        let nav_items = [
            (View::Dashboard, "Dashboard"),
            (View::Browse, "Browse"),
            (View::Installed, "Installed"),
            (View::Updates, "Updates"),
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

        let theme_selector = column![
            text("Theme").size(12),
            iced::widget::pick_list(
                &AppTheme::ALL[..],
                Some(self.selected_theme),
                Message::ThemeChanged,
            )
            .width(Length::Fill),
        ]
        .spacing(4)
        .padding(8);

        container(
            column![
                text(APP_NAME).size(20).center().width(Length::Fill),
                rule::horizontal(1),
                nav,
                space(),
                rule::horizontal(1),
                theme_selector,
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
