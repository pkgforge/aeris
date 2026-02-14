use iced::{
    Element, Length,
    widget::{button, column, container, lazy, row, scrollable, text, text_input},
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
    pub result_version: u64,
}

pub fn view<'a>(state: &'a BrowseState) -> Element<'a, Message> {
    let search_bar = text_input("Search packages...", &state.search_query)
        .on_input(|s| Message::Browse(BrowseMessage::SearchQueryChanged(s)))
        .on_submit(Message::Browse(BrowseMessage::SearchSubmit))
        .padding(10)
        .size(16);

    let content: Element<'_, Message> = if state.loading {
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
        lazy(version, move |_| {
            let cards: Vec<Element<'_, Message>> = results
                .iter()
                .map(|pkg| package_card(pkg))
                .collect();

            scrollable(column(cards).spacing(8).width(Length::Fill))
                .height(Length::Fill)
        })
        .into()
    };

    container(
        column![row![search_bar].width(Length::Fill), content,]
            .spacing(12)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn package_card(pkg: &Package) -> Element<'static, Message> {
    let name = text(pkg.name.clone()).size(16);
    let version = text(pkg.version.clone()).size(12);
    let adapter = text(format!("[{}]", pkg.adapter_id)).size(11);

    let header = row![name, version, adapter].spacing(8).align_y(iced::Alignment::Center);

    let description =
        text(pkg.description.clone().unwrap_or_else(|| "No description".into())).size(13);

    let mut info_parts: Vec<Element<'_, Message>> = Vec::new();
    if let Some(size) = pkg.size {
        info_parts.push(text(format_bytes(size, 2)).size(11).into());
    }
    if let Some(ref license) = pkg.license {
        info_parts.push(text(license.clone()).size(11).into());
    }
    let info_row = row(info_parts).spacing(12);

    let install_btn = if pkg.installed {
        button(text("Installed").size(12).center())
            .padding([4, 12])
            .style(button::secondary)
    } else {
        button(text("Install").size(12).center())
            .padding([4, 12])
            .style(button::primary)
            .on_press(Message::Browse(BrowseMessage::InstallPackage(pkg.clone())))
    };

    let left = column![header, description, info_row]
        .spacing(4)
        .width(Length::Fill);

    let card = row![left, install_btn]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    container(card)
        .padding(12)
        .width(Length::Fill)
        .style(container::bordered_box)
        .into()
}


