//! Tracking preferences (T-02a-018): the `[tracking]` switches, tracker credentials kept in the secret store,
//! and the auth-expired flags that live in `state.toml`.
//!
//! A tracker secret never reaches `config.toml` or `state.toml`. [`SecretPref`] reads and writes only through
//! [`SecretStore`]; the auth-expired flag is an ordinary private bool and goes to `state.toml`.

use std::sync::Arc;

use kuro_platform::secrets::{SecretError, SecretStore};

use crate::pref::Pref;
use crate::registry::define_prefs;
use crate::store::PrefStore;
use crate::values::PrefValue;
use crate::{PrefError, pref_enum};

/// Secret store service name for every tracker secret.
pub const SECRET_SERVICE: &str = "kurobayomi";

pref_enum! {
    /// `pref_auto_update_manga_on_mark_read`, stored as the constant name.
    pub enum AutoTrackState { ALWAYS, ASK, NEVER }
}

define_prefs! {
    /// `[tracking]` section: the switches of Aniyomi `TrackPreferences`.
    pub struct TrackPrefs in "tracking" {
        anilist_score_type: String = "anilist_score_type" => String::from("POINT_10");
        auto_update_track: bool = "pref_auto_update_manga_sync_key" => true;
        track_on_adding_to_library: bool = "track_on_adding_to_library" => true;
        show_next_episode_airing_time: bool = "show_next_episode_airing_time" => true;
        auto_update_track_on_mark_read: AutoTrackState = "pref_auto_update_manga_on_mark_read" => AutoTrackState::ALWAYS;
    }
}

/// Failure of a credential operation: either the secret store or the preference file.
#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error(transparent)]
    Pref(#[from] PrefError),
}

/// One tracker secret. `get` gives "" when nothing is stored, like the Kotlin default.
#[derive(Clone)]
pub struct SecretPref {
    store: Arc<dyn SecretStore>,
    account: String,
}

impl SecretPref {
    pub fn get(&self) -> Result<String, SecretError> {
        Ok(self.store.get(SECRET_SERVICE, &self.account)?.unwrap_or_default())
    }

    pub fn set(&self, value: &str) -> Result<(), SecretError> {
        self.store.set(SECRET_SERVICE, &self.account, value)
    }

    pub fn delete(&self) -> Result<(), SecretError> {
        self.store.delete(SECRET_SERVICE, &self.account)
    }

    /// The secret-store account name, for example `pref_mangasync_username_3`.
    pub fn account(&self) -> &str {
        &self.account
    }
}

/// Tracker credentials and their flags, per tracker id.
#[derive(Clone)]
pub struct TrackingCredentials {
    prefs: PrefStore,
    secrets: Arc<dyn SecretStore>,
}

/// `__PRIVATE_pref_mangasync_username_<id>`
pub fn username_key(tracker: i64) -> String {
    format!("__PRIVATE_pref_mangasync_username_{tracker}")
}

/// `__PRIVATE_pref_mangasync_password_<id>`
pub fn password_key(tracker: i64) -> String {
    format!("__PRIVATE_pref_mangasync_password_{tracker}")
}

/// `__PRIVATE_pref_tracker_auth_expired_<id>`
pub fn auth_expired_key(tracker: i64) -> String {
    format!("__PRIVATE_pref_tracker_auth_expired_{tracker}")
}

/// `__PRIVATE_track_token_<id>`
pub fn token_key(tracker: i64) -> String {
    format!("__PRIVATE_track_token_{tracker}")
}

/// The four private key families of one tracker, in export order.
fn private_keys(tracker: i64) -> [String; 4] {
    [username_key(tracker), password_key(tracker), auth_expired_key(tracker), token_key(tracker)]
}

impl TrackingCredentials {
    pub fn new(prefs: PrefStore, secrets: Arc<dyn SecretStore>) -> Self {
        Self { prefs, secrets }
    }

    pub fn track_username(&self, tracker: i64) -> SecretPref {
        self.secret(username_key(tracker))
    }

    pub fn track_password(&self, tracker: i64) -> SecretPref {
        self.secret(password_key(tracker))
    }

    pub fn track_token(&self, tracker: i64) -> SecretPref {
        self.secret(token_key(tracker))
    }

    pub fn track_auth_expired(&self, tracker: i64) -> Pref<bool> {
        self.prefs.pref(auth_expired_key(tracker), false)
    }

    /// `setCredentials`: username, then password, then the auth-expired flag is cleared. A failing write stops
    /// the sequence, so the flag stays as it was.
    pub fn set_credentials(&self, tracker: i64, username: &str, password: &str) -> Result<(), CredentialError> {
        self.track_username(tracker).set(username)?;
        self.track_password(tracker).set(password)?;
        self.track_auth_expired(tracker).set(false)?;
        Ok(())
    }

    /// The `__PRIVATE_` entries of a backup for the given trackers. Empty unless `include_private` is set.
    pub fn export_private(&self, trackers: &[i64], include_private: bool) -> Result<Vec<(String, PrefValue)>, CredentialError> {
        if !include_private {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for &tracker in trackers {
            let [user, pass, expired, token] = private_keys(tracker);
            out.push((user, PrefValue::Str(self.track_username(tracker).get()?)));
            out.push((pass, PrefValue::Str(self.track_password(tracker).get()?)));
            out.push((expired, PrefValue::Bool(self.track_auth_expired(tracker).get())));
            out.push((token, PrefValue::Str(self.track_token(tracker).get()?)));
        }
        Ok(out)
    }

    /// Writes one restored private entry to its destination (keyring for strings, `state.toml` for the flag).
    /// Returns `Ok(false)` when `key` is not a tracker private key.
    pub fn restore_private(&self, key: &str, value: &PrefValue) -> Result<bool, CredentialError> {
        let Some(tracker) = tracker_of(key) else {
            return Ok(false);
        };
        match (key, value) {
            (k, PrefValue::Str(s)) if k == username_key(tracker) => self.track_username(tracker).set(s)?,
            (k, PrefValue::Str(s)) if k == password_key(tracker) => self.track_password(tracker).set(s)?,
            (k, PrefValue::Str(s)) if k == token_key(tracker) => self.track_token(tracker).set(s)?,
            (k, PrefValue::Bool(b)) if k == auth_expired_key(tracker) => self.track_auth_expired(tracker).set(*b)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn secret(&self, key: String) -> SecretPref {
        SecretPref { store: Arc::clone(&self.secrets), account: key.trim_start_matches("__PRIVATE_").to_owned() }
    }
}

/// The tracker id of a private tracker key, or `None` for any other key.
fn tracker_of(key: &str) -> Option<i64> {
    let rest = key
        .strip_prefix("__PRIVATE_pref_mangasync_username_")
        .or_else(|| key.strip_prefix("__PRIVATE_pref_mangasync_password_"))
        .or_else(|| key.strip_prefix("__PRIVATE_pref_tracker_auth_expired_"))
        .or_else(|| key.strip_prefix("__PRIVATE_track_token_"))?;
    rest.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{MemorySecrets, TestDir};
    use crate::toml_backend::TomlBackend;

    fn creds() -> (TrackingCredentials, PrefStore, Arc<MemorySecrets>) {
        let store = PrefStore::in_memory();
        let secrets = Arc::new(MemorySecrets::default());
        (TrackingCredentials::new(store.clone(), secrets.clone()), store, secrets)
    }

    #[test]
    fn switches_have_the_plan_defaults_and_registered_keys() {
        let prefs = TrackPrefs::new(PrefStore::in_memory());
        assert!(prefs.auto_update_track.get());
        assert!(prefs.track_on_adding_to_library.get());
        assert!(prefs.show_next_episode_airing_time.get());
        assert_eq!(prefs.anilist_score_type.get(), "POINT_10");
        assert_eq!(prefs.auto_update_track_on_mark_read.get(), AutoTrackState::ALWAYS);
        assert_eq!(prefs.auto_update_track_on_mark_read.key(), "pref_auto_update_manga_on_mark_read");
        assert_eq!(TrackPrefs::TABLE, "tracking");
        crate::testutil::assert_section_registered(TrackPrefs::TABLE, TrackPrefs::FIELDS, &prefs.defaults());
    }

    #[test]
    fn secrets_round_trip_through_the_secret_store_only() {
        let (c, store, secrets) = creds();
        c.track_token(3).set("tok").unwrap();
        assert_eq!(c.track_token(3).get().unwrap(), "tok");
        assert_eq!(c.track_token(4).get().unwrap(), "", "absent is empty");
        assert_eq!(secrets.get(SECRET_SERVICE, "track_token_3").unwrap().as_deref(), Some("tok"));
        assert_eq!(store.get_raw("__PRIVATE_track_token_3"), None, "never in the preference store");
        c.track_token(3).delete().unwrap();
        assert_eq!(c.track_token(3).get().unwrap(), "");
    }

    #[test]
    fn set_credentials_writes_username_then_password_then_clears_the_flag() {
        let (c, _store, secrets) = creds();
        c.track_auth_expired(7).set(true).unwrap();
        c.set_credentials(7, "user", "pass").unwrap();
        assert_eq!(c.track_username(7).get().unwrap(), "user");
        assert_eq!(c.track_password(7).get().unwrap(), "pass");
        assert!(!c.track_auth_expired(7).get());
        assert!(secrets.get(SECRET_SERVICE, "pref_mangasync_password_7").unwrap().is_some());
    }

    #[test]
    fn a_failing_password_write_leaves_the_flag_set() {
        struct FailPassword(MemorySecrets);
        impl SecretStore for FailPassword {
            fn get(&self, s: &str, a: &str) -> Result<Option<String>, SecretError> {
                self.0.get(s, a)
            }
            fn set(&self, s: &str, a: &str, v: &str) -> Result<(), SecretError> {
                if a.contains("password") {
                    return Err(SecretError::Failed("disk".into()));
                }
                self.0.set(s, a, v)
            }
            fn delete(&self, s: &str, a: &str) -> Result<(), SecretError> {
                self.0.delete(s, a)
            }
        }
        let store = PrefStore::in_memory();
        let c = TrackingCredentials::new(store, Arc::new(FailPassword(MemorySecrets::default())));
        c.track_auth_expired(2).set(true).unwrap();
        assert!(c.set_credentials(2, "u", "p").is_err());
        assert!(c.track_auth_expired(2).get(), "flag unchanged after the failed password write");
    }

    #[test]
    fn export_omits_private_entries_unless_asked() {
        let (c, _store, _secrets) = creds();
        c.set_credentials(5, "user", "pass").unwrap();
        assert!(c.export_private(&[5], false).unwrap().is_empty());
        let exported = c.export_private(&[5], true).unwrap();
        let keys: Vec<&str> = exported.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            [
                "__PRIVATE_pref_mangasync_username_5",
                "__PRIVATE_pref_mangasync_password_5",
                "__PRIVATE_pref_tracker_auth_expired_5",
                "__PRIVATE_track_token_5",
            ]
        );
        assert_eq!(exported[0].1, PrefValue::Str("user".into()));
    }

    #[test]
    fn restore_private_writes_each_family_to_its_destination() {
        let (c, store, _secrets) = creds();
        assert!(c.restore_private("__PRIVATE_track_token_9", &PrefValue::Str("t".into())).unwrap());
        assert_eq!(c.track_token(9).get().unwrap(), "t");
        assert!(c.restore_private("__PRIVATE_pref_tracker_auth_expired_9", &PrefValue::Bool(true)).unwrap());
        assert!(c.track_auth_expired(9).get());
        assert!(!c.restore_private("theme", &PrefValue::Str("x".into())).unwrap());
        assert_eq!(store.get_raw("__PRIVATE_pref_tracker_auth_expired_9"), Some(PrefValue::Bool(true)));
    }

    #[test]
    fn no_secret_reaches_config_or_state_files() {
        let dir = TestDir::new("tracking-secrets");
        let config = dir.path().join("config");
        let state = dir.path().join("state");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        let store = PrefStore::new(Box::new(TomlBackend::open(&config, &state).unwrap())).unwrap();
        let c = TrackingCredentials::new(store.clone(), Arc::new(MemorySecrets::default()));
        c.set_credentials(1, "visible-user", "hunter2-secret").unwrap();
        c.track_token(1).set("tok-secret").unwrap();
        c.track_auth_expired(1).set(true).unwrap();
        store.flush().unwrap();
        for file in [config.join("config.toml"), state.join("state.toml")] {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            assert!(!text.contains("hunter2-secret"), "{}", file.display());
            assert!(!text.contains("tok-secret"), "{}", file.display());
            assert!(!text.contains("visible-user"), "{}", file.display());
        }
    }
}
