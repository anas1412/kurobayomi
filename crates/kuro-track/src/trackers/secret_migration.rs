//! First-start move of the tracker credentials that the backup importer and the old preference layout keep under the
//! service `kurobayomi` into the layout the trackers read (T-32a-065, R-7, TST-64, TST-65).
//!
//! One layout is kept: service `kurobayomi.track`, account `tracker/<id>/<suffix>` ([`crate::base::account`]). The
//! logical keys of the R-7 table map onto it: `track.7.session` is `tracker/7/password`, `track.101.token` is
//! `tracker/101/oauth`. The move copies first and deletes the old entry after, so an interruption loses nothing, and a
//! second run finds nothing to move.

use kuro_platform::secrets::{SecretError, SecretStore};

use crate::base::{Field, SECRET_SERVICE, account};
use crate::model::TrackerId;

/// Shown in the tracking settings while the secret store cannot be reached (TST-64).
pub const UNENCRYPTED_WARNING: &str = "Tokens stored unencrypted";

/// The old entries: `(old account, tracker, field)`. The old service is `kuro_prefs::tracking::SECRET_SERVICE`.
/// MangaUpdates keeps its session token in the password slot, Simkl its access token in the password slot and the
/// token JSON in `track_token_101`.
const LEGACY: [(&str, TrackerId, Field); 5] = [
    ("pref_mangasync_username_7", TrackerId::MANGAUPDATES, Field::Username),
    ("pref_mangasync_password_7", TrackerId::MANGAUPDATES, Field::Password),
    ("pref_mangasync_username_101", TrackerId::SIMKL, Field::Username),
    ("pref_mangasync_password_101", TrackerId::SIMKL, Field::Password),
    ("track_token_101", TrackerId::SIMKL, Field::Token),
];

/// What the migration did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Migration {
    /// This many entries were moved. Zero on every start after the first.
    Moved(usize),
    /// No secret service is reachable. Nothing was moved and the old entries stay. The settings show
    /// [`UNENCRYPTED_WARNING`].
    StoreUnavailable,
    /// The store answered with an error. Entries already moved stay moved, the rest stay where they were.
    Failed(String),
}

impl Migration {
    /// The visible warning of an unreachable store (TST-64).
    pub fn warning(&self) -> Option<&'static str> {
        matches!(self, Migration::StoreUnavailable).then_some(UNENCRYPTED_WARNING)
    }
}

enum Stop {
    Unavailable,
    Failed(String),
}

impl From<SecretError> for Stop {
    fn from(e: SecretError) -> Self {
        match e {
            SecretError::Unavailable(_) => Stop::Unavailable,
            SecretError::Failed(m) => Stop::Failed(m),
        }
    }
}

/// Moves the old entries of MangaUpdates and Simkl into the tracker layout. The text `null` and empty values count as
/// absent (TST-42): they are deleted and not moved. A value already in the new place wins and the old one is dropped.
/// Run before the trackers are built, because they read their login state when they are created.
pub fn migrate_legacy_tracker_secrets(secrets: &dyn SecretStore) -> Migration {
    match run(secrets) {
        Ok(moved) => Migration::Moved(moved),
        Err(Stop::Unavailable) => Migration::StoreUnavailable,
        Err(Stop::Failed(message)) => Migration::Failed(message),
    }
}

fn run(secrets: &dyn SecretStore) -> Result<usize, Stop> {
    let old_service = kuro_prefs::tracking::SECRET_SERVICE;
    let mut moved = 0;
    for (old, tracker, field) in LEGACY {
        let Some(value) = secrets.get(old_service, old)? else {
            continue;
        };
        let new = account(tracker, field);
        let present = |v: &Option<String>| v.as_deref().is_some_and(|v| !v.is_empty());
        let wanted = !matches!(value.trim(), "" | "null");
        if wanted && !present(&secrets.get(SECRET_SERVICE, &new)?) {
            secrets.set(SECRET_SERVICE, &new, &value)?;
            moved += 1;
        }
        secrets.delete(old_service, old)?;
    }
    Ok(moved)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_prefs::store::PrefStore;
    use kuro_prefs::tracking::TrackingCredentials;
    use kuro_prefs::values::PrefValue;

    use super::*;
    use crate::events::EventBus;
    use crate::fakes::MemSecrets;
    use crate::mangaupdates::MangaUpdatesTracker;
    use crate::simkl::SimklTracker;
    use crate::tracker::Tracker;

    const OLD: &str = kuro_prefs::tracking::SECRET_SERVICE;

    fn seeded() -> Arc<MemSecrets> {
        let s = MemSecrets::new();
        for (account, value) in [
            ("pref_mangasync_username_7", "42"),
            ("pref_mangasync_password_7", "SESSION"),
            ("pref_mangasync_username_101", "7"),
            ("pref_mangasync_password_101", "AT"),
            ("track_token_101", r#"{"access_token":"AT"}"#),
        ] {
            s.set(OLD, account, value).unwrap();
        }
        s
    }

    #[test]
    fn t_32a_065_the_old_entries_move_to_the_tracker_layout_and_are_deleted() {
        let s = seeded();
        assert_eq!(migrate_legacy_tracker_secrets(s.as_ref()), Migration::Moved(5));
        assert_eq!(s.raw("tracker/7/password").as_deref(), Some("SESSION"));
        assert_eq!(s.raw("tracker/7/account").as_deref(), Some("42"));
        assert_eq!(s.raw("tracker/101/password").as_deref(), Some("AT"));
        assert_eq!(s.raw("tracker/101/oauth").as_deref(), Some(r#"{"access_token":"AT"}"#));
        for (old, _, _) in LEGACY {
            assert_eq!(s.get(OLD, old).unwrap(), None, "{old} is deleted");
        }
    }

    #[test]
    fn t_32a_065_a_second_start_moves_nothing_and_keeps_a_changed_value() {
        let s = seeded();
        migrate_legacy_tracker_secrets(s.as_ref());
        s.set(SECRET_SERVICE, "tracker/7/password", "NEWER").unwrap();
        assert_eq!(migrate_legacy_tracker_secrets(s.as_ref()), Migration::Moved(0));
        assert_eq!(s.raw("tracker/7/password").as_deref(), Some("NEWER"));
    }

    #[test]
    fn t_32a_065_a_value_in_the_new_place_wins_and_the_old_one_is_dropped() {
        let s = seeded();
        s.set(SECRET_SERVICE, "tracker/7/password", "KEPT").unwrap();
        assert_eq!(migrate_legacy_tracker_secrets(s.as_ref()), Migration::Moved(4));
        assert_eq!(s.raw("tracker/7/password").as_deref(), Some("KEPT"));
        assert_eq!(s.get(OLD, "pref_mangasync_password_7").unwrap(), None);
    }

    #[test]
    fn t_32a_042_the_text_null_and_empty_values_are_absent() {
        let s = MemSecrets::new();
        s.set(OLD, "track_token_101", "null").unwrap();
        s.set(OLD, "pref_mangasync_password_101", "").unwrap();
        assert_eq!(migrate_legacy_tracker_secrets(s.as_ref()), Migration::Moved(0));
        assert_eq!(s.raw("tracker/101/oauth"), None);
        assert_eq!(s.raw("tracker/101/password"), None);
        assert_eq!(s.get(OLD, "track_token_101").unwrap(), None, "the null text is cleaned up");
    }

    /// A store with no secret service behind it.
    struct NoService;
    impl SecretStore for NoService {
        fn get(&self, _: &str, _: &str) -> Result<Option<String>, SecretError> {
            Err(SecretError::Unavailable("no session bus".into()))
        }
        fn set(&self, _: &str, _: &str, _: &str) -> Result<(), SecretError> {
            Err(SecretError::Unavailable("no session bus".into()))
        }
        fn delete(&self, _: &str, _: &str) -> Result<(), SecretError> {
            Err(SecretError::Unavailable("no session bus".into()))
        }
    }

    #[test]
    fn t_32a_065_an_unreachable_store_gives_the_visible_warning_and_moves_nothing() {
        let m = migrate_legacy_tracker_secrets(&NoService);
        assert_eq!(m, Migration::StoreUnavailable);
        assert_eq!(m.warning(), Some("Tokens stored unencrypted"));
        assert_eq!(Migration::Moved(0).warning(), None);
    }

    #[test]
    fn t_32a_065_a_failed_write_keeps_the_old_entry() {
        let s = seeded();
        s.fail_set_for("tracker/7/password");
        let m = migrate_legacy_tracker_secrets(s.as_ref());
        assert!(matches!(m, Migration::Failed(_)), "{m:?}");
        assert_eq!(
            s.get(OLD, "pref_mangasync_password_7").unwrap().as_deref(),
            Some("SESSION"),
            "nothing is lost when the copy fails"
        );
    }

    #[test]
    fn t_32a_065_a_credential_from_the_backup_import_is_read_by_both_trackers() {
        let secrets = MemSecrets::new();
        // What the importer writes for a backup that holds the private keys.
        let creds = TrackingCredentials::new(PrefStore::in_memory(), secrets.clone());
        for (key, value) in [
            ("__PRIVATE_pref_mangasync_username_7", "42"),
            ("__PRIVATE_pref_mangasync_password_7", "SESSION"),
            ("__PRIVATE_pref_mangasync_username_101", "7"),
            ("__PRIVATE_pref_mangasync_password_101", "AT"),
            ("__PRIVATE_track_token_101", r#"{"access_token":"AT"}"#),
        ] {
            assert!(
                creds
                    .restore_private(key, &PrefValue::Str(value.into()))
                    .unwrap()
            );
        }
        let trackers = super::super::registry::start_trackers(secrets, EventBus::default()).unwrap();
        let mu = trackers.trackers.iter().find(|t| t.id() == TrackerId::MANGAUPDATES).unwrap();
        let sk = trackers.trackers.iter().find(|t| t.id() == TrackerId::SIMKL).unwrap();
        assert!(mu.is_logged_in() && sk.is_logged_in(), "not logged out after an import");
        assert_eq!((mu.username().as_str(), mu.password().as_str()), ("42", "SESSION"));
        assert_eq!((sk.username().as_str(), sk.password().as_str()), ("7", "AT"));
        assert_eq!(trackers.migration, Migration::Moved(5));
    }

    #[test]
    fn t_32a_065_trackers_built_without_the_migration_read_only_the_new_layout() {
        // The reason the registry migrates first: the trackers never look at the old service.
        let s = seeded();
        let dyn_s: Arc<dyn SecretStore> = s;
        let mu = MangaUpdatesTracker::with_base_url("http://127.0.0.1:9", dyn_s.clone(), EventBus::default()).unwrap();
        let sk = SimklTracker::with_base_url("http://127.0.0.1:9", dyn_s, EventBus::default()).unwrap();
        assert!(!mu.is_logged_in() && !sk.is_logged_in());
    }
}
