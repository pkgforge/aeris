use iced::{
    Element, Length,
    widget::{
        button, column, container, lazy, mouse_area, row, rule, scrollable, text, text_input,
    },
};

use crate::{
    app::message::{BrowseMessage, Message},
    core::package::Package,
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
}

pub fn view<'a>(state: &'a BrowseState) -> Element<'a, Message> {
    let search_bar = text_input("Search packages...", &state.search_query)
        .on_input(|s| Message::Browse(BrowseMessage::SearchQueryChanged(s)))
        .on_submit(Message::Browse(BrowseMessage::SearchSubmit))
        .padding(10)
        .size(16);

    let results_content: Element<'_, Message> = if state.loading {
        container(text("Searching...").size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(text(format!("Search failed: {err}")).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if state.search_results.is_empty() {
        let msg = if state.has_searched {
            "No packages found"
        } else {
            "Search for packages above"
        };
        container(text(msg).size(14))
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

            scrollable(column(cards).spacing(8).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    let mut content = column![row![search_bar].width(Length::Fill)]
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Fill);

    if let Some(ref err) = state.install_error {
        let error_banner = row![
            text(format!("Install failed: {err}")).size(13),
            button(text("Dismiss").size(12))
                .on_press(Message::Browse(BrowseMessage::DismissInstallError))
                .style(button::secondary)
                .padding([2, 8]),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center);

        content = content.push(
            container(error_banner)
                .padding([6, 12])
                .width(Length::Fill)
                .style(|theme: &iced::Theme| {
                    let palette = theme.extended_palette();
                    container::Style {
                        background: Some(palette.danger.weak.color.into()),
                        border: iced::Border {
                            width: 1.0,
                            color: palette.danger.base.color,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                }),
        );
    }

    content = content.push(results_content);

    container(content)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn package_card(pkg: &Package, installing: &Option<String>) -> Element<'static, Message> {
    let pkg_id = pkg.id.clone();
    let name = text(pkg.name.clone()).size(16);
    let version = text(pkg.version.clone()).size(12);
    let adapter = text(format!("[{}]", pkg.adapter_id)).size(11);

    let header = row![name, version, adapter]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let description = text(
        pkg.description
            .clone()
            .unwrap_or_else(|| "No description".into()),
    )
    .size(13);

    let mut info_parts: Vec<Element<'_, Message>> = Vec::new();
    if let Some(size) = pkg.size {
        info_parts.push(text(format_bytes(size, 2)).size(11).into());
    }
    if let Some(ref license) = pkg.license {
        info_parts.push(text(license.clone()).size(11).into());
    }
    let info_row = row(info_parts).spacing(12);

    let is_installing = installing.as_deref() == Some(&pkg.id);
    let install_btn = if pkg.installed && pkg.update_available {
        button(text("Update Available").size(12).center())
            .padding([4, 12])
            .style(button::secondary)
    } else if pkg.installed {
        button(text("Installed").size(12).center())
            .padding([4, 12])
            .style(button::secondary)
    } else if is_installing {
        button(text("Installing...").size(12).center())
            .padding([4, 12])
            .style(button::secondary)
    } else {
        let mut btn = button(text("Install").size(12).center())
            .padding([4, 12])
            .style(button::primary);
        if installing.is_none() {
            btn = btn.on_press(Message::Browse(BrowseMessage::InstallPackage(pkg.clone())));
        }
        btn
    };

    let left = column![header, description, info_row]
        .spacing(4)
        .width(Length::Fill);

    let card = row![left, install_btn]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    let card_container: Element<'static, Message> = container(card)
        .padding(12)
        .width(Length::Fill)
        .style(container::bordered_box)
        .into();

    mouse_area(card_container)
        .on_press(Message::Browse(BrowseMessage::SelectPackage(pkg_id)))
        .into()
}

pub fn package_detail_view(pkg: &Package) -> Element<'_, Message> {
    let name = text(pkg.name.clone()).size(20);
    let version = text(format!("v{}", pkg.version)).size(14);
    let header = row![name, version]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let description = text(
        pkg.description
            .clone()
            .unwrap_or_else(|| "No description available".into()),
    )
    .size(14);

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

    let status = if pkg.installed {
        "Installed"
    } else {
        "Not installed"
    };
    details.push(detail_row("Status", status));

    let close_btn = button(text("Close").size(13))
        .padding([6, 14])
        .style(button::secondary)
        .on_press(Message::Browse(BrowseMessage::CloseDetail));

    let mut content = column![header, description, rule::horizontal(1)]
        .spacing(10)
        .padding(24);

    for detail in details {
        content = content.push(detail);
    }

    content = content.push(
        row![iced::widget::space().width(Length::Fill), close_btn].align_y(iced::Alignment::Center),
    );

    container(content)
        .style(container::rounded_box)
        .width(400)
        .into()
}

fn detail_row<'a>(label: &str, value: &str) -> Element<'a, Message> {
    row![
        text(label.to_string()).size(13).width(100),
        text(value.to_string()).size(13),
    ]
    .spacing(8)
    .into()
}
