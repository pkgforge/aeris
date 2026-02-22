use iced::{
    Element, Length,
    widget::{column, container, row, rule, scrollable, text},
};

use crate::{
    app::message::Message,
    core::{adapter::AdapterInfo, capabilities::Capabilities},
    styles::{self, font_size, spacing},
};

pub fn view(adapters: Vec<AdapterInfo>) -> Element<'static, Message> {
    let header = text("Adapters").size(font_size::TITLE);

    let mut content = column![header].spacing(spacing::LG).width(Length::Fill);

    for info in adapters {
        content = content.push(adapter_card(info));
        content = content.push(rule::horizontal(1));
    }

    container(scrollable(content).height(Length::Fill))
        .padding(spacing::XL)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn adapter_card(info: AdapterInfo) -> Element<'static, Message> {
    let name_row = row![
        text("Name").size(font_size::BODY).width(120),
        text(info.name.clone()).size(font_size::BODY),
    ]
    .spacing(spacing::SM);

    let version_row = row![
        text("Version").size(font_size::BODY).width(120),
        text(info.version.clone()).size(font_size::BODY),
    ]
    .spacing(spacing::SM);

    let mut type_row = row![text("Type").size(font_size::BODY).width(120)].spacing(spacing::SM);
    if info.is_builtin {
        type_row = type_row.push(
            container(text("Built-in").size(font_size::CAPTION))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_primary),
        );
    } else {
        type_row = type_row.push(
            container(text("Plugin").size(font_size::CAPTION))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_neutral),
        );
    }

    let desc_row = row![
        text("Description").size(font_size::BODY).width(120),
        text(info.description.clone()).size(font_size::BODY),
    ]
    .spacing(spacing::SM);

    let info_card =
        container(column![name_row, version_row, type_row, desc_row].spacing(spacing::SM))
            .padding(spacing::LG)
            .width(Length::Fill)
            .style(styles::card);

    let caps_header = text("Capabilities").size(font_size::HEADING);
    let caps_view = capabilities_view(info.capabilities);

    column![info_card, caps_header, caps_view]
        .spacing(spacing::SM)
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
