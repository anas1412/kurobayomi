//! Startup migrations. The app stores the version code it last ran with as `last_version_code` in
//! `state.toml`. On each start the steps newer than that code run, then the stored code moves to
//! the current one. A downgrade (stored code above the current one) changes nothing.

pub mod list;

use crate::PrefError;
use crate::store::PrefStore;

/// State key for the version code the app last ran with.
pub const LAST_VERSION_CODE: &str = "__APP_STATE_last_version_code";

/// One step, run once when the version code moves past `version`.
pub struct Migration {
    pub version: i32,
    pub run: fn(&PrefStore),
}

/// Runs the registered steps for a start with version code `current`.
pub fn migrate(store: &PrefStore, current: i32) -> Result<(), PrefError> {
    run(store, current, list::MIGRATIONS)
}

pub(crate) fn run(store: &PrefStore, current: i32, steps: &[Migration]) -> Result<(), PrefError> {
    let stored = store.pref(LAST_VERSION_CODE, 0i32);
    let old = stored.get();
    if old > current {
        tracing::warn!("Downgrade detected ({old} > {current})");
        return Ok(());
    }
    tracing::info!("Migration from {old} to {current}");
    for step in steps
        .iter()
        .filter(|s| s.version > old && s.version <= current)
    {
        (step.run)(store);
    }
    tracing::info!("Updating last version to {current}");
    stored.set(current)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use kuro_platform::paths::Paths;

    use super::*;
    use crate::toml_backend::state_file;
    use crate::settings_load;
    use crate::testutil::{TestDir, TestPaths};
    use crate::values::PrefValue;

    fn step_two(s: &PrefStore) {
        s.set_raw("ran_2", PrefValue::Bool(true));
    }

    fn step_three(s: &PrefStore) {
        s.set_raw("ran_3", PrefValue::Bool(true));
    }

    const STEPS: &[Migration] = &[
        Migration {
            version: 2,
            run: step_two,
        },
        Migration {
            version: 3,
            run: step_three,
        },
    ];

    fn stored(store: &PrefStore) -> i32 {
        store.pref(LAST_VERSION_CODE, 0i32).get()
    }

    #[test]
    fn first_install_runs_every_step_and_stores_the_version() {
        let store = PrefStore::in_memory();
        run(&store, 3, STEPS).unwrap();
        assert_eq!(stored(&store), 3);
        assert_eq!(store.get_raw("ran_2"), Some(PrefValue::Bool(true)));
        assert_eq!(store.get_raw("ran_3"), Some(PrefValue::Bool(true)));
    }

    #[test]
    fn upgrade_runs_only_the_steps_after_the_stored_version() {
        let store = PrefStore::in_memory();
        store.pref(LAST_VERSION_CODE, 0i32).set(2).unwrap();
        run(&store, 3, STEPS).unwrap();
        assert_eq!(stored(&store), 3);
        assert_eq!(store.get_raw("ran_2"), None);
        assert_eq!(store.get_raw("ran_3"), Some(PrefValue::Bool(true)));
    }

    #[test]
    fn downgrade_runs_nothing_and_keeps_the_stored_version() {
        let store = PrefStore::in_memory();
        store.pref(LAST_VERSION_CODE, 0i32).set(5).unwrap();
        run(&store, 3, STEPS).unwrap();
        assert_eq!(stored(&store), 5);
        assert_eq!(store.get_raw("ran_2"), None);
        assert_eq!(store.get_raw("ran_3"), None);
    }

    #[test]
    fn an_empty_plan_still_stores_the_version() {
        let store = PrefStore::in_memory();
        migrate(&store, 1).unwrap();
        assert_eq!(stored(&store), 1);
    }

    #[test]
    fn the_version_is_written_to_state_toml() {
        let paths = TestPaths(TestDir::new("migrate-state"));
        let store = settings_load::open(&paths).unwrap();
        migrate(&store, 1).unwrap();
        store.flush().unwrap();
        let text = fs::read_to_string(state_file(&paths.state_dir())).unwrap();
        assert!(text.contains("last_version_code"), "{text}");
    }
}
