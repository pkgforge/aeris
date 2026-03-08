use std::collections::{HashMap, HashSet};

use iced::{
    Alignment, Element, Length,
    widget::{
        button, checkbox, column, container, lazy, progress_bar, row, rule, scrollable, space,
        text, text_input,
    },
};

use crate::{
    app::{
        OperationStatus,
        message::{BrowseMessage, Message},
    },
    core::package::Package,
    styles::{self, font_size, spacing},
};
use soar_utils::bytes::format_bytes;

#[derive(Debug, Default)]
pub struct BrowseState {
    pub search_query: String,
    pub search_results: Vec<Package>,
    pub loading: bool,
    pub has_searched: bool,
    pub error: Option<String>,
    pub install_error: Option<String>,
    pub result_version: u64,
    pub installing: Option<String>,
    pub installing_batch_ids: Vec<String>,
    pub selected_package: Option<Package>,
    pub search_debounce_version: u64,
    pub selected: HashSet<String>,
    pub package_progress: HashMap<String, OperationStatus>,
}

pub fn view<'a>(state: &'a BrowseState) -> Element<'a, Message> {
    let search_bar = container(
        text_input("Search packages...", &state.search_query)
            .on_input(|s| Message::Browse(BrowseMessage::SearchQueryChanged(s)))
            .on_submit(Message::Browse(BrowseMessage::SearchSubmit))
            .padding(10)
            .size(font_size::HEADING),
    )
    .style(styles::search_container)
    .padding(spacing::XXS);

    let result_count: Element<'_, Message> = if state.loading {
        text("Searching...").size(font_size::CAPTION + 1.0).into()
    } else if !state.search_results.is_empty() {
        let count = state.search_results.len();
        text(format!(
            "{count} package{} found",
            if count == 1 { "" } else { "s" }
        ))
        .size(font_size::CAPTION + 1.0)
        .into()
    } else {
        text("").size(font_size::CAPTION + 1.0).into()
    };

    let results_content: Element<'_, Message> = if state.loading {
        container(text("Searching...").size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(
            column![
                text("Search failed").size(font_size::HEADING),
                text(err.as_str()).size(font_size::SMALL),
            ]
            .spacing(spacing::SM)
            .align_x(Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
    } else if state.search_results.is_empty() {
        let msg = if state.has_searched {
            "No packages found"
        } else {
            "Type to search for packages"
        };
        container(text(msg).size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let version = state.result_version;
        let results = state.search_results.clone();
        let installing = state.installing.clone();
        let selected = state.selected.clone();
        let package_progress = state.package_progress.clone();
        lazy(("browse", version), move |_| {
            let cards: Vec<Element<'_, Message>> = results
                .iter()
                .map(|pkg| package_card(pkg, &installing, &selected, &package_progress))
                .collect();

            scrollable(column(cards).spacing(spacing::SM).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    let mut browse_list = column![search_bar, result_count]
        .spacing(spacing::SM)
        .width(Length::Fill)
        .height(Length::Fill);

    if let Some(ref err) = state.install_error {
        let error_banner = row![
            text(format!("Install failed: {err}")).size(font_size::SMALL),
            space().width(Length::Fill),
            button(text("Dismiss").size(font_size::CAPTION + 1.0))
                .on_press(Message::Browse(BrowseMessage::DismissInstallError))
                .style(button::secondary)
                .padding([spacing::XXS, 10.0]),
        ]
        .spacing(spacing::SM)
        .align_y(Alignment::Center);

        browse_list = browse_list.push(
            container(error_banner)
                .padding([spacing::SM, spacing::MD])
                .width(Length::Fill)
                .style(styles::error_banner),
        );
    }

    browse_list = browse_list.push(results_content);

    if !state.selected.is_empty() {
        browse_list = browse_list.push(floating_action_bar(
            state.selected.len(),
            "Install",
            Message::Browse(BrowseMessage::InstallSelected),
            Message::Browse(BrowseMessage::ClearSelection),
            false,
        ));
    }

    let browse_panel = container(browse_list)
        .padding(spacing::XL)
        .width(Length::Fill)
        .height(Length::Fill);

    if let Some(ref pkg) = state.selected_package {
        row![browse_panel, rule::vertical(1), detail_side_panel(pkg),]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        browse_panel.into()
    }
}

fn package_card(
    pkg: &Package,
    installing: &Option<String>,
    selected: &HashSet<String>,
    package_progress: &HashMap<String, OperationStatus>,
) -> Element<'static, Message> {
    let pkg_id = pkg.id.clone();
    let is_selected = selected.contains(&pkg.id);
    let name = text(pkg.name.clone()).size(font_size::HEADING);
    let adapter = container(text(pkg.adapter_id.clone()).size(font_size::BADGE))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_adapter(&pkg.adapter_id));

    let mut header = row![name].spacing(spacing::SM).align_y(Alignment::Center);

    if !pkg.version.is_empty() {
        header = header.push(
            container(text(pkg.version.clone()).size(font_size::CAPTION))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_neutral),
        );
    }

    header = header.push(adapter);

    let description = text(
        pkg.description
            .clone()
            .unwrap_or_else(|| "No description".into()),
    )
    .size(font_size::SMALL);

    let mut info_parts: Vec<Element<'_, Message>> = Vec::new();
    if let Some(size) = pkg.size {
        info_parts.push(text(format_bytes(size, 2)).size(font_size::CAPTION).into());
    }
    if let Some(ref license) = pkg.license {
        info_parts.push(text(license.clone()).size(font_size::CAPTION).into());
    }
    let info_row = row(info_parts).spacing(spacing::MD);

    let is_installing = installing.as_deref() == Some(&pkg.id)
        || (installing.as_deref() == Some("__batch__") && package_progress.contains_key(&pkg.name));
    let pkg_status = package_progress.get(&pkg.name);

    let install_btn: Element<'static, Message> = if pkg.installed && pkg.update_available {
        container(text("Update Available").size(font_size::CAPTION))
            .padding([spacing::XXS, 10.0])
            .style(styles::badge_warning)
            .into()
    } else if pkg.installed {
        container(text("Installed").size(font_size::CAPTION))
            .padding([spacing::XXS, 10.0])
            .style(styles::badge_success)
            .into()
    } else if is_installing {
        let label = pkg_status
            .map(|s| s.label())
            .unwrap_or_else(|| "Installing...".into());
        container(text(label).size(font_size::CAPTION))
            .padding([spacing::XXS, 10.0])
            .style(styles::badge_primary)
            .into()
    } else {
        let mut btn = button(text("Install").size(font_size::CAPTION + 1.0).center())
            .padding([spacing::XXS, 14.0])
            .style(button::primary);
        if installing.is_none() {
            btn = btn.on_press(Message::Browse(BrowseMessage::InstallPackage(pkg.clone())));
        }
        btn.into()
    };

    let mut left = column![header, description, info_row]
        .spacing(spacing::XXS)
        .width(Length::Fill);

    // Add inline progress bar when downloading
    if let Some(progress) = pkg_status.and_then(|s| s.progress()) {
        left = left.push(
            container(progress_bar(0.0..=1.0, progress))
                .height(4)
                .width(Length::Fill),
        );
    }

    // Only show checkbox for non-installed packages
    let card_content: Element<'static, Message> = if !pkg.installed {
        let cb_id = pkg.id.clone();
        let cb = checkbox(is_selected)
            .on_toggle(move |_| Message::Browse(BrowseMessage::ToggleSelect(cb_id.clone())));
        row![cb, left, install_btn]
            .spacing(spacing::MD)
            .align_y(Alignment::Center)
            .into()
    } else {
        row![left, install_btn]
            .spacing(spacing::MD)
            .align_y(Alignment::Center)
            .into()
    };

    let card_style = if is_selected {
        styles::card_button_selected
            as fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style
    } else {
        styles::card_button
    };

    button(
        container(card_content)
            .padding(spacing::MD)
            .width(Length::Fill),
    )
    .on_press(Message::Browse(BrowseMessage::SelectPackage(pkg_id)))
    .width(Length::Fill)
    .style(card_style)
    .into()
}

fn detail_side_panel(pkg: &Package) -> Element<'_, Message> {
    let close_btn = button(text("\u{00d7}").size(font_size::TITLE).center())
        .on_press(Message::Browse(BrowseMessage::CloseDetail))
        .style(styles::header_icon_button)
        .padding([spacing::XXS, spacing::SM]);

    let header = row![
        text(pkg.name.clone()).size(font_size::TITLE),
        space().width(Length::Fill),
        close_btn,
    ]
    .align_y(Alignment::Center);

    let description = text(
        pkg.description
            .clone()
            .unwrap_or_else(|| "No description available".into()),
    )
    .size(font_size::BODY);

    let mut details: Vec<Element<'_, Message>> = Vec::new();

    if let Some(ref homepage) = pkg.homepage {
        details.push(detail_row("Homepage", homepage));
    }
    if let Some(ref license) = pkg.license {
        details.push(detail_row("License", license));
    }
    if let Some(size) = pkg.size {
        details.push(detail_row("Size", &format_bytes(size, 2)));
    }
    if let Some(ref category) = pkg.category {
        details.push(detail_row("Category", category));
    }
    if !pkg.tags.is_empty() {
        details.push(detail_row("Tags", &pkg.tags.join(", ")));
    }

    let status_badge: Element<'_, Message> = if pkg.installed {
        container(text("Installed").size(font_size::CAPTION))
            .padding([3.0, spacing::SM])
            .style(styles::badge_success)
            .into()
    } else {
        container(text("Not installed").size(font_size::CAPTION))
            .padding([3.0, spacing::SM])
            .style(styles::badge_neutral)
            .into()
    };

    let install_btn: Element<'_, Message> = if !pkg.installed {
        button(text("Install").size(font_size::SMALL))
            .padding([spacing::XS, spacing::LG])
            .style(button::primary)
            .on_press(Message::Browse(BrowseMessage::InstallPackage(pkg.clone())))
            .into()
    } else {
        space().width(0).into()
    };

    let close_bottom_btn = button(text("Close").size(font_size::SMALL))
        .padding([spacing::XS, spacing::LG])
        .style(button::secondary)
        .on_press(Message::Browse(BrowseMessage::CloseDetail));

    let mut content = column![header].spacing(10).padding(spacing::XL);

    if !pkg.version.is_empty() {
        content = content.push(
            container(text(pkg.version.clone()).size(font_size::CAPTION + 1.0))
                .padding([3.0, spacing::SM])
                .style(styles::badge_primary),
        );
    }

    content = content.push(description);
    content = content.push(rule::horizontal(1));

    for detail in details {
        content = content.push(detail);
    }

    content = content.push(
        row![
            text("Status").size(font_size::SMALL).width(100),
            status_badge
        ]
        .spacing(spacing::SM)
        .align_y(Alignment::Center),
    );

    content = content.push(rule::horizontal(1));

    content = content.push(
        row![space().width(Length::Fill), install_btn, close_bottom_btn]
            .spacing(spacing::SM)
            .align_y(Alignment::Center),
    );

    container(scrollable(content).height(Length::Fill))
        .style(styles::detail_panel)
        .width(320)
        .height(Length::Fill)
        .into()
}

fn detail_row<'a>(label: &str, value: &str) -> Element<'a, Message> {
    row![
        text(label.to_string()).size(font_size::SMALL).width(100),
        text(value.to_string()).size(font_size::SMALL),
    ]
    .spacing(spacing::SM)
    .into()
}

pub fn floating_action_bar<'a>(
    count: usize,
    action_label: &str,
    action_msg: Message,
    clear_msg: Message,
    is_danger: bool,
) -> Element<'a, Message> {
    let label = text(format!("{count} selected")).size(font_size::BODY);

    let clear_btn = button(text("Clear").size(font_size::CAPTION + 1.0).center())
        .on_press(clear_msg)
        .style(button::secondary)
        .padding([spacing::XS, 14.0]);

    let action_btn = button(
        text(format!("{action_label} {count}"))
            .size(font_size::CAPTION + 1.0)
            .center(),
    )
    .on_press(action_msg)
    .padding([spacing::XS, 14.0]);

    let action_btn = if is_danger {
        action_btn.style(button::danger)
    } else {
        action_btn.style(button::primary)
    };

    container(
        row![label, space().width(Length::Fill), clear_btn, action_btn]
            .spacing(spacing::SM)
            .align_y(Alignment::Center)
            .padding([spacing::SM, spacing::LG]),
    )
    .width(Length::Fill)
    .style(styles::progress_container)
    .into()
}
