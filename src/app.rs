use iced::{
    Alignment, Element, Length, Task,
    widget::{column, pick_list, row, text},
};

pub const APP_NAME: &str = "Aeris";
pub const APP_DESCRIPTION: &str = "Unbounded Package Manager";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl AppTheme {
    const ALL: [AppTheme; 3] = [AppTheme::System, AppTheme::Light, AppTheme::Dark];
}

impl std::fmt::Display for AppTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppTheme::System => write!(f, "System"),
            AppTheme::Light => write!(f, "Light"),
            AppTheme::Dark => write!(f, "Dark"),
        }
    }
}

#[derive(Default)]
pub struct App {
    selected_theme: AppTheme,
}

impl App {
    pub fn title(&self) -> String {
        format!("{APP_NAME} {APP_VERSION}")
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(theme) => {
                self.selected_theme = theme;
                log::info!("Theme switched to {:?}", self.selected_theme);
            }
        }
        Task::none()
    }

    pub fn view(&'_ self) -> Element<'_, Message> {
        column![
            row![
                text(format!("{APP_NAME} {APP_VERSION}"))
                    .size(24)
                    .width(Length::Fill),
                text("Theme:").size(18),
                pick_list(
                    &AppTheme::ALL[..],
                    Some(self.selected_theme),
                    Message::ThemeChanged
                )
            ]
            .align_y(Alignment::Center)
            .spacing(8),
            text(format!("{APP_DESCRIPTION} - Coming soon!")).size(14),
        ]
        .spacing(16)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    pub fn theme(&self) -> Option<iced::Theme> {
        match self.selected_theme {
            AppTheme::System => None,
            AppTheme::Light => Some(iced::Theme::Light),
            AppTheme::Dark => Some(iced::Theme::Dark),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Message {
    ThemeChanged(AppTheme),
}
