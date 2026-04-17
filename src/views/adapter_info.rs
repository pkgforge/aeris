use gpui::*;

use crate::{
    app::App,
    core::{
        adapter::AdapterInfo, capabilities::Capabilities, privilege::PackageMode,
        registry::PluginEntry,
    },
    styles, theme,
};

use crate::app::message::RepoInfo;

impl App {
    pub fn render_adapter_info(
        &mut self,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let primary = theme.primary;
        let hover = theme.hover;
        let success = theme.success;
        let danger = theme.danger;

        // Auto-load repos on first render
        let any_loaded = self.adapter_view.repos_loaded.values().any(|v| *v);
        if !any_loaded {
            self.load_repos(cx);
        }

        let mut adapters = self.adapter_manager.list_adapters_with_status();
        // Sort: built-in adapters first
        adapters.sort_by(|(a, _), (b, _)| b.is_builtin.cmp(&a.is_builtin));
        let installed_ids: Vec<String> = adapters.iter().map(|(info, _)| info.id.clone()).collect();
        let mode = self.current_mode;

        let header = div()
            .text_size(px(styles::font_size::TITLE))
            .child("Adapters");

        let mut content = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::LG))
            .w_full()
            .child(header);

        // Installed adapters section
        content = content.child(
            div()
                .text_size(px(styles::font_size::HEADING))
                .child("Installed Adapters"),
        );

        for (info, enabled) in &adapters {
            let has_repos = info.capabilities.can_list_repos && *enabled;
            content = content.child(self.render_adapter_card(info, *enabled, theme, cx));

            if has_repos {
                content = content.child(self.render_repos_section(&info.id, theme, cx));
            }

            content = content.child(div().w_full().h(px(1.0)).bg(border));
        }

        // Separator
        content = content.child(div().w_full().h(px(2.0)).bg(border));

        // Available plugins section
        content = content.child(
            div()
                .text_size(px(styles::font_size::HEADING))
                .child("Available Plugins"),
        );

        if self.adapter_view.registry_plugins.is_empty() && !self.adapter_view.registry_loading {
            let fetch_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.fetch_registry(cx);
            });

            let mut fetch_row = div()
                .flex()
                .flex_row()
                .gap(px(styles::spacing::SM))
                .items_center()
                .child(
                    div()
                        .id("fetch-plugins-btn")
                        .px(px(styles::spacing::LG))
                        .py(px(styles::spacing::SM))
                        .rounded(px(styles::radius::MD))
                        .bg(primary)
                        .text_color(gpui::white())
                        .cursor_pointer()
                        .text_size(px(styles::font_size::BODY))
                        .on_click(fetch_listener)
                        .child("Fetch Plugins"),
                );

            if let Some(ref err) = self.adapter_view.registry_error {
                fetch_row = fetch_row.child(
                    div()
                        .text_size(px(styles::font_size::SMALL))
                        .text_color(danger)
                        .child(err.clone()),
                );
            }
            content = content.child(fetch_row);
        } else if self.adapter_view.registry_loading {
            content = content.child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child("Fetching plugin registry..."),
            );
        } else {
            let mut has_available = false;

            for entry in self.adapter_view.registry_plugins.clone() {
                if installed_ids.iter().any(|id| id == &entry.id) {
                    continue;
                }
                has_available = true;
                let is_installing =
                    self.adapter_view.installing_plugin.as_deref() == Some(&entry.id);
                content = content.child(self.render_registry_card(&entry, is_installing, theme));
            }

            if !has_available {
                content = content.child(
                    div()
                        .text_size(px(styles::font_size::BODY))
                        .child("All available plugins are installed."),
                );
            }

            let refresh_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
                app.fetch_registry(cx);
            });
            content = content.child(
                div()
                    .id("refresh-plugins-btn")
                    .px(px(styles::spacing::SM))
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

            if let Some(ref err) = self.adapter_view.registry_error {
                content = content.child(
                    div()
                        .text_size(px(styles::font_size::SMALL))
                        .text_color(danger)
                        .child(err.clone()),
                );
            }
        }

        div()
            .p(px(styles::spacing::XL))
            .flex_1()
            .flex()
            .flex_col()
            .child(content)
    }

    fn render_adapter_card(
        &self,
        info: &AdapterInfo,
        enabled: bool,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let text_muted = theme.text_muted;
        let danger = theme.danger;
        let success = theme.success;
        let hover = theme.hover;

        let adapter_color = Self::adapter_color(&info.id);

        // Name + version
        let name_row = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child(info.name.clone()),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(text_muted)
                    .child(format!("v{}", info.version)),
            );

        // Type badge
        let type_label = if info.is_builtin {
            "Built-in"
        } else {
            "Plugin"
        };
        let type_badge = div()
            .px(px(styles::spacing::XS))
            .py(px(styles::spacing::XXXS))
            .rounded(px(styles::radius::SM))
            .bg(adapter_color.opacity(0.2))
            .border_1()
            .border_color(adapter_color.opacity(0.4))
            .text_color(adapter_color)
            .text_size(px(styles::font_size::CAPTION))
            .child(type_label);

        let header_row = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(name_row)
            .child(type_badge);

        let desc = div()
            .text_size(px(styles::font_size::SMALL))
            .text_color(text_muted)
            .child(info.description.clone());

        let caps_view = self.render_capabilities(info.capabilities, theme);

        // Toggle and actions
        let toggle_label = if enabled { "Enabled" } else { "Disabled" };
        let toggle_bg = if enabled {
            success.opacity(0.2)
        } else {
            surface
        };
        let toggle_border = if enabled {
            success.opacity(0.4)
        } else {
            border
        };

        let adapter_id = info.id.clone();
        let toggle_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            let new_enabled = !app.adapter_manager.is_enabled(&adapter_id);
            app.adapter_manager
                .set_adapter_enabled(&adapter_id, new_enabled);
            // Persist to config
            if new_enabled {
                app.aeris_config
                    .disabled_adapters
                    .retain(|id| id != &adapter_id);
            } else {
                if !app.aeris_config.disabled_adapters.contains(&adapter_id) {
                    app.aeris_config.disabled_adapters.push(adapter_id.clone());
                }
            }
            let _ = app.aeris_config.save();
            cx.notify();
        });

        let mut actions = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(
                div()
                    .id(SharedString::from(format!("toggle-adapter-{}", info.id)))
                    .px(px(styles::spacing::SM))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::SM))
                    .bg(toggle_bg)
                    .border_1()
                    .border_color(toggle_border)
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .hover(move |s| s.bg(hover))
                    .on_click(toggle_listener)
                    .child(toggle_label),
            );

        if !info.is_builtin {
            let remove_id = info.id.clone();
            let remove_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                app.adapter_manager.unregister(&remove_id);
                // TODO: also remove plugin files
                cx.notify();
            });

            actions = actions.child(
                div()
                    .id(SharedString::from(format!("remove-adapter-{}", info.id)))
                    .px(px(styles::spacing::SM))
                    .py(px(styles::spacing::XS))
                    .rounded(px(styles::radius::MD))
                    .bg(danger)
                    .text_color(gpui::white())
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .on_click(remove_listener)
                    .child("Remove"),
            );
        }

        div()
            .p(px(styles::spacing::LG))
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::SM))
            .child(header_row)
            .child(desc)
            .child(caps_view)
            .child(actions)
    }

    fn render_repos_section(
        &self,
        adapter_id: &str,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;

        let mode = self.current_mode;
        let title = match mode {
            PackageMode::User => "Repositories (User)",
            PackageMode::System => "Repositories (System)",
        };

        // Header row
        let sync_all_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.sync_all_repos(cx);
        });
        let refresh_repos_listener = cx.listener(|app, _: &ClickEvent, _window, cx| {
            app.load_repos(cx);
        });

        let is_loading = self
            .adapter_view
            .repos_loading
            .get(adapter_id)
            .copied()
            .unwrap_or(false);
        let sync_label = if self.adapter_view.syncing.is_some() {
            "Syncing..."
        } else {
            "Sync All"
        };
        let refresh_label = if is_loading { "Loading..." } else { "Refresh" };

        let header_row = div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(div().text_size(px(styles::font_size::HEADING)).child(title))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::SM))
                    .child(
                        div()
                            .id(SharedString::from(format!("sync-repos-{adapter_id}")))
                            .px(px(14.0))
                            .py(px(styles::spacing::XS))
                            .rounded(px(styles::radius::MD))
                            .bg(primary)
                            .text_color(gpui::white())
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .on_click(sync_all_listener)
                            .child(sync_label),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("refresh-repos-{adapter_id}")))
                            .px(px(14.0))
                            .py(px(styles::spacing::XS))
                            .rounded(px(styles::radius::MD))
                            .bg(surface)
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_size(px(styles::font_size::SMALL))
                            .hover(move |s| s.bg(hover))
                            .on_click(refresh_repos_listener)
                            .child(refresh_label),
                    ),
            );

        // Repos content
        let repos = self.adapter_view.repos_by_adapter.get(adapter_id);
        let repos_error = self.adapter_view.repos_error.get(adapter_id);

        let repos_content = if is_loading {
            div()
                .text_size(px(styles::font_size::BODY))
                .child("Loading repositories...")
        } else if let Some(err) = repos_error {
            div()
                .text_size(px(styles::font_size::BODY))
                .child(format!("Failed to load: {err}"))
        } else if repos.map_or(true, |r| r.is_empty()) {
            div()
                .text_size(px(styles::font_size::BODY))
                .child("No repositories configured")
        } else {
            let repo_list: Vec<_> = repos.unwrap().clone();
            let mut cards_container = div()
                .flex()
                .flex_col()
                .gap(px(styles::spacing::SM))
                .w_full();

            for (idx, repo) in repo_list.iter().enumerate() {
                cards_container =
                    cards_container.child(self.render_repo_card(repo, idx, adapter_id, theme, cx));
            }

            cards_container
        };

        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::MD))
            .w_full()
            .child(header_row)
            .child(repos_content);

        if let Some(ref err) = self.adapter_view.sync_error {
            let danger = theme.danger;
            section = section.child(
                div()
                    .px(px(styles::spacing::MD))
                    .py(px(styles::spacing::XS))
                    .rounded(px(styles::radius::MD))
                    .bg(danger.opacity(0.15))
                    .border_1()
                    .border_color(danger.opacity(0.3))
                    .text_size(px(styles::font_size::SMALL))
                    .child(format!("Sync error: {err}")),
            );
        }

        div()
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::MD))
            .w_full()
            .child(section)
    }

    fn render_repo_card(
        &self,
        repo: &RepoInfo,
        idx: usize,
        adapter_id: &str,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;
        let success = theme.success;
        let danger = theme.danger;
        let text_muted = theme.text_muted;

        let is_syncing = self.adapter_view.syncing.as_deref() == Some(&repo.name)
            || self.adapter_view.syncing.as_deref() == Some("__all__");

        // Header
        let header = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::HEADING))
                    .child(repo.name.clone()),
            );

        let url = div()
            .text_size(px(styles::font_size::SMALL))
            .text_color(text_muted)
            .child(repo.url.clone());

        // Tags
        let mut tags = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::XS))
            .flex_wrap();

        if repo.enabled {
            tags = tags.child(self.badge("Enabled", success, theme));
        } else {
            tags = tags.child(self.badge("Disabled", danger, theme));
        }

        if repo.desktop_integration {
            tags = tags.child(self.badge_neutral("Desktop", theme));
        }

        if repo.has_pubkey {
            tags = tags.child(self.badge("Signed", primary, theme));
        }

        if repo.signature_verification {
            tags = tags.child(self.badge("Verified", primary, theme));
        }

        if let Some(ref interval) = repo.sync_interval {
            tags = tags.child(self.badge_neutral(&format!("Sync: {interval}"), theme));
        }

        // Buttons
        let toggle_label = if repo.enabled { "Disable" } else { "Enable" };
        let sync_label = if is_syncing { "Syncing..." } else { "Sync" };

        let repo_name = repo.name.clone();
        let new_enabled = !repo.enabled;
        let toggle_adapter_id = adapter_id.to_string();
        let toggle_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            let adapter = app.adapter_manager.get_adapter(&toggle_adapter_id);
            if let Some(adapter) = adapter {
                let name = repo_name.clone();
                let aid = toggle_adapter_id.clone();
                let mode = app.current_mode;
                cx.spawn(
                    async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                        let result = crate::tokio_spawn(async move {
                            adapter.set_repo_enabled(&name, new_enabled, mode).await
                        })
                        .await
                        .unwrap_or_else(|e| {
                            Err(crate::core::adapter::AdapterError::Other(format!("{e}")))
                        });

                        match result {
                            Ok(_) => {
                                let _ = cx.update(|cx| {
                                    this.update(cx, |app, cx| {
                                        app.load_repos(cx);
                                    })
                                });
                            }
                            Err(e) => log::warn!("Failed to toggle repo: {e}"),
                        }
                    },
                )
                .detach();
            }
        });

        let sync_repo_name = repo.name.clone();
        let sync_adapter_id = adapter_id.to_string();
        let sync_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            let adapter = app.adapter_manager.get_adapter(&sync_adapter_id);
            if let Some(adapter) = adapter {
                let name = sync_repo_name.clone();
                app.adapter_view.syncing = Some(name.clone());
                cx.spawn(
                    async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                        crate::tokio_spawn(async move {
                            match adapter.sync(None).await {
                                Ok(_) => log::info!("Synced repo"),
                                Err(e) => log::warn!("Sync failed: {e}"),
                            }
                        })
                        .await
                        .unwrap_or_default();
                        let _ = cx.update(|cx| {
                            this.update(cx, |app, cx| {
                                app.adapter_view.syncing = None;
                                app.load_repos(cx);
                                cx.notify();
                            })
                        });
                    },
                )
                .detach();
            }
        });

        let left = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XXS))
            .child(header)
            .child(url)
            .child(tags);

        div()
            .id(SharedString::from(format!("repo-{adapter_id}-{idx}")))
            .px(px(styles::spacing::MD))
            .py(px(styles::spacing::MD))
            .rounded(px(styles::radius::MD))
            .bg(surface)
            .border_1()
            .border_color(border)
            .hover(move |s| s.bg(hover))
            .flex()
            .flex_row()
            .gap(px(styles::spacing::MD))
            .items_center()
            .child(left)
            .child(
                div()
                    .id(SharedString::from(format!(
                        "repo-toggle-{adapter_id}-{idx}"
                    )))
                    .px(px(10.0))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::MD))
                    .bg(surface)
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .on_click(toggle_listener)
                    .child(toggle_label),
            )
            .child(
                div()
                    .id(SharedString::from(format!("repo-sync-{adapter_id}-{idx}")))
                    .px(px(10.0))
                    .py(px(styles::spacing::XXS))
                    .rounded(px(styles::radius::MD))
                    .bg(if is_syncing { surface } else { primary })
                    .text_color(if is_syncing {
                        theme.text
                    } else {
                        gpui::white()
                    })
                    .border_1()
                    .border_color(if is_syncing { border } else { primary })
                    .cursor_pointer()
                    .text_size(px(styles::font_size::SMALL))
                    .on_click(sync_listener)
                    .child(sync_label),
            )
    }

    fn render_registry_card(
        &self,
        entry: &PluginEntry,
        installing: bool,
        theme: &theme::Theme,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let text_muted = theme.text_muted;

        let header = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::SM))
            .items_center()
            .child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child(entry.name.clone()),
            )
            .child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(text_muted)
                    .child(format!("v{}", entry.version)),
            );

        let desc = div()
            .text_size(px(styles::font_size::SMALL))
            .text_color(text_muted)
            .child(entry.description.clone());

        let action = if installing {
            div()
                .text_size(px(styles::font_size::SMALL))
                .child("Installing...")
        } else {
            div()
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XS))
                .rounded(px(styles::radius::MD))
                .bg(primary)
                .text_color(gpui::white())
                .text_size(px(styles::font_size::SMALL))
                .cursor_pointer()
                .child("Install")
        };

        div()
            .p(px(styles::spacing::LG))
            .rounded(px(styles::radius::LG))
            .bg(surface)
            .border_1()
            .border_color(border)
            .w_full()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::SM))
            .child(header)
            .child(desc)
            .child(action)
    }

    fn render_capabilities(&self, caps: Capabilities, theme: &theme::Theme) -> impl IntoElement {
        let entries: Vec<(&str, bool)> = vec![
            ("Search", caps.can_search),
            ("Install", caps.can_install),
            ("Remove", caps.can_remove),
            ("Update", caps.can_update),
            ("List", caps.can_list),
            ("Sync", caps.can_sync),
            ("Run", caps.can_run),
            ("Add Repo", caps.can_add_repo),
            ("Remove Repo", caps.can_remove_repo),
            ("List Repos", caps.can_list_repos),
            ("Profiles", caps.has_profiles),
            ("Groups", caps.has_groups),
            ("Dependencies", caps.has_dependencies),
            ("Size Info", caps.has_size_info),
            ("Package Detail", caps.has_package_detail),
            ("Dry Run", caps.supports_dry_run),
            ("Verification", caps.supports_verification),
            ("Locks", caps.supports_locks),
            ("Batch Install", caps.supports_batch_install),
            ("Portable", caps.supports_portable),
            ("Hooks", caps.supports_hooks),
            ("Build from Source", caps.supports_build_from_source),
            ("Declarative", caps.supports_declarative),
            ("Snapshots", caps.supports_snapshots),
        ];

        let success = theme.success;
        let surface = theme.surface;
        let border = theme.border;

        let mut rows: Vec<Div> = Vec::new();
        let mut current_row = div()
            .flex()
            .flex_row()
            .gap(px(styles::spacing::XS))
            .flex_wrap();

        for (i, (name, supported)) in entries.iter().enumerate() {
            let badge_color = if *supported { success } else { surface };
            let badge_border = if *supported {
                success.opacity(0.4)
            } else {
                border
            };

            current_row = current_row.child(
                div()
                    .px(px(styles::spacing::SM))
                    .py(px(3.0))
                    .rounded(px(styles::radius::SM))
                    .bg(badge_color.opacity(0.2))
                    .border_1()
                    .border_color(badge_border)
                    .text_size(px(styles::font_size::CAPTION))
                    .child(name.to_string()),
            );

            if (i + 1) % 6 == 0 && i + 1 < entries.len() {
                rows.push(current_row);
                current_row = div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::XS))
                    .flex_wrap();
            }
        }
        rows.push(current_row);

        div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XS))
            .children(rows)
    }

    fn badge(&self, label: &str, color: Hsla, _theme: &theme::Theme) -> Div {
        div()
            .px(px(styles::spacing::XS))
            .py(px(styles::spacing::XXXS))
            .rounded(px(styles::radius::SM))
            .bg(color.opacity(0.2))
            .border_1()
            .border_color(color.opacity(0.4))
            .text_size(px(styles::font_size::BADGE))
            .child(label.to_string())
    }

    fn badge_neutral(&self, label: &str, theme: &theme::Theme) -> Div {
        let surface = theme.surface;
        let border = theme.border;
        div()
            .px(px(styles::spacing::XS))
            .py(px(styles::spacing::XXXS))
            .rounded(px(styles::radius::SM))
            .bg(surface)
            .border_1()
            .border_color(border)
            .text_size(px(styles::font_size::BADGE))
            .child(label.to_string())
    }
}
