use std::collections::{HashMap, HashSet};

use gpui::*;

use crate::{
    app::{App, OperationStatus},
    core::{package::Update, privilege::PackageMode},
    styles, theme,
};

#[derive(Debug, Default)]
pub struct UpdatesState {
    pub updates: Vec<Update>,
    pub loading: bool,
    pub checked: bool,
    pub error: Option<String>,
    pub result_version: u64,
    pub updating: Option<String>,
    pub no_update_listing: Vec<(String, String)>,
    pub selected: HashSet<String>,
    pub package_progress: HashMap<String, OperationStatus>,
}

impl App {
    pub fn render_updates(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Auto-load on first render
        if !self.updates_state.checked && !self.updates_state.loading {
            self.check_updates(cx);
        }

        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let hover = theme.hover;

        let mode = self.current_mode;
        let title = match mode {
            PackageMode::User => "Updates (User)",
            PackageMode::System => "Updates (System)",
        };

        let is_busy = self.updates_state.updating.is_some() || self.updates_state.loading;

        // Header
        let mut header_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .child(div().text_size(px(styles::font_size::TITLE)).child(title));

        let mut header_buttons = div().flex().flex_row().gap(px(styles::spacing::SM));

        // Update All button
        if !self.updates_state.updates.is_empty() && !is_busy {
            let update_all_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.update_all(cx);
            });
            header_buttons = header_buttons.child(
                div()
                    .id("update-all-btn")
                    .px(px(18.0))
                    .py(px(styles::spacing::SM))
                    .rounded(px(styles::radius::MD))
                    .bg(primary)
                    .text_color(gpui::white())
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .on_click(update_all_listener)
                    .child("Update All"),
            );
        }

        // Check button
        if !is_busy {
            let check_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.check_updates(cx);
            });
            header_buttons = header_buttons.child(
                div()
                    .id("check-updates-btn")
                    .px(px(14.0))
                    .py(px(styles::spacing::XS))
                    .rounded(px(styles::radius::MD))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .hover(move |s| s.bg(hover))
                    .on_click(check_listener)
                    .child("Check"),
            );
        }

        // Sync button
        let sync_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.sync_all_repos(cx);
        });
        let syncing = self.adapter_view.syncing.is_some();
        let sync_label = if syncing { "Syncing..." } else { "Sync" };
        header_buttons = header_buttons.child(
            div()
                .id("updates-sync-btn")
                .px(px(14.0))
                .py(px(styles::spacing::XS))
                .rounded(px(styles::radius::MD))
                .bg(surface)
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_size(px(styles::font_size::SMALL))
                .hover(move |s| s.bg(hover))
                .on_click(sync_listener)
                .child(sync_label),
        );

        header_row = header_row.child(header_buttons);

        // Content
        let content = if self.updates_state.loading {
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child("Checking for updates..."),
            )
        } else if let Some(ref err) = self.updates_state.error {
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child(format!("Failed: {err}")),
            )
        } else if self.updates_state.updates.is_empty() {
            let msg = if self.updates_state.checked {
                "All packages are up to date"
            } else {
                "Click Check to look for updates"
            };

            let mut col = div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::SM))
                .items_center()
                .child(div().text_size(px(styles::font_size::BODY)).child(msg));

            // Show adapter-specific update notes
            if self.updates_state.checked && !self.updates_state.no_update_listing.is_empty() {
                for (_adapter_id, adapter_name) in &self.updates_state.no_update_listing {
                    col = col.child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(styles::spacing::SM))
                            .items_center()
                            .child(
                                div()
                                    .text_size(px(styles::font_size::CAPTION))
                                    .text_color(text_muted)
                                    .child(format!(
                                        "{adapter_name} cannot detect available updates."
                                    )),
                            )
                            .child(
                                div()
                                    .px(px(styles::spacing::SM))
                                    .py(px(styles::spacing::XXXS))
                                    .rounded(px(styles::radius::MD))
                                    .bg(surface)
                                    .border_1()
                                    .border_color(border)
                                    .text_size(px(styles::font_size::CAPTION))
                                    .child(format!("Update All {adapter_name}")),
                            ),
                    );
                }
            }

            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .child(col)
        } else {
            let mut cards_col = div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::SM));

            for (idx, update) in self.updates_state.updates.iter().enumerate() {
                cards_col = cards_col.child(self.render_update_card(update, idx, theme));
            }

            // Per-adapter notes
            for (_adapter_id, adapter_name) in &self.updates_state.no_update_listing {
                cards_col = cards_col.child(
                    div()
                        .py(px(styles::spacing::SM))
                        .flex()
                        .flex_row()
                        .gap(px(styles::spacing::SM))
                        .items_center()
                        .child(
                            div()
                                .text_size(px(styles::font_size::CAPTION))
                                .text_color(text_muted)
                                .child(format!("{adapter_name} cannot detect available updates.")),
                        )
                        .child(
                            div()
                                .px(px(styles::spacing::SM))
                                .py(px(styles::spacing::XXXS))
                                .rounded(px(styles::radius::MD))
                                .bg(surface)
                                .border_1()
                                .border_color(border)
                                .text_size(px(styles::font_size::CAPTION))
                                .child(format!("Update All {adapter_name}")),
                        ),
                );
            }

            cards_col
        };

        let mut main_col = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::MD))
            .w_full()
            .child(header_row)
            .child(content);

        // Floating action bar for batch update
        if !self.updates_state.selected.is_empty() {
            let count = self.updates_state.selected.len();
            let update_selected = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.update_selected(cx);
            });
            let clear_selection = cx.listener(|app, _: &ClickEvent, _window, _cx| {
                app.updates_state.selected.clear();
            });

            main_col = main_col.child(self.floating_action_bar(
                count,
                "Update",
                "updates-update-selected",
                update_selected,
                "updates-clear-selection",
                clear_selection,
                false,
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

    fn render_update_card(
        &self,
        update: &Update,
        idx: usize,
        theme: &theme::Theme,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;
        let success = theme.success;
        let warning = theme.warning;
        let text_muted = theme.text_muted;

        let is_selected = self.updates_state.selected.contains(&update.package.id);
        let pkey =
            crate::core::adapter::progress_key(&update.package.adapter_id, &update.package.id);
        let pkg_status = self.updates_state.package_progress.get(&pkey);
        let is_updating_this = self.updates_state.updating.as_deref() == Some(&update.package.id);
        let is_updating_all = self.updates_state.updating.as_deref() == Some("__all__");
        let is_updating_batch =
            self.updates_state.updating.as_deref() == Some("__batch__") && pkg_status.is_some();

        // Header
        let header = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::XS))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .child(update.package.name.clone()),
            )
            .child(
                div()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .text_size(px(styles::font_size::CAPTION))
                    .child(update.current_version.clone()),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .child("\u{2192}"),
            )
            .child(
                div()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(success.opacity(0.2))
                    .border_1()
                    .border_color(success.opacity(0.4))
                    .text_size(px(styles::font_size::CAPTION))
                    .child(update.new_version.clone()),
            );

        // Info row
        let mut info_row = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center();

        if let Some(size) = update.download_size {
            info_row = info_row.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(format!(
                        "Download: {}",
                        crate::views::browse::format_bytes_pub(size)
                    )),
            );
        }

        if update.is_security {
            info_row = info_row.child(
                div()
                    .px(px(styles::spacing::XS))
                    .py(px(styles::spacing::XXXS))
                    .rounded(px(styles::radius::SM))
                    .bg(warning.opacity(0.2))
                    .border_1()
                    .border_color(warning.opacity(0.4))
                    .text_size(px(styles::font_size::BADGE))
                    .child("Security"),
            );
        }

        // Update button
        let update_btn = if is_updating_this || is_updating_all || is_updating_batch {
            let label = pkg_status
                .map(|s| s.label())
                .unwrap_or_else(|| "Updating...".into());
            div()
                .px(px(styles::spacing::MD))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::MD))
                .bg(surface)
                .border_1()
                .border_color(border)
                .text_size(px(styles::font_size::SMALL))
                .child(label)
        } else {
            div()
                .px(px(styles::spacing::MD))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::MD))
                .bg(primary)
                .text_color(gpui::white())
                .text_size(px(styles::font_size::SMALL))
                .cursor_pointer()
                .child("Update")
        };

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

        // Checkbox
        let checkbox_label = if is_selected { "[x]" } else { "[ ]" };

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

        div()
            .id(SharedString::from(format!("update-pkg-{idx}")))
            .px(px(styles::spacing::MD))
            .py(px(styles::spacing::MD))
            .rounded(px(styles::radius::MD))
            .bg(card_bg)
            .border_1()
            .border_color(card_border)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child(checkbox_label),
            )
            .child(left)
            .child(update_btn)
    }
}
