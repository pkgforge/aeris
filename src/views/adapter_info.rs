use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, rule, scrollable, text, toggler},
};

use crate::{
    app::{
        AdapterViewState,
        message::{AdapterMessage, Message},
    },
    core::{adapter::AdapterInfo, capabilities::Capabilities, registry::PluginEntry},
    styles::{self, font_size, spacing},
};

pub fn view(state: &AdapterViewState, adapters: Vec<(AdapterInfo, bool)>) -> Element<'_, Message> {
    let header = text("Adapters").size(font_size::TITLE);

    let mut content = column![header].spacing(spacing::LG).width(Length::Fill);

    // Installed adapters section
    let section_header = text("Installed Adapters").size(font_size::HEADING);
    content = content.push(section_header);

    let installed_ids: Vec<String> = adapters.iter().map(|(info, _)| info.id.clone()).collect();

    for (info, enabled) in adapters {
        content = content.push(adapter_card(info, enabled));
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
