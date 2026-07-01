use std::path::PathBuf;

use gpui::*;

use crate::{app::App, components::TextInput, styles, theme};

#[derive(Debug, Clone, Default)]
pub struct ManifestEntry {
    pub name: String,
    pub pkg_id: Option<String>,
    pub current_version: Option<String>,
    pub new_version: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ManifestDiff {
    pub to_install: Vec<ManifestEntry>,
    pub to_update: Vec<ManifestEntry>,
    pub to_remove: Vec<ManifestEntry>,
    pub in_sync: Vec<String>,
    pub not_found: Vec<String>,
    /// Map of package name to the name of the missing profile it references.
    pub invalid_profiles: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ManifestApplyReport {
    pub installed: usize,
    pub updated: usize,
    pub removed: usize,
    pub failed: usize,
}

/// Flat snapshot of an editable manifest entry. Strings use "" for unset to
/// keep the modal form bindings simple. The adapter decides Simple vs
/// Detailed when serializing back.
#[derive(Debug, Clone, Default)]
pub struct ManifestEntrySnapshot {
    pub name: String,
    pub version: String,
    pub pkg_id: String,
    pub repo: String,
    pub url: String,
    pub github: String,
    pub gitlab: String,
    pub asset_pattern: String,
    pub tag_pattern: String,
    pub include_prerelease: bool,
    pub build_commands: String,
    pub build_dependencies: String,
    pub install_patterns: String,
    pub profile: String,
    pub pinned: bool,
    pub binary_only: bool,
}

impl ManifestEntrySnapshot {
    /// Whether this entry has any field set beyond a bare version. Used to
    /// decide whether to write Simple or Detailed form.
    pub fn needs_detailed(&self) -> bool {
        !self.pkg_id.is_empty()
            || !self.repo.is_empty()
            || !self.url.is_empty()
            || !self.github.is_empty()
            || !self.gitlab.is_empty()
            || !self.asset_pattern.is_empty()
            || !self.tag_pattern.is_empty()
            || self.include_prerelease
            || !self.build_commands.is_empty()
            || !self.build_dependencies.is_empty()
            || !self.install_patterns.is_empty()
            || !self.profile.is_empty()
            || self.pinned
            || self.binary_only
    }
}

#[derive(Debug)]
pub enum ManifestStatus {
    Idle,
    Loading,
    FileMissing,
    ParseError(String),
    Loaded(ManifestDiff),
    Failed(String),
}

impl Default for ManifestStatus {
    fn default() -> Self {
        Self::Idle
    }
}

#[derive(Debug)]
pub enum ManifestEditKind {
    Add,
    Edit(String),
}

pub struct ManifestEditModal {
    pub kind: ManifestEditKind,
    pub name_input: Entity<TextInput>,
    pub version_input: Entity<TextInput>,
    pub pkg_id_input: Entity<TextInput>,
    pub repo_input: Entity<TextInput>,
    pub url_input: Entity<TextInput>,
    pub github_input: Entity<TextInput>,
    pub gitlab_input: Entity<TextInput>,
    pub asset_pattern_input: Entity<TextInput>,
    pub tag_pattern_input: Entity<TextInput>,
    pub build_commands_input: Entity<TextInput>,
    pub build_dependencies_input: Entity<TextInput>,
    pub install_patterns_input: Entity<TextInput>,
    pub profile_input: Entity<TextInput>,
    pub include_prerelease: bool,
    pub pinned: bool,
    pub binary_only: bool,
}

#[derive(Default)]
pub struct ManifestState {
    pub path: Option<PathBuf>,
    pub status: ManifestStatus,
    pub prune: bool,
    pub applying: bool,
    pub last_report: Option<ManifestApplyReport>,
    pub apply_error: Option<String>,
    pub save_error: Option<String>,
    pub edit: Option<ManifestEditModal>,
    pub pending_edit_focus: bool,
    pub selected_entry: Option<String>,
    pub selected_snapshot: Option<ManifestEntrySnapshot>,
}

impl App {
    pub fn render_manifest(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if matches!(self.manifest_state.status, ManifestStatus::Idle) {
            self.load_manifest_diff(cx);
        }

        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let hover = theme.hover;
        let danger = theme.danger;
        let warning = theme.warning;
        let success = theme.success;

        let path_label = self
            .manifest_state
            .path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.config/soar/packages.toml".to_string());

        let title_block = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(
                div()
                    .text_size(px(styles::font_size::TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Manifest"),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(text_muted)
                    .child("Reconciles your soar manifest with what is installed. For new upstream releases, see the Updates view."),
            );

        let is_loading = matches!(self.manifest_state.status, ManifestStatus::Loading);
        let applying = self.manifest_state.applying;
        let has_changes = matches!(
            &self.manifest_state.status,
            ManifestStatus::Loaded(diff) if !diff.to_install.is_empty()
                || !diff.to_update.is_empty()
                || !diff.to_remove.is_empty()
        );
        let apply_enabled = has_changes && !applying && !is_loading;

        let reload_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.load_manifest_diff(cx);
        });
        let apply_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.request_manifest_apply(cx);
        });
        let add_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.open_manifest_add(cx);
        });
        let import_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.confirm_dialog = Some(crate::app::ConfirmAction::ImportInstalledManifest);
            cx.notify();
        });
        let import_listener_empty = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.confirm_dialog = Some(crate::app::ConfirmAction::ImportInstalledManifest);
            cx.notify();
        });
        let create_empty_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.create_empty_manifest(cx);
        });

        let reload_btn = div()
            .id("manifest-reload")
            .px(px(14.0))
            .py(px(styles::spacing::XS))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .text_size(px(styles::font_size::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .hover(move |s| s.bg(hover))
            .on_click(reload_listener)
            .child(if is_loading { "Reloading…" } else { "Reload" });

        let apply_label = if applying {
            "Applying…"
        } else {
            "Apply"
        };
        let apply_bg = if apply_enabled { primary } else { surface };
        let apply_fg = if apply_enabled {
            gpui::white()
        } else {
            text_muted
        };
        let apply_border_color = if apply_enabled { primary } else { border };
        let apply_hover_bg = if apply_enabled {
            primary.opacity(0.85)
        } else {
            surface
        };

        let mut apply_btn = div()
            .id("manifest-apply")
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::XS))
            .rounded(px(styles::radius::MD))
            .bg(apply_bg)
            .text_color(apply_fg)
            .border_1()
            .border_color(apply_border_color)
            .text_size(px(styles::font_size::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .child(apply_label.to_string());
        if apply_enabled {
            apply_btn = apply_btn
                .cursor_pointer()
                .hover(move |s| s.bg(apply_hover_bg))
                .on_click(apply_listener);
        }

        let add_btn = div()
            .id("manifest-add")
            .px(px(14.0))
            .py(px(styles::spacing::XS))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .text_size(px(styles::font_size::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .hover(move |s| s.bg(hover))
            .on_click(add_listener)
            .child("Add package");

        let import_btn = div()
            .id("manifest-import")
            .px(px(14.0))
            .py(px(styles::spacing::XS))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .text_size(px(styles::font_size::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .hover(move |s| s.bg(hover))
            .on_click(import_listener)
            .child("Import installed");

        let header_row = div()
            .flex()
            .flex_row()
            .items_start()
            .justify_between()
            .w_full()
            .child(title_block)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::SM))
                    .child(reload_btn)
                    .child(add_btn)
                    .child(import_btn)
                    .child(apply_btn),
            );

        let prune_state = self.manifest_state.prune;
        let prune_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.manifest_state.prune = !app.manifest_state.prune;
            cx.notify();
        });
        let prune_switch = switch_pill(prune_state, primary, border, Box::new(prune_listener));

        let path_card = div()
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::MD))
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::SM))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(styles::spacing::XXXS))
                            .child(
                                div()
                                    .text_size(px(styles::font_size::CAPTION))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(text_muted)
                                    .child("STATE FILE"),
                            )
                            .child(
                                div()
                                    .text_size(px(styles::font_size::SMALL))
                                    .child(path_label),
                            ),
                    ),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(border)
                    .pt(px(styles::spacing::SM))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(styles::spacing::XXXS))
                            .child(
                                div()
                                    .text_size(px(styles::font_size::BODY))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Prune"),
                            )
                            .child(
                                div()
                                    .text_size(px(styles::font_size::CAPTION))
                                    .text_color(text_muted)
                                    .child(
                                        "When on, Apply removes undeclared packages from the system.",
                                    ),
                            ),
                    )
                    .child(prune_switch),
            );

        let report_card: Option<Div> = self.manifest_state.last_report.map(|report| {
            let color = if report.failed > 0 { danger } else { success };
            let summary = if report.failed > 0 {
                format!(
                    "Last apply: {} installed, {} updated, {} removed, {} failed",
                    report.installed, report.updated, report.removed, report.failed
                )
            } else {
                format!(
                    "Last apply: {} installed, {} updated, {} removed",
                    report.installed, report.updated, report.removed
                )
            };
            div()
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::SM))
                .rounded(px(styles::radius::MD))
                .bg(color.opacity(0.12))
                .border_1()
                .border_color(color.opacity(0.3))
                .text_size(px(styles::font_size::SMALL))
                .text_color(color)
                .child(summary)
        });

        let apply_error_card: Option<Div> =
            self.manifest_state.apply_error.as_ref().map(|err| {
                div()
                    .px(px(styles::spacing::LG))
                    .py(px(styles::spacing::SM))
                    .rounded(px(styles::radius::MD))
                    .bg(danger.opacity(0.12))
                    .border_1()
                    .border_color(danger.opacity(0.3))
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(danger)
                    .child(format!("Apply failed: {err}"))
            });

        let body: AnyElement = match &self.manifest_state.status {
            ManifestStatus::Idle | ManifestStatus::Loading => empty_panel(
                if is_loading {
                    "Reading manifest…"
                } else {
                    "Loading manifest…"
                },
                None,
                text_muted,
            )
            .into_any_element(),
            ManifestStatus::FileMissing => missing_file_panel(
                theme,
                primary,
                Box::new(create_empty_listener),
                Box::new(import_listener_empty),
            )
            .into_any_element(),
            ManifestStatus::ParseError(err) => banner_panel(
                "Manifest parse error",
                err.as_str(),
                danger,
                border,
            )
            .into_any_element(),
            ManifestStatus::Failed(err) => banner_panel(
                "Failed to load manifest",
                err.as_str(),
                danger,
                border,
            )
            .into_any_element(),
            ManifestStatus::Loaded(diff) => {
                let diff = diff.clone();
                render_diff_sections(diff, theme, primary, warning, success, danger, cx)
                    .into_any_element()
            }
        };

        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::LG))
            .w_full()
            .child(header_row)
            .child(path_card);
        if let Some(card) = report_card {
            content = content.child(card);
        }
        if let Some(card) = apply_error_card {
            content = content.child(card);
        }
        content = content.child(body);

        let detail = self.manifest_state.selected_snapshot.clone().map(|snap| {
            let close = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.clear_manifest_selection(cx);
            });
            render_manifest_detail(snap, theme, Box::new(close))
        });

        let scroll = div()
            .id("manifest-scroll")
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .child(
                div()
                    .p(px(styles::spacing::XL))
                    .flex()
                    .flex_col()
                    .w_full()
                    .min_w_0()
                    .child(content),
            );

        let mut outer = div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(scroll);
        if let Some(panel) = detail {
            outer = outer.child(panel);
        }
        outer
    }
}

fn render_diff_sections(
    diff: ManifestDiff,
    theme: &theme::Theme,
    primary: Hsla,
    warning: Hsla,
    success: Hsla,
    danger: Hsla,
    cx: &mut Context<App>,
) -> Div {
    let mut col = div()
        .flex()
        .flex_col()
        .gap(px(styles::spacing::MD))
        .w_full();

    let invalid_profiles = &diff.invalid_profiles;
    let mut summary = div()
        .flex()
        .flex_row()
        .gap(px(styles::spacing::SM))
        .flex_wrap()
        .child(summary_chip(
            "missing",
            diff.to_install.len(),
            primary,
            theme,
        ))
        .child(summary_chip(
            "drifted",
            diff.to_update.len(),
            warning,
            theme,
        ))
        .child(summary_chip("undeclared", diff.to_remove.len(), danger, theme))
        .child(summary_chip("in sync", diff.in_sync.len(), success, theme))
        .child(summary_chip(
            "unresolved",
            diff.not_found.len(),
            theme.text_muted,
            theme,
        ));
    if !invalid_profiles.is_empty() {
        summary = summary.child(summary_chip(
            "missing profile",
            invalid_profiles.len(),
            warning,
            theme,
        ));
    }
    col = col.child(summary);

    col = col.child(diff_section(
        "Declared but not installed",
        primary,
        &diff.to_install,
        DiffKind::Install,
        theme,
        invalid_profiles,
        cx,
    ));
    col = col.child(diff_section(
        "Installed version drifted from manifest",
        warning,
        &diff.to_update,
        DiffKind::Update,
        theme,
        invalid_profiles,
        cx,
    ));
    if !diff.to_remove.is_empty() {
        col = col.child(diff_section(
            "Installed but not declared",
            danger,
            &diff.to_remove,
            DiffKind::Remove,
            theme,
            invalid_profiles,
            cx,
        ));
    }
    col = col.child(in_sync_section(
        success,
        &diff.in_sync,
        theme,
        invalid_profiles,
        cx,
    ));
    if !diff.not_found.is_empty() {
        col = col.child(not_found_section(
            theme.text_muted,
            &diff.not_found,
            theme,
            invalid_profiles,
            cx,
        ));
    }

    col
}

#[derive(Copy, Clone)]
enum DiffKind {
    Install,
    Update,
    Remove,
}

fn diff_section(
    title: &str,
    accent: Hsla,
    entries: &[ManifestEntry],
    kind: DiffKind,
    theme: &theme::Theme,
    invalid_profiles: &std::collections::HashMap<String, String>,
    cx: &mut Context<App>,
) -> Div {
    let surface = theme.surface;
    let border = theme.border;
    let text_muted = theme.text_muted;

    let mut card = div()
        .rounded(px(styles::radius::LG))
        .bg(surface)
        .border_1()
        .border_color(border)
        .w_full()
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::MD))
                .border_b_1()
                .border_color(border)
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(styles::font_size::HEADING))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(accent)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .text_color(text_muted)
                        .child(format!("{}", entries.len())),
                ),
        );

    if entries.is_empty() {
        card = card.child(
            div()
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::MD))
                .text_size(px(styles::font_size::SMALL))
                .text_color(text_muted)
                .child("Nothing to do here."),
        );
        return card;
    }

    let count = entries.len();
    let mut body = div().flex().flex_col().w_full();
    for (i, entry) in entries.iter().enumerate() {
        let missing_profile = invalid_profiles.get(&entry.name).cloned();
        let row = entry_row(entry, kind, accent, theme, i, missing_profile, cx);
        let mut row = row.py(px(styles::spacing::SM));
        if i + 1 < count {
            row = row.border_b_1().border_color(border);
        }
        body = body.child(row);
    }
    card.child(
        div()
            .w_full()
            .px(px(styles::spacing::LG))
            .child(body),
    )
}

fn entry_row(
    entry: &ManifestEntry,
    kind: DiffKind,
    accent: Hsla,
    theme: &theme::Theme,
    idx: usize,
    missing_profile: Option<String>,
    cx: &mut Context<App>,
) -> Stateful<Div> {
    let surface = theme.surface;
    let border = theme.border;
    let text_muted = theme.text_muted;
    let hover = theme.hover;
    let danger = theme.danger;
    let warning = theme.warning;

    let kind_prefix = match kind {
        DiffKind::Install => "install",
        DiffKind::Update => "update",
        DiffKind::Remove => "remove",
    };

    let mut row = div()
        .flex()
        .flex_row()
        .w_full()
        .min_w_0()
        .gap(px(styles::spacing::SM))
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .text_size(px(styles::font_size::BODY))
                .font_weight(FontWeight::MEDIUM)
                .child(entry.name.clone()),
        );

    if let Some(ref missing) = missing_profile {
        row = row.child(chip(
            &format!("profile {missing} missing"),
            warning.opacity(0.2),
            warning.opacity(0.4),
            warning,
        ));
    }

    match kind {
        DiffKind::Install => {
            if let Some(ver) = entry.new_version.as_ref().or(entry.current_version.as_ref()) {
                row = row.child(chip(ver, accent.opacity(0.2), accent.opacity(0.4), accent));
            } else {
                row = row.child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .text_color(text_muted)
                        .child("latest"),
                );
            }
        }
        DiffKind::Update => {
            if let Some(ref cur) = entry.current_version {
                row = row.child(chip(cur, surface, border, text_muted));
                row = row.child(
                    div()
                        .text_size(px(styles::font_size::SMALL))
                        .text_color(text_muted)
                        .child("\u{2192}"),
                );
            }
            if let Some(ref new_v) = entry.new_version {
                row = row.child(chip(
                    new_v,
                    accent.opacity(0.2),
                    accent.opacity(0.4),
                    accent,
                ));
            }
        }
        DiffKind::Remove => {
            if let Some(ref cur) = entry.current_version {
                row = row.child(chip(cur, surface, border, text_muted));
            }
        }
    }

    if matches!(kind, DiffKind::Install | DiffKind::Update) {
        let edit_name = entry.name.clone();
        let edit_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            app.open_manifest_edit(edit_name.clone(), cx);
        });
        let remove_name = entry.name.clone();
        let remove_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            app.confirm_dialog = Some(crate::app::ConfirmAction::RemoveManifestEntry {
                name: remove_name.clone(),
            });
            cx.notify();
        });
        row = row.child(
            div()
                .id(SharedString::from(format!("manifest-edit-{kind_prefix}-{idx}")))
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(surface)
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_size(px(styles::font_size::CAPTION))
                .hover(move |s| s.bg(hover))
                .on_click(edit_listener)
                .child("Edit"),
        );
        row = row.child(
            div()
                .id(SharedString::from(format!(
                    "manifest-remove-{kind_prefix}-{idx}"
                )))
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(danger.opacity(0.12))
                .border_1()
                .border_color(danger.opacity(0.3))
                .text_color(danger)
                .cursor_pointer()
                .text_size(px(styles::font_size::CAPTION))
                .hover(move |s| s.bg(danger.opacity(0.2)))
                .on_click(remove_listener)
                .child("Remove"),
        );
    }

    let select_name = entry.name.clone();
    let select_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
        app.select_manifest_entry(select_name.clone(), cx);
    });
    let _ = text_muted;
    div()
        .id(SharedString::from(format!("manifest-row-{kind_prefix}-{idx}")))
        .w_full()
        .min_w_0()
        .cursor_pointer()
        .on_click(select_listener)
        .child(row)
}

fn in_sync_section(
    accent: Hsla,
    names: &[String],
    theme: &theme::Theme,
    invalid_profiles: &std::collections::HashMap<String, String>,
    cx: &mut Context<App>,
) -> Div {
    name_section_with_actions(
        "In sync",
        accent,
        names,
        theme,
        true,
        "sync",
        invalid_profiles,
        cx,
    )
}

fn not_found_section(
    accent: Hsla,
    names: &[String],
    theme: &theme::Theme,
    invalid_profiles: &std::collections::HashMap<String, String>,
    cx: &mut Context<App>,
) -> Div {
    name_section_with_actions(
        "Unresolved by any repo",
        accent,
        names,
        theme,
        false,
        "nf",
        invalid_profiles,
        cx,
    )
}

fn name_section_with_actions(
    title: &str,
    accent: Hsla,
    names: &[String],
    theme: &theme::Theme,
    show_edit: bool,
    id_prefix: &'static str,
    invalid_profiles: &std::collections::HashMap<String, String>,
    cx: &mut Context<App>,
) -> Div {
    let surface = theme.surface;
    let border = theme.border;
    let text_muted = theme.text_muted;
    let hover = theme.hover;
    let danger = theme.danger;

    let mut card = div()
        .rounded(px(styles::radius::LG))
        .bg(surface)
        .border_1()
        .border_color(border)
        .w_full()
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::MD))
                .border_b_1()
                .border_color(border)
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(styles::font_size::HEADING))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(accent)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .text_color(text_muted)
                        .child(format!("{}", names.len())),
                ),
        );

    if names.is_empty() {
        card = card.child(
            div()
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::MD))
                .text_size(px(styles::font_size::SMALL))
                .text_color(text_muted)
                .child("Nothing here."),
        );
        return card;
    }

    let warning = theme.warning;
    let count = names.len();
    let mut body = div().flex().flex_col().w_full();
    for (i, name) in names.iter().enumerate() {
        // Soar suffixes in_sync entries with " (local)" or similar source labels.
        // The manifest key is just the bare name, so strip the suffix before
        // routing edit and remove actions back through the adapter.
        let base_name = name
            .split_once(' ')
            .map(|(n, _)| n.to_string())
            .unwrap_or_else(|| name.clone());
        let edit_name = base_name.clone();
        let remove_name = base_name.clone();
        let edit_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            app.open_manifest_edit(edit_name.clone(), cx);
        });
        let remove_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            cx.stop_propagation();
            app.confirm_dialog = Some(crate::app::ConfirmAction::RemoveManifestEntry {
                name: remove_name.clone(),
            });
            cx.notify();
        });
        let missing_profile = invalid_profiles.get(&base_name).cloned();
        let mut row = div()
            .flex()
            .flex_row()
            .w_full()
            .min_w_0()
            .items_center()
            .gap(px(styles::spacing::SM))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_size(px(styles::font_size::BODY))
                    .child(name.clone()),
            );
        if let Some(ref missing) = missing_profile {
            row = row.child(chip(
                &format!("profile {missing} missing"),
                warning.opacity(0.2),
                warning.opacity(0.4),
                warning,
            ));
        }
        if show_edit {
            row = row.child(
                div()
                    .id(SharedString::from(format!("manifest-edit-{id_prefix}-{i}")))
                    .px(px(styles::spacing::SM))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::SM))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_size(px(styles::font_size::CAPTION))
                    .hover(move |s| s.bg(hover))
                    .on_click(edit_listener)
                    .child("Edit"),
            );
        }
        row = row.child(
            div()
                .id(SharedString::from(format!(
                    "manifest-remove-{id_prefix}-{i}"
                )))
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(danger.opacity(0.12))
                .border_1()
                .border_color(danger.opacity(0.3))
                .text_color(danger)
                .cursor_pointer()
                .text_size(px(styles::font_size::CAPTION))
                .hover(move |s| s.bg(danger.opacity(0.2)))
                .on_click(remove_listener)
                .child("Remove"),
        );
        let select_name = base_name.clone();
        let select_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            app.select_manifest_entry(select_name.clone(), cx);
        });
        let mut clickable = div()
            .id(SharedString::from(format!("manifest-row-{id_prefix}-{i}")))
            .w_full()
            .py(px(styles::spacing::SM))
            .cursor_pointer()
            .on_click(select_listener)
            .child(row);
        if i + 1 < count {
            clickable = clickable.border_b_1().border_color(border);
        }
        body = body.child(clickable);
    }
    card.child(
        div()
            .w_full()
            .px(px(styles::spacing::LG))
            .child(body),
    )
}

fn summary_chip(label: &str, count: usize, color: Hsla, theme: &theme::Theme) -> Div {
    div()
        .px(px(styles::spacing::MD))
        .py(px(styles::spacing::XXS))
        .rounded(px(styles::radius::MD))
        .bg(color.opacity(0.15))
        .border_1()
        .border_color(color.opacity(0.3))
        .flex()
        .flex_row()
        .gap(px(styles::spacing::XS))
        .items_center()
        .child(
            div()
                .text_size(px(styles::font_size::BODY))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(format!("{count}")),
        )
        .child(
            div()
                .text_size(px(styles::font_size::CAPTION))
                .text_color(theme.text_muted)
                .child(label.to_string()),
        )
}

fn render_manifest_detail(
    snap: ManifestEntrySnapshot,
    theme: &theme::Theme,
    close: Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static>,
) -> Stateful<Div> {
    let surface = theme.surface;
    let border = theme.border;
    let text_muted = theme.text_muted;
    let hover = theme.hover;

    let field = |label: &str, value: String| -> Option<Div> {
        if value.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::XXXS))
                .child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text_muted)
                        .child(label.to_uppercase()),
                )
                .child(div().text_size(px(styles::font_size::SMALL)).child(value)),
        )
    };

    let bool_field = |label: &str, on: bool| -> Option<Div> {
        if !on {
            return None;
        }
        Some(
            div()
                .text_size(px(styles::font_size::SMALL))
                .child(format!("{label}: on")),
        )
    };

    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(styles::spacing::MD))
        .w_full()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(styles::font_size::HEADING))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(snap.name.clone()),
                )
                .child(
                    div()
                        .id("manifest-detail-close")
                        .px(px(styles::spacing::SM))
                        .py(px(styles::spacing::XXS))
                        .rounded(px(styles::radius::SM))
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .cursor_pointer()
                        .text_size(px(styles::font_size::CAPTION))
                        .hover(move |s| s.bg(hover))
                        .on_click(close)
                        .child("Close"),
                ),
        );

    let version_display = if snap.version.is_empty() {
        "*".to_string()
    } else {
        snap.version.clone()
    };
    if let Some(f) = field("Version", version_display) {
        body = body.child(f);
    }
    for (label, value) in [
        ("Package ID", snap.pkg_id.clone()),
        ("Repository", snap.repo.clone()),
        ("URL", snap.url.clone()),
        ("GitHub", snap.github.clone()),
        ("GitLab", snap.gitlab.clone()),
        ("Asset pattern", snap.asset_pattern.clone()),
        ("Tag pattern", snap.tag_pattern.clone()),
        ("Profile", snap.profile.clone()),
        ("Install patterns", snap.install_patterns.clone()),
        ("Build commands", snap.build_commands.clone()),
        ("Build dependencies", snap.build_dependencies.clone()),
    ] {
        if let Some(f) = field(label, value) {
            body = body.child(f);
        }
    }

    let flags: Vec<Div> = [
        bool_field("Pinned", snap.pinned),
        bool_field("Include prereleases", snap.include_prerelease),
        bool_field("Binary only", snap.binary_only),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !flags.is_empty() {
        let mut flags_block = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXXS))
            .child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text_muted)
                    .child("FLAGS"),
            );
        for f in flags {
            flags_block = flags_block.child(f);
        }
        body = body.child(flags_block);
    }

    div()
        .id("manifest-detail")
        .flex_shrink()
        .w(px(360.0))
        .min_w(px(220.0))
        .min_h_0()
        .border_l_1()
        .border_color(border)
        .bg(theme.bg)
        .overflow_y_scroll()
        .child(div().p(px(styles::spacing::XL)).child(body))
}

pub fn build_manifest_edit_modal(
    kind: ManifestEditKind,
    snap: &ManifestEntrySnapshot,
    cx: &mut Context<App>,
) -> ManifestEditModal {
    fn make_input(cx: &mut Context<App>, placeholder: &str, value: &str) -> Entity<TextInput> {
        let placeholder_owned = placeholder.to_string();
        let value_owned = value.to_string();
        cx.new(|cx| {
            let mut ti = TextInput::new(cx, placeholder_owned.clone());
            if !value_owned.is_empty() {
                ti.set_content(value_owned, cx);
            }
            ti
        })
    }

    fn make_multiline_input(
        cx: &mut Context<App>,
        placeholder: &str,
        value: &str,
        min_lines: usize,
    ) -> Entity<TextInput> {
        let placeholder_owned = placeholder.to_string();
        let value_owned = value.to_string();
        cx.new(|cx| {
            let mut ti = TextInput::new(cx, placeholder_owned.clone()).multiline(min_lines);
            if !value_owned.is_empty() {
                ti.set_content(value_owned, cx);
            }
            ti
        })
    }

    // Convert the stored ;-separated build commands into a newline form so
    // multiline editing is natural. We will collapse newlines back into ;
    // on save.
    let build_commands_display = snap
        .build_commands
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    ManifestEditModal {
        kind,
        name_input: make_input(cx, "package name", &snap.name),
        version_input: make_input(
            cx,
            "version (* for latest)",
            if snap.version.is_empty() { "*" } else { &snap.version },
        ),
        pkg_id_input: make_input(cx, "optional pkg_id", &snap.pkg_id),
        repo_input: make_input(cx, "optional repository name", &snap.repo),
        url_input: make_input(cx, "direct download URL", &snap.url),
        github_input: make_input(cx, "owner/repo on GitHub", &snap.github),
        gitlab_input: make_input(cx, "owner/repo on GitLab", &snap.gitlab),
        asset_pattern_input: make_input(cx, "*linux*.AppImage", &snap.asset_pattern),
        tag_pattern_input: make_input(cx, "v*-stable", &snap.tag_pattern),
        build_commands_input: make_multiline_input(
            cx,
            "one command per line",
            &build_commands_display,
            4,
        ),
        build_dependencies_input: make_input(cx, "gcc, make, pkg-config", &snap.build_dependencies),
        install_patterns_input: make_input(cx, "*.AppImage, *.tar.gz", &snap.install_patterns),
        profile_input: make_input(cx, "profile name", &snap.profile),
        include_prerelease: snap.include_prerelease,
        pinned: snap.pinned,
        binary_only: snap.binary_only,
    }
}

fn switch_pill(
    on: bool,
    on_color: Hsla,
    off_color: Hsla,
    listener: Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static>,
) -> Stateful<Div> {
    let track = if on { on_color } else { off_color };
    let thumb = if on {
        div()
            .ml_auto()
            .w(px(16.0))
            .h(px(16.0))
            .rounded_full()
            .bg(gpui::white())
    } else {
        div().w(px(16.0)).h(px(16.0)).rounded_full().bg(gpui::white())
    };
    div()
        .id("manifest-prune-switch")
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
        .child(thumb)
}

fn chip(text: &str, bg: Hsla, border: Hsla, fg: Hsla) -> Div {
    div()
        .px(px(styles::spacing::XS))
        .py(px(styles::spacing::XXXS))
        .rounded(px(styles::radius::SM))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(px(styles::font_size::CAPTION))
        .text_color(fg)
        .font_weight(FontWeight::MEDIUM)
        .child(text.to_string())
}

fn missing_file_panel(
    theme: &theme::Theme,
    primary: Hsla,
    create_empty_listener: Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static>,
    import_listener: Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static>,
) -> Div {
    let surface = theme.surface;
    let border = theme.border;
    let text_muted = theme.text_muted;
    let hover = theme.hover;

    div()
        .py(px(styles::spacing::XXL))
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(styles::spacing::SM))
        .child(
            div()
                .text_size(px(styles::font_size::HEADING))
                .font_weight(FontWeight::SEMIBOLD)
                .child("No manifest file yet"),
        )
        .child(
            div()
                .text_size(px(styles::font_size::SMALL))
                .text_color(text_muted)
                .child(
                    "Create an empty manifest or import your currently installed packages.",
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(px(styles::spacing::SM))
                .pt(px(styles::spacing::SM))
                .child(
                    div()
                        .id("manifest-empty-create")
                        .px(px(styles::spacing::LG))
                        .py(px(styles::spacing::XS))
                        .rounded(px(styles::radius::MD))
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .cursor_pointer()
                        .text_size(px(styles::font_size::SMALL))
                        .font_weight(FontWeight::MEDIUM)
                        .hover(move |s| s.bg(hover))
                        .on_click(create_empty_listener)
                        .child("Create empty"),
                )
                .child(
                    div()
                        .id("manifest-empty-import")
                        .px(px(styles::spacing::LG))
                        .py(px(styles::spacing::XS))
                        .rounded(px(styles::radius::MD))
                        .bg(primary)
                        .text_color(gpui::white())
                        .border_1()
                        .border_color(primary)
                        .cursor_pointer()
                        .text_size(px(styles::font_size::SMALL))
                        .font_weight(FontWeight::MEDIUM)
                        .hover(move |s| s.bg(primary.opacity(0.85)))
                        .on_click(import_listener)
                        .child("Import installed"),
                ),
        )
}

fn empty_panel(title: &str, hint: Option<&str>, muted: Hsla) -> Div {
    let mut col = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(styles::spacing::XXS))
        .child(
            div()
                .text_size(px(styles::font_size::BODY))
                .text_color(muted)
                .child(title.to_string()),
        );
    if let Some(h) = hint {
        col = col.child(
            div()
                .text_size(px(styles::font_size::CAPTION))
                .text_color(muted)
                .child(h.to_string()),
        );
    }
    div()
        .py(px(styles::spacing::XXL))
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .child(col)
}

fn banner_panel(title: &str, body: &str, accent: Hsla, border: Hsla) -> Div {
    div()
        .px(px(styles::spacing::LG))
        .py(px(styles::spacing::MD))
        .rounded(px(styles::radius::LG))
        .bg(accent.opacity(0.12))
        .border_1()
        .border_color(border)
        .w_full()
        .flex()
        .flex_col()
        .gap(px(styles::spacing::XXS))
        .child(
            div()
                .text_size(px(styles::font_size::HEADING))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(accent)
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(px(styles::font_size::SMALL))
                .child(body.to_string()),
        )
}
