use iced::{Color, Theme, theme::Palette};

use crate::app::AppTheme;

const PRIMARY: Color = Color::from_rgb(0.18, 0.52, 0.89);
const PRIMARY_LIGHT: Color = Color::from_rgb(0.35, 0.65, 0.95);

const SUCCESS: Color = Color::from_rgb(0.14, 0.62, 0.42);
const WARNING: Color = Color::from_rgb(0.85, 0.60, 0.18);
const DANGER: Color = Color::from_rgb(0.82, 0.28, 0.26);

const LIGHT_BG: Color = Color::from_rgb(0.98, 0.98, 0.99);
const LIGHT_TEXT: Color = Color::from_rgb(0.12, 0.12, 0.14);

const DARK_BG: Color = Color::from_rgb(0.08, 0.09, 0.11);
const DARK_TEXT: Color = Color::from_rgb(0.92, 0.92, 0.94);

const PALETTE_LIGHT: Palette = Palette {
    background: LIGHT_BG,
    text: LIGHT_TEXT,
    primary: PRIMARY,
    success: SUCCESS,
    warning: WARNING,
    danger: DANGER,
};

const PALETTE_DARK: Palette = Palette {
    background: DARK_BG,
    text: DARK_TEXT,
    primary: PRIMARY_LIGHT,
    success: SUCCESS,
    warning: WARNING,
    danger: DANGER,
};

pub fn resolve_theme(theme: AppTheme) -> Option<Theme> {
    match theme {
        AppTheme::System => None,
        AppTheme::Light => Some(Theme::custom("Aeris Light", PALETTE_LIGHT)),
        AppTheme::Dark => Some(Theme::custom("Aeris Dark", PALETTE_DARK)),
    }
}
