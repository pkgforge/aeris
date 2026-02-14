use iced::{
    Element, Length,
    widget::{column, container, row, rule, scrollable, text},
};

use crate::{
    app::message::Message,
    core::{adapter::AdapterInfo, capabilities::Capabilities},
};

pub fn view(info: &AdapterInfo) -> Element<'_, Message> {
    let header = text("Adapter Info").size(22);

    let name_row = row![text("Name").size(14).width(120), text(&info.name).size(14),].spacing(8);

    let version_row = row![
        text("Version").size(14).width(120),
        text(&info.version).size(14),
    ]
    .spacing(8);

    let mut type_row = row![text("Type").size(14).width(120)].spacing(8);
    if info.is_builtin {
        type_row = type_row.push(
            container(text("Built-in").size(11))
                .padding([2, 6])
                .style(container::bordered_box),
        );
    } else {
        type_row = type_row.push(text("Plugin").size(14));
    }

    let desc_row = row![
        text("Description").size(14).width(120),
        text(&info.description).size(14),
    ]
    .spacing(8);

    let info_card = column![name_row, version_row, type_row, desc_row].spacing(8);

    let caps_header = text("Capabilities").size(16);
    let caps_view = capabilities_view(&info.capabilities);

    let content = column![
        header,
        container(info_card)
            .padding(12)
            .width(Length::Fill)
            .style(container::bordered_box),
        rule::horizontal(1),
        caps_header,
        caps_view,
    ]
    .spacing(16)
    .width(Length::Fill);

    container(scrollable(content).height(Length::Fill))
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn capabilities_view(caps: &Capabilities) -> Element<'_, Message> {
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

    // Wrap badges into rows of ~6
    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    let mut current_row: Vec<Element<'_, Message>> = Vec::new();
    for badge in badges {
        current_row.push(badge);
        if current_row.len() >= 6 {
            rows.push(row(std::mem::take(&mut current_row)).spacing(6).into());
        }
    }
    if !current_row.is_empty() {
        rows.push(row(current_row).spacing(6).into());
    }

    column(rows).spacing(6).into()
}

fn capability_badge<'a>(name: &str, supported: bool) -> Element<'a, Message> {
    let label = text(name.to_string()).size(11);

    let style = if supported {
        cap_supported_style
    } else {
        cap_unsupported_style
    };

    container(label).padding([3, 8]).style(style).into()
}

fn cap_supported_style(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.success.weak.color.into()),
        border: iced::Border {
            radius: 4.0.into(),
            width: 1.0,
            color: palette.success.base.color,
        },
        text_color: Some(palette.success.strong.color),
        ..Default::default()
    }
}

fn cap_unsupported_style(theme: &iced::Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: iced::Border {
            radius: 4.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        text_color: Some(palette.background.strong.color),
        ..Default::default()
    }
}
