use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, lazy, row, scrollable, text},
};

use crate::{
    app::message::{Message, RepoInfo, RepositoriesMessage},
    core::privilege::PackageMode,
    styles::{self, font_size, spacing},
};

#[derive(Debug, Default)]
pub struct RepositoriesState {
    pub repositories: Vec<RepoInfo>,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub result_version: u64,
    pub syncing: Option<String>,
    pub sync_error: Option<String>,
}

pub fn view<'a>(state: &'a RepositoriesState, mode: PackageMode) -> Element<'a, Message> {
    let title = match mode {
        PackageMode::User => "Repositories (User)",
        PackageMode::System => "Repositories (System)",
    };
    let mut header_row = row![text(title).size(font_size::TITLE)].spacing(spacing::SM);

    let sync_all_btn = if state.syncing.is_some() {
        button(text("Syncing...").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::secondary)
    } else {
        button(text("Sync All").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::SyncAll))
    };

    let refresh_btn = if state.loading {
        button(text("Loading...").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::secondary)
    } else {
        button(text("Refresh").size(font_size::SMALL))
            .padding([spacing::XS, 14.0])
            .style(button::secondary)
            .on_press(Message::Repositories(RepositoriesMessage::Refresh))
    };

    header_row = header_row
        .push(iced::widget::space().width(Length::Fill))
        .push(sync_all_btn)
        .push(refresh_btn)
        .align_y(Alignment::Center);

    let content: Element<'_, Message> = if state.loading {
        container(text("Loading repositories...").size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(text(format!("Failed to load: {err}")).size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if state.repositories.is_empty() {
        container(text("No repositories configured").size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let version = state.result_version;
        let repos = state.repositories.clone();
        let syncing = state.syncing.clone();
        lazy(("repos", version), move |_| {
            let cards: Vec<Element<'_, Message>> =
                repos.iter().map(|repo| repo_card(repo, &syncing)).collect();
            scrollable(column(cards).spacing(spacing::SM).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    let mut main_col = column![header_row, content]
        .spacing(spacing::MD)
        .width(Length::Fill)
        .height(Length::Fill);

    if let Some(ref err) = state.sync_error {
        main_col = main_col.push(
            container(text(format!("Sync error: {err}")).size(font_size::CAPTION + 1.0))
                .padding([spacing::XS, spacing::MD])
                .width(Length::Fill)
                .style(styles::error_banner),
        );
    }

    container(main_col)
        .padding(spacing::XL)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn repo_card(repo: &RepoInfo, syncing: &Option<String>) -> Element<'static, Message> {
    let name = text(repo.name.clone()).size(font_size::HEADING);
    let url = text(repo.url.clone()).size(font_size::CAPTION + 1.0);

    let header = row![name].spacing(spacing::SM).align_y(Alignment::Center);

    let mut tags: Vec<Element<'_, Message>> = Vec::new();

    if repo.enabled {
        tags.push(
            container(text("Enabled").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_success)
                .into(),
        );
    } else {
        tags.push(
            container(text("Disabled").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_danger)
                .into(),
        );
    }
    if repo.desktop_integration {
        tags.push(
            container(text("Desktop").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_neutral)
                .into(),
        );
    }
    if repo.has_pubkey {
        tags.push(
            container(text("Signed").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_primary)
                .into(),
        );
    }
    if repo.signature_verification {
        tags.push(
            container(text("Verified").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_primary)
                .into(),
        );
    }
    if let Some(ref interval) = repo.sync_interval {
        tags.push(
            container(text(format!("Sync: {interval}")).size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_neutral)
                .into(),
        );
    }

    let tags_row = row(tags).spacing(spacing::XS);

    let is_syncing =
        syncing.as_deref() == Some(&repo.name) || syncing.as_deref() == Some("__all__");

    let toggle_btn = if syncing.is_some() {
        if repo.enabled {
            button(text("Disable").size(font_size::CAPTION + 1.0))
                .padding([spacing::XXS, 10.0])
                .style(button::secondary)
        } else {
            button(text("Enable").size(font_size::CAPTION + 1.0))
                .padding([spacing::XXS, 10.0])
                .style(button::secondary)
        }
    } else if repo.enabled {
        button(text("Disable").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::secondary)
            .on_press(Message::Repositories(RepositoriesMessage::ToggleEnabled(
                repo.name.clone(),
                false,
            )))
    } else {
        button(text("Enable").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::ToggleEnabled(
                repo.name.clone(),
                true,
            )))
    };

    let sync_btn = if is_syncing {
        button(text("Syncing...").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::secondary)
    } else if syncing.is_some() {
        button(text("Sync").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::secondary)
    } else {
        button(text("Sync").size(font_size::CAPTION + 1.0))
            .padding([spacing::XXS, 10.0])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::SyncRepo(
                repo.name.clone(),
            )))
    };

    let left = column![header, url, tags_row]
        .spacing(spacing::XXS)
        .width(Length::Fill);

    let card_content = row![left, toggle_btn, sync_btn]
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
