use std::collections::{HashMap, HashSet};

use gpui::*;

use crate::{
    app::{App, OperationStatus},
    core::package::Package,
    styles, theme,
};

/// How large a package's icon is drawn on its card.
pub const PACKAGE_ICON_SIZE: f32 = 28.0;

/// How many icons may be fetched at once.
const ICONS_AT_ONCE: usize = 4;

#[derive(Debug, Default)]
pub struct BrowseState {
    pub search_query: String,
    pub search_results: Vec<Package>,
    pub loading: bool,
    pub has_searched: bool,
    pub error: Option<String>,
    pub install_error: Option<String>,
    pub result_version: u64,
    /// The [`progress_key`] of the package being installed, not its id: two
    /// managers can offer the same name, and only one of them is installing.
    ///
    /// [`progress_key`]: crate::core::adapter::progress_key
    pub installing: Option<String>,
    pub selected_package: Option<Package>,
    pub selected_detail: Option<crate::core::package::PackageDetail>,
    pub detail_loading: bool,
    pub detail_error: Option<String>,
    pub detail_request_id: u64,
    pub search_debounce_version: u64,
    pub selected: HashSet<String>,
    pub package_progress: HashMap<String, OperationStatus>,
    /// The managers the search is narrowed to. Empty means every one of them,
    /// which is also what a fresh window starts with.
    pub manager_filter: HashSet<String>,
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
        let hover = theme.hover;
        let danger = theme.danger;

        // Browse says which results are already held, which needs the list of
        // what is held even when Installed has not been opened.
        if !self.installed_state.loaded && !self.installed_state.loading {
            self.load_installed(cx);
        }

        // Sync search query from input entity
        let current_query = self.search_input.read(cx).content().to_string();
        if current_query != self.browse_state.search_query {
            self.browse_state.search_query = current_query.clone();
            if !current_query.is_empty() {
                self.perform_search(cx);
            } else {
                self.abandon_search();
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

        let sync_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.sync_all_repos(cx);
        });
        let syncing = self.adapter_view.syncing.is_some();
        let sync_label = if syncing { "Syncing..." } else { "Sync" };

        // Only worth offering a choice when there is one to make.
        let searchable = self.searchable_adapter_ids(self.current_mode);
        let mut narrowing = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(styles::spacing::MD));
        if searchable.len() > 1 {
            narrowing = narrowing.child(self.render_manager_filter(&searchable, theme, cx));
        }
        narrowing = narrowing.child(
            div()
                .text_size(px(styles::font_size::SMALL))
                .text_color(text_muted)
                .child(result_count_text),
        );

        let result_count = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .child(narrowing)
            .child(
                div()
                    .id("browse-sync")
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
            // Only what is on screen is built. A search for a common letter
            // turns up thousands of packages, and drawing them all is what
            // made scrolling one of them lurch.
            let found = self.browse_state.search_results.len();
            if self.browse_list_version != self.browse_state.result_version {
                self.browse_list.reset(found);
                self.browse_list_version = self.browse_state.result_version;
            }

            let held = cx.entity();
            let theme = theme.clone();
            div().flex_1().min_h_0().w_full().child(
                list(self.browse_list.clone(), move |idx, _window, cx| {
                    held.update(cx, |app, cx| {
                        let Some(pkg) = app.browse_state.search_results.get(idx).cloned() else {
                            return div().into_any_element();
                        };

                        // Asked for as it comes into view, so a long list
                        // costs the icons somebody actually looked at.
                        app.want_icons_for(
                            std::iter::once((pkg.adapter_id.as_str(), pkg.name.as_str())),
                            cx,
                        );

                        div()
                            .pb(px(styles::spacing::SM))
                            .child(app.render_package_card(&pkg, idx, &theme, cx))
                            .into_any_element()
                    })
                })
                .size_full(),
            )
        };

        // Build the browse list
        // The search box and what it found stay put while the results move,
        // so the box is always there to type in.
        let search_header = div()
            .flex_shrink_0()
            .w_full()
            .px(px(styles::spacing::XL))
            .pt(px(styles::spacing::XL))
            .pb(px(styles::spacing::SM))
            .flex()
            .flex_col()
            .gap(px(styles::spacing::SM))
            .child(search_bar)
            .child(result_count);

        let mut browse_list = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::SM))
            .w_full()
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
                let count = app.browse_state.selected.len();
                app.confirm_dialog = Some(crate::app::ConfirmAction::BatchInstall { count });
                cx.notify();
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
            .flex_1()
            .min_h_0()
            .min_w_0()
            .w_full()
            .flex()
            .flex_col()
            .child(search_header)
            // The results scroll themselves, drawing only what is on screen,
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
                    .child(browse_list),
            );

        // Detail side panel
        if let Some(ref pkg) = self.browse_state.selected_package.clone() {
            div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .flex()
                .flex_row()
                .child(browse_panel)
                .child(div().w(px(1.0)).h_full().bg(border))
                .child(self.render_detail_panel(pkg, theme, cx))
        } else {
            div()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .flex()
                .flex_row()
                .child(browse_panel)
        }
    }

    /// Whether a package a search answered with is one already held.
    ///
    /// Asking the installed list rather than taking the search's word for it:
    /// a manager may not report what it already has, and soar answers `false`
    /// for a package installed under metadata that has since changed.
    fn is_held(&self, pkg: &Package) -> bool {
        use crate::core::adapter::package_key;

        pkg.installed
            || self
                .installed_state
                .held
                .contains(&package_key(&pkg.adapter_id, &pkg.id))
            || self
                .installed_state
                .held
                .contains(&package_key(&pkg.adapter_id, &pkg.name))
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

        let is_selected = self
            .browse_state
            .selected
            .contains(&crate::core::adapter::package_key(&pkg.adapter_id, &pkg.id));
        let pkey = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
        let is_installing = self.browse_state.installing.as_deref() == Some(pkey.as_str())
            || self.browse_state.package_progress.contains_key(&pkey);
        let pkg_status = self.browse_state.package_progress.get(&pkey);

        // Header: name + version badge + adapter badge
        let mut header = div()
            .flex()
            .flex_row()
            .min_w_0()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(self.package_icon(&pkg.adapter_id, &pkg.name, theme))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(styles::font_size::HEADING))
                    .child(pkg.name.clone()),
            );

        if !pkg.version.is_empty() {
            header = header.child(
                div()
                    .flex_shrink_0()
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
            .w_full()
            .truncate()
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

        let can_install = self
            .adapter_manager
            .get_adapter(&pkg.adapter_id)
            .is_some_and(|a| a.capabilities().can_install);

        let install_status: AnyElement = if pkg.installed && pkg.update_available {
            div()
                .px(px(10.0))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(warning.opacity(0.2))
                .border_1()
                .border_color(warning.opacity(0.4))
                .text_size(px(styles::font_size::CAPTION))
                .text_color(warning)
                .font_weight(FontWeight::MEDIUM)
                .child("Update Available")
                .into_any_element()
        } else if self.is_held(pkg) {
            div()
                .px(px(10.0))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(success.opacity(0.2))
                .border_1()
                .border_color(success.opacity(0.4))
                .text_size(px(styles::font_size::CAPTION))
                .text_color(success)
                .font_weight(FontWeight::MEDIUM)
                .child("Installed")
                .into_any_element()
        } else if is_installing {
            let label = pkg_status
                .map(|s| s.label())
                .unwrap_or_else(|| "Installing...".into());
            self.status_pill(&pkey, label, theme, cx)
        } else if !can_install {
            // Offering a button the manager has no command for would only fail
            // once it was pressed.
            div().into_any_element()
        } else {
            let install_pkg = pkg.clone();
            let install_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                cx.stop_propagation();
                app.confirm_dialog = Some(crate::app::ConfirmAction::Install(
                    install_pkg.clone(),
                    app.current_mode,
                ));
                cx.notify();
            });
            div()
                .id(SharedString::from(format!("install-pkg-{idx}")))
                .px(px(styles::spacing::LG))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::SM))
                .bg(primary)
                .text_color(gpui::white())
                .text_size(px(styles::font_size::SMALL))
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .on_click(install_listener)
                .child("Install")
                .into_any_element()
        };

        // Left column. A flex child will not shrink below what it holds
        // unless it is told it may, so without this a long description pushes
        // the row wider than the window and the button off the end of it.
        let mut left = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(header)
            .child(description)
            .child(info_parts);

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

        // Checkbox for non-installed
        let mut card_content = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center();

        if !pkg.installed {
            // Choosing a package and looking at one are different intentions,
            // so the box answers for itself and leaves the row alone.
            let ticking = crate::core::adapter::package_key(&pkg.adapter_id, &pkg.id);
            let tick = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                cx.stop_propagation();

                if !app.browse_state.selected.remove(&ticking) {
                    app.browse_state.selected.insert(ticking.clone());
                }

                cx.notify();
            });

            let checkbox = div()
                .id(SharedString::from(format!("select-pkg-{idx}")))
                .size(px(18.0))
                .rounded(px(styles::radius::SM))
                .border_1()
                .border_color(if is_selected { primary } else { border })
                .bg(if is_selected { primary } else { surface })
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(tick)
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

        // The button keeps its width; the text beside it is what gives way.
        card_content = card_content
            .child(left)
            .child(div().flex_shrink_0().child(install_status));

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

        let pkg_clone = pkg.clone();
        // The row itself only opens the package. Choosing it belongs to the
        // box, and installing to the button, both of which stop here.
        let card_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            app.browse_state.selected_package = Some(pkg_clone.clone());
            app.load_package_detail(pkg_clone.clone(), cx);
        });

        let card = div()
            .id(SharedString::from(format!("browse-pkg-{idx}")))
            .w_full()
            .min_w_0()
            .px(px(styles::spacing::MD))
            .py(px(styles::spacing::MD))
            .rounded(px(styles::radius::MD))
            .bg(card_bg)
            .border_1()
            .border_color(card_border)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_click(card_listener)
            .child(card_content);

        let mut row = div().w_full().min_w_0().flex().flex_col().child(card);

        if self.output_is_open(&pkey) {
            let titled = format!("{} · {}", pkg.adapter_id, pkg.name);
            row = row.child(self.render_output_log(&pkey, &titled, theme, cx));
        }

        row
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
            app.browse_state.selected_detail = None;
            app.browse_state.detail_loading = false;
            app.browse_state.detail_error = None;
        });

        let mut content = div()
            .id("detail-scroll")
            .flex_shrink()
            .w(px(320.0))
            .min_w(px(220.0))
            .h_full()
            .min_h_0()
            .overflow_y_scroll()
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

        // Version badge. Some managers only say which version a package is
        // when asked about that one package, so the detail fills this in when
        // the listing could not.
        let version = if pkg.version.is_empty() {
            self.browse_state
                .selected_detail
                .as_ref()
                .map(|detail| detail.package.version.clone())
                .unwrap_or_default()
        } else {
            pkg.version.clone()
        };

        if !version.is_empty() {
            content = content.child(
                div()
                    .px(px(styles::spacing::SM))
                    .py(px(3.0))
                    .rounded(px(styles::radius::SM))
                    .bg(primary.opacity(0.2))
                    .border_1()
                    .border_color(primary.opacity(0.4))
                    .text_size(px(styles::font_size::SMALL))
                    .child(version),
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

        // Detail (loaded asynchronously via Adapter::package_detail)
        if self.browse_state.detail_loading {
            content = content.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child("Loading details..."),
            );
        } else if let Some(ref err) = self.browse_state.detail_error {
            content = content.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(err.clone()),
            );
        } else if let Some(ref detail) = self.browse_state.selected_detail {
            // What the search result carried is already shown above, so these
            // fill in only what the detail lookup added.
            if pkg.homepage.is_none()
                && let Some(ref homepage) = detail.package.homepage
            {
                content = content.child(self.detail_row("Homepage", homepage, theme));
            }
            if pkg.license.is_none()
                && let Some(ref license) = detail.package.license
            {
                content = content.child(self.detail_row("License", license, theme));
            }
            if pkg.category.is_none()
                && let Some(ref category) = detail.package.category
            {
                content = content.child(self.detail_row("Category", category, theme));
            }
            if let Some(ref kind) = detail.pkg_type {
                content = content.child(self.detail_row("Type", kind, theme));
            }
            if let Some(ref source) = detail.source {
                content = content.child(self.detail_row("Source", source, theme));
            }
            if let Some(ref date) = detail.build_date {
                content = content.child(self.detail_row("Build date", &on_day(date), theme));
            }

            // Whatever else the manager reports, labelled as its manifest
            // asked. Shown last, since aeris cannot judge how it ranks.
            for (label, value) in &detail.extra {
                content = content.child(self.detail_row(label, value, theme));
            }
        }

        // Separator
        content = content.child(div().w_full().h(px(1.0)).bg(border));

        // Bottom buttons
        let close_bottom = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.browse_state.selected_package = None;
            app.browse_state.selected_detail = None;
            app.browse_state.detail_loading = false;
            app.browse_state.detail_error = None;
        });

        let mut buttons = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .justify_end();

        let detail_can_install = self
            .adapter_manager
            .get_adapter(&pkg.adapter_id)
            .is_some_and(|a| a.capabilities().can_install);

        if !pkg.installed && detail_can_install {
            let detail_pkey = crate::core::adapter::progress_key(&pkg.adapter_id, &pkg.id);
            // Only work still going counts. A record left by something that
            // already finished would otherwise stand in for the button.
            let is_installing = self
                .browse_state
                .package_progress
                .get(&detail_pkey)
                .is_some_and(|status| !status.is_finished())
                || self.browse_state.installing.as_deref() == Some(detail_pkey.as_str());
            if is_installing {
                // The panel is narrow and shares its width with Close, so the
                // status says the least that still means something.
                let status_label = self
                    .browse_state
                    .package_progress
                    .get(&detail_pkey)
                    .map(|s| s.short_label())
                    .unwrap_or_else(|| "Installing...".into());
                buttons = buttons.child(
                    div()
                        .flex_shrink()
                        .min_w(px(0.0))
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
                    .flex_shrink_0()
                    .text_size(px(styles::font_size::SMALL))
                    .w(px(100.0))
                    .child(label.to_string()),
            )
            .child(
                // Takes what is left and no more, so a long value wraps inside
                // the panel rather than running off the edge of it.
                div()
                    .flex_1()
                    .min_w(px(0.0))
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

    /// The row of managers a search can be narrowed to, one chip each. With
    /// nothing picked every manager answers, so the chips all read as on.
    fn render_manager_filter(
        &self,
        searchable: &[String],
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let asked_for = &self.browse_state.manager_filter;
        let narrowed = searchable.iter().any(|id| asked_for.contains(id));
        let border = theme.border;
        let text_muted = theme.text_muted;
        let hover = theme.hover;

        let mut row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(styles::spacing::XS));

        for adapter_id in searchable {
            let asking = !narrowed || asked_for.contains(adapter_id);
            let color = Self::adapter_color(adapter_id);
            let id = adapter_id.clone();
            let toggle = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                app.toggle_search_manager(&id, cx);
            });

            let chip = div()
                .id(SharedString::from(format!("browse-filter-{adapter_id}")))
                .flex_shrink_0()
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XXS))
                .rounded(px(styles::radius::FULL))
                .border_1()
                .cursor_pointer()
                .text_size(px(styles::font_size::CAPTION))
                .on_click(toggle)
                .child(adapter_id.clone());

            row = row.child(if asking {
                chip.bg(color.opacity(0.2))
                    .border_color(color.opacity(0.4))
                    .text_color(color)
                    .hover(move |s| s.bg(color.opacity(0.3)))
            } else {
                chip.border_color(border)
                    .text_color(text_muted)
                    .hover(move |s| s.bg(hover))
            });
        }

        row
    }

    /// Ask for the icons of everything on show that is a known application.
    ///
    /// Queued rather than fetched, in the order the results came back, so what
    /// is at the top of a list arrives first. Only applications named in the
    /// map are ever asked for, and each is fetched once and kept, so a long
    /// list of results costs no more than the applications in it.
    pub fn want_icons_for<'a>(
        &mut self,
        packages: impl Iterator<Item = (&'a str, &'a str)>,
        cx: &mut Context<Self>,
    ) {
        let map = self.icon_map.clone();

        let wanted: Vec<String> = packages
            .map(|(adapter, name)| map.icon_of(adapter, name))
            .filter(|icon| !self.icons.contains_key(*icon) && !self.icon_asked.contains(*icon))
            .map(str::to_string)
            .collect();

        for application in wanted {
            self.icon_asked.insert(application.clone());

            match crate::core::icons::cached_icon(&application) {
                Some(path) => {
                    self.icons.insert(application, path);
                }
                // Only what the index names is ever asked for, so a command
                // line tool is drawn as a package without a request going out.
                None if self.icon_index.url_of(&application).is_some()
                    || crate::core::icons::is_fetchable(&application) =>
                {
                    self.icon_queue.push_back(application)
                }
                None => {}
            }
        }

        self.pump_icon_queue(cx);
    }

    /// Start as many waiting icons as may be in flight at once.
    ///
    /// Capped so a search that turns up hundreds of applications opens a few
    /// connections rather than hundreds. Whatever does not start now starts as
    /// each one finishes.
    fn pump_icon_queue(&mut self, cx: &mut Context<Self>) {
        while self.icons_in_flight < ICONS_AT_ONCE {
            let Some(application) = self.icon_queue.pop_front() else {
                return;
            };
            self.icons_in_flight += 1;

            let index = self.icon_index.clone();
            cx.spawn(
                async move |this: WeakEntity<App>, cx: &mut gpui::AsyncApp| {
                    let fetching = application.clone();
                    let found = cx
                        .background_executor()
                        .spawn(async move { crate::core::icons::fetch_icon(&index, &fetching) })
                        .await;

                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            app.icons_in_flight = app.icons_in_flight.saturating_sub(1);
                            match found {
                                Ok(path) => {
                                    app.icons.insert(application, path);
                                }
                                // Left in the asked set, so a miss is not retried
                                // on every frame that redraws the row.
                                Err(e) => log::debug!("no icon: {e}"),
                            }
                            app.pump_icon_queue(cx);
                            cx.notify();
                        })
                    });
                },
            )
            .detach();
        }
    }

    /// The icon a package installed, the icon of the application it is, or a
    /// package drawn in place of either.
    ///
    /// The slot is drawn whichever it is, so a list where only some have an
    /// icon still reads down one edge.
    pub fn package_icon(&self, adapter_id: &str, name: &str, theme: &theme::Theme) -> AnyElement {
        let held = self.desktop.find(name).and_then(|entry| entry.icon.clone());
        let published = || {
            self.icons
                .get(self.icon_map.icon_of(adapter_id, name))
                .cloned()
        };

        match held.or_else(published) {
            Some(path) => img(path)
                .flex_shrink_0()
                .size(px(PACKAGE_ICON_SIZE))
                .rounded(px(styles::radius::SM))
                .into_any_element(),
            None => svg()
                .path("icons/package.svg")
                .flex_shrink_0()
                .size(px(PACKAGE_ICON_SIZE))
                .text_color(theme.text_muted)
                .into_any_element(),
        }
    }

    pub fn adapter_badge(&self, adapter_id: &str, _theme: &theme::Theme) -> Div {
        let color = Self::adapter_color(adapter_id);
        div()
            .flex_shrink_0()
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

/// The day part of a timestamp, which is as much of it as is worth showing.
fn on_day(timestamp: &str) -> String {
    timestamp
        .split_once('T')
        .map_or(timestamp, |(day, _)| day)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::on_day;

    #[test]
    fn a_timestamp_is_shown_as_the_day_it_names() {
        assert_eq!(on_day("2026-04-05T06:37:57.756197230+00:00"), "2026-04-05");
        assert_eq!(on_day("2026-04-05"), "2026-04-05");
        assert_eq!(on_day(""), "");
    }
}
