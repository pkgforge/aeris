use iced::{
    Element, Length,
    widget::{column, container, row, text, text_input},
};

use crate::app::message::{BrowseMessage, Message};

#[derive(Debug, Default)]
pub struct BrowseState {
    pub search_query: String,
    pub loading: bool,
}

pub fn view<'a>(state: &'a BrowseState) -> Element<'a, Message> {
    let search_bar = text_input("Search packages...", &state.search_query)
        .on_input(|s| Message::Browse(BrowseMessage::SearchQueryChanged(s)))
        .on_submit(Message::Browse(BrowseMessage::SearchSubmit))
        .padding(10)
        .size(16);

    let content = if state.loading {
        container(text("Searching...").size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
    } else {
        container(text("Search for packages above").size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
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
