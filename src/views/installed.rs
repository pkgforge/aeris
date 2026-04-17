use std::collections::{HashMap, HashSet};

use gpui::*;

use crate::{
    app::{App, OperationStatus},
    core::{package::InstalledPackage, privilege::PackageMode},
    styles, theme,
};

#[derive(Debug, Default)]
pub struct InstalledState {
    pub packages: Vec<InstalledPackage>,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub result_version: u64,
    pub removing: Option<String>,
    pub updating: Option<String>,
    pub updatable_adapters: HashSet<String>,
    pub selected: HashSet<String>,
    pub package_progress: HashMap<String, OperationStatus>,
}

impl App {
    pub fn render_installed(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Auto-load on first render
        if !self.installed_state.loaded && !self.installed_state.loading {
            self.load_installed(cx);
        }

        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let hover = theme.hover;
        let danger = theme.danger;
        let success = theme.success;
        let warning = theme.warning;

        let mode = self.current_mode;
        let title = match mode {
            PackageMode::User => "Installed Packages (User)",
            PackageMode::System => "Installed Packages (System)",
        };

        let refresh_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.load_installed(cx);
        });

        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .child(div().text_size(px(styles::font_size::TITLE)).child(title))
            .child(
                div()
                    .id("installed-refresh")
                    .px(px(14.0))
                    .py(px(styles::spacing::XS))
                    .rounded(px(styles::radius::MD))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .hover(move |s| s.bg(hover))
                    .on_click(refresh_listener)
                    .child("Refresh"),
            );

        let has_packages = !self.installed_state.packages.is_empty();
        let content = if self.installed_state.loading && !has_packages {
            // Only show loading placeholder on first load
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child("Loading installed packages..."),
            )
        } else if let Some(ref err) = self.installed_state.error {
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child(format!("Failed to load: {err}")),
            )
        } else if !has_packages {
            let msg = if self.installed_state.loaded {
                "No packages installed"
            } else {
                "Loading..."
            };
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_size(px(styles::font_size::BODY)).child(msg))
        } else {
            let packages = self.installed_state.packages.clone();
            let mut list = div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::SM));
            for (idx, pkg) in packages.iter().enumerate() {
                list = list.child(self.render_installed_card(pkg, idx, theme, cx));
            }
            list
        };

        let mut main_col = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::MD))
            .w_full()
            .child(header)
            .child(content);

        // Floating action bar for batch removal
        if !self.installed_state.selected.is_empty() {
            let count = self.installed_state.selected.len();
            let remove_selected = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.remove_selected_installed(cx);
            });
            let clear_selection = cx.listener(|app, _: &ClickEvent, _window, _cx| {
                app.installed_state.selected.clear();
            });

            main_col = main_col.child(self.floating_action_bar(
                count,
                "Remove",
                "installed-remove-selected",
                remove_selected,
                "installed-clear-selection",
                clear_selection,
                true,
                theme,
            ));
        }

        div()
            .p(px(styles::spacing::XL))
            .flex_1()
            .flex()
            .flex_col()
            .child(main_col)
    }

    fn render_installed_card(
        &self,
        pkg: &InstalledPackage,
        idx: usize,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;
        let danger = theme.danger;
        let warning = theme.warning;
        let text_muted = theme.text_muted;

        let is_selected = self.installed_state.selected.contains(&pkg.package.id);
        let pkey = crate::core::adapter::progress_key(&pkg.package.adapter_id, &pkg.package.id);
        let pkg_status = self.installed_state.package_progress.get(&pkey);
        let is_removing = self.installed_state.removing.is_some()
            && (self.installed_state.removing.as_deref() == Some(&pkg.package.id)
                || self.installed_state.package_progress.contains_key(&pkey));

        // Header
        let mut header = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .child(pkg.package.name.clone()),
            );

        if !pkg.package.version.is_empty() {
            header = header.child(
                div()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .text_size(px(styles::font_size::CAPTION))
                    .child(pkg.package.version.clone()),
            );
        }

        header = header.child(self.adapter_badge(&pkg.package.adapter_id, theme));

        if !pkg.is_healthy {
            header = header.child(
                div()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(warning.opacity(0.2))
                    .border_1()
                    .border_color(warning.opacity(0.4))
                    .text_size(px(styles::font_size::BADGE))
                    .child("partial install"),
            );
        }

        // Info row
        let mut info_row = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center();

        if pkg.install_size > 0 {
            info_row = info_row.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(format!(
                        "Size: {}",
                        crate::views::browse::format_bytes_pub(pkg.install_size)
                    )),
            );
        }

        if let Some(ref path) = pkg.install_path {
            info_row = info_row.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(path.clone()),
            );
        }

        if let Some(ref profile) = pkg.profile {
            info_row = info_row.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(format!("Profile: {profile}")),
            );
        }

        if pkg.pinned {
            info_row = info_row.child(
                div()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(primary.opacity(0.2))
                    .border_1()
                    .border_color(primary.opacity(0.4))
                    .text_size(px(styles::font_size::BADGE))
                    .child("Pinned"),
            );
        }

        // Left column
        let mut left = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(header)
            .child(info_row);

        // Progress bar
        if let Some(progress) = pkg_status.and_then(|s| s.progress()) {
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

        // Buttons
        let mut buttons = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::XS))
            .items_center();

        let show_update = self
            .installed_state
            .updatable_adapters
            .contains(&pkg.package.adapter_id);

        if show_update {
            let is_updating = self.installed_state.updating.as_deref() == Some(&pkg.package.id)
                || (self.installed_state.updating.as_deref() == Some("__batch__")
                    && pkg_status.is_some());

            if is_updating {
                let label = pkg_status
                    .map(|s| s.label())
                    .unwrap_or_else(|| "Updating...".into());
                buttons = buttons.child(
                    div()
                        .px(px(14.0))
                        .py(px(styles::spacing::XXS))
                        .rounded(px(styles::radius::MD))
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .text_size(px(styles::font_size::SMALL))
                        .child(label),
                );
            } else {
                buttons = buttons.child(
                    div()
                        .px(px(14.0))
                        .py(px(styles::spacing::XXS))
                        .rounded(px(styles::radius::MD))
                        .bg(primary)
                        .text_color(gpui::white())
                        .text_size(px(styles::font_size::SMALL))
                        .cursor_pointer()
                        .child("Update"),
                );
            }
        }

        // Remove button
        if is_removing {
            let label = pkg_status
                .map(|s| s.label())
                .unwrap_or_else(|| "Removing...".into());
            buttons = buttons.child(
                div()
                    .px(px(14.0))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::MD))
                    .bg(danger.opacity(0.3))
                    .text_color(text_muted)
                    .text_size(px(styles::font_size::SMALL))
                    .child(label),
            );
        } else {
            let remove_pkg = pkg.package.clone();
            let remove_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                app.remove_package(remove_pkg.clone(), app.current_mode, cx);
            });
            buttons = buttons.child(
                div()
                    .id(SharedString::from(format!("remove-pkg-{idx}")))
                    .px(px(14.0))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::MD))
                    .bg(danger)
                    .text_color(gpui::white())
                    .text_size(px(styles::font_size::SMALL))
                    .cursor_pointer()
                    .on_click(remove_listener)
                    .child("Remove"),
            );
        }

        // Checkbox
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

        let toggle_pkg_id = pkg.package.id.clone();
        let card_listener = cx.listener(move |app, _: &ClickEvent, _window, _cx| {
            if app.installed_state.selected.contains(&toggle_pkg_id) {
                app.installed_state.selected.remove(&toggle_pkg_id);
            } else {
                app.installed_state.selected.insert(toggle_pkg_id.clone());
            }
        });

        div()
            .id(SharedString::from(format!("installed-pkg-{idx}")))
            .px(px(styles::spacing::MD))
            .py(px(styles::spacing::MD))
            .rounded(px(styles::radius::MD))
            .bg(card_bg)
            .border_1()
            .border_color(card_border)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_click(card_listener)
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center()
            .child(checkbox)
            .child(left)
            .child(buttons)
    }
}
