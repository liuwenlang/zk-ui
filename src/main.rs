mod app;
mod config;
mod db;
mod zk;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> anyhow::Result<()> {
    // Set panic hook to get better diagnostics on crash
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("PANIC: {}", info);
        let bt = std::backtrace::Backtrace::capture();
        eprintln!("Backtrace:\n{}", bt);
        default_hook(info);
    }));

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::Cli::parse();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 500.0])
            .with_title("zk-ui — ZooKeeper Visualization Tool"),
        ..Default::default()
    };

    eframe::run_native(
        "zk-ui",
        native_options,
        Box::new(|cc| Ok(Box::new(app::ZkApp::new(cc, config)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}
