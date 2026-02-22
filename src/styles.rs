use iced::{Border, Color, Shadow, Theme, Vector, border::Radius, widget::container};

pub mod font_size {
    pub const DISPLAY: f32 = 32.0;
    pub const TITLE: f32 = 20.0;
    pub const HEADING: f32 = 16.0;
    pub const BODY: f32 = 14.0;
    pub const SMALL: f32 = 13.0;
    pub const CAPTION: f32 = 11.0;
    pub const BADGE: f32 = 10.0;
}

pub mod line_height {
    pub const TIGHT: f32 = 1.2;
    pub const NORMAL: f32 = 1.4;
    pub const RELAXED: f32 = 1.6;
}

pub mod spacing {
    pub const XXXS: f32 = 2.0;
    pub const XXS: f32 = 4.0;
    pub const XS: f32 = 6.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 20.0;
    pub const XXL: f32 = 24.0;
    pub const XXXL: f32 = 32.0;
}

pub mod radius {
    pub const NONE: f32 = 0.0;
    pub const SM: f32 = 4.0;
    pub const MD: f32 = 6.0;
    pub const LG: f32 = 8.0;
    pub const XL: f32 = 12.0;
    pub const FULL: f32 = 9999.0;
}

fn radius_left(r: f32) -> Radius {
    Radius {
        top_left: r,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: r,
    }
}

pub fn loading_spinner(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        text_color: Some(palette.primary.base.color),
        ..Default::default()
    }
}

pub fn skeleton_loader(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        ..Default::default()
    }
}

// --- Header Bar ---

pub fn header_container(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: radius::NONE.into(),
        },
        ..Default::default()
    }
}

pub fn header_icon_button(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::FULL.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => iced::widget::button::Style {
            background: None,
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::FULL.into(),
                ..Default::default()
            },
            ..Default::default()
        },
    }
}

// --- Sidebar ---

pub fn sidebar_container(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn sidebar_active_button(theme: &Theme) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    iced::widget::button::Style {
        background: Some(palette.primary.weak.color.into()),
        text_color: palette.primary.base.text,
        border: Border {
            radius: radius::MD.into(),
            width: 0.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn sidebar_button(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let is_dark = palette.background.base.color.r < 0.5;
    let text_color = if is_dark {
        Color::from_rgb(0.85, 0.85, 0.85)
    } else {
        Color::from_rgb(0.25, 0.25, 0.25)
    };
    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color,
            border: Border {
                radius: radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        iced::widget::button::Status::Pressed => iced::widget::button::Style {
            background: Some(palette.primary.weak.color.into()),
            text_color: palette.primary.base.text,
            border: Border {
                radius: radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => iced::widget::button::Style {
            background: None,
            text_color,
            border: Border {
                radius: radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        },
    }
}

// --- Nav Icon ---

pub fn nav_icon_active(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.primary.base.color.into()),
        border: Border {
            radius: radius::MD.into(),
            ..Default::default()
        },
        text_color: Some(palette.primary.base.text),
        ..Default::default()
    }
}

pub fn nav_icon_inactive(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let is_dark = palette.background.base.color.r < 0.5;
    container::Style {
        background: None,
        border: Border {
            radius: radius::MD.into(),
            ..Default::default()
        },
        text_color: Some(if is_dark {
            Color::from_rgb(0.85, 0.85, 0.85)
        } else {
            Color::from_rgb(0.35, 0.35, 0.35)
        }),
        ..Default::default()
    }
}

// --- Cards ---

pub fn card(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.base.color.into()),
        border: Border {
            radius: radius::LG.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        shadow: Shadow {
            color: Color {
                a: 0.08,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 2.0),
            blur_radius: 8.0,
        },
        ..Default::default()
    }
}

pub fn card_button(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(palette.background.weak.color.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::LG.into(),
                width: 1.0,
                color: palette.primary.weak.color,
            },
            shadow: Shadow {
                color: Color {
                    a: 0.12,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            snap: false,
        },
        iced::widget::button::Status::Pressed => iced::widget::button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::LG.into(),
                width: 1.0,
                color: palette.primary.base.color,
            },
            shadow: Shadow {
                color: Color {
                    a: 0.06,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 1.0),
                blur_radius: 4.0,
            },
            snap: false,
        },
        _ => iced::widget::button::Style {
            background: Some(palette.background.base.color.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::LG.into(),
                width: 1.0,
                color: palette.background.strong.color,
            },
            shadow: Shadow {
                color: Color {
                    a: 0.08,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, 2.0),
                blur_radius: 8.0,
            },
            snap: false,
        },
    }
}

// --- Detail Panel ---

pub fn detail_panel(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 0.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

// --- Search ---

pub fn search_container(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            radius: radius::LG.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        ..Default::default()
    }
}

// --- Badges ---

pub fn badge_success(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.success.weak.color.into()),
        border: Border {
            radius: radius::SM.into(),
            width: 1.0,
            color: palette.success.base.color,
        },
        text_color: Some(palette.success.strong.color),
        ..Default::default()
    }
}

pub fn badge_warning(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    let is_dark = palette.background.base.color.r < 0.5;
    if is_dark {
        container::Style {
            background: Some(Color::from_rgb(0.5, 0.38, 0.05).into()),
            border: Border {
                radius: radius::SM.into(),
                width: 1.0,
                color: Color::from_rgb(0.75, 0.58, 0.12),
            },
            text_color: Some(Color::from_rgb(1.0, 0.92, 0.65)),
            ..Default::default()
        }
    } else {
        container::Style {
            background: Some(Color::from_rgb(1.0, 0.96, 0.88).into()),
            border: Border {
                radius: radius::SM.into(),
                width: 1.0,
                color: Color::from_rgb(0.82, 0.65, 0.15),
            },
            text_color: Some(Color::from_rgb(0.52, 0.38, 0.02)),
            ..Default::default()
        }
    }
}

pub fn badge_neutral(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            radius: radius::SM.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        text_color: Some(palette.background.base.text),
        ..Default::default()
    }
}

pub fn badge_danger(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.danger.weak.color.into()),
        border: Border {
            radius: radius::SM.into(),
            width: 1.0,
            color: palette.danger.base.color,
        },
        text_color: Some(palette.danger.strong.color),
        ..Default::default()
    }
}

pub fn badge_primary(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.primary.weak.color.into()),
        border: Border {
            radius: radius::SM.into(),
            width: 1.0,
            color: palette.primary.base.color,
        },
        text_color: Some(palette.primary.strong.color),
        ..Default::default()
    }
}

// --- Modal ---

pub fn modal_backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(
            Color {
                a: 0.6,
                ..Color::BLACK
            }
            .into(),
        ),
        ..Default::default()
    }
}

pub fn modal_card(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.base.color.into()),
        border: Border {
            radius: radius::XL.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        shadow: Shadow {
            color: Color {
                a: 0.25,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..Default::default()
    }
}

pub fn stat_card_accent_left(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.primary.weak.color.into()),
        border: Border {
            radius: radius_left(radius::SM),
            width: 0.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

// --- Progress ---

pub fn progress_container(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: radius::NONE.into(),
        },
        shadow: Shadow {
            color: Color {
                a: 0.1,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, -2.0),
            blur_radius: 6.0,
        },
        ..Default::default()
    }
}

// --- Error Banner ---

pub fn error_banner(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.danger.weak.color.into()),
        border: Border {
            width: 1.0,
            color: palette.danger.base.color,
            radius: radius::MD.into(),
        },
        ..Default::default()
    }
}

// --- Settings Card ---

pub fn settings_card(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.base.color.into()),
        border: Border {
            radius: radius::LG.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
        shadow: Shadow {
            color: Color {
                a: 0.05,
                ..Color::BLACK
            },
            offset: Vector::new(0.0, 1.0),
            blur_radius: 4.0,
        },
        ..Default::default()
    }
}

// --- Pill button style (rounded) ---

pub fn pill_button_danger(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    let is_dark = palette.background.base.color.r < 0.5;
    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(palette.danger.base.color.into()),
            text_color: palette.danger.base.text,
            border: Border {
                radius: radius::FULL.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        iced::widget::button::Status::Pressed => iced::widget::button::Style {
            background: Some(palette.danger.strong.color.into()),
            text_color: palette.danger.weak.color,
            border: Border {
                radius: radius::FULL.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => iced::widget::button::Style {
            background: Some(palette.danger.weak.color.into()),
            text_color: if is_dark {
                Color::from_rgb(1.0, 0.7, 0.7)
            } else {
                Color::from_rgb(0.6, 0.15, 0.15)
            },
            border: Border {
                radius: radius::FULL.into(),
                width: 1.0,
                color: palette.danger.base.color,
            },
            ..Default::default()
        },
    }
}

// --- Outlined button style ---

pub fn outlined_button(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(palette.primary.weak.color.into()),
            text_color: palette.primary.strong.color,
            border: Border {
                radius: radius::MD.into(),
                width: 1.0,
                color: palette.primary.base.color,
            },
            ..Default::default()
        },
        iced::widget::button::Status::Pressed => iced::widget::button::Style {
            background: Some(palette.primary.strong.color.into()),
            text_color: palette.primary.weak.color,
            border: Border {
                radius: radius::MD.into(),
                width: 1.0,
                color: palette.primary.strong.color,
            },
            ..Default::default()
        },
        _ => iced::widget::button::Style {
            background: None,
            text_color: palette.primary.base.color,
            border: Border {
                radius: radius::MD.into(),
                width: 1.0,
                color: palette.primary.base.color,
            },
            ..Default::default()
        },
    }
}

// --- Ghost button (text-only with hover) ---

pub fn ghost_button(
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let palette = theme.extended_palette();
    match status {
        iced::widget::button::Status::Hovered => iced::widget::button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        iced::widget::button::Status::Pressed => iced::widget::button::Style {
            background: Some(palette.background.strong.color.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        _ => iced::widget::button::Style {
            background: None,
            text_color: palette.background.base.text,
            border: Border {
                radius: radius::MD.into(),
                ..Default::default()
            },
            ..Default::default()
        },
    }
}
