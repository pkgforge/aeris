use iced::{
    Element, Length,
    widget::{button, column, container, lazy, row, scrollable, space, text},
};

use crate::{
    app::message::{InstalledMessage, Message},
    core::package::InstalledPackage,
};
use soar_utils::bytes::format_bytes;

#[derive(Debug, Default)]
pub struct InstalledState {
    pub packages: Vec<InstalledPackage>,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub result_version: u64,
    pub removing: Option<String>,
}

pub fn view<'a>(state: &'a InstalledState) -> Element<'a, Message> {
    let header = row![
        text("Installed Packages").size(20),
        space().width(Length::Fill),
        button(text("Refresh").size(12).center())
            .padding([6, 14])
            .style(button::secondary)
            .on_press(Message::Installed(InstalledMessage::Refresh)),
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    let content: Element<'_, Message> = if state.loading {
        container(text("Loading installed packages...").size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(text(format!("Failed to load: {err}")).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if state.packages.is_empty() {
        let msg = if state.loaded {
            "No packages installed"
        } else {
            "Loading..."
        };
        container(text(msg).size(14))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let version = state.result_version;
        let packages = state.packages.clone();
        let removing = state.removing.clone();
        lazy(version, move |_| {
            let cards: Vec<Element<'_, Message>> = packages
                .iter()
                .map(|pkg| installed_card(pkg, &removing))
                .collect();

            scrollable(column(cards).spacing(8).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    container(
        column![header, content]
            .spacing(12)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding(20)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn installed_card(pkg: &InstalledPackage, removing: &Option<String>) -> Element<'static, Message> {
    let name = text(pkg.package.name.clone()).size(16);
    let version = text(pkg.package.version.clone()).size(12);
    let adapter = text(format!("[{}]", pkg.package.adapter_id)).size(11);

    let mut header = row![name, version, adapter]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    if !pkg.is_healthy {
        header = header.push(text("(partial install)").size(11));
    }

    let mut info_parts: Vec<Element<'_, Message>> = Vec::new();

    if pkg.install_size > 0 {
        info_parts.push(
            text(format!("Size: {}", format_bytes(pkg.install_size, 2)))
                .size(11)
                .into(),
        );
    }

    if let Some(ref path) = pkg.install_path {
        info_parts.push(text(path.clone()).size(11).into());
    }

    if let Some(ref profile) = pkg.profile {
        info_parts.push(text(format!("Profile: {profile}")).size(11).into());
    }

    if pkg.pinned {
        info_parts.push(text("Pinned").size(11).into());
    }

    let info_row = row(info_parts).spacing(12);

    let is_removing = removing.as_deref() == Some(&pkg.package.id);
    let remove_btn = if is_removing {
        button(text("Removing...").size(12).center())
            .padding([4, 12])
            .style(button::secondary)
    } else {
        let mut btn = button(text("Remove").size(12).center())
            .padding([4, 12])
            .style(button::danger);
        if removing.is_none() {
            btn = btn.on_press(Message::Installed(InstalledMessage::RemovePackage(
                pkg.package.clone(),
            )));
        }
        btn
    };

    let left = column![header, info_row].spacing(4).width(Length::Fill);

    let card = row![left, remove_btn]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    container(card)
        .padding(12)
        .width(Length::Fill)
        .style(container::bordered_box)
        .into()
}
