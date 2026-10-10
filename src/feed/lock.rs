//! The app-lock rule (T-50a-008): the Boolean `use_biometric_lock`, default false, read exactly as Android does.
//! Fail closed: a preference file that cannot be read is an error, never "unlocked".

use std::path::PathBuf;

use super::FeedError;

/// Reads one Boolean preference of a table. `Ok(None)` when it is not set.
pub trait PrefsReader {
    fn get_bool(&self, table: &str, field: &str) -> Result<Option<bool>, String>;
}

/// True when the lock flag is on. `lock_app_after` is not read (E-8).
pub fn is_locked(prefs: &dyn PrefsReader) -> Result<bool, FeedError> {
    prefs
        .get_bool("security", "use_authenticator")
        .map(|v| v.unwrap_or(false))
        .map_err(FeedError::Prefs)
}

/// The settings file read without the cross-process lock the running app holds: the CLI only reads.
pub struct ConfigFile(pub PathBuf);

impl PrefsReader for ConfigFile {
    fn get_bool(&self, table: &str, field: &str) -> Result<Option<bool>, String> {
        let text = match std::fs::read_to_string(&self.0) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("cannot read {}: {e}", self.0.display())),
        };
        let doc: toml_edit::DocumentMut =
            text.parse().map_err(|e| format!("cannot parse {}: {e}", self.0.display()))?;
        match doc.get(table).and_then(|t| t.get(field)) {
            None => Ok(None),
            Some(item) => item.as_bool().map(Some).ok_or_else(|| format!("{table}.{field} is not a boolean")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Result<Option<bool>, String>);
    impl PrefsReader for Fake {
        fn get_bool(&self, _: &str, _: &str) -> Result<Option<bool>, String> {
            self.0.clone()
        }
    }

    #[test]
    fn flag_true_locks_false_and_missing_do_not() {
        assert!(is_locked(&Fake(Ok(Some(true)))).unwrap());
        assert!(!is_locked(&Fake(Ok(Some(false)))).unwrap());
        assert!(!is_locked(&Fake(Ok(None))).unwrap());
    }

    #[test]
    fn a_read_error_is_a_preferences_error() {
        assert!(matches!(is_locked(&Fake(Err("x".into()))), Err(FeedError::Prefs(_))));
    }

    /// The real store writes the flag where the reader looks, and `lock_app_after` changes nothing.
    #[test]
    fn reads_what_the_preference_store_writes() {
        use kuro_platform::paths::Paths as _;
        let paths = kuro_testkit::TempPaths::new().unwrap();
        let store = kuro_prefs::settings_load::open(&paths).unwrap();
        let sec = kuro_prefs::security::SecurityPrefs::new(store.clone());
        let file = ConfigFile(kuro_prefs::toml_backend::config_file(&paths.config_dir()));
        sec.lock_app_after.set(5).unwrap();
        store.flush().unwrap();
        assert!(!is_locked(&file).unwrap());
        sec.use_authenticator.set(true).unwrap();
        store.flush().unwrap();
        assert!(is_locked(&file).unwrap());
    }

    #[test]
    fn missing_file_is_open_and_garbage_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_prefs::toml_backend::config_file(dir.path());
        assert!(!is_locked(&ConfigFile(path.clone())).unwrap());
        std::fs::write(&path, "[security\nuse_authenticator = ").unwrap();
        assert!(matches!(is_locked(&ConfigFile(path.clone())), Err(FeedError::Prefs(_))));
        std::fs::write(&path, "[security]\nuse_authenticator = \"yes\"\n").unwrap();
        assert!(matches!(is_locked(&ConfigFile(path)), Err(FeedError::Prefs(_))));
    }
}
