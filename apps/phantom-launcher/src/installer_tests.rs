//! Temp-dir end-to-end unit tests for discovery, install, SHA-256 verification, and uninstall.

use super::*;
use crate::discovery::{locate_source_artifacts, scan_plugin};
use crate::integrity::VerificationStatus;
use crate::manifest::{find_plugin, vst3_inner_binary_rel};
use crate::paths::{host_dist_os_dir, InstallPaths};
use std::time::{SystemTime, UNIX_EPOCH};

fn make_temp_sandbox(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("phantom_launcher_{tag}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn install_verify_update_and_uninstall_roundtrip() {
    let sandbox = make_temp_sandbox("roundtrip");
    let dist_os = sandbox.join("dist").join(host_dist_os_dir());
    let plugin = find_plugin("phantom-eq").unwrap();

    // Build synthetic VST3 bundle (nested layout) and CLAP file
    let vst3_bundle = dist_os.join("vst3").join(plugin.vst3_bundle);
    let vst3_bin = vst3_bundle.join(vst3_inner_binary_rel(plugin));
    fs::create_dir_all(vst3_bin.parent().unwrap()).unwrap();
    fs::write(&vst3_bin, b"PHANTOM_EQ_VST3_V1_PAYLOAD").unwrap();

    let clap_src = dist_os.join("clap").join(plugin.clap_file);
    fs::create_dir_all(clap_src.parent().unwrap()).unwrap();
    fs::write(&clap_src, b"PHANTOM_EQ_CLAP_V1_PAYLOAD").unwrap();

    let paths = InstallPaths {
        vst3_dir: sandbox.join("target_vst3"),
        clap_dir: sandbox.join("target_clap"),
    };

    // 1. Before install: NotInstalled, sources discovered
    let pre = scan_plugin(plugin, &paths, Some(&sandbox));
    assert!(!pre.is_any_installed());
    assert!(pre.has_any_source());
    assert_eq!(pre.vst3.status, VerificationStatus::NotInstalled);
    assert_eq!(pre.clap.status, VerificationStatus::NotInstalled);

    // 2. Install both VST3 & CLAP
    let report = install_plugin(plugin, &paths, Some(&sandbox), InstallOptions::default())
        .expect("install should succeed");
    assert_eq!(report.items.len(), 2);

    // 3. Scan after install: VerifiedMatch for both formats
    let post = scan_plugin(plugin, &paths, Some(&sandbox));
    assert!(post.is_any_installed());
    assert!(post.is_verified_match());
    assert_eq!(post.vst3.status, VerificationStatus::VerifiedMatch);
    assert_eq!(post.clap.status, VerificationStatus::VerifiedMatch);
    assert_eq!(post.vst3.source_hash, post.vst3.installed_hash);
    assert_eq!(post.clap.source_hash, post.clap.installed_hash);

    // 4. Modify source binary to simulate a new release -> UpdateAvailable
    fs::write(&vst3_bin, b"PHANTOM_EQ_VST3_V2_PAYLOAD").unwrap();
    let updated = scan_plugin(plugin, &paths, Some(&sandbox));
    assert!(updated.has_update());
    assert_eq!(updated.vst3.status, VerificationStatus::UpdateAvailable);

    // 5. Uninstall cleanly removes both targets
    let removed = uninstall_plugin(plugin, &paths).expect("uninstall should succeed");
    assert_eq!(removed.len(), 2);
    let after_rm = scan_plugin(plugin, &paths, Some(&sandbox));
    assert!(!after_rm.is_any_installed());
    assert_eq!(after_rm.vst3.status, VerificationStatus::NotInstalled);
    assert_eq!(after_rm.clap.status, VerificationStatus::NotInstalled);

    let _ = fs::remove_dir_all(&sandbox);
}

#[test]
fn locates_flat_linux_style_dist_layout() {
    let sandbox = make_temp_sandbox("flat_layout");
    let dist_os = sandbox.join("dist").join(host_dist_os_dir());
    let plugin = find_plugin("phantom-eq").unwrap();

    let vst3_bundle = dist_os.join(plugin.vst3_bundle);
    let vst3_bin = vst3_bundle.join(vst3_inner_binary_rel(plugin));
    fs::create_dir_all(vst3_bin.parent().unwrap()).unwrap();
    fs::write(&vst3_bin, b"FLAT_VST3_PAYLOAD").unwrap();

    let clap_src = dist_os.join(plugin.clap_file);
    fs::write(&clap_src, b"FLAT_CLAP_PAYLOAD").unwrap();

    let found = locate_source_artifacts(plugin, Some(&sandbox));
    assert_eq!(found.vst3_binary.as_deref(), Some(vst3_bin.as_path()));
    assert_eq!(found.clap_file.as_deref(), Some(clap_src.as_path()));

    let _ = fs::remove_dir_all(&sandbox);
}
