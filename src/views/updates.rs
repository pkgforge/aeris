use std::collections::{HashMap, HashSet};

use gpui::*;

use crate::{
    app::{App, OperationStatus},
    core::{package::Update, privilege::PackageMode},
    styles, theme,
};

/// Something a manager cannot do that the Updates view would otherwise be
/// expected to offer, and the manager it is about.
#[derive(Debug, Clone)]
pub struct ManagerLimit {
    pub adapter_id: String,
    pub adapter_name: String,
    pub said: String,
    /// Whether updating everything it holds is still on the table.
    pub can_update_all: bool,
}

#[derive(Debug, Default)]
pub struct UpdatesState {
    pub updates: Vec<Update>,
    pub loading: bool,
    pub checked: bool,
    pub error: Option<String>,
    pub result_version: u64,
    /// What is being updated: a package's [`progress_key`], a manager's id when
    /// it is updating everything it holds, or `__all__` / `__batch__`. Packages
    /// carry their manager because two managers can offer the same name.
    ///
    /// [`progress_key`]: crate::core::adapter::progress_key
    pub updating: Option<String>,
    pub limits: Vec<ManagerLimit>,
    pub selected: HashSet<String>,
    pub package_progress: HashMap<String, OperationStatus>,
}

impl App {
    pub fn render_updates(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
        let subtitle = match mode {
            PackageMode::User => "Available updates for your user packages.",
            PackageMode::System => "Available updates for system packages.",
        };

        let is_busy = self.updates_state.updating.is_some() || self.updates_state.loading;

        let mut title_block = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(
                div()
                    .text_size(px(styles::font_size::TITLE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(text_muted)
                    .child(subtitle),
            );

        if let Some(note) = crate::app::scope_note(&self.adapter_manager.outside_mode(mode), mode) {
            title_block = title_block.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(note),
            );
        }

        let mut header_buttons = div().flex().flex_row().gap(px(styles::spacing::SM));

        if !self.updates_state.updates.is_empty() && !is_busy {
            let update_all_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.confirm_dialog = Some(crate::app::ConfirmAction::UpdateAll(app.current_mode));
                cx.notify();
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

        let header_row = div()
            .flex()
            .flex_row()
            .items_start()
            .justify_between()
            .w_full()
            .child(title_block)
            .child(header_buttons);

        let content: AnyElement = if self.updates_state.loading {
            div()
                .py(px(styles::spacing::XXL))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(styles::font_size::BODY))
                        .text_color(text_muted)
                        .child("Checking for updates..."),
                )
                .into_any_element()
        } else if let Some(ref err) = self.updates_state.error {
            div()
                .py(px(styles::spacing::XXL))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(styles::font_size::BODY))
                        .text_color(text_muted)
                        .child(format!("Failed: {err}")),
                )
                .into_any_element()
        } else if self.updates_state.updates.is_empty() {
            let msg = if self.updates_state.checked {
                "All packages are up to date"
            } else {
                "Click Check to look for updates"
            };
            div()
                .py(px(styles::spacing::XXL))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_size(px(styles::font_size::BODY))
                        .text_color(text_muted)
                        .child(msg),
                )
                .into_any_element()
        } else {
            // Only what is on screen is built, the same as the other listings.
            let waiting = self.updates_state.updates.len();
            if self.updates_list_version != self.updates_state.result_version {
                self.updates_list.reset(waiting);
                self.updates_list_version = self.updates_state.result_version;
            }

            let held = cx.entity();
            let theme = theme.clone();
            div()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    list(self.updates_list.clone(), move |idx, _window, cx| {
                        held.update(cx, |app, cx| {
                            let Some(update) = app.updates_state.updates.get(idx).cloned() else {
                                return div().into_any_element();
                            };

                            app.want_icons_for(
                                std::iter::once((
                                    update.package.adapter_id.as_str(),
                                    update.package.name.as_str(),
                                )),
                                cx,
                            );

                            div()
                                .pb(px(styles::spacing::SM))
                                .child(app.render_update_card(&update, idx, &theme, cx))
                                .into_any_element()
                        })
                    })
                    .size_full(),
                )
                .into_any_element()
        };

        let mut notes_col = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XS))
            .w_full();
        let mut has_notes = false;

        if self.updates_state.checked {
            for limit in &self.updates_state.limits {
                has_notes = true;
                let ManagerLimit {
                    adapter_id,
                    adapter_name,
                    said,
                    can_update_all,
                } = limit;

                let mut note = div()
                    .px(px(styles::spacing::MD))
                    .py(px(styles::spacing::SM))
                    .rounded(px(styles::radius::MD))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .gap(px(styles::spacing::MD))
                    .w_full()
                    .child(
                        div()
                            .text_size(px(styles::font_size::SMALL))
                            .text_color(text_muted)
                            .child(format!("{adapter_name} {said}")),
                    );

                let manager_key = crate::core::adapter::manager_progress_key(adapter_id);
                let working = self
                    .updates_state
                    .package_progress
                    .get(&manager_key)
                    .cloned();

                if let Some(status) = working {
                    note = note.child(self.status_pill(&manager_key, status.label(), theme, cx));
                } else if *can_update_all && !is_busy {
                    let for_adapter = adapter_id.clone();
                    let named = adapter_name.clone();
                    let update_everything = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                        app.confirm_dialog = Some(crate::app::ConfirmAction::UpdateEverythingIn {
                            adapter_id: for_adapter.clone(),
                            adapter_name: named.clone(),
                            mode: app.current_mode,
                        });
                        cx.notify();
                    });

                    note = note.child(
                        div()
                            .id(SharedString::from(format!(
                                "update-everything-{adapter_id}"
                            )))
                            .flex_shrink_0()
                            .px(px(14.0))
                            .py(px(styles::spacing::XS))
                            .rounded(px(styles::radius::MD))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .hover(move |s| s.bg(hover))
                            .on_click(update_everything)
                            .child("Update all"),
                    );
                }

                notes_col = notes_col.child(note);

                if self.output_is_open(&manager_key) {
                    notes_col = notes_col.child(self.render_output_log(
                        &manager_key,
                        adapter_name,
                        theme,
                        cx,
                    ));
                }
            }
        }

        // The title, its buttons and what a manager cannot answer stay put, so
        // a long list of updates cannot push them out of sight.
        let mut pinned = div()
            .flex_shrink_0()
            .w_full()
            .px(px(styles::spacing::XL))
            .pt(px(styles::spacing::XL))
            .pb(px(styles::spacing::MD))
            .flex()
            .flex_col()
            .gap(px(styles::spacing::MD))
            .child(header_row);

        if has_notes {
            pinned = pinned.child(notes_col);
        }

        let mut main_col = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::LG))
            .w_full()
            .child(content);

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
            .flex_1()
            .min_h_0()
            .min_w_0()
            .w_full()
            .flex()
            .flex_col()
            .child(pinned)
            // The cards scroll themselves, drawing only what is on screen,
            // so nothing here may scroll around them.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .w_full()
                    .px(px(styles::spacing::XL))
                    .pb(px(styles::spacing::XL))
                    .flex()
                    .flex_col()
                    .child(main_col),
            )
    }

    fn render_update_card(
        &self,
        update: &Update,
        idx: usize,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;
        let success = theme.success;
        let warning = theme.warning;
        let text_muted = theme.text_muted;

        let is_selected = self
            .updates_state
            .selected
            .contains(&crate::core::adapter::package_key(
                &update.package.adapter_id,
                &update.package.id,
            ));
        let pkey =
            crate::core::adapter::progress_key(&update.package.adapter_id, &update.package.id);
        let pkg_status = self.updates_state.package_progress.get(&pkey);
        let is_updating_this = self.updates_state.updating.as_deref() == Some(pkey.as_str());
        let is_updating_all = self.updates_state.updating.as_deref() == Some("__all__");
        let is_updating_batch =
            self.updates_state.updating.as_deref() == Some("__batch__") && pkg_status.is_some();

        let header = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::XS))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .font_weight(FontWeight::SEMIBOLD)
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
                    .text_color(text_muted)
                    .child(update.current_version.clone()),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(text_muted)
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
                    .text_color(success)
                    .font_weight(FontWeight::MEDIUM)
                    .child(update.new_version.clone()),
            )
            .child(self.adapter_badge(&update.package.adapter_id, theme));

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
                    .text_color(warning)
                    .font_weight(FontWeight::MEDIUM)
                    .child("Security"),
            );
        }

        // A manager that cannot be pointed at one package is asked from the
        // note above the list instead.
        let can_update_one = self
            .adapter_manager
            .get_adapter(&update.package.adapter_id)
            .is_some_and(|a| a.capabilities().can_update_one);

        let update_btn: AnyElement = if !can_update_one {
            div().into_any_element()
        } else if is_updating_this || is_updating_all || is_updating_batch {
            let label = pkg_status
                .map(|s| s.label())
                .unwrap_or_else(|| "Updating...".into());
            self.status_pill(&pkey, label, theme, cx).into_any_element()
        } else {
            let pkg_for_update = update.package.clone();
            let update_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                cx.stop_propagation();
                app.confirm_dialog = Some(crate::app::ConfirmAction::Update(
                    pkg_for_update.clone(),
                    app.current_mode,
                ));
                cx.notify();
            });
            div()
                .id(SharedString::from(format!("update-pkg-btn-{idx}")))
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::XS))
                .rounded(px(styles::radius::MD))
                .bg(primary)
                .text_color(gpui::white())
                .text_size(px(styles::font_size::SMALL))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .on_click(update_listener)
                .child("Update")
                .into_any_element()
        };

        let mut left = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XS))
            .child(header)
            .child(info_row);

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
            primary.opacity(0.08)
        } else {
            surface
        };
        let card_border = if is_selected {
            primary.opacity(0.4)
        } else {
            border
        };

        let toggle_key =
            crate::core::adapter::package_key(&update.package.adapter_id, &update.package.id);
        let card_listener = cx.listener(move |app, _: &ClickEvent, _window, _cx| {
            if app.updates_state.selected.contains(&toggle_key) {
                app.updates_state.selected.remove(&toggle_key);
            } else {
                app.updates_state.selected.insert(toggle_key.clone());
            }
        });

        let card = div()
            .id(SharedString::from(format!("update-pkg-{idx}")))
            .w_full()
            .min_w_0()
            .px(px(styles::spacing::LG))
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
            .child(update_btn);

        let mut row = div().w_full().min_w_0().flex().flex_col().child(card);

        if self.output_is_open(&pkey) {
            let titled = format!("{} · {}", update.package.adapter_id, update.package.name);
            row = row.child(self.render_output_log(&pkey, &titled, theme, cx));
        }

        row
    }
}
