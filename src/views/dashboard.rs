use iced::{
    Element, Length,
    widget::{column, container, text},
};

use crate::app::message::Message;

pub fn view<'a>() -> Element<'a, Message> {
    container(
        column![text("Dashboard").size(24), text("Overview coming soon").size(14),].spacing(12),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
