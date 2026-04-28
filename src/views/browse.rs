use std::collections::{HashMap, HashSet};

use gpui::*;

use crate::{
    app::{App, OperationStatus},
    core::package::Package,
    styles, theme,
};

#[derive(Debug, Default)]
pub struct BrowseState {
    pub search_query: String,
    pub search_results: Vec<Package>,
    pub loading: bool,
    pub has_searched: bool,
    pub error: Option<String>,
    pub install_error: Option<String>,
    pub result_version: u64,
    pub installing: Option<String>,
    pub selected_package: Option<Package>,
    pub search_debounce_version: u64,
    pub selected: HashSet<String>,
    pub package_progress: HashMap<String, OperationStatus>,
}

impl App {
    pub fn render_browse(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let hover = theme.hover;
        let danger = theme.danger;
        let success = theme.success;
        let warning = theme.warning;
        let text_color = theme.text;

        // Sync search query from input entity
        let current_query = self.search_input.read(cx).content().to_string();
        if current_query != self.browse_state.search_query {
            self.browse_state.search_query = current_query.clone();
            if !current_query.is_empty() {
                self.perform_search(cx);
            } else {
                self.browse_state.search_results.clear();
                self.browse_state.has_searched = false;
            }
        }

        // Search bar
        let search_bar = div()
            .px(px(styles::spacing::MD))
            .py(px(10.0))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .text_size(px(styles::font_size::HEADING))
            .child(self.search_input.clone());

        // Result count
        let result_count_text = if self.browse_state.loading {
            "Searching...".to_string()
        } else if !self.browse_state.search_results.is_empty() {
            let count = self.browse_state.search_results.len();
            format!("{count} package{} found", if count == 1 { "" } else { "s" })
        } else {
            String::new()
        };

        let result_count = div()
            .text_size(px(styles::font_size::SMALL))
            .text_color(text_muted)
            .child(result_count_text);

        // Results content
        let results_content = if self.browse_state.loading {
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child("Searching..."),
            )
        } else if let Some(ref err) = self.browse_state.error {
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(styles::spacing::SM))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(styles::font_size::HEADING))
                            .child("Search failed"),
                    )
                    .child(
                        div()
                            .text_size(px(styles::font_size::SMALL))
                            .child(err.clone()),
                    ),
            )
        } else if self.browse_state.search_results.is_empty() {
            let msg = if self.browse_state.has_searched {
                "No packages found"
            } else {
                "Type to search for packages"
            };
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_size(px(styles::font_size::BODY)).child(msg))
        } else {
            let results = self.browse_state.search_results.clone();
            let mut list = div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::SM));
            for (idx, pkg) in results.iter().enumerate() {
                list = list.child(self.render_package_card(pkg, idx, theme, cx));
            }
            list
        };

        // Build the browse list
        let mut browse_list = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::SM))
            .w_full()
            .child(search_bar)
            .child(result_count)
            .child(results_content);

        // Install error banner
        if let Some(ref err) = self.browse_state.install_error {
            let dismiss_listener = cx.listener(|app, _: &ClickEvent, _window, _cx| {
                app.browse_state.install_error = None;
            });

            browse_list = browse_list.child(
                div()
                    .px(px(styles::spacing::MD))
                    .py(px(styles::spacing::SM))
                    .rounded(px(styles::radius::MD))
                    .bg(danger.opacity(0.15))
                    .border_1()
                    .border_color(danger.opacity(0.3))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        div()
                            .text_size(px(styles::font_size::SMALL))
                            .child(format!("Install failed: {err}")),
                    )
                    .child(
                        div()
                            .id("dismiss-install-error")
                            .px(px(10.0))
                            .py(px(styles::spacing::XXS))
                            .rounded(px(styles::radius::MD))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .hover(move |s| s.bg(hover))
                            .on_click(dismiss_listener)
                            .child("Dismiss"),
                    ),
            );
        }

        // Floating action bar for batch selection
        if !self.browse_state.selected.is_empty() {
            let count = self.browse_state.selected.len();
            let install_selected = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.install_selected_browse(cx);
            });
            let clear_selection = cx.listener(|app, _: &ClickEvent, _window, _cx| {
                app.browse_state.selected.clear();
            });

            browse_list = browse_list.child(self.floating_action_bar(
                count,
                "Install",
                "browse-install-selected",
                install_selected,
                "browse-clear-selection",
                clear_selection,
                false,
                theme,
            ));
        }

        let browse_panel = div()
            .p(px(styles::spacing::XL))
            .flex_1()
            .flex()
            .flex_col()
            .child(browse_list);

        // Detail side panel
        if let Some(ref pkg) = self.browse_state.selected_package.clone() {
            div()
                .flex_1()
                .flex()
                .flex_row()
                .child(browse_panel)
                .child(div().w(px(1.0)).h_full().bg(border))
                .child(self.render_detail_panel(pkg, theme, cx))
        } else {
            div().flex_1().flex().flex_row().child(browse_panel)
        }
    }

    fn render_package_card(
        &self,
        pkg: &Package,
        idx: usize,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let success = theme.success;
        let warning = theme.warning;
        let hover = theme.hover;
        let text_muted = theme.text_muted;

        let is_selected = self.browse_state.selected.contains(&pkg.id);
        let pkey = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
        let is_installing = self.browse_state.installing.is_some()
            && (self.browse_state.installing.as_deref() == Some(&pkg.id)
                || self.browse_state.package_progress.contains_key(&pkey));
        let pkg_status = self.browse_state.package_progress.get(&pkey);

        // Header: name + version badge + adapter badge
        let mut header = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .child(pkg.name.clone()),
            );

        if !pkg.version.is_empty() {
            header = header.child(
                div()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .text_size(px(styles::font_size::CAPTION))
                    .child(pkg.version.clone()),
            );
        }

        header = header.child(self.adapter_badge(&pkg.adapter_id, theme));

        let description = div()
            .text_size(px(styles::font_size::SMALL))
            .text_color(text_muted)
            .child(
                pkg.description
                    .clone()
                    .unwrap_or_else(|| "No description".into()),
            );

        // Info row
        let mut info_parts = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center();

        if let Some(size) = pkg.size {
            info_parts = info_parts.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(format_bytes(size)),
            );
        }
        if let Some(ref license) = pkg.license {
            info_parts = info_parts.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(license.clone()),
            );
        }

        // Install button / status
        let install_status = if pkg.installed && pkg.update_available {
            div()
                .px(px(10.0))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(warning.opacity(0.2))
                .border_1()
                .border_color(warning.opacity(0.4))
                .text_size(px(styles::font_size::CAPTION))
                .child("Update Available")
        } else if pkg.installed {
            div()
                .px(px(10.0))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(success.opacity(0.2))
                .border_1()
                .border_color(success.opacity(0.4))
                .text_size(px(styles::font_size::CAPTION))
                .child("Installed")
        } else if is_installing {
            let label = pkg_status
                .map(|s| s.label())
                .unwrap_or_else(|| "Installing...".into());
            div()
                .px(px(10.0))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(primary.opacity(0.2))
                .border_1()
                .border_color(primary.opacity(0.4))
                .text_size(px(styles::font_size::CAPTION))
                .child(label)
        } else {
            div()
                .px(px(14.0))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(primary)
                .text_color(gpui::white())
                .text_size(px(styles::font_size::SMALL))
                .cursor_pointer()
                .child("Install")
        };

        // Left column
        let mut left = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(header)
            .child(description)
            .child(info_parts);

        // Progress bar
        if let Some(progress) = pkg_status.and_then(|s| s.progress()) {
            let pct = (progress * 100.0).min(100.0);
            left = left.child(
                div().w_full().h(px(4.0)).rounded(px(2.0)).bg(border).child(
                    div()
                        .h(px(4.0))
                        .rounded(px(2.0))
                        .bg(primary)
                        .w(relative(progress)),
                ),
            );
        }

        // Checkbox for non-installed
        let mut card_content = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center();

        if !pkg.installed {
            let checkbox = div()
                .size(px(18.0))
                .rounded(px(styles::radius::SM))
                .border_1()
                .border_color(if is_selected { primary } else { border })
                .bg(if is_selected { primary } else { surface })
                .flex()
                .items_center()
                .justify_center()
                .child(if is_selected {
                    div()
                        .text_size(px(12.0))
                        .text_color(gpui::white())
                        .child("\u{2713}")
                } else {
                    div()
                });
            card_content = card_content.child(checkbox);
        }

        card_content = card_content.child(left).child(install_status);

        let card_bg = if is_selected {
            primary.opacity(0.1)
        } else {
            surface
        };
        let card_border = if is_selected {
            primary.opacity(0.3)
        } else {
            border
        };

        let pkg_id = pkg.id.clone();
        let pkg_clone = pkg.clone();
        let is_installed = pkg.installed;
        let card_listener = cx.listener(move |app, _: &ClickEvent, _window, _cx| {
            if !is_installed {
                if app.browse_state.selected.contains(&pkg_id) {
                    app.browse_state.selected.remove(&pkg_id);
                } else {
                    app.browse_state.selected.insert(pkg_id.clone());
                }
            }
            app.browse_state.selected_package = Some(pkg_clone.clone());
        });

        div()
            .id(SharedString::from(format!("browse-pkg-{idx}")))
            .px(px(styles::spacing::MD))
            .py(px(styles::spacing::MD))
            .rounded(px(styles::radius::MD))
            .bg(card_bg)
            .border_1()
            .border_color(card_border)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_click(card_listener)
            .child(card_content)
    }

    fn render_detail_panel(
        &self,
        pkg: &Package,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let success = theme.success;
        let hover = theme.hover;

        let close_listener = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.browse_state.selected_package = None;
        });

        let mut content = div()
            .w(px(320.0))
            .h_full()
            .bg(surface)
            .border_l_1()
            .border_color(border)
            .p(px(styles::spacing::XL))
            .flex()
            .flex_col()
            .gap(px(10.0))
            // Header
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(styles::font_size::TITLE))
                            .child(pkg.name.clone()),
                    )
                    .child(
                        div()
                            .id("close-detail")
                            .px(px(styles::spacing::SM))
                            .py(px(styles::spacing::XXS))
                            .cursor_pointer()
                            .text_size(px(styles::font_size::TITLE))
                            .hover(move |s| s.bg(hover))
                            .rounded(px(styles::radius::SM))
                            .on_click(close_listener)
                            .child("\u{00d7}"),
                    ),
            );

        // Version badge
        if !pkg.version.is_empty() {
            content = content.child(
                div()
                    .px(px(styles::spacing::SM))
                    .py(px(3.0))
                    .rounded(px(styles::radius::SM))
                    .bg(primary.opacity(0.2))
                    .border_1()
                    .border_color(primary.opacity(0.4))
                    .text_size(px(styles::font_size::SMALL))
                    .child(pkg.version.clone()),
            );
        }

        // Description
        content = content.child(
            div().text_size(px(styles::font_size::BODY)).child(
                pkg.description
                    .clone()
                    .unwrap_or_else(|| "No description available".into()),
            ),
        );

        // Separator
        content = content.child(div().w_full().h(px(1.0)).bg(border));

        // Detail rows
        if let Some(ref homepage) = pkg.homepage {
            content = content.child(self.detail_row("Homepage", homepage, theme));
        }
        if let Some(ref license) = pkg.license {
            content = content.child(self.detail_row("License", license, theme));
        }
        if let Some(size) = pkg.size {
            content = content.child(self.detail_row("Size", &format_bytes(size), theme));
        }
        if let Some(ref category) = pkg.category {
            content = content.child(self.detail_row("Category", category, theme));
        }
        if !pkg.tags.is_empty() {
            content = content.child(self.detail_row("Tags", &pkg.tags.join(", "), theme));
        }

        // Status
        let status_badge = if pkg.installed {
            div()
                .px(px(styles::spacing::SM))
                .py(px(3.0))
                .rounded(px(styles::radius::SM))
                .bg(success.opacity(0.2))
                .border_1()
                .border_color(success.opacity(0.4))
                .text_size(px(styles::font_size::CAPTION))
                .child("Installed")
        } else {
            div()
                .px(px(styles::spacing::SM))
                .py(px(3.0))
                .rounded(px(styles::radius::SM))
                .bg(surface)
                .border_1()
                .border_color(border)
                .text_size(px(styles::font_size::CAPTION))
                .child("Not installed")
        };

        content = content.child(
            div()
                .flex()
                .flex_row()
                .gap(px(styles::spacing::SM))
                .items_center()
                .child(
                    div()
                        .text_size(px(styles::font_size::SMALL))
                        .w(px(100.0))
                        .child("Status"),
                )
                .child(status_badge),
        );

        // Separator
        content = content.child(div().w_full().h(px(1.0)).bg(border));

        // Bottom buttons
        let close_bottom = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.browse_state.selected_package = None;
        });

        let mut buttons = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .justify_end();

        if !pkg.installed {
            let detail_pkey = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
            let is_installing = self
                .browse_state
                .package_progress
                .contains_key(&detail_pkey)
                || self.browse_state.installing.as_deref() == Some(&pkg.id);
            if is_installing {
                let status_label = self
                    .browse_state
                    .package_progress
                    .get(&detail_pkey)
                    .map(|s| s.label())
                    .unwrap_or_else(|| "Installing...".into());
                buttons = buttons.child(
                    div()
                        .px(px(styles::spacing::LG))
                        .py(px(styles::spacing::XS))
                        .rounded(px(styles::radius::MD))
                        .bg(primary.opacity(0.3))
                        .text_color(text_muted)
                        .text_size(px(styles::font_size::SMALL))
                        .child(status_label),
                );
            } else {
                let install_pkg = pkg.clone();
                let install_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                    app.install_package(install_pkg.clone(), app.current_mode, cx);
                });
                buttons = buttons.child(
                    div()
                        .id("detail-install")
                        .px(px(styles::spacing::LG))
                        .py(px(styles::spacing::XS))
                        .rounded(px(styles::radius::MD))
                        .bg(primary)
                        .text_color(gpui::white())
                        .cursor_pointer()
                        .text_size(px(styles::font_size::SMALL))
                        .on_click(install_listener)
                        .child("Install"),
                );
            }
        }

        buttons = buttons.child(
            div()
                .id("detail-close-bottom")
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::XS))
                .rounded(px(styles::radius::MD))
                .bg(surface)
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_size(px(styles::font_size::SMALL))
                .hover(move |s| s.bg(hover))
                .on_click(close_bottom)
                .child("Close"),
        );

        content = content.child(buttons);

        content
    }

    fn detail_row(&self, label: &str, value: &str, _theme: &theme::Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .w(px(100.0))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .child(value.to_string()),
            )
    }

    pub fn floating_action_bar(
        &self,
        count: usize,
        action_label: &str,
        action_id: &str,
        action_handler: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
        clear_id: &str,
        clear_handler: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
        is_danger: bool,
        theme: &theme::Theme,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let danger = theme.danger;
        let hover = theme.hover;

        let action_bg = if is_danger { danger } else { primary };

        div()
            .w_full()
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::SM))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .border_1()
            .border_color(border)
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child(format!("{count} selected")),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::SM))
                    .child(
                        div()
                            .id(SharedString::from(clear_id.to_string()))
                            .px(px(14.0))
                            .py(px(styles::spacing::XS))
                            .rounded(px(styles::radius::MD))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .hover(move |s| s.bg(hover))
                            .on_click(clear_handler)
                            .child("Clear"),
                    )
                    .child(
                        div()
                            .id(SharedString::from(action_id.to_string()))
                            .px(px(14.0))
                            .py(px(styles::spacing::XS))
                            .rounded(px(styles::radius::MD))
                            .bg(action_bg)
                            .text_color(gpui::white())
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .on_click(action_handler)
                            .child(format!("{action_label} {count}")),
                    ),
            )
    }

    pub fn adapter_color(adapter_id: &str) -> Hsla {
        // Deterministic hue from adapter ID
        let hash = adapter_id
            .bytes()
            .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
        let hue = (hash % 360) as f32 / 360.0;
        Hsla {
            h: hue,
            s: 0.65,
            l: 0.55,
            a: 1.0,
        }
    }

    pub fn adapter_badge(&self, adapter_id: &str, _theme: &theme::Theme) -> Div {
        let color = Self::adapter_color(adapter_id);
        div()
            .px(px(styles::spacing::XS))
            .py(px(styles::spacing::XXXS))
            .rounded(px(styles::radius::SM))
            .bg(color.opacity(0.2))
            .border_1()
            .border_color(color.opacity(0.4))
            .text_color(color)
            .text_size(px(styles::font_size::BADGE))
            .child(adapter_id.to_string())
    }
}

pub fn format_bytes_pub(bytes: u64) -> String {
    format_bytes(bytes)
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1_048_576 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1_073_741_824 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    }
}
