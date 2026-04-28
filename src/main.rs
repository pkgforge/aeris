mod adapters;
mod app;
mod components;
mod config;
mod core;
mod styles;
mod theme;
mod views;

use app::App;
use gpui::{AppContext as _, Application, WindowOptions, px, size};

static TOKIO_RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

pub fn tokio_spawn<F, T>(f: F) -> tokio::task::JoinHandle<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    TOKIO_RUNTIME
        .get()
        .expect("Tokio runtime not initialized")
        .spawn(f)
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting {}", app::APP_NAME);

    // Start a background Tokio runtime for adapters that need it
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    TOKIO_RUNTIME.set(tokio_rt).expect("Runtime already set");

    Application::new().run(|cx| {
        components::text_input::bind_text_input_keys(cx);
        app::bind_app_keys(cx);

        let options = WindowOptions {
            window_min_size: Some(size(px(900.0), px(600.0))),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| cx.new(|cx| App::new(window, cx)))
            .unwrap();
    });
}
