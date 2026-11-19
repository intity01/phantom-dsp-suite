//! Cross-platform VST3 and CLAP User / System / Custom directory resolution.

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScope {
    User,
    System,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPaths {
    pub vst3_dir: PathBuf,
    pub clap_dir: PathBuf,
}

/// Platform directory name under `dist/` (`windows`, `linux`, `macos`).
pub fn host_dist_os_dir() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "linux"
    }
}

pub fn default_user_vst3_dir() -> PathBuf {
    user_vst3_dir_for_env(
        std::env::var("LOCALAPPDATA").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

pub fn default_system_vst3_dir() -> PathBuf {
    system_vst3_dir_for_env(std::env::var("CommonProgramFiles").ok().as_deref())
}

pub fn default_user_clap_dir() -> PathBuf {
    user_clap_dir_for_env(
        std::env::var("LOCALAPPDATA").ok().as_deref(),
        std::env::var("HOME").ok().as_deref(),
    )
}

pub fn default_system_clap_dir() -> PathBuf {
    system_clap_dir_for_env(std::env::var("CommonProgramFiles").ok().as_deref())
}

pub fn user_vst3_dir_for_env(local_app_data: Option<&str>, home: Option<&str>) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let _ = home;
        if let Some(base) = local_app_data.filter(|s| !s.is_empty()) {
            return PathBuf::from(base)
                .join("Programs")
                .join("Common")
                .join("VST3");
        }
        PathBuf::from(r"C:\Program Files\Common Files\VST3")
    }
    #[cfg(target_os = "macos")]
    {
        let _ = local_app_data;
        if let Some(h) = home.filter(|s| !s.is_empty()) {
            return PathBuf::from(h).join("Library/Audio/Plug-Ins/VST3");
        }
        PathBuf::from("/Library/Audio/Plug-Ins/VST3")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = local_app_data;
        if let Some(h) = home.filter(|s| !s.is_empty()) {
            return PathBuf::from(h).join(".vst3");
        }
        PathBuf::from("/usr/lib/vst3")
    }
}

pub fn system_vst3_dir_for_env(common_program_files: Option<&str>) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = common_program_files.filter(|s| !s.is_empty()) {
            return PathBuf::from(base).join("VST3");
        }
        PathBuf::from(r"C:\Program Files\Common Files\VST3")
    }
    #[cfg(target_os = "macos")]
    {
        let _ = common_program_files;
        PathBuf::from("/Library/Audio/Plug-Ins/VST3")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = common_program_files;
        PathBuf::from("/usr/lib/vst3")
    }
}

pub fn user_clap_dir_for_env(local_app_data: Option<&str>, home: Option<&str>) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let _ = home;
        if let Some(base) = local_app_data.filter(|s| !s.is_empty()) {
            return PathBuf::from(base)
                .join("Programs")
                .join("Common")
                .join("CLAP");
        }
        PathBuf::from(r"C:\Program Files\Common Files\CLAP")
    }
    #[cfg(target_os = "macos")]
    {
        let _ = local_app_data;
        if let Some(h) = home.filter(|s| !s.is_empty()) {
            return PathBuf::from(h).join("Library/Audio/Plug-Ins/CLAP");
        }
        PathBuf::from("/Library/Audio/Plug-Ins/CLAP")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = local_app_data;
        if let Some(h) = home.filter(|s| !s.is_empty()) {
            return PathBuf::from(h).join(".clap");
        }
        PathBuf::from("/usr/lib/clap")
    }
}

pub fn system_clap_dir_for_env(common_program_files: Option<&str>) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(base) = common_program_files.filter(|s| !s.is_empty()) {
            return PathBuf::from(base).join("CLAP");
        }
        PathBuf::from(r"C:\Program Files\Common Files\CLAP")
    }
    #[cfg(target_os = "macos")]
    {
        let _ = common_program_files;
        PathBuf::from("/Library/Audio/Plug-Ins/CLAP")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = common_program_files;
        PathBuf::from("/usr/lib/clap")
    }
}

pub fn resolve_install_paths(
    scope: InstallScope,
    custom_vst3: &str,
    custom_clap: &str,
) -> InstallPaths {
    match scope {
        InstallScope::User => InstallPaths {
            vst3_dir: default_user_vst3_dir(),
            clap_dir: default_user_clap_dir(),
        },
        InstallScope::System => InstallPaths {
            vst3_dir: default_system_vst3_dir(),
            clap_dir: default_system_clap_dir(),
        },
        InstallScope::Custom => InstallPaths {
            vst3_dir: if custom_vst3.trim().is_empty() {
                default_user_vst3_dir()
            } else {
                PathBuf::from(custom_vst3.trim())
            },
            clap_dir: if custom_clap.trim().is_empty() {
                default_user_clap_dir()
            } else {
                PathBuf::from(custom_clap.trim())
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_scope_overrides_and_fallbacks() {
        let paths = resolve_install_paths(InstallScope::Custom, "/tmp/my-vst3", "/tmp/my-clap");
        assert_eq!(paths.vst3_dir, PathBuf::from("/tmp/my-vst3"));
        assert_eq!(paths.clap_dir, PathBuf::from("/tmp/my-clap"));

        let fallback = resolve_install_paths(InstallScope::Custom, "   ", "");
        assert_eq!(fallback.vst3_dir, default_user_vst3_dir());
        assert_eq!(fallback.clap_dir, default_user_clap_dir());
    }

    #[test]
    fn env_path_resolution_includes_format_segment() {
        let vst3 =
            user_vst3_dir_for_env(Some(r"C:\Users\Test\AppData\Local"), Some("/home/tester"));
        let clap =
            user_clap_dir_for_env(Some(r"C:\Users\Test\AppData\Local"), Some("/home/tester"));
        assert!(vst3.to_string_lossy().to_lowercase().contains("vst3"));
        assert!(clap.to_string_lossy().to_lowercase().contains("clap"));

        let sys_v = system_vst3_dir_for_env(Some(r"C:\Program Files\Common Files"));
        let sys_c = system_clap_dir_for_env(Some(r"C:\Program Files\Common Files"));
        assert!(sys_v.to_string_lossy().to_lowercase().contains("vst3"));
        assert!(sys_c.to_string_lossy().to_lowercase().contains("clap"));
    }
}
