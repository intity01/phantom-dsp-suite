//! OS integration helpers: Cubase process detection, opening folders/URLs, and UTC timestamps.

use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Detects whether Cubase appears to be running on Windows (non-blocking check).
/// Returns the matched process name (e.g. `"Cubase15.exe"`) if detected.
pub fn detect_running_cubase() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let output = Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Cubase*", "/FO", "CSV", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .ok()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_tasklist_cubase(&stdout)
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Parses CSV `tasklist /FO CSV /NH` output for a running Cubase executable.
#[cfg(any(target_os = "windows", test))]
pub fn parse_tasklist_cubase(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("INFO:") {
            continue;
        }
        let first_col = trimmed.split(',').next()?.trim().trim_matches('"');
        if first_col.to_ascii_lowercase().starts_with("cubase")
            && first_col.to_ascii_lowercase().ends_with(".exe")
        {
            return Some(first_col.to_string());
        }
    }
    None
}

/// Opens a directory in the host OS file manager (creates directory if requested/absent).
pub fn open_folder(path: &Path) -> Result<(), String> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("Could not create folder {}: {e}", path.display()))?;
    }

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("explorer");
        c.arg(path);
        c
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(path);
        c
    };

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(path);
        c
    };

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open {}: {e}", path.display()))
}

/// Opens a validated HTTP(S) URL in the user's default browser (only on explicit user action).
pub fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Refused to open non-HTTP(S) URL".to_string());
    }

    #[cfg(target_os = "windows")]
    let mut cmd = {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut c = Command::new("cmd");
        c.args(["/C", "start", "", url])
            .creation_flags(CREATE_NO_WINDOW);
        c
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = Command::new("open");
        c.arg(url);
        c
    };

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut cmd = {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };

    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to open browser for {url}: {e}"))
}

/// Returns current UTC `HH:MM:SS` timestamp without pulling external date crates.
pub fn utc_hms_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let day_secs = secs % 86_400;
    let h = day_secs / 3_600;
    let m = (day_secs % 3_600) / 60;
    let s = day_secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tasklist_cubase_detection() {
        let found = "\"Cubase15.exe\",\"12480\",\"Console\",\"1\",\"1,245,000 K\"\r\n";
        assert_eq!(
            parse_tasklist_cubase(found).as_deref(),
            Some("Cubase15.exe")
        );

        let missing = "INFO: No tasks are running which match the specified criteria.\r\n";
        assert_eq!(parse_tasklist_cubase(missing), None);
    }

    #[test]
    fn rejects_non_http_urls() {
        assert!(open_url("file:///etc/passwd").is_err());
        assert!(open_url("javascript:alert(1)").is_err());
    }
}
