mod adapters;
mod app;
mod core;
mod theme;
mod views;

use iced::application;

use crate::app::{APP_NAME, App};

fn main() -> iced::Result {
    if std::env::var_os("WGPU_POWER_PREF").is_none() {
        unsafe { std::env::set_var("WGPU_POWER_PREF", "low") };
    }

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .init();

    log::info!("Starting {APP_NAME}");

    application(App::default, App::update, App::view)
        .theme(App::theme)
        .title(App::title)
        .run()
}
