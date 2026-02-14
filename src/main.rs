mod app;
mod core;
mod theme;
mod views;

use iced::application;

use crate::app::{APP_NAME, App};

fn main() -> iced::Result {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting {APP_NAME}");

    application(App::default, App::update, App::view)
        .theme(App::theme)
        .title(App::title)
        .run()
}
