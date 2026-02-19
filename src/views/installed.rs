use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, lazy, row, scrollable, space, text},
};

use crate::{
    app::message::{InstalledMessage, Message},
    core::{package::InstalledPackage, privilege::PackageMode},
    styles::{self, font_size, spacing},
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

pub fn view<'a>(state: &'a InstalledState, mode: PackageMode) -> Element<'a, Message> {
    let title = match mode {
        PackageMode::User => "Installed Packages (User)",
        PackageMode::System => "Installed Packages (System)",
    };
    let header = row![
        text(title).size(font_size::TITLE),
        space().width(Length::Fill),
        button(text("Refresh").size(font_size::CAPTION + 1.0).center())
            .padding([spacing::XS, 14.0])
            .style(button::secondary)
            .on_press(Message::Installed(InstalledMessage::Refresh)),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let content: Element<'_, Message> = if state.loading {
        container(text("Loading installed packages...").size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if let Some(ref err) = state.error {
        container(text(format!("Failed to load: {err}")).size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else if state.packages.is_empty() {
        let msg = if state.loaded {
            "No packages installed"
        } else {
            "Loading..."
        };
        container(text(msg).size(font_size::BODY))
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    } else {
        let version = state.result_version;
        let packages = state.packages.clone();
        let removing = state.removing.clone();
        lazy(("installed", version), move |_| {
            let cards: Vec<Element<'_, Message>> = packages
                .iter()
                .map(|pkg| installed_card(pkg, &removing))
                .collect();

            scrollable(column(cards).spacing(spacing::SM).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    container(
        column![header, content]
            .spacing(spacing::MD)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding(spacing::XL)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn installed_card(pkg: &InstalledPackage, removing: &Option<String>) -> Element<'static, Message> {
    let name = text(pkg.package.name.clone()).size(font_size::HEADING);
    let version = container(text(pkg.package.version.clone()).size(font_size::CAPTION))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_neutral);
    let adapter = container(text(pkg.package.adapter_id.clone()).size(font_size::BADGE))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_neutral);

    let mut header = row![name, version, adapter]
        .spacing(spacing::SM)
        .align_y(Alignment::Center);

    if !pkg.is_healthy {
        header = header.push(
            container(text("partial install").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_warning),
        );
    }

    let mut info_parts: Vec<Element<'_, Message>> = Vec::new();

    if pkg.install_size > 0 {
        info_parts.push(
            text(format!("Size: {}", format_bytes(pkg.install_size, 2)))
                .size(font_size::CAPTION)
                .into(),
        );
    }

    if let Some(ref path) = pkg.install_path {
        info_parts.push(text(path.clone()).size(font_size::CAPTION).into());
    }

    if let Some(ref profile) = pkg.profile {
        info_parts.push(
            text(format!("Profile: {profile}"))
                .size(font_size::CAPTION)
                .into(),
        );
    }

    if pkg.pinned {
        info_parts.push(
            container(text("Pinned").size(font_size::BADGE))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_primary)
                .into(),
        );
    }

    let info_row = row(info_parts)
        .spacing(spacing::MD)
        .align_y(Alignment::Center);

    let is_removing = removing.as_deref() == Some(&pkg.package.id);
    let remove_btn = if is_removing {
        button(text("Removing...").size(font_size::CAPTION + 1.0).center())
            .padding([spacing::XXS, 14.0])
            .style(button::secondary)
    } else {
        let mut btn = button(text("Remove").size(font_size::CAPTION + 1.0).center())
            .padding([spacing::XXS, 14.0])
            .style(|theme, status| styles::pill_button_danger(theme, status));
        if removing.is_none() {
            btn = btn.on_press(Message::Installed(InstalledMessage::RemovePackage(
                pkg.package.clone(),
            )));
        }
        btn
    };

    let left = column![header, info_row]
        .spacing(spacing::XXS)
        .width(Length::Fill);

    let card_content = row![left, remove_btn]
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
