//! Plugin action executors returning structured activity log events for the GUI.

use crate::discovery::{scan_plugin, InstallScanResult};
use crate::installer::{install_plugin, uninstall_plugin, InstallOptions};
use crate::integrity::{format_file_size, VerificationStatus};
use crate::manifest::find_plugin;
use crate::paths::InstallPaths;
use crate::ui_log::LogLevel;

pub struct ActionOutcome {
    pub messages: Vec<(LogLevel, String)>,
    pub needs_rescan: bool,
    pub updated_scan: Option<(&'static str, InstallScanResult)>,
}

pub fn execute_install(
    id: &'static str,
    paths: &InstallPaths,
    opts: InstallOptions,
) -> ActionOutcome {
    let mut messages = Vec::new();
    let Some(plugin) = find_plugin(id) else {
        return ActionOutcome {
            messages,
            needs_rescan: false,
            updated_scan: None,
        };
    };

    match install_plugin(plugin, paths, None, opts) {
        Ok(report) => {
            for w in report.warnings {
                messages.push((LogLevel::Warn, w));
            }
            for item in report.items {
                messages.push((
                    LogLevel::Success,
                    format!(
                        "Installed {} {} -> {} ({}) | SHA-256 verified: {}",
                        plugin.name,
                        item.format,
                        item.dest_path.display(),
                        format_file_size(item.size_bytes),
                        item.sha256
                    ),
                ));
            }
            ActionOutcome {
                messages,
                needs_rescan: true,
                updated_scan: None,
            }
        }
        Err(err) => {
            messages.push((
                LogLevel::Error,
                format!("Install failed for {}: {err}", plugin.name),
            ));
            ActionOutcome {
                messages,
                needs_rescan: false,
                updated_scan: None,
            }
        }
    }
}

pub fn execute_uninstall(id: &'static str, paths: &InstallPaths) -> ActionOutcome {
    let mut messages = Vec::new();
    let Some(plugin) = find_plugin(id) else {
        return ActionOutcome {
            messages,
            needs_rescan: false,
            updated_scan: None,
        };
    };

    match uninstall_plugin(plugin, paths) {
        Ok(removed) if removed.is_empty() => {
            messages.push((
                LogLevel::Info,
                format!("{} was not present in target folders.", plugin.name),
            ));
            ActionOutcome {
                messages,
                needs_rescan: false,
                updated_scan: None,
            }
        }
        Ok(removed) => {
            for p in removed {
                messages.push((
                    LogLevel::Success,
                    format!("Uninstalled {} -> removed {}", plugin.name, p.display()),
                ));
            }
            ActionOutcome {
                messages,
                needs_rescan: true,
                updated_scan: None,
            }
        }
        Err(err) => {
            messages.push((
                LogLevel::Error,
                format!("Uninstall failed for {}: {err}", plugin.name),
            ));
            ActionOutcome {
                messages,
                needs_rescan: false,
                updated_scan: None,
            }
        }
    }
}

pub fn execute_verify(id: &'static str, paths: &InstallPaths) -> ActionOutcome {
    let mut messages = Vec::new();
    let Some(plugin) = find_plugin(id) else {
        return ActionOutcome {
            messages,
            needs_rescan: false,
            updated_scan: None,
        };
    };

    let scan = scan_plugin(plugin, paths, None);
    for (fmt, cmp) in [("VST3", &scan.vst3), ("CLAP", &scan.clap)] {
        let lvl = match cmp.status {
            VerificationStatus::VerifiedMatch => LogLevel::Success,
            VerificationStatus::UpdateAvailable => LogLevel::Warn,
            _ => LogLevel::Info,
        };
        let detail = match (&cmp.source_hash, &cmp.installed_hash) {
            (Some(s), Some(i)) if s == i => format!("MATCH (SHA-256: {i})"),
            (Some(s), Some(i)) => format!("DIFFERS | src={s} | dst={i}"),
            (None, Some(i)) => format!("INSTALLED LOCAL ONLY (SHA-256: {i})"),
            (Some(s), None) => format!("NOT INSTALLED (source SHA-256: {s})"),
            (None, None) => "NOT PRESENT".to_string(),
        };
        messages.push((lvl, format!("Verify {} [{fmt}]: {detail}", plugin.name)));
    }
    ActionOutcome {
        messages,
        needs_rescan: false,
        updated_scan: Some((plugin.id, scan)),
    }
}
