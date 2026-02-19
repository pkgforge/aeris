use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, space, text},
};

use crate::app::View;
use crate::app::message::{InstalledMessage, Message, RepositoriesMessage, UpdatesMessage};
use crate::core::privilege::PackageMode;
use crate::styles::{self, font_size, spacing};

pub struct DashboardStats {
    pub installed_count: usize,
    pub update_count: usize,
    pub updates_checked: bool,
    pub repo_count: usize,
    pub current_mode: PackageMode,
    pub unhealthy_count: usize,
}

pub fn view<'a>(stats: &DashboardStats) -> Element<'a, Message> {
    let mode_label = match stats.current_mode {
        PackageMode::User => "User",
        PackageMode::System => "System",
    };

    let welcome = column![
        text("Dashboard").size(font_size::TITLE),
        text(format!("Managing {mode_label} packages")).size(font_size::SMALL),
    ]
    .spacing(spacing::XXS);

    let installed_label = match stats.current_mode {
        PackageMode::User => "Installed (User)",
        PackageMode::System => "Installed (System)",
    };

    let update_value = if stats.updates_checked {
        if stats.update_count == 0 {
            "Up to date".to_string()
        } else {
            stats.update_count.to_string()
        }
    } else {
        "?".to_string()
    };

    let stat_cards = row![
        stat_card_button(
            installed_label,
            &stats.installed_count.to_string(),
            Message::NavigateTo(View::Installed),
        ),
        stat_card_button("Updates", &update_value, Message::NavigateTo(View::Updates),),
        stat_card_button(
            "Repositories",
            &stats.repo_count.to_string(),
            Message::NavigateTo(View::Repositories),
        ),
    ]
    .spacing(spacing::MD)
    .width(Length::Fill);

    let quick_actions = column![
        text("Quick Actions").size(font_size::HEADING),
        row![
            button(text("Search").size(font_size::SMALL).center())
                .padding([spacing::SM, spacing::LG])
                .style(styles::outlined_button)
                .on_press(Message::NavigateTo(View::Browse)),
            button(text("Refresh").size(font_size::SMALL).center())
                .padding([spacing::SM, spacing::LG])
                .style(styles::outlined_button)
                .on_press(Message::Installed(InstalledMessage::Refresh)),
            button(text("Check Updates").size(font_size::SMALL).center())
                .padding([spacing::SM, spacing::LG])
                .style(styles::outlined_button)
                .on_press(Message::Updates(UpdatesMessage::CheckUpdates)),
            button(text("Sync Repos").size(font_size::SMALL).center())
                .padding([spacing::SM, spacing::LG])
                .style(styles::outlined_button)
                .on_press(Message::Repositories(RepositoriesMessage::SyncAll)),
        ]
        .spacing(spacing::SM),
    ]
    .spacing(10);

    let mut content = column![welcome, stat_cards, quick_actions]
        .spacing(spacing::XXL)
        .width(Length::Fill);

    if stats.unhealthy_count > 0 {
        let health_warning = container(
            row![
                text(format!(
                    "\u{26a0} {} package(s) with issues",
                    stats.unhealthy_count
                ))
                .size(font_size::SMALL),
                space().width(Length::Fill),
                button(text("View").size(font_size::CAPTION + 1.0).center())
                    .padding([spacing::XXS, spacing::MD])
                    .style(button::secondary)
                    .on_press(Message::NavigateTo(View::Installed)),
            ]
            .spacing(spacing::SM)
            .align_y(Alignment::Center),
        )
        .padding([spacing::SM, spacing::MD])
        .width(Length::Fill)
        .style(styles::error_banner);

        content = content.push(health_warning);
    }

    container(content)
        .padding(spacing::XL)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn stat_card_button<'a>(label: &str, value: &str, on_press: Message) -> Element<'a, Message> {
    let accent_bar = container(space().width(4).height(Length::Fill))
        .style(styles::stat_card_accent_left)
        .height(Length::Fill);

    let card_content = column![
        text(value.to_string()).size(font_size::DISPLAY),
        text(label.to_string()).size(font_size::SMALL),
    ]
    .spacing(spacing::XXS)
    .align_x(Alignment::Center)
    .width(Length::Fill);

    button(
        container(
            row![accent_bar, container(card_content).padding(spacing::LG)]
                .height(Length::Shrink)
                .width(Length::Fill),
        )
        .width(Length::Fill),
    )
    .on_press(on_press)
    .width(Length::Fill)
    .style(styles::card_button)
    .into()
}
