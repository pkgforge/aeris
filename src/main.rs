mod adapters;
mod app;
mod assets;
mod components;
mod config;
mod core;
mod manifest_file;
mod styles;
mod theme;
mod views;
mod xdg;

use app::App;
use gpui::{AppContext as _, Application, WindowDecorations, WindowOptions, px, size};

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
    // `default_filter_or` rather than `filter_level`, which would undo the
    // whole point: it inserts the same unnamed directive `RUST_LOG=debug`
    // parses into, and the later one wins.
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting {}", app::APP_NAME);

    // Start a background Tokio runtime for adapters that need it
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    TOKIO_RUNTIME.set(tokio_rt).expect("Runtime already set");

    Application::new().with_assets(assets::Assets).run(|cx| {
        components::text_input::bind_text_input_keys(cx);
        app::bind_app_keys(cx);

        // Asked for outright, because a compositor offering no decoration
        // protocol at all is reported as decorating the window itself. GNOME
        // offers none, so the window came back with nothing drawn and no way
        // to move, resize or close it.
        let options = WindowOptions {
            app_id: Some(app::APP_ID.into()),
            window_min_size: Some(size(px(900.0), px(600.0))),
            window_decorations: Some(WindowDecorations::Client),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            window.set_window_title(app::APP_NAME);
            cx.new(|cx| App::new(window, cx))
        })
        .unwrap();

        // Aeris is its window. Without saying so, closing it leaves the work
        // behind it running, still asking for frames of a window that has gone.
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
}
