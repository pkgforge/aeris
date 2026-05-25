use std::path::PathBuf;

use gpui::*;

use crate::{app::App, styles, theme};

#[derive(Debug, Clone, Default)]
pub struct ManifestEntry {
    pub name: String,
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

#[derive(Debug, Default)]
pub struct ManifestState {
    pub path: Option<PathBuf>,
    pub status: ManifestStatus,
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
                    .child("Declared packages from your soar manifest, compared against what is installed."),
            );

        let is_loading = matches!(self.manifest_state.status, ManifestStatus::Loading);
        let reload_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.load_manifest_diff(cx);
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

        let apply_btn = div()
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::XS))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .text_color(text_muted)
            .border_1()
            .border_color(border)
            .text_size(px(styles::font_size::SMALL))
            .font_weight(FontWeight::MEDIUM)
            .child("Apply (coming soon)");

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
                    .child(apply_btn),
            );

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
            );

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
            ManifestStatus::FileMissing => empty_panel(
                "No manifest file yet",
                Some("Create ~/.config/soar/packages.toml to declare packages."),
                text_muted,
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
            ManifestStatus::Loaded(diff) => render_diff_sections(
                diff.clone(),
                theme,
                primary,
                warning,
                success,
                danger,
            )
            .into_any_element(),
        };

        let content = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::LG))
            .w_full()
            .child(header_row)
            .child(path_card)
            .child(body);

        div()
            .id("manifest-scroll")
            .flex_1()
            .min_h_0()
            .w_full()
            .overflow_y_scroll()
            .child(
                div()
                    .p(px(styles::spacing::XL))
                    .flex()
                    .flex_col()
                    .w_full()
                    .child(content),
            )
    }
}

fn render_diff_sections(
    diff: ManifestDiff,
    theme: &theme::Theme,
    primary: Hsla,
    warning: Hsla,
    success: Hsla,
    danger: Hsla,
) -> Div {
    let mut col = div()
        .flex()
        .flex_col()
        .gap(px(styles::spacing::MD))
        .w_full();

    let summary = div()
        .flex()
        .flex_row()
        .gap(px(styles::spacing::SM))
        .flex_wrap()
        .child(summary_chip(
            "install",
            diff.to_install.len(),
            primary,
            theme,
        ))
        .child(summary_chip(
            "update",
            diff.to_update.len(),
            warning,
            theme,
        ))
        .child(summary_chip("remove", diff.to_remove.len(), danger, theme))
        .child(summary_chip("in sync", diff.in_sync.len(), success, theme))
        .child(summary_chip(
            "not found",
            diff.not_found.len(),
            theme.text_muted,
            theme,
        ));
    col = col.child(summary);

    col = col.child(diff_section(
        "To install",
        primary,
        &diff.to_install,
        DiffKind::Install,
        theme,
    ));
    col = col.child(diff_section(
        "To update",
        warning,
        &diff.to_update,
        DiffKind::Update,
        theme,
    ));
    if !diff.to_remove.is_empty() {
        col = col.child(diff_section(
            "To remove",
            danger,
            &diff.to_remove,
            DiffKind::Remove,
            theme,
        ));
    }
    col = col.child(plain_name_section(
        "In sync",
        success,
        &diff.in_sync,
        theme,
    ));
    if !diff.not_found.is_empty() {
        col = col.child(plain_name_section(
            "Not found",
            theme.text_muted,
            &diff.not_found,
            theme,
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
    let mut body = div().flex().flex_col();
    for (i, entry) in entries.iter().enumerate() {
        let row = entry_row(entry, kind, accent, theme);
        let mut row = row.py(px(styles::spacing::SM));
        if i + 1 < count {
            row = row.border_b_1().border_color(border);
        }
        body = body.child(row);
    }
    card.child(div().px(px(styles::spacing::LG)).child(body))
}

fn entry_row(
    entry: &ManifestEntry,
    kind: DiffKind,
    accent: Hsla,
    theme: &theme::Theme,
) -> Div {
    let surface = theme.surface;
    let border = theme.border;
    let text_muted = theme.text_muted;

    let mut row = div()
        .flex()
        .flex_row()
        .gap(px(styles::spacing::SM))
        .items_center()
        .child(
            div()
                .flex_1()
                .text_size(px(styles::font_size::BODY))
                .font_weight(FontWeight::MEDIUM)
                .child(entry.name.clone()),
        );

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

    row
}

fn plain_name_section(
    title: &str,
    accent: Hsla,
    names: &[String],
    theme: &theme::Theme,
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

    let count = names.len();
    let mut body = div().flex().flex_col();
    for (i, name) in names.iter().enumerate() {
        let mut row = div()
            .py(px(styles::spacing::SM))
            .text_size(px(styles::font_size::BODY))
            .child(name.clone());
        if i + 1 < count {
            row = row.border_b_1().border_color(border);
        }
        body = body.child(row);
    }
    card.child(div().px(px(styles::spacing::LG)).child(body))
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
