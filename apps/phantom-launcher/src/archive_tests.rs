//! Unit tests for pure-Rust ZIP archive extraction, CRC-32 validation, and zip-slip guard.

use super::*;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn build_test_zip(items: &[(&str, &[u8], bool)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut cd = Vec::new();
    for &(name, data, deflate) in items {
        let local_off = out.len() as u32;
        let crc = crc32fast::hash(data);
        let (method, comp): (u16, Vec<u8>) = if deflate {
            let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
            enc.write_all(data).unwrap();
            (8, enc.finish().unwrap())
        } else {
            (0, data.to_vec())
        };
        let nbytes = name.as_bytes();
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0]);
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(nbytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(nbytes);
        out.extend_from_slice(&comp);

        cd.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0]);
        cd.extend_from_slice(&method.to_le_bytes());
        cd.extend_from_slice(&[0; 4]);
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&(comp.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cd.extend_from_slice(&(nbytes.len() as u16).to_le_bytes());
        cd.extend_from_slice(&[0; 12]);
        cd.extend_from_slice(&local_off.to_le_bytes());
        cd.extend_from_slice(nbytes);
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    out.extend_from_slice(&(items.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[test]
fn extracts_valid_store_and_deflate_and_rejects_zip_slip() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("phantom_zip_test_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    let paths = InstallPaths {
        vst3_dir: dir.join("vst3"),
        clap_dir: dir.join("clap"),
    };

    let ok_zip = dir.join("bundle.zip");
    let zip_bytes = build_test_zip(&[
        (
            "VST3/Phantom EQ.vst3/Contents/x86_64-linux/Phantom EQ.so",
            b"ELF_VST3",
            true,
        ),
        ("CLAP/Phantom EQ.clap", b"ELF_CLAP", false),
    ]);
    fs::write(&ok_zip, zip_bytes).unwrap();
    let rep = install_from_zip(&ok_zip, &paths, InstallOptions::default()).unwrap();
    assert_eq!(rep.extracted_files.len(), 2);
    assert_eq!(rep.detected_plugin_ids, vec!["phantom-eq"]);
    assert_eq!(
        fs::read(paths.clap_dir.join("Phantom EQ.clap")).unwrap(),
        b"ELF_CLAP"
    );

    let bad_zip = dir.join("evil.zip");
    fs::write(&bad_zip, build_test_zip(&[("../evil.dll", b"BAD", false)])).unwrap();
    assert!(install_from_zip(&bad_zip, &paths, InstallOptions::default()).is_err());
    assert!(!dir.join("evil.dll").exists());

    let _ = fs::remove_dir_all(&dir);
}
