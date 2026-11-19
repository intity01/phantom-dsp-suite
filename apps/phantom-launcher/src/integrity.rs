//! Real SHA-256 cryptographic hashing and binary integrity verification (100% offline).

use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufReader, Read};
use std::path::Path;

const READ_BUF_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationStatus {
    /// Both source and installed binaries exist and their SHA-256 hashes match identically.
    VerifiedMatch,
    /// Both exist, but hashes differ (source is newer/different than installed).
    UpdateAvailable,
    /// Installed binary exists, no local source binary available to compare against.
    InstalledLocalOnly,
    /// Plugin is not installed at the target path.
    NotInstalled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashComparison {
    pub status: VerificationStatus,
    pub source_hash: Option<String>,
    pub installed_hash: Option<String>,
    pub source_size: Option<u64>,
    pub installed_size: Option<u64>,
}

impl HashComparison {
    pub fn display_hash(&self) -> Option<&str> {
        self.installed_hash
            .as_deref()
            .or(self.source_hash.as_deref())
    }

    pub fn display_size(&self) -> Option<u64> {
        self.installed_size.or(self.source_size)
    }
}

/// Computes lowercase hex SHA-256 digest of an in-memory byte slice.
#[cfg(test)]
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    bytes_to_hex(&hasher.finalize())
}

/// Computes lowercase hex SHA-256 digest of a file using streaming 64 KiB buffered reads.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(READ_BUF_SIZE, file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; READ_BUF_SIZE];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(bytes_to_hex(&hasher.finalize()))
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Compact human-readable SHA-256 fingerprint preview (e.g. `e3b0c44298…7852b855`).
pub fn short_fingerprint(hash: &str) -> String {
    if hash.len() <= 20 {
        return hash.to_string();
    }
    format!("{}…{}", &hash[..10], &hash[hash.len() - 8..])
}

pub fn format_file_size(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const KB: f64 = 1024.0;
    if bytes as f64 >= MB {
        format!("{:.2} MB", bytes as f64 / MB)
    } else if bytes as f64 >= KB {
        format!("{:.1} KB", bytes as f64 / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Computes SHA-256 hashes & sizes for an optional source and installed binary pair.
pub fn compare_files(source: Option<&Path>, installed: Option<&Path>) -> HashComparison {
    let (source_hash, source_size) = hash_and_size(source);
    let (installed_hash, installed_size) = hash_and_size(installed);

    let status = match (&source_hash, &installed_hash) {
        (Some(s), Some(i)) if s == i => VerificationStatus::VerifiedMatch,
        (Some(_), Some(_)) => VerificationStatus::UpdateAvailable,
        (None, Some(_)) => VerificationStatus::InstalledLocalOnly,
        (_, None) => VerificationStatus::NotInstalled,
    };

    HashComparison {
        status,
        source_hash,
        installed_hash,
        source_size,
        installed_size,
    }
}

fn hash_and_size(path: Option<&Path>) -> (Option<String>, Option<u64>) {
    let Some(p) = path else {
        return (None, None);
    };
    if !p.is_file() {
        return (None, None);
    }
    let size = fs::metadata(p).ok().map(|m| m.len());
    let hash = sha256_file(p).ok();
    (hash, size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn sha256_matches_nist_vectors() {
        assert_eq!(
            sha256_bytes(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_file_and_compare_states() {
        let uniq = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("phantom_integrity_test_{uniq}"));
        fs::create_dir_all(&dir).unwrap();

        let src = dir.join("src.bin");
        let dst = dir.join("dst.bin");
        fs::write(&src, b"phantom-dsp-binary-v1").unwrap();

        let not_inst = compare_files(Some(&src), Some(&dst));
        assert_eq!(not_inst.status, VerificationStatus::NotInstalled);
        assert!(not_inst.source_hash.is_some());

        fs::write(&dst, b"phantom-dsp-binary-v1").unwrap();
        let matched = compare_files(Some(&src), Some(&dst));
        assert_eq!(matched.status, VerificationStatus::VerifiedMatch);
        assert_eq!(matched.source_hash, matched.installed_hash);

        fs::write(&dst, b"phantom-dsp-binary-old").unwrap();
        let update = compare_files(Some(&src), Some(&dst));
        assert_eq!(update.status, VerificationStatus::UpdateAvailable);

        let local_only = compare_files(None, Some(&dst));
        assert_eq!(local_only.status, VerificationStatus::InstalledLocalOnly);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn fingerprint_and_size_formatting() {
        let h = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(short_fingerprint(h), "e3b0c44298…7852b855");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(2048), "2.0 KB");
        assert_eq!(format_file_size(5 * 1024 * 1024), "5.00 MB");
    }
}
