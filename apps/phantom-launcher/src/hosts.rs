//! Read-only multi-DAW detection (Cubase, Ableton Live, FL Studio, REAPER, Bitwig Studio).

use crate::manifest::PluginManifest;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub formats: &'static [&'static str],
    pub installed: bool,
    pub running: bool,
}

struct KnownHost {
    id: &'static str,
    name: &'static str,
    formats: &'static [&'static str],
    processes: &'static [&'static str],
}

const KNOWN_HOSTS: &[KnownHost] = &[
    KnownHost {
        id: "cubase",
        name: "Cubase",
        formats: &["VST3"],
        processes: &["cubase"],
    },
    KnownHost {
        id: "live",
        name: "Ableton Live",
        formats: &["VST3"],
        processes: &["ableton live"],
    },
    KnownHost {
        id: "fl",
        name: "FL Studio",
        formats: &["VST3", "CLAP"],
        processes: &["fl64", "fl studio", "osxfl"],
    },
    KnownHost {
        id: "reaper",
        name: "REAPER",
        formats: &["VST3", "CLAP"],
        processes: &["reaper"],
    },
    KnownHost {
        id: "bitwig",
        name: "Bitwig Studio",
        formats: &["CLAP", "VST3"],
        processes: &["bitwig"],
    },
];

pub fn detect_hosts() -> Vec<HostInfo> {
    let running_procs = list_running_process_names();
    KNOWN_HOSTS
        .iter()
        .map(|h| {
            let running = running_procs
                .iter()
                .any(|p| h.processes.iter().any(|needle| p.contains(needle)));
            HostInfo {
                id: h.id,
                name: h.name,
                formats: h.formats,
                installed: running || is_host_installed(h.id),
                running,
            }
        })
        .collect()
}

pub fn host_supports_plugin(host: Option<&HostInfo>, plugin: &PluginManifest) -> bool {
    match host {
        Some(h) => plugin
            .formats
            .iter()
            .any(|fmt| h.formats.iter().any(|hf| hf.eq_ignore_ascii_case(fmt))),
        None => true,
    }
}

pub fn running_hosts_for_plugin(hosts: &[HostInfo], plugin: &PluginManifest) -> Vec<String> {
    hosts
        .iter()
        .filter(|h| h.running && host_supports_plugin(Some(h), plugin))
        .map(|h| h.name.to_string())
        .collect()
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn has_entry_with_prefix(dir: &Path, prefix: &str) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    let prefix_lower = prefix.to_ascii_lowercase();
    rd.flatten().any(|e| {
        e.file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with(&prefix_lower)
    })
}

#[cfg(target_os = "windows")]
fn is_host_installed(id: &str) -> bool {
    let pf =
        PathBuf::from(std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into()));
    let pd =
        PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into()));
    match id {
        "cubase" => {
            has_entry_with_prefix(&pf.join("Steinberg"), "Cubase")
                || has_entry_with_prefix(&pd.join("Steinberg"), "Cubase")
        }
        "live" => pd.join("Ableton").exists() || has_entry_with_prefix(&pd.join("Ableton"), "Live"),
        "fl" => has_entry_with_prefix(&pf.join("Image-Line"), "FL Studio"),
        "reaper" => pf.join("REAPER (x64)").exists() || pf.join("REAPER").exists(),
        "bitwig" => pf.join("Bitwig Studio").exists(),
        _ => false,
    }
}

#[cfg(target_os = "macos")]
fn is_host_installed(id: &str) -> bool {
    let apps = Path::new("/Applications");
    match id {
        "cubase" => has_entry_with_prefix(apps, "Cubase"),
        "live" => has_entry_with_prefix(apps, "Ableton Live"),
        "fl" => has_entry_with_prefix(apps, "FL Studio"),
        "reaper" => has_entry_with_prefix(apps, "REAPER"),
        "bitwig" => has_entry_with_prefix(apps, "Bitwig Studio"),
        _ => false,
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn is_host_installed(id: &str) -> bool {
    let home = std::env::var("HOME").map(PathBuf::from).unwrap_or_default();
    let candidates: Vec<PathBuf> = match id {
        "reaper" => vec![
            home.join("opt/REAPER"),
            PathBuf::from("/opt/REAPER"),
            PathBuf::from("/usr/bin/reaper"),
            PathBuf::from("/usr/local/bin/reaper"),
        ],
        "bitwig" => vec![
            PathBuf::from("/opt/bitwig-studio"),
            PathBuf::from("/usr/bin/bitwig-studio"),
            home.join(".local/share/flatpak/app/com.bitwig.BitwigStudio"),
            PathBuf::from("/var/lib/flatpak/app/com.bitwig.BitwigStudio"),
        ],
        _ => vec![],
    };
    candidates.iter().any(|p| p.exists())
}

fn list_running_process_names() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let Ok(out) = std::process::Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        else {
            return Vec::new();
        };
        parse_process_lines(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(target_os = "macos")]
    {
        let Ok(out) = std::process::Command::new("ps")
            .args(["-axo", "comm="])
            .output()
        else {
            return Vec::new();
        };
        parse_process_lines(&String::from_utf8_lossy(&out.stdout))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return Vec::new();
        };
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let fname = entry.file_name();
            if fname.to_string_lossy().chars().all(|c| c.is_ascii_digit()) {
                if let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) {
                    let t = comm.trim().to_ascii_lowercase();
                    if !t.is_empty() {
                        names.push(t);
                    }
                }
            }
        }
        names
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", test))]
pub fn parse_process_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("INFO:") {
                return None;
            }
            let first = trimmed.split(',').next()?.trim().trim_matches('"');
            let lower = first.to_ascii_lowercase();
            (!lower.is_empty()).then_some(lower)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::find_plugin;

    #[test]
    fn parses_csv_and_plain_process_lists() {
        let sample = "\"Cubase15.exe\",\"124\",\"Console\",\"1\",\"900 K\"\n\"REAPER.exe\",\"456\",\"Console\",\"1\",\"500 K\"\nINFO: skip\n";
        let names = parse_process_lines(sample);
        assert_eq!(names, vec!["cubase15.exe", "reaper.exe"]);
    }

    #[test]
    fn running_hosts_filter_respects_formats() {
        let eq = find_plugin("phantom-eq").unwrap();
        let hosts = vec![
            HostInfo {
                id: "cubase",
                name: "Cubase",
                formats: &["VST3"],
                installed: true,
                running: true,
            },
            HostInfo {
                id: "reaper",
                name: "REAPER",
                formats: &["VST3", "CLAP"],
                installed: true,
                running: false,
            },
        ];
        assert_eq!(running_hosts_for_plugin(&hosts, eq), vec!["Cubase"]);
        assert!(host_supports_plugin(Some(&hosts[0]), eq));
    }
}
