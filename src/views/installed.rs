use std::collections::{HashMap, HashSet};

use iced::{
    Alignment, Element, Length,
    widget::{
        button, checkbox, column, container, lazy, progress_bar, row, scrollable, space, text,
    },
};

use crate::{
    app::{
        OperationStatus,
        message::{InstalledMessage, Message},
    },
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
    pub updating: Option<String>,
    /// Adapter IDs that support updating but not listing available updates.
    pub updatable_adapters: HashSet<String>,
    pub selected: HashSet<String>,
    pub package_progress: HashMap<String, OperationStatus>,
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
        let updating = state.updating.clone();
        let updatable_adapters = state.updatable_adapters.clone();
        let selected = state.selected.clone();
        let package_progress = state.package_progress.clone();
        lazy(("installed", version), move |_| {
            let cards: Vec<Element<'_, Message>> = packages
                .iter()
                .map(|pkg| {
                    installed_card(
                        pkg,
                        &removing,
                        &updating,
                        &updatable_adapters,
                        &selected,
                        &package_progress,
                    )
                })
                .collect();

            scrollable(column(cards).spacing(spacing::SM).width(Length::Fill)).height(Length::Fill)
        })
        .into()
    };

    let mut main_col = column![header, content]
        .spacing(spacing::MD)
        .width(Length::Fill)
        .height(Length::Fill);

    if !state.selected.is_empty() {
        main_col = main_col.push(super::browse::floating_action_bar(
            state.selected.len(),
            "Remove",
            Message::Installed(InstalledMessage::RemoveSelected),
            Message::Installed(InstalledMessage::ClearSelection),
            true,
        ));
    }

    container(main_col)
        .padding(spacing::XL)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn installed_card(
    pkg: &InstalledPackage,
    removing: &Option<String>,
    updating: &Option<String>,
    updatable_adapters: &HashSet<String>,
    selected: &HashSet<String>,
    package_progress: &HashMap<String, OperationStatus>,
) -> Element<'static, Message> {
    let is_selected = selected.contains(&pkg.package.id);
    let name = text(pkg.package.name.clone()).size(font_size::HEADING);
    let adapter = container(text(pkg.package.adapter_id.clone()).size(font_size::BADGE))
        .padding([spacing::XXXS, spacing::XS])
        .style(styles::badge_adapter(&pkg.package.adapter_id));

    let mut header = row![name].spacing(spacing::SM).align_y(Alignment::Center);

    if !pkg.package.version.is_empty() {
        header = header.push(
            container(text(pkg.package.version.clone()).size(font_size::CAPTION))
                .padding([spacing::XXXS, spacing::XS])
                .style(styles::badge_neutral),
        );
    }

    header = header.push(adapter);

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

    let pkg_status = package_progress.get(&pkg.package.name);
    let is_removing = removing.as_deref() == Some(&pkg.package.id)
        || (removing.as_deref() == Some("__batch__") && pkg_status.is_some());
    let remove_btn = if is_removing {
        let label = pkg_status
            .map(|s| s.label())
            .unwrap_or_else(|| "Removing...".into());
        button(text(label).size(font_size::CAPTION + 1.0).center())
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

    let mut left = column![header, info_row]
        .spacing(spacing::XXS)
        .width(Length::Fill);

    // Add inline progress bar when downloading
    if let Some(progress) = pkg_status.and_then(|s| s.progress()) {
        left = left.push(
            container(progress_bar(0.0..=1.0, progress))
                .height(4)
                .width(Length::Fill),
        );
    }

    let is_busy = removing.is_some() || updating.is_some();
    let show_update = updatable_adapters.contains(&pkg.package.adapter_id);

    let mut buttons = row![].spacing(spacing::XS).align_y(Alignment::Center);

    if show_update {
        let is_updating = updating.as_deref() == Some(&pkg.package.id)
            || (updating.as_deref() == Some("__batch__") && pkg_status.is_some());
        let update_btn = if is_updating {
            let label = pkg_status
                .map(|s| s.label())
                .unwrap_or_else(|| "Updating...".into());
            button(text(label).size(font_size::CAPTION + 1.0).center())
                .padding([spacing::XXS, 14.0])
                .style(button::secondary)
        } else {
            let mut btn = button(text("Update").size(font_size::CAPTION + 1.0).center())
                .padding([spacing::XXS, 14.0])
                .style(button::primary);
            if !is_busy {
                btn = btn.on_press(Message::Installed(InstalledMessage::UpdatePackage(
                    pkg.package.clone(),
                )));
            }
            btn
        };
        buttons = buttons.push(update_btn);
    }

    buttons = buttons.push(remove_btn);

    let cb_id = pkg.package.id.clone();
    let cb = checkbox(is_selected)
        .on_toggle(move |_| Message::Installed(InstalledMessage::ToggleSelect(cb_id.clone())));

    let card_content = row![cb, left, buttons]
        .spacing(spacing::MD)
        .align_y(Alignment::Center);

    let card_style = if is_selected {
        styles::card_button_selected
            as fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style
    } else {
        styles::card_button
    };

    button(
        container(card_content)
            .padding(spacing::MD)
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .style(card_style)
    .into()
}
