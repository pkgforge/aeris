pub mod message;

use iced::{
    Element, Length, Task,
    widget::{button, column, container, row, rule, space, text},
};

use crate::views;

pub use message::Message;

pub const APP_NAME: &str = "Aeris";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppTheme {
    #[default]
    System,
    Light,
    Dark,
}

impl AppTheme {
    pub const ALL: [AppTheme; 3] = [AppTheme::System, AppTheme::Light, AppTheme::Dark];
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Dashboard,
    Browse,
    Installed,
    Updates,
}

impl std::fmt::Display for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            View::Dashboard => write!(f, "Dashboard"),
            View::Browse => write!(f, "Browse"),
            View::Installed => write!(f, "Installed"),
            View::Updates => write!(f, "Updates"),
        }
    }
}

#[derive(Default)]
pub struct App {
    selected_theme: AppTheme,
    current_view: View,
    browse: views::browse::BrowseState,
}

impl App {
    pub fn title(&self) -> String {
        format!("{APP_NAME} - {}", self.current_view)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NavigateTo(view) => {
                self.current_view = view;
            }
            Message::ThemeChanged(theme) => {
                self.selected_theme = theme;
            }
            Message::Browse(msg) => return self.update_browse(msg),
            Message::Installed(_msg) => {}
            Message::Updates(_msg) => {}
            Message::Adapters(_msg) => {}
        }
        Task::none()
    }

    fn update_browse(&mut self, msg: message::BrowseMessage) -> Task<Message> {
        match msg {
            message::BrowseMessage::SearchQueryChanged(query) => {
                self.browse.search_query = query;
            }
            message::BrowseMessage::SearchSubmit => {
                self.browse.loading = true;
                log::info!("Search: {}", self.browse.search_query);
            }
            _ => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.sidebar_view();
        let content = match self.current_view {
            View::Dashboard => views::dashboard::view(),
            View::Browse => views::browse::view(&self.browse),
            View::Installed => views::installed::view(),
            View::Updates => views::updates::view(),
        };

        row![sidebar, content].into()
    }

    fn sidebar_view(&self) -> Element<'_, Message> {
        let nav_items = [
            (View::Dashboard, "Dashboard"),
            (View::Browse, "Browse"),
            (View::Installed, "Installed"),
            (View::Updates, "Updates"),
        ];

        let mut nav = column![].spacing(4).padding(8);

        for (view, label) in nav_items {
            let is_active = self.current_view == view;
            let btn = button(text(label).size(14).width(Length::Fill).center())
                .on_press(Message::NavigateTo(view))
                .width(Length::Fill)
                .padding([8, 12]);

            let btn = if is_active {
                btn.style(button::primary)
            } else {
                btn.style(button::text)
            };

            nav = nav.push(btn);
        }

        let theme_selector = column![
            text("Theme").size(12),
            iced::widget::pick_list(
                &AppTheme::ALL[..],
                Some(self.selected_theme),
                Message::ThemeChanged,
            )
            .width(Length::Fill),
        ]
        .spacing(4)
        .padding(8);

        container(
            column![
                text(APP_NAME).size(20).center().width(Length::Fill),
                rule::horizontal(1),
                nav,
                space(),
                rule::horizontal(1),
                theme_selector,
            ]
            .spacing(8)
            .height(Length::Fill),
        )
        .width(180)
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
