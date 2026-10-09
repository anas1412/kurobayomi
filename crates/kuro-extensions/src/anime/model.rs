//! Core anime extension types (T-19a-001, `p1-rust-design.md` section 2). Field meanings follow the Aniyomi model
//! (`AnimeExtension.kt`) except where the Kurobayomi design says otherwise.

use std::path::PathBuf;

use super::abi::AbiVersion;

/// Packaging kind of a store entry. Decided in `store::map`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A valid `kuroext` block is present. Installable.
    Wasm,
    /// A `kuroext` block is present, but its `ed25519` or `publicKey` is missing or malformed. Listed, not installable.
    Unsigned,
    /// An Aniyomi APK entry, or a `kuroext` block with a malformed `sha256` or `url`. Listed, not installable.
    ApkOnly,
}

/// Lowercase hex SHA-256 of a 32-byte ed25519 public key. It identifies the signer of a package.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SignerDigest(pub String);

/// One source a package provides. `id` is an i64 end to end, never a float.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimeSourceRef {
    pub id: i64,
    pub name: String,
    pub lang: String,
    pub base_url: String,
}

/// The store a listing came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRef {
    pub index_url: String,
    pub name: String,
}

/// One row of the `anime_extension_store` table (`AnimeExtensionStore` in Aniyomi, data section 1.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRow {
    pub index_url: String,
    pub name: String,
    pub badge_label: String,
    pub signing_key: String,
    pub contact_website: String,
    pub contact_discord: Option<String>,
    pub is_legacy: bool,
    pub extension_list_url: Option<String>,
}

impl StoreRow {
    pub fn store_ref(&self) -> StoreRef {
        StoreRef { index_url: self.index_url.clone(), name: self.name.clone() }
    }
}

/// A package installed on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    pub abi: AbiVersion,
    pub lang: String,
    pub nsfw: bool,
    pub torrent: bool,
    pub sources: Vec<AnimeSourceRef>,
    pub icon: Option<PathBuf>,
    pub signer: SignerDigest,
    pub has_update: bool,
    pub is_obsolete: bool,
    pub store: Option<StoreRef>,
}

/// A package a store lists.
#[derive(Debug, Clone, PartialEq)]
pub struct Available {
    /// Display name: `Aniyomi: ` is stripped.
    pub name: String,
    /// The name as the store wrote it.
    pub raw_name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    /// Not filled from the index. The ABI comes from the package manifest after download.
    pub abi: Option<AbiVersion>,
    /// Aniyomi `extensionLib`, parsed with `str::parse::<f64>`. Set for `Kind::ApkOnly` rows.
    pub aniyomi_lib: Option<f64>,
    pub lang: String,
    pub nsfw: bool,
    pub torrent: bool,
    pub sources: Vec<AnimeSourceRef>,
    pub kind: Kind,
    pub apk_url: String,
    pub icon_url: String,
    /// `Some` only for `Kind::Wasm`.
    pub kuroext: Option<KuroextRef>,
    pub store: StoreRef,
}

/// The validated `resources.kuroext` block of a store entry (`p1-data-and-config.md` section 8.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KuroextRef {
    /// Absolute after resolution against the index URL.
    pub url: String,
    pub sha256: [u8; 32],
    /// Signature over the raw 32-byte file digest. Always 64 bytes once validated.
    pub ed25519: Vec<u8>,
    /// The signer's public key. Always 32 bytes once validated.
    pub public_key: Vec<u8>,
}

/// A package that loads but is not trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untrusted {
    pub name: String,
    pub pkg: String,
    pub version_name: String,
    pub version_code: i64,
    pub abi: AbiVersion,
    pub signer: SignerDigest,
}

/// The outcome of loading one installed package (`AnimeLoadResult`).
#[derive(Debug, Clone, PartialEq)]
pub enum LoadResult {
    Success(Installed),
    Untrusted(Untrusted),
    Error(LoadError),
}

/// Why one package cannot be used. The texts are for logs; the UI maps each variant to its own string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    #[error("package not found: {0}")]
    NotFound(String),
    #[error("missing version name for {0}")]
    MissingVersionName(String),
    #[error("unsupported abi {0}")]
    UnsupportedAbi(String),
    #[error("package is not signed: {0}")]
    NotSigned(String),
    #[error("nsfw package not allowed: {0}")]
    NsfwNotAllowed(String),
    #[error("manifest invalid: {0}")]
    Manifest(String),
    #[error("package container invalid: {0}")]
    Container(String),
    #[error("could not read the package: {0}")]
    Io(String),
    #[error("load failed for {pkg}: {reason}")]
    Load { pkg: String, reason: String },
}

/// Why a source cannot be used. A stub source (listed, not installed) answers `NotInstalled`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SourceError {
    #[error("the source is not installed")]
    NotInstalled,
}

/// The states of one install, in the order `InstallStep.kt` names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Idle,
    Pending,
    Downloading,
    Installing,
    Installed,
    Error,
}

impl InstallStep {
    /// `Idle`, `Installed` and `Error` are final. The rest are still running.
    pub fn is_completed(self) -> bool {
        matches!(self, InstallStep::Idle | InstallStep::Installed | InstallStep::Error)
    }
}

/// The Browse list buckets (`AnimeExtensions`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Buckets {
    pub updates: Vec<Installed>,
    pub installed: Vec<Installed>,
    pub available: Vec<AvailableRow>,
    pub untrusted: Vec<Untrusted>,
}

/// One Browse row for a listed package. Split rows carry one source each.
#[derive(Debug, Clone, PartialEq)]
pub struct AvailableRow {
    pub ext: Available,
    pub source: Option<AnimeSourceRef>,
    /// `<pkg>-<sourceId>` for split rows, `pkg` otherwise.
    pub row_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_final_states_are_completed() {
        assert!(InstallStep::Idle.is_completed());
        assert!(InstallStep::Installed.is_completed());
        assert!(InstallStep::Error.is_completed());
        for running in [InstallStep::Pending, InstallStep::Downloading, InstallStep::Installing] {
            assert!(!running.is_completed(), "{running:?}");
        }
    }

    #[test]
    fn a_store_row_names_its_store() {
        let row = StoreRow {
            index_url: "https://example.org/index.json".into(),
            name: "Example".into(),
            badge_label: "EX".into(),
            signing_key: "ab".repeat(32),
            contact_website: "https://example.org".into(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
        };
        assert_eq!(row.store_ref(), StoreRef { index_url: row.index_url.clone(), name: "Example".into() });
    }
}
