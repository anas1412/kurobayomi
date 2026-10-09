//! Startup load of the preference files. On first run the directories are created (mode 0700).
//! A corrupt file is renamed aside by the TOML backend and defaults are used.

use std::fs::DirBuilder;
use std::path::Path;

use kuro_platform::paths::Paths;

use crate::PrefError;
use crate::store::PrefStore;
use crate::toml_backend::TomlBackend;

/// Creates the config, data and cache directories when missing and opens the store.
/// Fails with [`PrefError::Dir`] (naming the directory) when one cannot be created, and with
/// [`PrefError::Locked`] when another process owns the settings.
pub fn open(paths: &dyn Paths) -> Result<PrefStore, PrefError> {
    let config = paths.config_dir();
    let state = paths.state_dir();
    for dir in [&config, &state, &paths.cache_dir()] {
        ensure_dir(dir)?;
    }
    let backend = TomlBackend::open(&config, &state)?;
    PrefStore::new(Box::new(backend))
}

fn ensure_dir(dir: &Path) -> Result<(), PrefError> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir).map_err(|source| PrefError::Dir {
        path: dir.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::toml_backend::{config_file, state_file};
    use crate::testutil::{TestDir, TestPaths};
    use crate::values::PrefValue;

    #[test]
    fn first_run_creates_the_directories() {
        let paths = TestPaths(TestDir::new("first-run"));
        let store = open(&paths).unwrap();
        assert!(paths.config_dir().is_dir());
        assert!(paths.state_dir().is_dir());
        assert!(paths.cache_dir().is_dir());
        assert!(store.all().is_empty());
    }

    #[test]
    fn app_state_is_written_to_state_toml_in_the_state_dir() {
        let paths = TestPaths(TestDir::new("state-dir"));
        let store = open(&paths).unwrap();
        store.set_raw("__APP_STATE_incognito_mode", PrefValue::Bool(true));
        store.flush().unwrap();
        assert!(state_file(&paths.state_dir()).is_file());
        assert!(!state_file(&paths.data_dir()).exists());
    }

    #[cfg(unix)]
    #[test]
    fn created_directories_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let paths = TestPaths(TestDir::new("modes"));
        let _store = open(&paths).unwrap();
        let mode = fs::metadata(paths.config_dir())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn a_corrupt_config_is_renamed_and_defaults_load() {
        let paths = TestPaths(TestDir::new("corrupt-open"));
        fs::create_dir_all(paths.config_dir()).unwrap();
        fs::write(config_file(&paths.config_dir()), "[[[ not toml").unwrap();
        let store = open(&paths).unwrap();
        assert_eq!(store.get_raw("display_unread_badge"), None);
        let renamed = fs::read_dir(paths.config_dir()).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("config.toml.corrupt-")
        });
        assert!(renamed);
    }

    #[test]
    fn saved_values_are_loaded_by_the_next_open() {
        let paths = TestPaths(TestDir::new("reopen"));
        {
            let store = open(&paths).unwrap();
            store.set_raw("display_unread_badge", PrefValue::Bool(false));
            store.flush().unwrap();
        }
        // The flusher thread may still hold the last clone for a moment, so retry the lock.
        let mut reopened = open(&paths);
        for _ in 0..200 {
            if reopened.is_ok() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            reopened = open(&paths);
        }
        let store = reopened.unwrap();
        assert_eq!(
            store.get_raw("display_unread_badge"),
            Some(PrefValue::Bool(false))
        );
    }

    #[test]
    fn a_second_open_while_the_first_is_alive_is_locked() {
        let paths = TestPaths(TestDir::new("locked-open"));
        let _first = open(&paths).unwrap();
        assert!(matches!(open(&paths), Err(PrefError::Locked)));
    }
}
