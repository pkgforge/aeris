use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, lazy, row, rule, scrollable, space, text, text_input},
};

use crate::{
    app::message::{BrowseMessage, Message},
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
    pub selected_package: Option<Package>,
    pub search_debounce_version: u64,
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
        lazy(("browse", version), move |_| {
            let cards: Vec<Element<'_, Message>> = results
                .iter()
                .map(|pkg| package_card(pkg, &installing))
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

fn package_card(pkg: &Package, installing: &Option<String>) -> Element<'static, Message> {
    let pkg_id = pkg.id.clone();
    let name = text(pkg.name.clone()).size(font_size::HEADING);
    let version = container(text(pkg.version.clone()).size(font_size::CAPTION))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_neutral);
    let adapter = container(text(pkg.adapter_id.clone()).size(font_size::BADGE))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_neutral);

    let header = row![name, version, adapter]
        .spacing(spacing::SM)
        .align_y(Alignment::Center);

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

    let is_installing = installing.as_deref() == Some(&pkg.id);
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
        container(text("Installing...").size(font_size::CAPTION))
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

    let left = column![header, description, info_row]
        .spacing(spacing::XXS)
        .width(Length::Fill);

    let card_content = row![left, install_btn]
        .spacing(spacing::MD)
        .align_y(Alignment::Center);

    button(
        container(card_content)
            .padding(spacing::MD)
            .width(Length::Fill),
    )
    .on_press(Message::Browse(BrowseMessage::SelectPackage(pkg_id)))
    .width(Length::Fill)
    .style(styles::card_button)
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

    let version_badge = container(text(format!("v{}", pkg.version)).size(font_size::CAPTION + 1.0))
        .padding([3.0, spacing::SM])
        .style(styles::badge_primary);

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

    let mut content = column![header, version_badge, description, rule::horizontal(1)]
        .spacing(10)
        .padding(spacing::XL);

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
