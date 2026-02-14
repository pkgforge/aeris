use iced::{
    Element, Length,
    widget::{button, column, container, lazy, row, scrollable, space, text},
};

use crate::{
    app::message::{Message, UpdatesMessage},
    core::package::Update,
};
use soar_utils::bytes::format_bytes;

#[derive(Debug, Default)]
pub struct UpdatesState {
    pub updates: Vec<Update>,
    pub loading: bool,
    pub checked: bool,
    pub error: Option<String>,
    pub result_version: u64,
    pub updating: Option<String>,
}

pub fn view<'a>(state: &'a UpdatesState) -> Element<'a, Message> {
    let mut header_row = row![text("Updates").size(20), space().width(Length::Fill),]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

    let is_busy = state.updating.is_some() || state.loading;

    if !state.updates.is_empty() {
        let mut update_all_btn = button(text("Update All").size(12).center())
            .padding([6, 14])
            .style(button::primary);
        if !is_busy {
            update_all_btn = update_all_btn.on_press(Message::Updates(UpdatesMessage::UpdateAll));
        }
        header_row = header_row.push(update_all_btn);
    }

    let mut check_btn = button(text("Check").size(12).center())
        .padding([6, 14])
        .style(button::secondary);
    if !is_busy {
        check_btn = check_btn.on_press(Message::Updates(UpdatesMessage::CheckUpdates));
    }
    header_row = header_row.push(check_btn);

    let content: Element<'_, Message> = if state.loading {
        container(text("Checking for updates...").size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(text(format!("Failed: {err}")).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if state.updates.is_empty() {
        let msg = if state.checked {
            "All packages are up to date"
        } else {
            "Click Check to look for updates"
        };
        container(text(msg).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let version = state.result_version;
        let updates = state.updates.clone();
        let updating = state.updating.clone();
        lazy(version, move |_| {
            let cards: Vec<Element<'_, Message>> =
                updates.iter().map(|u| update_card(u, &updating)).collect();

            scrollable(column(cards).spacing(8).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    container(
        column![header_row, content]
            .spacing(12)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn update_card(update: &Update, updating: &Option<String>) -> Element<'static, Message> {
    let name = text(update.package.name.clone()).size(16);
    let version_info = text(format!(
        "{} → {}",
        update.current_version, update.new_version
    ))
    .size(12);

    let header = row![name, version_info]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let mut info_parts: Vec<Element<'_, Message>> = Vec::new();

    if let Some(size) = update.download_size {
        info_parts.push(
            text(format!("Download: {}", format_bytes(size, 2)))
                .size(11)
                .into(),
        );
    }

    if update.is_security {
        info_parts.push(text("Security update").size(11).into());
    }

    let info_row = row(info_parts).spacing(12);

    let is_updating_this = updating.as_deref() == Some(&update.package.id);
    let is_updating_all = updating.as_deref() == Some("__all__");
    let update_btn = if is_updating_this || is_updating_all {
        button(text("Updating...").size(12).center())
            .padding([4, 12])
            .style(button::secondary)
    } else {
        let mut btn = button(text("Update").size(12).center())
            .padding([4, 12])
            .style(button::primary);
        if updating.is_none() {
            btn = btn.on_press(Message::Updates(UpdatesMessage::UpdatePackage(
                update.package.clone(),
            )));
        }
        btn
    };

    let left = column![header, info_row].spacing(4).width(Length::Fill);

    let card = row![left, update_btn]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    container(card)
        .padding(12)
        .width(Length::Fill)
        .style(container::bordered_box)
        .into()
}
