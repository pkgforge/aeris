use gpui::*;

use crate::{
    app::{App, View},
    core::privilege::PackageMode,
    styles, theme,
};

pub struct DashboardStats {
    pub installed_count: usize,
    pub update_count: usize,
    pub updates_checked: bool,
    pub current_mode: PackageMode,
    pub unhealthy_count: usize,
    pub adapter_count: usize,
}

impl App {
    pub fn render_dashboard(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Auto-load data for dashboard stats
        if !self.installed_state.loaded && !self.installed_state.loading {
            self.load_installed(cx);
        }
        if !self.updates_state.checked && !self.updates_state.loading {
            self.check_updates(cx);
        }

        let stats = self.dashboard_stats();

        let mode_label = match stats.current_mode {
            PackageMode::User => "User",
            PackageMode::System => "System",
        };

        let installed_label = match stats.current_mode {
            PackageMode::User => "Installed (User)",
            PackageMode::System => "Installed (System)",
        };

        let update_value = if stats.updates_checked {
            if stats.update_count == 0 {
                "Up to date".to_string()
            } else {
                stats.update_count.to_string()
            }
        } else {
            "?".to_string()
        };

        let nav_installed = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.current_view = View::Installed;
        });
        let nav_updates = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.current_view = View::Updates;
        });
        let nav_adapters = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.current_view = View::AdapterInfo;
        });
        let nav_browse = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.current_view = View::Browse;
        });
        let refresh_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.load_installed(cx);
        });
        let check_updates_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.check_updates(cx);
        });
        let sync_repos_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.sync_all_repos(cx);
        });

        let primary = theme.primary;
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let hover = theme.hover;
        let danger = theme.danger;
        let nav_installed_view = cx.listener(|app, _: &ClickEvent, _window, _cx| {
            app.current_view = View::Installed;
        });

        let mut content = div()
            .p(px(styles::spacing::XL))
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXL))
            .w_full()
            // Welcome section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(styles::spacing::XXS))
                    .child(
                        div()
                            .text_size(px(styles::font_size::TITLE))
                            .child("Dashboard"),
                    )
                    .child(
                        div()
                            .text_size(px(styles::font_size::SMALL))
                            .text_color(text_muted)
                            .child(format!("Managing {mode_label} packages")),
                    ),
            )
            // Stat cards
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::MD))
                    .w_full()
                    .child(self.stat_card_button(
                        installed_label,
                        &stats.installed_count.to_string(),
                        "stat-installed",
                        nav_installed,
                        theme,
                    ))
                    .child(self.stat_card_button(
                        "Updates",
                        &update_value,
                        "stat-updates",
                        nav_updates,
                        theme,
                    ))
                    .child(self.stat_card_button(
                        "Adapters",
                        &stats.adapter_count.to_string(),
                        "stat-adapters",
                        nav_adapters,
                        theme,
                    )),
            )
            // Quick actions
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(styles::font_size::HEADING))
                            .child("Quick Actions"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(styles::spacing::SM))
                            .child(self.outlined_button("Search", "qa-search", nav_browse, theme))
                            .child(self.outlined_button(
                                "Refresh",
                                "qa-refresh",
                                refresh_listener,
                                theme,
                            ))
                            .child(self.outlined_button(
                                "Check Updates",
                                "qa-check-updates",
                                check_updates_listener,
                                theme,
                            ))
                            .child(self.outlined_button(
                                "Sync Repos",
                                "qa-sync-repos",
                                sync_repos_listener,
                                theme,
                            )),
                    ),
            );

        // Health warning
        if stats.unhealthy_count > 0 {
            content = content.child(
                div()
                    .px(px(styles::spacing::MD))
                    .py(px(styles::spacing::SM))
                    .w_full()
                    .rounded(px(styles::radius::MD))
                    .bg(danger.opacity(0.15))
                    .border_1()
                    .border_color(danger.opacity(0.3))
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(styles::font_size::SMALL)).child(format!(
                        "\u{26a0} {} package(s) with issues",
                        stats.unhealthy_count
                    )))
                    .child(
                        div()
                            .id("health-view-btn")
                            .px(px(styles::spacing::MD))
                            .py(px(styles::spacing::XXS))
                            .rounded(px(styles::radius::MD))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .hover(move |s| s.bg(hover))
                            .on_click(nav_installed_view)
                            .child("View"),
                    ),
            );
        }

        content
    }

    fn stat_card_button(
        &self,
        label: &str,
        value: &str,
        id: &str,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
        theme: &theme::Theme,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;

        div()
            .id(SharedString::from(id.to_string()))
            .flex_1()
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::LG))
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .border_l(px(4.0))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .on_click(on_click)
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(
                div()
                    .text_size(px(styles::font_size::DISPLAY))
                    .font_weight(FontWeight::BOLD)
                    .child(value.to_string()),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .child(label.to_string()),
            )
    }

    fn outlined_button(
        &self,
        label: &str,
        id: &str,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
        theme: &theme::Theme,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let hover = theme.hover;

        div()
            .id(SharedString::from(id.to_string()))
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::SM))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .border_1()
            .border_color(border)
            .cursor_pointer()
            .text_size(px(styles::font_size::SMALL))
            .hover(move |s| s.bg(hover))
            .on_click(on_click)
            .child(label.to_string())
    }

    pub fn dashboard_stats(&self) -> DashboardStats {
        DashboardStats {
            installed_count: self.installed_state.packages.len(),
            update_count: self.updates_state.updates.len(),
            updates_checked: self.updates_state.checked,
            current_mode: self.current_mode,
            unhealthy_count: self
                .installed_state
                .packages
                .iter()
                .filter(|p| !p.is_healthy)
                .count(),
            adapter_count: self.adapter_manager.list_adapters().len(),
        }
    }
}
