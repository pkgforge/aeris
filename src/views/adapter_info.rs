use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, lazy, row, rule, scrollable, space, text, toggler},
};

use crate::{
    app::{
        AdapterViewState,
        message::{AdapterMessage, Message, RepoInfo, RepositoriesMessage},
    },
    core::{
        adapter::AdapterInfo, capabilities::Capabilities, privilege::PackageMode,
        registry::PluginEntry,
    },
    styles::{self, font_size, spacing},
};

pub fn view(
    state: &AdapterViewState,
    adapters: Vec<(AdapterInfo, bool)>,
    mode: PackageMode,
) -> Element<'_, Message> {
    let header = text("Adapters").size(font_size::TITLE);

    let mut content = column![header].spacing(spacing::LG).width(Length::Fill);

    // Installed adapters section
    let section_header = text("Installed Adapters").size(font_size::HEADING);
    content = content.push(section_header);

    let installed_ids: Vec<String> = adapters.iter().map(|(info, _)| info.id.clone()).collect();

    for (info, enabled) in adapters {
        let has_repos = info.capabilities.can_list_repos && enabled;
        content = content.push(adapter_card(info, enabled));

        // Show repos section inline for adapters that support it
        if has_repos {
            content = content.push(repos_section(state, mode));
        }

        content = content.push(rule::horizontal(1));
    }

    // Available plugins section
    content = content.push(rule::horizontal(2));
    let plugins_header = text("Available Plugins").size(font_size::HEADING);
    content = content.push(plugins_header);

    if state.registry_plugins.is_empty() && !state.registry_loading {
        let mut fetch_row = row![].spacing(spacing::SM).align_y(Alignment::Center);
        let fetch_btn = button(text("Fetch Plugins").size(font_size::BODY))
            .on_press(Message::Adapter(AdapterMessage::FetchRegistry))
            .style(button::primary)
            .padding([spacing::SM, spacing::LG]);
        fetch_row = fetch_row.push(fetch_btn);
        if let Some(ref err) = state.registry_error {
            fetch_row = fetch_row.push(text(err.clone()).size(font_size::SMALL));
        }
        content = content.push(fetch_row);
    } else if state.registry_loading {
        content = content.push(text("Fetching plugin registry...").size(font_size::BODY));
    } else {
        let mut has_available = false;

        for entry in &state.registry_plugins {
            if installed_ids.iter().any(|id| id == &entry.id) {
                continue;
            }
            has_available = true;
            let is_installing = state.installing_plugin.as_deref() == Some(&entry.id);
            content = content.push(registry_card(entry.clone(), is_installing));
        }

        if !has_available {
            content =
                content.push(text("All available plugins are installed.").size(font_size::BODY));
        }

        let refresh_btn = button(text("Refresh").size(font_size::SMALL))
            .on_press(Message::Adapter(AdapterMessage::FetchRegistry))
            .style(button::secondary)
            .padding([spacing::XS, spacing::SM]);
        content = content.push(refresh_btn);

        if let Some(ref err) = state.registry_error {
            content = content.push(text(err.clone()).size(font_size::SMALL));
        }
    }

    container(scrollable(content.padding(spacing::XL)).height(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn repos_section<'a>(state: &'a AdapterViewState, mode: PackageMode) -> Element<'a, Message> {
    let title = match mode {
        PackageMode::User => "Repositories (User)",
        PackageMode::System => "Repositories (System)",
    };
    let mut header_row = row![text(title).size(font_size::HEADING)].spacing(spacing::SM);

    let sync_all_btn = if state.syncing.is_some() {
        button(text("Syncing...").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::secondary)
    } else {
        button(text("Sync All").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::SyncAll))
    };

    let refresh_btn = if state.repos_loading {
        button(text("Loading...").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::secondary)
    } else {
        button(text("Refresh").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::secondary)
            .on_press(Message::Repositories(RepositoriesMessage::Refresh))
    };

    header_row = header_row
        .push(space().width(Length::Fill))
        .push(sync_all_btn)
        .push(refresh_btn)
        .align_y(Alignment::Center);

    let repos_content: Element<'_, Message> = if state.repos_loading {
        text("Loading repositories...").size(font_size::BODY).into()
    } else if let Some(ref err) = state.repos_error {
        text(format!("Failed to load: {err}"))
            .size(font_size::BODY)
            .into()
    } else if state.repositories.is_empty() {
        text("No repositories configured")
            .size(font_size::BODY)
            .into()
    } else {
        let version = state.repos_version;
        let repos = state.repositories.clone();
        let syncing = state.syncing.clone();
        lazy(("repos", version), move |_| {
            let cards: Vec<Element<'_, Message>> =
                repos.iter().map(|repo| repo_card(repo, &syncing)).collect();
            column(cards).spacing(spacing::SM).width(Length::Fill)
        })
        .into()
    };

    let mut section = column![header_row, repos_content]
        .spacing(spacing::MD)
        .width(Length::Fill);

    if let Some(ref err) = state.sync_error {
        section = section.push(
            container(text(format!("Sync error: {err}")).size(font_size::CAPTION + 1.0))
                .padding([spacing::XS, spacing::MD])
                .width(Length::Fill)
                .style(styles::error_banner),
        );
    }

    container(section)
        .padding([spacing::MD, spacing::LG])
        .width(Length::Fill)
        .into()
}

fn repo_card(repo: &RepoInfo, syncing: &Option<String>) -> Element<'static, Message> {
    let name = text(repo.name.clone()).size(font_size::HEADING);
    let url = text(repo.url.clone()).size(font_size::CAPTION + 1.0);

    let header = row![name].spacing(spacing::SM).align_y(Alignment::Center);

    let mut tags: Vec<Element<'_, Message>> = Vec::new();

    if repo.enabled {
        tags.push(
            container(text("Enabled").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_success)
                .into(),
        );
    } else {
        tags.push(
            container(text("Disabled").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_danger)
                .into(),
        );
    }
    if repo.desktop_integration {
        tags.push(
            container(text("Desktop").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_neutral)
                .into(),
        );
    }
    if repo.has_pubkey {
        tags.push(
            container(text("Signed").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_primary)
                .into(),
        );
    }
    if repo.signature_verification {
        tags.push(
            container(text("Verified").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_primary)
                .into(),
        );
    }
    if let Some(ref interval) = repo.sync_interval {
        tags.push(
            container(text(format!("Sync: {interval}")).size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_neutral)
                .into(),
        );
    }

    let tags_row = row(tags).spacing(spacing::XS);

    let is_syncing =
        syncing.as_deref() == Some(&repo.name) || syncing.as_deref() == Some("__all__");

    let toggle_btn = if syncing.is_some() {
        if repo.enabled {
            button(text("Disable").size(font_size::CAPTION + 1.0))
                .padding([spacing::XXS, 10.0])
                .style(button::secondary)
        } else {
            button(text("Enable").size(font_size::CAPTION + 1.0))
                .padding([spacing::XXS, 10.0])
                .style(button::secondary)
        }
    } else if repo.enabled {
        button(text("Disable").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::secondary)
            .on_press(Message::Repositories(RepositoriesMessage::ToggleEnabled(
                repo.name.clone(),
                false,
            )))
    } else {
        button(text("Enable").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::ToggleEnabled(
                repo.name.clone(),
                true,
            )))
    };

    let sync_btn = if is_syncing {
        button(text("Syncing...").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::secondary)
    } else if syncing.is_some() {
        button(text("Sync").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::secondary)
    } else {
        button(text("Sync").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::SyncRepo(
                repo.name.clone(),
            )))
    };

    let left = column![header, url, tags_row]
        .spacing(spacing::XXS)
        .width(Length::Fill);

    let card_content = row![left, toggle_btn, sync_btn]
        .spacing(spacing::MD)
        .align_y(Alignment::Center);

    button(
        container(card_content)
            .padding(spacing::MD)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .style(styles::card_button)
    .into()
}

fn adapter_card(info: AdapterInfo, enabled: bool) -> Element<'static, Message> {
    let id = info.id.clone();

    let name_row = row![
        text(info.name.clone()).size(font_size::BODY),
        text(format!("v{}", info.version)).size(font_size::SMALL),
    ]
    .spacing(spacing::SM)
    .align_y(Alignment::Center);

    let type_badge: Element<'_, Message> = if info.is_builtin {
        container(text("Built-in").size(font_size::CAPTION))
            .padding([spacing::XXXS, spacing::XS])
            .style(styles::badge_primary)
            .into()
    } else {
        container(text("Plugin").size(font_size::CAPTION))
            .padding([spacing::XXXS, spacing::XS])
            .style(styles::badge_neutral)
            .into()
    };

    let header_row = row![name_row, type_badge]
        .spacing(spacing::SM)
        .align_y(Alignment::Center);

    let desc = text(info.description.clone()).size(font_size::SMALL);

    let caps_view = capabilities_view(info.capabilities);

    let toggle = toggler(enabled).on_toggle({
        let id = id.clone();
        move |v| Message::Adapter(AdapterMessage::ToggleAdapter(id.clone(), v))
    });

    let mut actions = row![toggle].spacing(spacing::SM).align_y(Alignment::Center);

    if !info.is_builtin {
        let remove_btn = button(text("Remove").size(font_size::SMALL))
            .on_press(Message::Adapter(AdapterMessage::RemovePlugin(id)))
            .style(button::danger)
            .padding([spacing::XS, spacing::SM]);
        actions = actions.push(remove_btn);
    }

    let card_content = column![header_row, desc, caps_view, actions].spacing(spacing::SM);

    container(card_content)
        .padding(spacing::LG)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn registry_card(entry: PluginEntry, installing: bool) -> Element<'static, Message> {
    let header = row![
        text(entry.name.clone()).size(font_size::BODY),
        text(format!("v{}", entry.version)).size(font_size::SMALL),
    ]
    .spacing(spacing::SM)
    .align_y(Alignment::Center);

    let desc = text(entry.description.clone()).size(font_size::SMALL);

    let action: Element<'_, Message> = if installing {
        text("Installing...").size(font_size::SMALL).into()
    } else {
        button(text("Install").size(font_size::SMALL))
            .on_press(Message::Adapter(AdapterMessage::InstallPlugin(entry)))
            .style(button::primary)
            .padding([spacing::XS, spacing::SM])
            .into()
    };

    let card_content = column![header, desc, action].spacing(spacing::SM);

    container(card_content)
        .padding(spacing::LG)
        .width(Length::Fill)
        .style(styles::card)
        .into()
}

fn capabilities_view(caps: Capabilities) -> Element<'static, Message> {
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

    let badges: Vec<Element<'_, Message>> = entries
        .into_iter()
        .map(|(name, supported)| capability_badge(name, supported))
        .collect();

    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    let mut current_row: Vec<Element<'_, Message>> = Vec::new();
    for badge in badges {
        current_row.push(badge);
        if current_row.len() >= 6 {
            rows.push(
                row(std::mem::take(&mut current_row))
                    .spacing(spacing::XS)
                    .into(),
            );
        }
    }
    if !current_row.is_empty() {
        rows.push(row(current_row).spacing(spacing::XS).into());
    }

    column(rows).spacing(spacing::XS).into()
}

fn capability_badge<'a>(name: &str, supported: bool) -> Element<'a, Message> {
    let label = text(name.to_string()).size(font_size::CAPTION);

    let style: fn(&iced::Theme) -> container::Style = if supported {
        styles::badge_success
    } else {
        styles::badge_neutral
    };

    container(label)
        .padding([3.0, spacing::SM])
        .style(style)
        .into()
}
