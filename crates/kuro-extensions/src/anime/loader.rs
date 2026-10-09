//! The installed-package directory and the load pipeline (T-19a-005 and T-19a-006, `p1-rust-design.md` section 5).
//! Each package is checked on its own, and one bad package never stops the scan. Loading never runs package code:
//! the component is only checked for size, and it runs in the WASM host later.

use std::fs;
use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;

use crate::anime::abi::{AbiVersion, SUPPORTED_ABI};
use crate::anime::logs;
use crate::anime::model::{AnimeSourceRef, Installed, LoadError, LoadResult, SignerDigest, StoreRow, Untrusted};
use crate::anime::package;
use crate::anime::store::wire::ContentWarning;
use crate::anime::trust::TrustStore;

/// What one scan needs from the outside: the trust rule, the stores that sign packages, and the NSFW gate.
pub struct LoadContext<'a> {
    pub trust: &'a TrustStore,
    pub stores: &'a [StoreRow],
    pub nsfw_allowed: bool,
}

pub const PACKAGE_EXT: &str = "kuroext";
pub const SIGNER_EXT: &str = "signer";
pub const PART_SUFFIX: &str = ".kuroext.part";

/// `<root>` holds one `<pkg>.kuroext` and one `<pkg>.signer` per installed package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDir {
    root: PathBuf,
}

impl PackageDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// `<data>/extensions/anime`, resolved through the platform `Paths`.
    pub fn from_paths(paths: &dyn Paths) -> Self {
        Self::new(paths.data_dir().join("extensions").join("anime"))
    }

    /// `<cache>/ext-download`, where downloads are written before they are placed.
    pub fn download_dir(paths: &dyn Paths) -> PathBuf {
        paths.cache_dir().join("ext-download")
    }

    /// Startup cleanup: deletes `*.kuroext.part` leftovers from a crashed download or placement, in the package
    /// folder and in `download_dir`. Returns how many were removed.
    pub fn cleanup_stale(&self, download_dir: &Path) -> usize {
        remove_stale_parts(&self.root) + remove_stale_parts(download_dir)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn package_path(&self, pkg: &str) -> PathBuf {
        self.root.join(format!("{pkg}.{PACKAGE_EXT}"))
    }

    /// Loads every `<pkg>.kuroext` in path order. Other names are ignored. Each file is made read-only before it is
    /// parsed.
    pub fn scan(&self, ctx: &LoadContext<'_>) -> Vec<LoadResult> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| is_package_file(path))
            .collect();
        files.sort();
        files
            .iter()
            .map(|path| {
                make_read_only(path);
                load_package(path, ctx)
            })
            .collect()
    }
}

/// A regular file named `<pkg>.kuroext` with a non-empty name.
fn is_package_file(path: &Path) -> bool {
    let regular = fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_file());
    regular
        && path.extension().is_some_and(|ext| ext == PACKAGE_EXT)
        && path.file_stem().is_some_and(|stem| !stem.is_empty())
}

fn make_read_only(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    let mut perms = meta.permissions();
    if perms.readonly() {
        return;
    }
    perms.set_readonly(true);
    if let Err(err) = fs::set_permissions(path, perms) {
        tracing::warn!(path = %path.display(), error = %err, "could not make the package read-only");
    }
}

/// Deletes leftover `*.kuroext.part` files from an interrupted install. Returns how many were removed.
pub fn remove_stale_parts(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.ends_with(PART_SUFFIX)))
        .filter(|path| fs::remove_file(path).is_ok())
        .count()
}

/// Checks the package file at `path`, with its `<pkg>.signer` sidecar, against the trust rule and the NSFW gate.
pub fn load_package(path: &Path, ctx: &LoadContext<'_>) -> LoadResult {
    match check(path, ctx) {
        Ok(result) => result,
        Err(error) => {
            log_load_error(&error, path);
            LoadResult::Error(error)
        }
    }
}

/// The log text of each failure, as Aniyomi writes it (`p1-data-and-config.md` section 11). `NotSigned` and
/// `NsfwNotAllowed` are logged as WARN inside `check`, so they get no second event here. The extension name is only
/// known once the manifest is read, so a failure before that (an unreadable container or manifest) fills `extName`
/// with the package name taken from the file name.
fn log_load_error(error: &LoadError, path: &Path) {
    let file_pkg = path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    match error {
        LoadError::NotFound(pkg) => tracing::error!(pkg = %pkg, "{}", logs::package_not_found(pkg)),
        // The variant carries the extension name, as the Kotlin text does.
        LoadError::MissingVersionName(name) => tracing::warn!(pkg = %file_pkg, "{}", logs::missing_version_name(name)),
        LoadError::UnsupportedAbi(abi) => {
            let supported: Vec<String> = SUPPORTED_ABI.iter().map(ToString::to_string).collect();
            tracing::warn!(pkg = %file_pkg, "{}", logs::lib_version(abi, &supported));
        }
        LoadError::NotSigned(_) | LoadError::NsfwNotAllowed(_) => {}
        other => tracing::error!(pkg = %file_pkg, reason = %other, "{}", logs::load_error(&file_pkg, &file_pkg)),
    }
}

fn check(path: &Path, ctx: &LoadContext<'_>) -> Result<LoadResult, LoadError> {
    let package = package::open(path)?;
    let manifest = package.manifest;
    let pkg = manifest.pkg.clone();
    let Some(signer) = read_signer(&path.with_extension(SIGNER_EXT)) else {
        tracing::warn!(pkg = %pkg, "{}", logs::not_signed(&pkg));
        return Err(LoadError::NotSigned(pkg));
    };
    let abi = AbiVersion::parse(&manifest.abi).ok_or_else(|| LoadError::Manifest("abi".into()))?;
    let name = manifest.name.trim().to_owned();
    // The same rule the store listing applies to a content warning (TST-19-036).
    let nsfw = ContentWarning::from(manifest.nsfw).is_nsfw();
    if !ctx.trust.is_trusted(&pkg, manifest.version_code, std::slice::from_ref(&signer), ctx.stores) {
        tracing::warn!(pkg = %pkg, "{}", logs::untrusted(&pkg));
        return Ok(LoadResult::Untrusted(Untrusted {
            name,
            pkg,
            version_name: manifest.version_name,
            version_code: manifest.version_code,
            abi,
            nsfw,
            signer,
        }));
    }
    if nsfw && !ctx.nsfw_allowed {
        tracing::warn!(pkg = %pkg, "{}", logs::nsfw_not_allowed(&pkg));
        return Err(LoadError::NsfwNotAllowed(pkg));
    }
    let sources = manifest
        .sources
        .into_iter()
        .map(|source| AnimeSourceRef { id: source.id, name: source.name, lang: source.lang, base_url: source.base_url })
        .collect();
    Ok(LoadResult::Success(Installed {
        name,
        pkg,
        version_name: manifest.version_name,
        version_code: manifest.version_code,
        abi,
        lang: manifest.lang,
        nsfw,
        torrent: manifest.torrent,
        sources,
        icon: None,
        signer,
        has_update: false,
        is_obsolete: false,
        store: None,
    }))
}

/// The sidecar holds the 64-character lowercase digest. Anything else counts as not signed.
pub(crate) fn read_signer(sidecar: &Path) -> Option<SignerDigest> {
    let text = fs::read_to_string(sidecar).ok()?;
    let digest = text.trim();
    let valid = digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    valid.then(|| SignerDigest(digest.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_platform::paths::Paths;

    use crate::anime::testkit::{
        Fixture, MANIFEST, PKG, fixture_key, manifest_version, other_key, signed_package, signer_hex, store_document,
        trust_with_entries,
    };

    fn store_for(key: &str) -> StoreRow {
        let doc = store_document("S", key, Vec::new());
        let parsed: crate::anime::store::wire::StoreJson = serde_json::from_value(doc).unwrap();
        crate::anime::store::map::store_row(&parsed, "https://example.org/index.json")
    }

    /// Writes `<pkg>.kuroext` and its sidecar into `dir`, the way the installer places them.
    fn place(dir: &Path, pkg: &str, fixture: &Fixture) -> PathBuf {
        let path = dir.join(format!("{pkg}.{PACKAGE_EXT}"));
        fs::write(&path, &fixture.bytes).unwrap();
        fs::write(path.with_extension(SIGNER_EXT), &fixture.signer_hex).unwrap();
        path
    }

    #[test]
    fn a_signed_package_whose_store_signs_it_loads_as_installed() {
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        match load_package(&path, &ctx) {
            LoadResult::Success(installed) => {
                assert_eq!(installed.pkg, PKG);
                assert_eq!(installed.version_code, 7);
                assert_eq!(installed.lang, "en");
                assert_eq!(installed.sources.len(), 1);
                assert_eq!(installed.signer.0, fixture.signer_hex);
                assert!(installed.store.is_none(), "reconciliation attaches the store");
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn an_untrusted_nsfw_package_is_untrusted_not_refused() {
        let root = tempfile::tempdir().unwrap();
        let manifest = MANIFEST.replace("\"nsfw\": false", "\"nsfw\": true");
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        assert!(matches!(load_package(&path, &ctx), LoadResult::Untrusted(_)));
    }

    #[test]
    fn a_trusted_nsfw_package_is_blocked_by_the_setting() {
        let root = tempfile::tempdir().unwrap();
        let manifest = MANIFEST.replace("\"nsfw\": false", "\"nsfw\": true");
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: false };
        assert_eq!(load_package(&path, &ctx), LoadResult::Error(LoadError::NsfwNotAllowed(PKG.into())));
    }

    #[test]
    fn a_package_without_a_sidecar_is_not_signed() {
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        fs::remove_file(path.with_extension(SIGNER_EXT)).unwrap();
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        assert_eq!(load_package(&path, &ctx), LoadResult::Error(LoadError::NotSigned(PKG.into())));
    }

    #[test]
    fn tst_19_060_a_load_failure_logs_the_kotlin_text_with_the_package_field() {
        use crate::anime::testkit::capture;
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        fs::remove_file(path.with_extension(SIGNER_EXT)).unwrap();
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        let (_, logged) = capture(|| load_package(&path, &ctx));
        let message = format!("Package {PKG} isn't signed");
        assert_eq!(logged.len(), 1, "one event per failure: {logged:?}");
        let event = &logged[0];
        assert_eq!(event.message, message);
        assert_eq!(event.level, tracing::Level::WARN);
        assert!(event.fields.iter().any(|(name, value)| name == "pkg" && value == PKG));
    }

    #[test]
    fn tst_19_060_a_nsfw_refusal_logs_one_warn_and_no_error() {
        use crate::anime::testkit::capture;
        let root = tempfile::tempdir().unwrap();
        let manifest = MANIFEST.replace("\"nsfw\": false", "\"nsfw\": true");
        assert_ne!(manifest, MANIFEST, "the package was made NSFW");
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: false };
        let (result, logged) = capture(|| load_package(&path, &ctx));
        assert!(matches!(result, LoadResult::Error(LoadError::NsfwNotAllowed(_))), "{result:?}");
        assert_eq!(logged.len(), 1, "one event per failure: {logged:?}");
        assert_eq!(logged[0].message, format!("NSFW extension {PKG} not allowed"));
        assert_eq!(logged[0].level, tracing::Level::WARN);
    }

    #[test]
    fn tst_19_060_an_unreadable_container_logs_the_load_error_text_once() {
        use crate::anime::testkit::capture;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("eu.example.broken.kuroext");
        fs::write(&path, b"not a zip").unwrap();
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        let (_, logged) = capture(|| load_package(&path, &ctx));
        assert_eq!(logged.len(), 1, "{logged:?}");
        assert_eq!(logged[0].level, tracing::Level::ERROR);
        assert_eq!(logged[0].message, "Extension load error: eu.example.broken (eu.example.broken)");
    }

    #[test]
    fn the_torrent_flag_is_carried_into_the_installed_package() {
        let root = tempfile::tempdir().unwrap();
        let manifest = MANIFEST.replace("\"nsfw\": false,", "\"nsfw\": false, \"torrent\": true,");
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        match load_package(&path, &ctx) {
            LoadResult::Success(installed) => assert!(installed.torrent),
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn a_crash_during_placement_keeps_the_old_package_and_the_stale_part_is_removed() {
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        fs::write(root.path().join(format!("{PKG}.kuroext.part")), b"half written").unwrap();
        assert_eq!(remove_stale_parts(root.path()), 1);
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        assert!(matches!(load_package(&path, &ctx), LoadResult::Success(_)));
    }

    #[test]
    fn startup_cleanup_removes_parts_in_the_package_and_download_folders() {
        let work = tempfile::tempdir().unwrap();
        let (packages, downloads) = (work.path().join("anime"), work.path().join("ext-download"));
        fs::create_dir_all(&packages).unwrap();
        fs::create_dir_all(&downloads).unwrap();
        fs::write(packages.join("a.kuroext.part"), b"x").unwrap();
        fs::write(downloads.join("b-1.kuroext.part"), b"x").unwrap();
        fs::write(downloads.join("keep.txt"), b"x").unwrap();
        assert_eq!(PackageDir::new(&packages).cleanup_stale(&downloads), 2);
        assert!(!packages.join("a.kuroext.part").exists());
        assert!(!downloads.join("b-1.kuroext.part").exists());
        assert!(downloads.join("keep.txt").exists());
    }

    #[test]
    fn the_root_comes_from_the_platform_data_directory() {
        struct Roots(PathBuf);
        impl Paths for Roots {
            fn data_dir(&self) -> PathBuf {
                self.0.join("data")
            }
            fn config_dir(&self) -> PathBuf {
                self.0.join("config")
            }
            fn state_dir(&self) -> PathBuf {
                self.0.join("state")
            }
            fn cache_dir(&self) -> PathBuf {
                self.0.join("cache")
            }
            fn log_dir(&self) -> PathBuf {
                self.0.join("log")
            }
            fn temp_dir(&self) -> PathBuf {
                self.0.join("tmp")
            }
            fn crash_dir(&self) -> PathBuf {
                self.0.join("crash")
            }
            fn default_downloads_dir(&self) -> PathBuf {
                self.0.join("downloads")
            }
            fn default_backups_dir(&self) -> PathBuf {
                self.0.join("backups")
            }
            fn covers_dir(&self) -> PathBuf {
                self.0.join("covers")
            }
            fn plugin_dir(&self) -> PathBuf {
                self.0.join("plugins")
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let dir = PackageDir::from_paths(&Roots(temp.path().to_path_buf()));
        assert_eq!(dir.root(), temp.path().join("data").join("extensions").join("anime"));
    }

    #[test]
    fn an_empty_version_name_fails_with_missing_version_name() {
        let root = tempfile::tempdir().unwrap();
        let manifest = manifest_version("", 7);
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        assert_eq!(load_package(&path, &ctx), LoadResult::Error(LoadError::MissingVersionName("Demo".into())));
    }

    #[test]
    fn a_component_that_is_not_wasm_still_loads_because_nothing_runs_at_discovery() {
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"definitely not wasm");
        let path = place(root.path(), PKG, &fixture);
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        assert!(matches!(load_package(&path, &ctx), LoadResult::Success(_)));
    }

    #[test]
    fn tst_19_036_the_loader_and_the_store_listing_share_one_nsfw_rule() {
        use crate::anime::store::map;
        use crate::anime::store::wire::ExtensionListJson;
        use crate::anime::testkit::store_extension;
        let cases = [
            (false, "SAFE", false),
            (false, "UNSPECIFIED", false),
            (true, "MIXED", true),
            (true, "CONTENT_WARNING_MIXED", true),
            (true, "NSFW", true),
        ];
        for (flag, warning, expected) in cases {
            let root = tempfile::tempdir().unwrap();
            let manifest = MANIFEST.replace("\"nsfw\": false", &format!("\"nsfw\": {flag}"));
            let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
            let path = place(root.path(), PKG, &fixture);
            let stores = [store_for(&fixture.signer_hex)];
            let trust = trust_with_entries(&[]);
            let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
            let LoadResult::Success(installed) = load_package(&path, &ctx) else { panic!("{warning} did not load") };

            let mut entry = store_extension(PKG, 7, None);
            entry["contentWarning"] = serde_json::json!(warning);
            let list: ExtensionListJson = serde_json::from_value(serde_json::json!({ "extensions": [entry] })).unwrap();
            let listed = map::available(&list.extensions[0], &stores[0].store_ref()).unwrap();
            assert_eq!((installed.nsfw, listed.nsfw), (expected, expected), "flag {flag} and warning {warning}");
        }
        assert!(ContentWarning::from(true).is_nsfw() && !ContentWarning::from(false).is_nsfw());
    }

    #[test]
    fn one_broken_package_does_not_stop_the_scan() {
        let root = tempfile::tempdir().unwrap();
        let good = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        place(root.path(), "eu.example.animeext.en.good", &good);
        fs::write(root.path().join("eu.example.animeext.en.broken.kuroext"), b"not a zip").unwrap();
        let stores = [store_for(&good.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        let results = PackageDir::new(root.path()).scan(&ctx);
        let errors = results.iter().filter(|r| matches!(r, LoadResult::Error(_))).count();
        let successes = results.iter().filter(|r| matches!(r, LoadResult::Success(_))).count();
        assert_eq!((errors, successes), (1, 1));
    }

    #[test]
    fn the_scan_lists_only_package_files_and_makes_them_read_only() {
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        fs::write(root.path().join("notes.txt"), b"ignored").unwrap();
        fs::write(root.path().join("x.kuroext.part"), b"partial").unwrap();
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        let results = PackageDir::new(root.path()).scan(&ctx);
        assert_eq!(results.len(), 1);
        assert!(fs::metadata(&path).unwrap().permissions().readonly());
    }

    #[test]
    fn the_nsfw_setting_is_read_at_every_scan() {
        let root = tempfile::tempdir().unwrap();
        let manifest = MANIFEST.replace("\"nsfw\": false", "\"nsfw\": true");
        let fixture = signed_package(&fixture_key(), &manifest, b"\0asm");
        place(root.path(), PKG, &fixture);
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let dir = PackageDir::new(root.path());
        let blocked = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: false };
        assert!(matches!(dir.scan(&blocked)[0], LoadResult::Error(LoadError::NsfwNotAllowed(_))));
        let allowed = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        assert!(matches!(dir.scan(&allowed)[0], LoadResult::Success(_)));
    }

    #[test]
    fn leftover_part_files_are_removed_and_nothing_else_is_touched() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a.kuroext.part"), b"x").unwrap();
        fs::write(root.path().join("a.kuroext"), b"y").unwrap();
        assert_eq!(remove_stale_parts(root.path()), 1);
        assert!(root.path().join("a.kuroext").exists());
        assert!(!root.path().join("a.kuroext.part").exists());
        assert_eq!(remove_stale_parts(&root.path().join("missing")), 0);
    }

    #[test]
    fn a_missing_directory_scans_to_nothing() {
        let root = tempfile::tempdir().unwrap();
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &[], nsfw_allowed: true };
        assert!(PackageDir::new(root.path().join("absent")).scan(&ctx).is_empty());
    }

    #[test]
    fn a_wrong_signer_sidecar_does_not_make_the_package_trusted() {
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = place(root.path(), PKG, &fixture);
        fs::write(path.with_extension(SIGNER_EXT), signer_hex(&other_key())).unwrap();
        let stores = [store_for(&fixture.signer_hex)];
        let trust = trust_with_entries(&[]);
        let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
        assert!(matches!(load_package(&path, &ctx), LoadResult::Untrusted(_)));
    }
}
