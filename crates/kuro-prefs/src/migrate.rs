//! Startup migrations. The app stores the version code it last ran with as `last_version_code` in
//! `state.toml`. On each start the steps newer than that code run, and the stored code moves to the current one only
//! when a step ran and every step succeeded (T-11a-027): a failing step reruns on the next start, a start with nothing
//! to run leaves the key alone, and a downgrade or the same build never writes it.

pub mod list;

use crate::PrefError;
use crate::store::PrefStore;

/// State key for the version code the app last ran with.
pub const LAST_VERSION_CODE: &str = "__APP_STATE_last_version_code";

/// What a step returns: `Ok(true)` when it changed something, `Ok(false)` when it had nothing to do.
pub type StepResult = Result<bool, PrefError>;

/// One step, run once when the version code moves past `version`.
pub struct Migration {
    pub version: i32,
    pub run: fn(&PrefStore) -> StepResult,
}

/// Runs the registered steps for a start with version code `current`. `Ok(true)` when a step ran and the stored code
/// moved to `current`.
pub fn migrate(store: &PrefStore, current: i32) -> Result<bool, PrefError> {
    run(store, current, list::MIGRATIONS)
}

/// The steps due for a move from `old` to `current`: versions above `old` up to and including `current`, in
/// ascending order (equal versions keep their registered order).
fn due(old: i32, current: i32, steps: &[Migration]) -> Vec<&Migration> {
    let mut due: Vec<&Migration> = steps.iter().filter(|s| s.version > old && s.version <= current).collect();
    due.sort_by_key(|s| s.version);
    due
}

/// Runs the due steps of `steps` for a start with version code `current`.
pub(crate) fn run(store: &PrefStore, current: i32, steps: &[Migration]) -> Result<bool, PrefError> {
    run_with(store, current, steps, false)
}

/// Runs the due steps. Every step runs even when an earlier one returned `Ok(false)`; the first `Err` stops the run
/// and is returned, with the stored code unchanged. With `dry_run` no step body runs and the stored code is never
/// written, but a non-empty plan still reports `Ok(true)`.
pub(crate) fn run_with(store: &PrefStore, current: i32, steps: &[Migration], dry_run: bool) -> Result<bool, PrefError> {
    let stored = store.pref(LAST_VERSION_CODE, 0i32);
    let old = stored.get();
    if old > current {
        tracing::warn!("Downgrade detected ({old} > {current})");
        return Ok(false);
    }
    if old == current {
        return Ok(false);
    }
    tracing::info!("Migration from {old} to {current}");
    let due = due(old, current, steps);
    if due.is_empty() {
        return Ok(false);
    }
    let mut any = false;
    for step in due {
        if dry_run {
            tracing::debug!("(Dry-run) Running migration: {{ version = {}.0 }}", step.version);
            any = true;
            continue;
        }
        tracing::debug!("Running migration: {{ version = {}.0 }}", step.version);
        any |= (step.run)(store)?;
    }
    if any && !dry_run {
        tracing::info!("Updating last version to {current}");
        stored.set(current)?;
    }
    Ok(any)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use kuro_platform::paths::Paths;

    use super::*;
    use crate::settings_load;
    use crate::testutil::{TestDir, TestPaths};
    use crate::toml_backend::state_file;
    use crate::values::PrefValue;

    fn step_two(s: &PrefStore) -> StepResult {
        s.set_raw("ran_2", PrefValue::Bool(true));
        Ok(true)
    }

    fn step_three(s: &PrefStore) -> StepResult {
        s.set_raw("ran_3", PrefValue::Bool(true));
        Ok(true)
    }

    fn nothing_to_do(s: &PrefStore) -> StepResult {
        s.set_raw("ran_idle", PrefValue::Bool(true));
        Ok(false)
    }

    fn broken(_: &PrefStore) -> StepResult {
        Err(PrefError::Io(std::io::Error::other("step failed")))
    }

    const STEPS: &[Migration] = &[Migration { version: 2, run: step_two }, Migration { version: 3, run: step_three }];

    fn stored(store: &PrefStore) -> i32 {
        store.pref(LAST_VERSION_CODE, 0i32).get()
    }

    fn stored_is_written(store: &PrefStore) -> bool {
        store.get_raw(LAST_VERSION_CODE).is_some()
    }

    #[test]
    fn first_install_runs_every_step_and_stores_the_version() {
        let store = PrefStore::in_memory();
        assert_eq!(run(&store, 3, STEPS).unwrap(), true);
        assert_eq!(stored(&store), 3);
        assert_eq!(store.get_raw("ran_2"), Some(PrefValue::Bool(true)));
        assert_eq!(store.get_raw("ran_3"), Some(PrefValue::Bool(true)));
    }

    #[test]
    fn upgrade_runs_only_the_steps_after_the_stored_version() {
        let store = PrefStore::in_memory();
        store.pref(LAST_VERSION_CODE, 0i32).set(2).unwrap();
        assert_eq!(run(&store, 3, STEPS).unwrap(), true);
        assert_eq!(stored(&store), 3);
        assert_eq!(store.get_raw("ran_2"), None);
        assert_eq!(store.get_raw("ran_3"), Some(PrefValue::Bool(true)));
    }

    #[test]
    fn downgrade_runs_nothing_and_keeps_the_stored_version() {
        let store = PrefStore::in_memory();
        store.pref(LAST_VERSION_CODE, 0i32).set(5).unwrap();
        assert_eq!(run(&store, 3, STEPS).unwrap(), false);
        assert_eq!(stored(&store), 5);
        assert_eq!(store.get_raw("ran_2"), None);
        assert_eq!(store.get_raw("ran_3"), None);
    }

    #[test]
    fn the_same_build_never_writes_the_key() {
        let store = PrefStore::in_memory();
        assert_eq!(run(&store, 3, STEPS).unwrap(), true);
        store.remove(LAST_VERSION_CODE);
        store.pref(LAST_VERSION_CODE, 0i32).set(3).unwrap();
        store.remove("ran_3");
        assert_eq!(run(&store, 3, STEPS).unwrap(), false);
        assert_eq!(store.get_raw("ran_3"), None, "nothing runs for the same build");
        assert_eq!(stored(&store), 3);
    }

    #[test]
    fn an_empty_plan_does_not_store_the_version() {
        let store = PrefStore::in_memory();
        assert_eq!(migrate(&store, 1).unwrap(), false);
        assert!(!stored_is_written(&store), "no step ran, so the key stays unwritten");
        assert_eq!(run(&store, 1, STEPS).unwrap(), false, "no step is due at version 1");
        assert!(!stored_is_written(&store));
    }

    #[test]
    fn steps_that_report_nothing_to_do_leave_the_stored_version_alone() {
        const IDLE: &[Migration] = &[Migration { version: 2, run: nothing_to_do }];
        let store = PrefStore::in_memory();
        assert_eq!(run(&store, 2, IDLE).unwrap(), false);
        assert_eq!(store.get_raw("ran_idle"), Some(PrefValue::Bool(true)), "the step did run");
        assert!(!stored_is_written(&store));
    }

    #[test]
    fn one_changing_step_stores_the_version_and_a_false_step_does_not_stop_the_run() {
        const MIXED: &[Migration] = &[
            Migration { version: 2, run: nothing_to_do },
            Migration { version: 3, run: step_three },
        ];
        let store = PrefStore::in_memory();
        assert_eq!(run(&store, 3, MIXED).unwrap(), true);
        assert_eq!(store.get_raw("ran_idle"), Some(PrefValue::Bool(true)));
        assert_eq!(store.get_raw("ran_3"), Some(PrefValue::Bool(true)));
        assert_eq!(stored(&store), 3);
    }

    #[test]
    fn a_failing_step_stops_the_run_and_the_next_start_retries_it() {
        const STEPS_WITH_FAILURE: &[Migration] = &[
            Migration { version: 2, run: step_two },
            Migration { version: 3, run: broken },
            Migration { version: 4, run: step_three },
        ];
        let store = PrefStore::in_memory();
        assert!(run(&store, 4, STEPS_WITH_FAILURE).is_err());
        assert!(!stored_is_written(&store), "the key is not written after an error");
        assert_eq!(store.get_raw("ran_2"), Some(PrefValue::Bool(true)), "earlier steps are not rolled back");
        assert_eq!(store.get_raw("ran_3"), None, "later steps do not run");

        store.remove("ran_2");
        assert!(run(&store, 4, STEPS_WITH_FAILURE).is_err());
        assert_eq!(store.get_raw("ran_2"), Some(PrefValue::Bool(true)), "the same range runs again");
    }

    #[test]
    fn a_dry_run_calls_no_step_and_writes_nothing() {
        let store = PrefStore::in_memory();
        assert_eq!(run_with(&store, 3, STEPS, true).unwrap(), true);
        assert_eq!(store.get_raw("ran_2"), None);
        assert_eq!(store.get_raw("ran_3"), None);
        assert!(!stored_is_written(&store));
    }

    #[test]
    fn the_version_is_written_to_state_toml_once_a_step_ran() {
        let paths = TestPaths(TestDir::new("migrate-state"));
        let store = settings_load::open(&paths).unwrap();
        assert_eq!(run(&store, 3, STEPS).unwrap(), true);
        store.flush().unwrap();
        let text = fs::read_to_string(state_file(&paths.state_dir())).unwrap();
        assert!(text.contains("last_version_code"), "{text}");
    }
}
