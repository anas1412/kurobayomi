//! Test helpers: a temporary directory that is removed on drop, a `Paths` rooted in one, a TOML
//! reader for checking which table a value landed in, and the registry check for sections.

use std::fs;
use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;
use toml_edit::DocumentMut;

use crate::registry;
use crate::values::PrefValue;

/// Parses a settings file written by the TOML backend.
pub fn read_toml(path: &Path) -> DocumentMut {
    fs::read_to_string(path)
        .expect("read settings file")
        .parse()
        .expect("settings file is valid TOML")
}

/// A temporary directory removed on drop. Same contract as `kuro_testkit::TempDir`, which cannot
/// be used here because kuro-testkit depends on this crate.
pub struct TestDir(tempfile::TempDir);

impl TestDir {
    pub fn new(name: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix(&format!("kuro-prefs-{name}-"))
            .tempdir()
            .expect("create test directory");
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        self.0.path()
    }
}

/// `Paths` whose roles are subdirectories of one test directory. They are not created here.
pub struct TestPaths(pub TestDir);

impl TestPaths {
    fn sub(&self, name: &str) -> PathBuf {
        self.0.path().join(name)
    }
}

impl Paths for TestPaths {
    fn data_dir(&self) -> PathBuf {
        self.sub("data")
    }

    fn config_dir(&self) -> PathBuf {
        self.sub("config")
    }

    fn state_dir(&self) -> PathBuf {
        self.sub("state")
    }

    fn cache_dir(&self) -> PathBuf {
        self.sub("cache")
    }

    fn log_dir(&self) -> PathBuf {
        self.sub("log")
    }

    fn temp_dir(&self) -> PathBuf {
        self.sub("temp")
    }

    fn crash_dir(&self) -> PathBuf {
        self.sub("crash")
    }

    fn default_downloads_dir(&self) -> PathBuf {
        self.sub("downloads")
    }

    fn default_backups_dir(&self) -> PathBuf {
        self.sub("backups")
    }

    fn covers_dir(&self) -> PathBuf {
        self.sub("covers")
    }

    fn plugin_dir(&self) -> PathBuf {
        self.sub("plugins")
    }
}

/// Keys whose Linux default differs from the registry row on purpose. The registry keeps the
/// Kotlin class default, and the section declares the environment default (see `network.rs`).
pub const ENVIRONMENT_DEFAULTS: &[&str] = &["verbose_logging"];

/// Checks a settings section against the registry: every field's key is registered under the
/// section's table and the field's TOML name, and each declared default equals the registry
/// default (except the [`ENVIRONMENT_DEFAULTS`]).
pub fn assert_section_registered(
    table: &str,
    fields: &[(&str, &str)],
    defaults: &[(&str, PrefValue)],
) {
    for (field, key) in fields {
        let (meta, id) =
            registry::lookup(key).unwrap_or_else(|| panic!("{key} is not in the registry"));
        assert_eq!(id, None, "{key} is not a family key");
        assert_eq!(meta.table, table, "{key}: TOML table");
        // Shared keys (Kotlin twins such as the chapter and episode display modes) have one registry row
        // per field name, so the field must match one of them.
        let rows: Vec<_> = registry::REGISTRY.iter().filter(|m| m.key == *key).collect();
        assert!(rows.iter().any(|m| m.field == *field), "{key}: TOML field {field} not registered");
    }
    for (key, value) in defaults {
        if ENVIRONMENT_DEFAULTS.contains(key) {
            continue;
        }
        let (meta, _) = registry::lookup(key).unwrap();
        assert_eq!(&meta.default.value(), value, "{key}: registry default");
    }
}

pub use crate::memory::MemorySecrets;
