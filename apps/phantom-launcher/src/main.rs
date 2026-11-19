//! Phantom Launcher — Open-Source Audio Plugin Hub & 1-Click Offline Installer.
//! Supports headless CLI mode (`--cli`, `--install <id>`, `--uninstall <id>`, `--verify <id>`)
//! and the Dark Obsidian `eframe`/`egui` desktop GUI.

mod app;
mod app_actions;
mod cli;
mod discovery;
mod installer;
mod integrity;
mod manifest;
mod paths;
mod platform;
mod theme;
mod ui_cards;
mod ui_log;
mod ui_panels;

use app::PhantomLauncherApp;
use cli::{handle_cli_args, CliMode};
use eframe::egui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let CliMode::ExitCode(code) = handle_cli_args(&args) {
        std::process::exit(code);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Phantom Launcher — Open-Source DSP Plugin Hub")
            .with_inner_size([960.0, 720.0])
            .with_min_inner_size([780.0, 540.0]),
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
