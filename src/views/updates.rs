use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, lazy, row, scrollable, space, text},
};

use crate::{
    app::message::{Message, UpdatesMessage},
    core::{package::Update, privilege::PackageMode},
    styles::{self, font_size, spacing},
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

pub fn view<'a>(state: &'a UpdatesState, mode: PackageMode) -> Element<'a, Message> {
    let title = match mode {
        PackageMode::User => "Updates (User)",
        PackageMode::System => "Updates (System)",
    };
    let mut header_row = row![
        text(title).size(font_size::TITLE),
        space().width(Length::Fill)
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let is_busy = state.updating.is_some() || state.loading;

    if !state.updates.is_empty() {
        let mut update_all_btn = button(text("Update All").size(font_size::SMALL).center())
            .padding([spacing::SM, 18.0])
            .style(button::primary);
        if !is_busy {
            update_all_btn = update_all_btn.on_press(Message::Updates(UpdatesMessage::UpdateAll));
        }
        header_row = header_row.push(update_all_btn);
    }

    let mut check_btn = button(text("Check").size(font_size::CAPTION + 1.0).center())
        .padding([spacing::XS, 14.0])
        .style(button::secondary);
    if !is_busy {
        check_btn = check_btn.on_press(Message::Updates(UpdatesMessage::CheckUpdates));
    }
    header_row = header_row.push(check_btn);

    let content: Element<'_, Message> = if state.loading {
        container(text("Checking for updates...").size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(text(format!("Failed: {err}")).size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if state.updates.is_empty() {
        let msg = if state.checked {
            "All packages are up to date"
        } else {
            "Click Check to look for updates"
        };
        container(text(msg).size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let version = state.result_version;
        let updates = state.updates.clone();
        let updating = state.updating.clone();
        lazy(("updates", version), move |_| {
            let cards: Vec<Element<'_, Message>> =
                updates.iter().map(|u| update_card(u, &updating)).collect();

            scrollable(column(cards).spacing(spacing::SM).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    container(
        column![header_row, content]
            .spacing(spacing::MD)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding(spacing::XL)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn update_card(update: &Update, updating: &Option<String>) -> Element<'static, Message> {
    let name = text(update.package.name.clone()).size(font_size::HEADING);

    let old_version = container(text(update.current_version.clone()).size(font_size::CAPTION))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_neutral);
    let arrow = text("\u{2192}").size(font_size::SMALL);
    let new_version = container(text(update.new_version.clone()).size(font_size::CAPTION))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_success);

    let header = row![name, old_version, arrow, new_version]
        .spacing(spacing::XS)
        .align_y(Alignment::Center);

    let mut info_parts: Vec<Element<'_, Message>> = Vec::new();

    if let Some(size) = update.download_size {
        info_parts.push(
            text(format!("Download: {}", format_bytes(size, 2)))
                .size(font_size::CAPTION)
                .into(),
        );
    }

    if update.is_security {
        info_parts.push(
            container(text("Security").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_warning)
                .into(),
        );
    }

    let info_row = row(info_parts)
        .spacing(spacing::MD)
        .align_y(Alignment::Center);

    let is_updating_this = updating.as_deref() == Some(&update.package.id);
    let is_updating_all = updating.as_deref() == Some("__all__");
    let update_btn = if is_updating_this || is_updating_all {
        button(text("Updating...").size(font_size::CAPTION + 1.0).center())
            .padding([spacing::XXS, spacing::MD])
            .style(button::secondary)
    } else {
        let mut btn = button(text("Update").size(font_size::CAPTION + 1.0).center())
            .padding([spacing::XXS, spacing::MD])
            .style(button::primary);
        if updating.is_none() {
            btn = btn.on_press(Message::Updates(UpdatesMessage::UpdatePackage(
                update.package.clone(),
            )));
        }
        btn
    };

    let left = column![header, info_row]
        .spacing(spacing::XXS)
        .width(Length::Fill);

    let card_content = row![left, update_btn]
        .spacing(spacing::MD)
        .align_y(Alignment::Center);

    button(
        container(card_content)
            .padding(spacing::MD)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .style(styles::card_button)
    .into()
}
