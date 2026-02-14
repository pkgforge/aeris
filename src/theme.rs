use iced::theme::Palette;
use iced::Theme;

use crate::app::AppTheme;

pub fn resolve_theme(theme: AppTheme) -> Option<Theme> {
    match theme {
        AppTheme::System => None,
        AppTheme::Light => Some(Theme::Light),
        AppTheme::Dark => Some(Theme::Dark),
    }
}

pub fn palette(theme: &Theme) -> Palette {
    theme.palette()
}
