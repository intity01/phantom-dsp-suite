//! Phantom Launcher — Open-Source Audio Plugin Hub & 1-Click Offline Installer.
//! Supports headless CLI mode (`--cli`, `--install <id>`, `--uninstall <id>`, `--verify <id>`)
//! and the native Blue-Zinc `eframe`/`egui` desktop GUI (zero WebView).

mod app;
mod app_actions;
mod app_handlers;
mod app_overlays;
mod app_state;
mod archive;
mod brand;
mod brand_raster;
mod cli;
mod discovery;
mod hosts;
mod installer;
mod integrity;
mod manifest;
mod paths;
mod platform;
mod store;
mod theme;
mod ui_about;
mod ui_art;
mod ui_grid;
mod ui_icons;
mod ui_inspector;
mod ui_inspector_parts;
mod ui_log;
mod ui_settings;
mod ui_sidebar;
mod ui_tile;
mod ui_toolbar;

use app::PhantomLauncherApp;
use brand::load_window_icon;
use cli::{handle_cli_args, CliMode};
use eframe::egui;
use std::sync::Arc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let CliMode::ExitCode(code) = handle_cli_args(&args) {
        std::process::exit(code);
    }

    // Fixed-size window: the layout is designed for exactly this size, so no resizing or
    // maximizing (the maximize button is removed and edges can't be dragged).
    const WINDOW_SIZE: [f32; 2] = [1180.0, 760.0];
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Phantom DSP Suite — Offline Plugin Hub")
        .with_inner_size(WINDOW_SIZE)
        .with_min_inner_size(WINDOW_SIZE)
        .with_max_inner_size(WINDOW_SIZE)
        .with_resizable(false)
        .with_maximize_button(false);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    if let Err(err) = eframe::run_native(
        "Phantom Launcher",
        options,
        Box::new(|cc| Ok(Box::new(PhantomLauncherApp::new(cc)))),
    ) {
        eprintln!("[ERROR] Failed to launch Phantom Launcher GUI: {err}");
        eprintln!("Tip: Run with `--cli` or `--install phantom-eq` for headless mode.");
        std::process::exit(1);
    }
}
