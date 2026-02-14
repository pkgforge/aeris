use iced::{
    Element, Length,
    widget::{button, column, container, row, text},
};

use crate::app::View;
use crate::app::message::{InstalledMessage, Message, UpdatesMessage};

pub struct DashboardStats {
    pub installed_count: usize,
    pub repo_count: usize,
}

pub fn view<'a>(stats: &DashboardStats) -> Element<'a, Message> {
    let header = text("Dashboard").size(20);

    let stat_cards = row![
        stat_card(
            "Installed",
            &stats.installed_count.to_string(),
            "View",
            Message::NavigateTo(View::Installed)
        ),
        stat_card(
            "Repositories",
            &stats.repo_count.to_string(),
            "Browse",
            Message::NavigateTo(View::Browse)
        ),
    ]
    .spacing(12)
    .width(Length::Fill);

    let quick_actions = column![
        text("Quick Actions").size(16),
        row![
            button(text("Search Packages").size(13).center())
                .padding([8, 16])
                .style(button::primary)
                .on_press(Message::NavigateTo(View::Browse)),
            button(text("Refresh Installed").size(13).center())
                .padding([8, 16])
                .style(button::secondary)
                .on_press(Message::Installed(InstalledMessage::Refresh)),
            button(text("Check Updates").size(13).center())
                .padding([8, 16])
                .style(button::secondary)
                .on_press(Message::Updates(UpdatesMessage::CheckUpdates)),
        ]
        .spacing(8),
    ]
    .spacing(8);

    container(
        column![header, stat_cards, quick_actions]
            .spacing(20)
            .width(Length::Fill),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn stat_card<'a>(
    label: &str,
    value: &str,
    action_label: &str,
    action: Message,
) -> Element<'a, Message> {
    container(
        column![
            text(value.to_string()).size(28),
            text(label.to_string()).size(13),
            button(text(action_label.to_string()).size(12).center())
                .padding([4, 12])
                .style(button::text)
                .on_press(action),
        ]
        .spacing(4)
        .align_x(iced::Alignment::Center)
        .width(Length::Fill),
    )
    .padding(16)
    .width(Length::Fill)
    .style(container::bordered_box)
    .into()
}
