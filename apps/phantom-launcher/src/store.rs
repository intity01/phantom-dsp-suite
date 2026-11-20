//! Persistent offline settings (`settings.json`) and portable plugin list export/import.

use crate::discovery::InstallScanResult;
use crate::manifest::{find_plugin, DEFAULT_GITHUB_REPO};
use crate::paths::InstallScope;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct PersistedSettings {
    pub scope: String,
    pub custom_vst3: String,
    pub custom_clap: String,
    pub install_vst3: bool,
    pub install_clap: bool,
    pub auto_rescan_daws: bool,
    pub github_repo: String,
}

impl Default for PersistedSettings {
    fn default() -> Self {
        Self {
            scope: "user".into(),
            custom_vst3: String::new(),
            custom_clap: String::new(),
            install_vst3: true,
            install_clap: true,
            auto_rescan_daws: true,
            github_repo: DEFAULT_GITHUB_REPO.into(),
        }
    }
}

impl PersistedSettings {
    pub fn install_scope(&self) -> InstallScope {
        match self.scope.as_str() {
            "system" => InstallScope::System,
            "custom" => InstallScope::Custom,
            _ => InstallScope::User,
        }
    }

    pub fn scope_str(scope: InstallScope) -> &'static str {
        match scope {
            InstallScope::User => "user",
            InstallScope::System => "system",
            InstallScope::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportedPluginEntry {
    pub id: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportedList {
    pub app: String,
    pub exported: String,
    pub plugins: Vec<ExportedPluginEntry>,
}

pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            if !local.is_empty() {
                return PathBuf::from(local).join("PhantomDSP");
            }
        }
        PathBuf::from(r"C:\ProgramData\PhantomDSP")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join("Library/Application Support/PhantomDSP")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".config/phantom-dsp")
    }
}

pub fn settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

pub fn plugin_list_path() -> PathBuf {
    app_data_dir().join("phantom-plugin-list.json")
}

pub fn load_settings() -> PersistedSettings {
    let Ok(raw) = fs::read_to_string(settings_path()) else {
        return PersistedSettings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save_settings(settings: &PersistedSettings) {
    let dir = app_data_dir();
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(settings_path(), json);
        }
    }
}

pub fn build_exported_list(
    scans: &HashMap<&'static str, InstallScanResult>,
    timestamp: String,
) -> ExportedList {
    let mut entries: Vec<ExportedPluginEntry> = scans
        .iter()
        .filter(|(_, scan)| scan.is_any_installed())
        .filter_map(|(id, scan)| {
            let manifest = find_plugin(id)?;
            Some(ExportedPluginEntry {
                id: manifest.id.to_string(),
                version: manifest.version.to_string(),
                sha256: scan.primary_hash().map(|s| s.to_string()),
            })
        })
        .collect();
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    ExportedList {
        app: "phantom-dsp-suite".into(),
        exported: timestamp,
        plugins: entries,
    }
}

pub fn export_installed_list(
    scans: &HashMap<&'static str, InstallScanResult>,
    timestamp: String,
) -> Result<(PathBuf, String), String> {
    let list = build_exported_list(scans, timestamp);
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    let path = plugin_list_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed creating {}: {e}", parent.display()))?;
    }
    fs::write(&path, &json).map_err(|e| format!("Failed writing {}: {e}", path.display()))?;
    Ok((path, json))
}

pub fn parse_exported_list(raw: &str) -> Result<ExportedList, String> {
    let list: ExportedList =
        serde_json::from_str(raw).map_err(|e| format!("Invalid plugin list JSON: {e}"))?;
    if list.app != "phantom-dsp-suite" {
        return Err("Not a Phantom DSP Suite plugin list".into());
    }
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_and_exported_list_json_roundtrip() {
        let s = PersistedSettings {
            scope: "custom".into(),
            custom_vst3: "/tmp/v".into(),
            ..Default::default()
        };
        let encoded = serde_json::to_string(&s).unwrap();
        let decoded: PersistedSettings = serde_json::from_str(&encoded).unwrap();
        assert_eq!(s, decoded);
        assert_eq!(decoded.install_scope(), InstallScope::Custom);

        let list = ExportedList {
            app: "phantom-dsp-suite".into(),
            exported: "12:00:00".into(),
            plugins: vec![ExportedPluginEntry {
                id: "phantom-eq".into(),
                version: "0.1.0".into(),
                sha256: Some("abc".into()),
            }],
        };
        let list_json = serde_json::to_string_pretty(&list).unwrap();
        assert_eq!(parse_exported_list(&list_json).unwrap(), list);
        assert!(parse_exported_list(r#"{"app":"other","exported":"","plugins":[]}"#).is_err());
    }
}
