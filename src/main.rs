mod app;
mod config;
mod db;
mod zk;

use clap::Parser;
use gpui_kit::component::TitleBar;
use gpui_kit::{application, assets, init, open_window, px, size, AppContext, WindowBounds};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = config::Cli::parse();

    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);

        let mut options = TitleBar::window_options();
        options.window_bounds = Some(WindowBounds::centered(size(px(1280.), px(800.)), cx));
        options.window_min_size = Some(size(px(960.), px(640.)));
        // Follow the display refresh instead of the 30 Hz inactive-window cap.
        options.inactive_frame_interval = None;
        if let Some(titlebar) = options.titlebar.as_mut() {
            titlebar.title = Some("zk-ui".into());
        }

        let cli = cli.clone();
        open_window(options, cx, move |window, cx| {
            cx.new(|cx| app::ZkApp::new(window, cx, cli))
        })
        .expect("failed to open window");
    });

    Ok(())
}
