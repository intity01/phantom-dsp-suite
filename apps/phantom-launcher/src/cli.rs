//! Headless / scripted CLI interface for Phantom Launcher (`--cli`, `--install`, `--uninstall`, `--verify`).

use crate::discovery::scan_plugin;
use crate::installer::{install_plugin, uninstall_plugin, InstallOptions};
use crate::integrity::{format_file_size, short_fingerprint, VerificationStatus};
use crate::manifest::{find_plugin, PluginStatus, AVAILABLE_PLUGINS};
use crate::paths::{resolve_install_paths, InstallPaths, InstallScope};
use std::path::PathBuf;

pub enum CliMode {
    LaunchGui,
    ExitCode(i32),
}

pub fn handle_cli_args(args: &[String]) -> CliMode {
    if args.is_empty() {
        return CliMode::LaunchGui;
    }

    let mut scope = InstallScope::User;
    let mut custom_vst3 = String::new();
    let mut custom_clap = String::new();
    let mut source_override: Option<PathBuf> = None;
    let mut opts = InstallOptions::default();

    let mut action_list = false;
    let mut action_install: Option<String> = None;
    let mut action_uninstall: Option<String> = None;
    let mut action_verify: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return CliMode::ExitCode(0);
            }
            "--cli" | "--list" => action_list = true,
            "--system" => scope = InstallScope::System,
            "--user" => scope = InstallScope::User,
            "--no-vst3" => opts.install_vst3 = false,
            "--no-clap" => opts.install_clap = false,
            "--vst3-dir" => {
                if let Some(v) = args.get(i + 1) {
                    custom_vst3 = v.clone();
                    scope = InstallScope::Custom;
                    i += 1;
                }
            }
            "--clap-dir" => {
                if let Some(v) = args.get(i + 1) {
                    custom_clap = v.clone();
                    scope = InstallScope::Custom;
                    i += 1;
                }
            }
            "--source" => {
                if let Some(v) = args.get(i + 1) {
                    source_override = Some(PathBuf::from(v));
                    i += 1;
                }
            }
            "--install" => {
                action_install = Some(
                    args.get(i + 1)
                        .cloned()
                        .unwrap_or_else(|| "phantom-eq".into()),
                );
                if args.get(i + 1).is_some() {
                    i += 1;
                }
            }
            "--uninstall" => {
                action_uninstall = Some(
                    args.get(i + 1)
                        .cloned()
                        .unwrap_or_else(|| "phantom-eq".into()),
                );
                if args.get(i + 1).is_some() {
                    i += 1;
                }
            }
            "--verify" => {
                action_verify = Some(
                    args.get(i + 1)
                        .cloned()
                        .unwrap_or_else(|| "phantom-eq".into()),
                );
                if args.get(i + 1).is_some() {
                    i += 1;
                }
            }
            other => {
                eprintln!("Unknown argument: {other}\nRun with --help for usage.");
                return CliMode::ExitCode(2);
            }
        }
        i += 1;
    }

    let paths = resolve_install_paths(scope, &custom_vst3, &custom_clap);
    let src = source_override.as_deref();

    if let Some(id) = action_install {
        return CliMode::ExitCode(run_install(&id, &paths, src, opts));
    }
    if let Some(id) = action_uninstall {
        return CliMode::ExitCode(run_uninstall(&id, &paths));
    }
    if let Some(id) = action_verify {
        return CliMode::ExitCode(run_verify(&id, &paths, src));
    }
    if action_list {
        print_catalog(&paths, src);
        return CliMode::ExitCode(0);
    }

    CliMode::LaunchGui
}

fn print_help() {
    println!(
        "Phantom Launcher — 100% Offline, Zero-Telemetry Plugin Hub & Installer\n\
         \n\
         USAGE:\n\
           phantom-launcher                       Launch Desktop GUI\n\
           phantom-launcher --cli                 List plugins, target paths & SHA-256 status\n\
           phantom-launcher --install <id>        Install plugin (e.g. phantom-eq)\n\
           phantom-launcher --uninstall <id>      Remove installed VST3 & CLAP files\n\
           phantom-launcher --verify <id>         Compute & compare SHA-256 checksums\n\
         \n\
         OPTIONS:\n\
           --user | --system                      Choose User (default) or System plugin directory\n\
           --vst3-dir <path> --clap-dir <path>    Override target directories\n\
           --source <path>                        Override local dist/ directory root\n\
           --no-vst3 | --no-clap                  Skip one plugin format"
    );
}

fn print_catalog(paths: &InstallPaths, src: Option<&std::path::Path>) {
    println!("=============================================================");
    println!("  PHANTOM LAUNCHER — 100% OFFLINE • ZERO TELEMETRY");
    println!("=============================================================");
    println!("  VST3 Target : {}", paths.vst3_dir.display());
    println!("  CLAP Target : {}", paths.clap_dir.display());
    println!();
    for p in AVAILABLE_PLUGINS {
        let scan = scan_plugin(p, paths, src);
        let badge = match p.status {
            PluginStatus::Planned => "PLANNED",
            PluginStatus::Ready if scan.is_verified_match() => "INSTALLED (VERIFIED)",
            PluginStatus::Ready if scan.has_update() => "UPDATE AVAILABLE",
            PluginStatus::Ready if scan.is_any_installed() => "INSTALLED",
            PluginStatus::Ready => "READY",
        };
        println!("  [{}] {} v{}  [{}]", p.id, p.name, p.version, badge);
        if let Some(h) = scan.primary_hash() {
            println!("      SHA-256: {}", short_fingerprint(h));
        }
    }
}

fn run_install(
    id: &str,
    paths: &InstallPaths,
    src: Option<&std::path::Path>,
    opts: InstallOptions,
) -> i32 {
    let Some(plugin) = find_plugin(id) else {
        eprintln!("[ERROR] Unknown plugin id '{id}'.");
        return 1;
    };
    if plugin.status == PluginStatus::Planned {
        eprintln!(
            "[ERROR] '{}' is a planned plugin and not yet available.",
            plugin.name
        );
        return 1;
    }
    match install_plugin(plugin, paths, src, opts) {
        Ok(report) => {
            for warn in &report.warnings {
                eprintln!("[WARN] {warn}");
            }
            for item in &report.items {
                println!(
                    "[OK] {} installed -> {} ({})\n     SHA-256: {}",
                    item.format,
                    item.dest_path.display(),
                    format_file_size(item.size_bytes),
                    item.sha256
                );
            }
            0
        }
        Err(err) => {
            eprintln!("[ERROR] {err}");
            1
        }
    }
}

fn run_uninstall(id: &str, paths: &InstallPaths) -> i32 {
    let Some(plugin) = find_plugin(id) else {
        eprintln!("[ERROR] Unknown plugin id '{id}'.");
        return 1;
    };
    match uninstall_plugin(plugin, paths) {
        Ok(removed) if removed.is_empty() => {
            println!(
                "[INFO] {} was not installed in target directories.",
                plugin.name
            );
            0
        }
        Ok(removed) => {
            for path in removed {
                println!("[OK] Removed {}", path.display());
            }
            0
        }
        Err(err) => {
            eprintln!("[ERROR] {err}");
            1
        }
    }
}

fn run_verify(id: &str, paths: &InstallPaths, src: Option<&std::path::Path>) -> i32 {
    let Some(plugin) = find_plugin(id) else {
        eprintln!("[ERROR] Unknown plugin id '{id}'.");
        return 1;
    };
    let scan = scan_plugin(plugin, paths, src);
    for (label, cmp) in [("VST3", &scan.vst3), ("CLAP", &scan.clap)] {
        println!("[{label}] Status: {:?}", cmp.status);
        if let Some(h) = &cmp.source_hash {
            println!("  Source SHA-256   : {h}");
        }
        if let Some(h) = &cmp.installed_hash {
            println!("  Installed SHA-256: {h}");
        }
    }
    if scan.vst3.status == VerificationStatus::UpdateAvailable
        || scan.clap.status == VerificationStatus::UpdateAvailable
    {
        return 2;
    }
    0
}
