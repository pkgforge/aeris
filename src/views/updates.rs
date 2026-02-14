use iced::{
    Element, Length,
    widget::{column, container, text},
};

use crate::app::message::Message;

pub fn view<'a>() -> Element<'a, Message> {
    container(
        column![
            text("Updates").size(24),
            text("No updates available").size(14),
        ]
        .spacing(12),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
