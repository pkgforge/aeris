use iced::{
    Element, Length,
    widget::{button, column, container, lazy, row, scrollable, text},
};

use crate::app::message::{Message, RepoInfo, RepositoriesMessage};

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

pub fn view(state: &RepositoriesState) -> Element<'_, Message> {
    let mut header_row = row![text("Repositories").size(22)].spacing(8);

    let sync_all_btn = if state.syncing.is_some() {
        button(text("Syncing...").size(13))
            .padding([6, 14])
            .style(button::secondary)
    } else {
        button(text("Sync All").size(13))
            .padding([6, 14])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::SyncAll))
    };

    let refresh_btn = if state.loading {
        button(text("Loading...").size(13))
            .padding([6, 14])
            .style(button::secondary)
    } else {
        button(text("Refresh").size(13))
            .padding([6, 14])
            .style(button::secondary)
            .on_press(Message::Repositories(RepositoriesMessage::Refresh))
    };

    header_row = header_row
        .push(iced::widget::space().width(Length::Fill))
        .push(sync_all_btn)
        .push(refresh_btn)
        .align_y(iced::Alignment::Center);

    let content: Element<'_, Message> = if state.loading {
        container(text("Loading repositories...").size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(text(format!("Failed to load: {err}")).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if state.repositories.is_empty() {
        container(text("No repositories configured").size(14))
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
            scrollable(column(cards).spacing(8).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    let mut main_col = column![header_row, content]
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Fill);

    if let Some(ref err) = state.sync_error {
        main_col = main_col.push(text(format!("Sync error: {err}")).size(12));
    }

    container(main_col)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn repo_card(repo: &RepoInfo, syncing: &Option<String>) -> Element<'static, Message> {
    let name = text(repo.name.clone()).size(16);
    let url = text(repo.url.clone()).size(12);

    let header = row![name].spacing(8).align_y(iced::Alignment::Center);

    let mut tags: Vec<Element<'_, Message>> = Vec::new();

    if repo.enabled {
        tags.push(badge("Enabled"));
    } else {
        tags.push(badge("Disabled"));
    }
    if repo.desktop_integration {
        tags.push(badge("Desktop"));
    }
    if repo.has_pubkey {
        tags.push(badge("Signed"));
    }
    if repo.signature_verification {
        tags.push(badge("Verified"));
    }
    if let Some(ref interval) = repo.sync_interval {
        tags.push(badge(&format!("Sync: {interval}")));
    }

    let tags_row = row(tags).spacing(6);

    let is_syncing =
        syncing.as_deref() == Some(&repo.name) || syncing.as_deref() == Some("__all__");

    let toggle_btn = if syncing.is_some() {
        if repo.enabled {
            button(text("Disable").size(12))
                .padding([4, 10])
                .style(button::secondary)
        } else {
            button(text("Enable").size(12))
                .padding([4, 10])
                .style(button::secondary)
        }
    } else if repo.enabled {
        button(text("Disable").size(12))
            .padding([4, 10])
            .style(button::secondary)
            .on_press(Message::Repositories(RepositoriesMessage::ToggleEnabled(
                repo.name.clone(),
                false,
            )))
    } else {
        button(text("Enable").size(12))
            .padding([4, 10])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::ToggleEnabled(
                repo.name.clone(),
                true,
            )))
    };

    let sync_btn = if is_syncing {
        button(text("Syncing...").size(12))
            .padding([4, 10])
            .style(button::secondary)
    } else if syncing.is_some() {
        button(text("Sync").size(12))
            .padding([4, 10])
            .style(button::secondary)
    } else {
        button(text("Sync").size(12))
            .padding([4, 10])
            .style(button::primary)
            .on_press(Message::Repositories(RepositoriesMessage::SyncRepo(
                repo.name.clone(),
            )))
    };

    let left = column![header, url, tags_row]
        .spacing(4)
        .width(Length::Fill);

    let card = row![left, toggle_btn, sync_btn]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    container(card)
        .padding(12)
        .width(Length::Fill)
        .style(container::bordered_box)
        .into()
}

fn badge(label: &str) -> Element<'static, Message> {
    container(text(label.to_string()).size(10))
        .padding([2, 6])
        .style(container::bordered_box)
        .into()
}
