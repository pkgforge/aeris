use std::sync::OnceLock;

use gpui::Hsla;

use crate::app::AppTheme;

#[derive(Clone)]
pub struct Theme {
    pub bg: Hsla,
    pub surface: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub primary: Hsla,
    pub primary_light: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
    pub border: Hsla,
    pub hover: Hsla,
}

fn light() -> Theme {
    Theme {
        bg: hsla(230.0 / 360.0, 0.15, 0.98, 1.0),
        surface: hsla(230.0 / 360.0, 0.10, 0.95, 1.0),
        text: hsla(240.0 / 360.0, 0.10, 0.13, 1.0),
        text_muted: hsla(230.0 / 360.0, 0.08, 0.45, 1.0),
        primary: hsla(214.0 / 360.0, 0.77, 0.53, 1.0),
        primary_light: hsla(214.0 / 360.0, 0.85, 0.65, 1.0),
        success: hsla(155.0 / 360.0, 0.63, 0.38, 1.0),
        warning: hsla(37.0 / 360.0, 0.70, 0.52, 1.0),
        danger: hsla(2.0 / 360.0, 0.58, 0.55, 1.0),
        border: hsla(230.0 / 360.0, 0.12, 0.85, 1.0),
        hover: hsla(230.0 / 360.0, 0.10, 0.90, 1.0),
    }
}

fn dark() -> Theme {
    Theme {
        bg: hsla(220.0 / 360.0, 0.15, 0.09, 1.0),
        surface: hsla(220.0 / 360.0, 0.12, 0.13, 1.0),
        text: hsla(220.0 / 360.0, 0.10, 0.93, 1.0),
        text_muted: hsla(220.0 / 360.0, 0.08, 0.55, 1.0),
        primary: hsla(214.0 / 360.0, 0.85, 0.65, 1.0),
        primary_light: hsla(214.0 / 360.0, 0.90, 0.75, 1.0),
        success: hsla(155.0 / 360.0, 0.63, 0.38, 1.0),
        warning: hsla(37.0 / 360.0, 0.70, 0.52, 1.0),
        danger: hsla(2.0 / 360.0, 0.58, 0.55, 1.0),
        border: hsla(220.0 / 360.0, 0.12, 0.22, 1.0),
        hover: hsla(220.0 / 360.0, 0.10, 0.18, 1.0),
    }
}

fn detect_system_dark_mode() -> bool {
    static CACHED: OnceLock<bool> = OnceLock::new();
    return *CACHED.get_or_init(detect_system_dark_mode_inner);
}

fn detect_system_dark_mode_inner() -> bool {
    // Check freedesktop portal / GNOME setting
    if let Ok(output) = std::process::Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply=literal",
            "--dest=org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings.Read",
            "string:org.freedesktop.appearance",
            "string:color-scheme",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // color-scheme: 1 = prefer dark, 2 = prefer light, 0 = no preference
        if stdout.contains("uint32 1") {
            return true;
        }
        if stdout.contains("uint32 2") {
            return false;
        }
    }

    // Fallback: check GTK_THEME env
    if let Ok(gtk_theme) = std::env::var("GTK_THEME") {
        let lower = gtk_theme.to_lowercase();
        if lower.contains("dark") {
            return true;
        }
        if !lower.is_empty() {
            return false;
        }
    }

    // Default to dark
    true
}

pub fn current_theme(theme: AppTheme) -> Theme {
    match theme {
        AppTheme::System => {
            if detect_system_dark_mode() {
                dark()
            } else {
                light()
            }
        }
        AppTheme::Light => light(),
        AppTheme::Dark => dark(),
    }
}

fn hsla(h: f32, s: f32, l: f32, a: f32) -> Hsla {
    Hsla { h, s, l, a }
}
