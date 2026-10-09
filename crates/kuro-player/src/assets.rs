//! Bundled mpv assets (area 45 `PlayerActivity.copyAssets`, T-45a-028). `subfont.ttf` is the
//! fallback subtitle font (DejaVu Sans, see the crate README note in DECISIONS). `cacert.pem` is
//! the TLS root bundle that `tls-ca-file` points at.

use std::fs;
use std::path::Path;

pub struct BundledAsset {
    pub name: &'static str,
    pub bytes: &'static [u8],
}

pub const ASSETS: [BundledAsset; 2] = [
    BundledAsset {
        name: "subfont.ttf",
        bytes: include_bytes!("../assets/subfont.ttf"),
    },
    BundledAsset {
        name: "cacert.pem",
        bytes: include_bytes!("../assets/cacert.pem"),
    },
];

/// Installs every bundled asset into `mpv_dir`. A file that already exists with the same length
/// is kept (the Kotlin rule compares lengths only). A failed copy is logged and the rest still run.
pub fn install_assets(mpv_dir: &Path) {
    if let Err(e) = fs::create_dir_all(mpv_dir) {
        tracing::error!(error = %e, "Failed to create mpv directory");
        return;
    }
    for asset in &ASSETS {
        let target = mpv_dir.join(asset.name);
        let same_len = fs::metadata(&target).is_ok_and(|m| m.len() == asset.bytes.len() as u64);
        if same_len {
            tracing::debug!(
                "Skipping copy of asset file (exists same size): {}",
                asset.name
            );
            continue;
        }
        if fs::write(&target, asset.bytes).is_err() {
            tracing::error!("Failed to copy asset file: {}", asset.name);
        } else {
            tracing::warn!("Copied asset file: {}", asset.name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_both_assets_with_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        install_assets(dir.path());
        for asset in &ASSETS {
            assert_eq!(fs::read(dir.path().join(asset.name)).unwrap(), asset.bytes);
        }
    }

    #[test]
    fn same_length_file_is_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let cert = &ASSETS[1];
        let target = dir.path().join(cert.name);
        let placeholder = vec![b'x'; cert.bytes.len()];
        fs::write(&target, &placeholder).unwrap();
        install_assets(dir.path());
        assert_eq!(fs::read(&target).unwrap(), placeholder);
    }

    #[test]
    fn different_length_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let cert = &ASSETS[1];
        let target = dir.path().join(cert.name);
        fs::write(&target, b"short").unwrap();
        install_assets(dir.path());
        assert_eq!(fs::read(&target).unwrap(), cert.bytes);
    }
}
