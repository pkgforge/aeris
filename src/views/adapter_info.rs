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
        let primary = theme.primary;
        let hover = theme.hover;
        let danger = theme.danger;

        let any_loaded = self.adapter_view.repos_loaded.values().any(|v| *v);
        if !any_loaded {
            self.load_repos(cx);
        }

        self.consider_registry(cx);

        let mut adapters = self.adapter_manager.list_adapters_with_status();
        adapters.sort_by(|(a, _), (b, _)| b.is_builtin.cmp(&a.is_builtin));

        // Added but not usable, because whatever they drive is missing. They
        // belong with the installed ones: that is where someone who added
        // them will look.
        let unusable = self.unusable_adapters();

        let installed_ids: Vec<String> = adapters
            .iter()
            .map(|(info, _)| info.id.clone())
            .chain(unusable.iter().map(|(info, _)| info.id.clone()))
            .collect();

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
            let has_profiles = info.capabilities.has_profiles && *enabled;
            content = content.child(self.render_adapter_card(info, *enabled, theme, cx));

            if has_profiles {
                content = content.child(self.render_profiles_section(&info.id, theme, cx));
            }

            if has_repos {
                content = content.child(self.render_repos_section(&info.id, theme, cx));
            }

            content = content.child(div().w_full().h(px(1.0)).bg(border));
        }

        // Separator
        content = content.child(div().w_full().h(px(2.0)).bg(border));

        for (info, reason) in &unusable {
            content = content.child(self.render_unusable_card(info, reason, theme, cx));
        }

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
                // An adapter already added is shown again only when the
                // registry offers a newer manifest than the one on disk.
                if installed_ids.iter().any(|id| id == &entry.id)
                    && crate::core::registry::update_for(&entry).is_none()
                {
                    continue;
                }
                has_available = true;
                let is_installing =
                    self.adapter_view.installing_plugin.as_deref() == Some(&entry.id);

                // Kept on disk but not in use, because the command it drives
                // is not installed. Saying so beats offering to add it again.
                let waiting_for = (!installed_ids.iter().any(|id| id == &entry.id))
                    .then(|| crate::core::registry::installed_needs(&entry.id))
                    .flatten();

                content = content.child(self.render_registry_card(
                    &entry,
                    is_installing,
                    waiting_for,
                    theme,
                    cx,
                ));
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
            .id("adapters-scroll")
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
                match crate::core::registry::remove_plugin(&remove_id) {
                    Ok(_) => app.add_toast(
                        crate::app::ToastLevel::Success,
                        format!("Removed plugin {remove_id}"),
                    ),
                    Err(e) => app.add_toast(
                        crate::app::ToastLevel::Error,
                        format!("Failed to remove plugin files: {e}"),
                    ),
                }
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

    fn render_profiles_section(
        &self,
        adapter_id: &str,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let surface = theme.surface;
        let border = theme.border;
        let primary = theme.primary;
        let hover = theme.hover;
        let text_muted = theme.text_muted;
        let success = theme.success;

        let profiles = self.adapter_view.profiles_by_adapter.get(adapter_id);
        let is_loading = self
            .adapter_view
            .profiles_loading
            .get(adapter_id)
            .copied()
            .unwrap_or(false);
        let load_error = self.adapter_view.profiles_error.get(adapter_id);
        let switching = &self.adapter_view.switching_profile;

        // Trigger load on first render
        if profiles.is_none() && !is_loading && load_error.is_none() {
            let aid = adapter_id.to_string();
            cx.spawn(
                async move |this: WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                    let _ = cx.update(|cx| {
                        let _ = this.update(cx, |app, cx| {
                            app.load_profiles(&aid, cx);
                        });
                    });
                },
            )
            .detach();
        }

        let title = div()
            .text_size(px(styles::font_size::HEADING))
            .child("Profiles");

        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::XS))
            .w_full();

        if is_loading {
            list = list.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child("Loading profiles..."),
            );
        } else if let Some(err) = load_error {
            list = list.child(
                div()
                    .text_size(px(styles::font_size::CAPTION))
                    .text_color(text_muted)
                    .child(format!("Failed to load: {err}")),
            );
        } else if let Some(profiles) = profiles {
            if profiles.is_empty() {
                list = list.child(
                    div()
                        .text_size(px(styles::font_size::CAPTION))
                        .text_color(text_muted)
                        .child("No profiles configured"),
                );
            } else {
                for (idx, profile) in profiles.iter().enumerate() {
                    let pid = profile.id.clone();
                    let aid = adapter_id.to_string();
                    let is_active = profile.is_active;
                    let is_switching = switching.as_deref() == Some(profile.id.as_str());
                    let listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                        app.switch_to_profile(&aid, &pid, cx);
                    });
                    let bg = if is_active {
                        primary.opacity(0.15)
                    } else {
                        surface
                    };
                    let border_color = if is_active {
                        primary.opacity(0.4)
                    } else {
                        border
                    };
                    let label = if is_switching {
                        format!("{} (switching...)", profile.name)
                    } else if is_active {
                        format!("{} (active)", profile.name)
                    } else {
                        profile.name.clone()
                    };
                    let mut row = div()
                        .id(SharedString::from(format!("profile-{adapter_id}-{idx}")))
                        .flex()
                        .flex_row()
                        .items_center()
                        .justify_between()
                        .px(px(styles::spacing::MD))
                        .py(px(styles::spacing::XS))
                        .rounded(px(styles::radius::MD))
                        .bg(bg)
                        .border_1()
                        .border_color(border_color)
                        .text_size(px(styles::font_size::SMALL))
                        .child(label);
                    if !is_active && !is_switching {
                        row = row
                            .cursor_pointer()
                            .hover(move |s| s.bg(hover))
                            .on_click(listener);
                    }
                    if is_active {
                        row = row.child(
                            div()
                                .text_size(px(styles::font_size::CAPTION))
                                .text_color(success)
                                .child("\u{2713}"),
                        );
                    }
                    list = list.child(row);
                }
            }
        }

        div()
            .px(px(styles::spacing::LG))
            .py(px(styles::spacing::MD))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(styles::spacing::SM))
            .child(title)
            .child(list)
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

    /// An adapter that was added but cannot run, and what it is missing.
    fn render_unusable_card(
        &self,
        info: &AdapterInfo,
        reason: &str,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id = info.id.as_str();
        let surface = theme.surface;
        let border = theme.border;
        let hover = theme.hover;

        let retrying = id.to_string();
        let retry = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            app.retry_adapter(retrying.clone(), cx);
        });

        let forgetting = id.to_string();
        let forget = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            app.forget_adapter(forgetting.clone(), cx);
        });

        // Turning it off is what stops aeris looking for something that is
        // not there, so this has to work whether or not it does.
        let disabled = !self.adapter_manager.is_enabled(id);
        let toggling = id.to_string();
        let toggle = cx.listener(move |app, _: &ClickEvent, _window, cx| {
            app.adapter_manager.set_adapter_enabled(&toggling, disabled);
            app.aeris_config.set_adapter_disabled(&toggling, !disabled);
            let _ = app.aeris_config.save();
            cx.notify();
        });

        /// Both buttons look the same and differ only in what they do.
        type Clicked = Box<dyn Fn(&ClickEvent, &mut Window, &mut gpui::App)>;

        let button = |label: &str, element_id: String, listener: Clicked| {
            div()
                .id(SharedString::from(element_id))
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XS))
                .rounded(px(styles::radius::MD))
                .bg(surface)
                .border_1()
                .border_color(border)
                .cursor_pointer()
                .text_size(px(styles::font_size::SMALL))
                .hover(move |s| s.bg(hover))
                .on_click(listener)
                .child(label.to_string())
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
            .child({
                let mut header = div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::SM))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(styles::font_size::HEADING))
                            .child(info.name.clone()),
                    );

                if !info.version.is_empty() {
                    header = header.child(self.badge_neutral(&format!("v{}", info.version), theme));
                }

                let kind = if info.is_builtin {
                    "Built-in"
                } else {
                    "Plugin"
                };

                header
                    .child(self.badge_neutral(kind, theme))
                    .child(self.badge_neutral("unavailable", theme))
            })
            .child(
                div()
                    .text_size(px(styles::font_size::BODY))
                    .child(info.description.clone()),
            )
            .child({
                // Turned off is a choice; missing is a problem. They should
                // not read the same.
                let (colour, said) = if disabled {
                    (
                        theme.text_muted,
                        "Turned off, so aeris does not look for it".to_string(),
                    )
                } else {
                    (
                        // Already a sentence, from whoever worked out that it
                        // could not run.
                        theme.warning,
                        reason.to_string(),
                    )
                };

                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(colour)
                    .child(said)
            })
            .child(self.render_capabilities(info.capabilities, theme))
            .child({
                let mut actions = div()
                    .flex()
                    .flex_row()
                    .gap(px(styles::spacing::SM))
                    .child(button(
                        "Check again",
                        format!("retry-{id}"),
                        Box::new(retry),
                    ))
                    .child(button(
                        if disabled { "Enable" } else { "Disable" },
                        format!("toggle-missing-{id}"),
                        Box::new(toggle),
                    ));

                // What aeris ships with stays; only what was added can go.
                if !info.is_builtin {
                    actions =
                        actions.child(button("Remove", format!("forget-{id}"), Box::new(forget)));
                }

                actions
            })
    }

    fn render_registry_card(
        &self,
        entry: &PluginEntry,
        installing: bool,
        waiting_for: Option<String>,
        theme: &theme::Theme,
        cx: &mut Context<Self>,
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
                .id(SharedString::from(format!(
                    "installing-plugin-{}",
                    entry.id
                )))
                .text_size(px(styles::font_size::SMALL))
                .child("Installing...")
        } else {
            let wanted = entry.clone();
            let install_listener = cx.listener(move |app, _: &ClickEvent, _window, cx| {
                app.install_plugin(wanted.clone(), cx);
            });

            let label = match (&waiting_for, crate::core::registry::update_for(entry)) {
                // Already added, so the thing to offer is another look once
                // the missing command has been installed.
                (Some(_), _) => "Check again".to_string(),
                (None, Some(newer)) => format!("Update to {newer}"),
                (None, None) => "Install".to_string(),
            };

            let waiting = waiting_for.is_some();

            div()
                .id(SharedString::from(format!("install-plugin-{}", entry.id)))
                .px(px(styles::spacing::SM))
                .py(px(styles::spacing::XS))
                .rounded(px(styles::radius::MD))
                .bg(if waiting { surface } else { primary })
                .text_color(gpui::white())
                .text_size(px(styles::font_size::SMALL))
                .cursor_pointer()
                .on_click(install_listener)
                .child(label)
        };

        let mut card = div()
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
            .child(desc);

        // What is missing is worth a sentence rather than a cramped label.
        if let Some(command) = &waiting_for {
            card = card.child(
                div()
                    .text_size(px(styles::font_size::SMALL))
                    .text_color(theme.warning)
                    .child(format!(
                        "Added, but the {command} command is not installed yet"
                    )),
            );
        }

        card
            // In a column every child is stretched to the full width, so the
            // button needs a row of its own to be only as wide as its label.
            .child(div().flex().flex_row().child(action))
    }

    fn render_capabilities(&self, caps: Capabilities, theme: &theme::Theme) -> impl IntoElement {
        // Only what the manager can do is worth a badge. Listing the rest says
        // little, since a manager not doing something is the ordinary case.
        let entries: Vec<&str> = [
            ("Search", caps.can_search),
            ("Install", caps.can_install),
            ("Remove", caps.can_remove),
            ("Update", caps.can_update),
            ("List", caps.can_list),
            ("List Updates", caps.can_list_updates),
            ("Sync", caps.can_sync),
            ("Run", caps.can_run),
            ("Add Repo", caps.can_add_repo),
            ("Remove Repo", caps.can_remove_repo),
            ("List Repos", caps.can_list_repos),
            ("Profiles", caps.has_profiles),
            ("Size Info", caps.has_size_info),
            ("Package Detail", caps.has_package_detail),
            ("Declarative", caps.supports_declarative),
            ("User Packages", caps.supports_user_packages),
            ("System Packages", caps.supports_system_packages),
        ]
        .into_iter()
        .filter_map(|(name, supported)| supported.then_some(name))
        .collect();

        let success = theme.success;

        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(styles::spacing::XS))
            .children(entries.into_iter().map(|name| {
                div()
                    .px(px(styles::spacing::SM))
                    .py(px(3.0))
                    .rounded(px(styles::radius::SM))
                    .bg(success.opacity(0.2))
                    .border_1()
                    .border_color(success.opacity(0.4))
                    .text_size(px(styles::font_size::CAPTION))
                    .child(name.to_string())
            }))
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
