//! Kitsu OAuth token and its stored form (T-30a-004, T-30a-005). The token JSON is one secret in the platform secret
//! store. A missing, empty, `null` or malformed value restores as no token.
//!
//! Decision (T-30a-005): there is no fallback to the `track_token_3` setting when the secret store is unavailable. A
//! token in preferences would sit in plain text in the settings file and in backups, and `base.rs` keeps tracker
//! secrets out of preferences. An unavailable store is `KitsuError::SecretStorage`, so login fails with a message
//! instead of storing the token somewhere weaker.

use std::sync::Arc;

use kuro_platform::secrets::SecretStore;
use serde::{Deserialize, Serialize};

use super::error::KitsuError;
use crate::base::SECRET_SERVICE;
use crate::model::TrackerId;

/// An access token. Unknown keys such as `scope` are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuth {
    pub access_token: String,
    pub token_type: String,
    /// Unix seconds, from the server.
    pub created_at: i64,
    /// Seconds.
    pub expires_in: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

impl OAuth {
    /// Expired one hour before the server's end, so a request never starts with a token about to die.
    pub fn is_expired(&self, now_secs: i64) -> bool {
        now_secs > self.created_at + self.expires_in - 3600
    }
}

/// The secret-store account of the token: `tracker/3/oauth`.
pub fn token_account() -> String {
    crate::base::account(TrackerId::KITSU, crate::base::Field::Token)
}

/// The secret-store account of the user id: `tracker/3/user_id`. The user id is not a credential, but it lives with
/// the login so that logout clears it.
pub fn user_id_account() -> String {
    format!("tracker/{}/user_id", TrackerId::KITSU.0)
}

/// The token JSON to restore. `""`, `null` and malformed JSON are no token.
pub fn parse_stored(text: Option<&str>) -> Option<OAuth> {
    let text = text?.trim();
    if text.is_empty() || text == "null" {
        return None;
    }
    serde_json::from_str(text).ok()
}

/// Reads the stored token. A store that cannot be read counts as no token.
pub fn load(secrets: &Arc<dyn SecretStore>) -> Option<OAuth> {
    let text = secrets.get(SECRET_SERVICE, &token_account()).ok().flatten();
    parse_stored(text.as_deref())
}

/// Writes the token JSON. A `null` is never written.
pub fn save(secrets: &dyn SecretStore, token: &OAuth) -> Result<(), KitsuError> {
    let json = serde_json::to_string(token)?;
    secrets.set(SECRET_SERVICE, &token_account(), &json)?;
    Ok(())
}

/// Deletes the token, the user id and the username. Deleting a missing entry is fine, so logout is idempotent.
pub fn clear(secrets: &dyn SecretStore) -> Result<(), KitsuError> {
    secrets.delete(SECRET_SERVICE, &token_account())?;
    secrets.delete(SECRET_SERVICE, &user_id_account())?;
    secrets.delete(
        SECRET_SERVICE,
        &crate::base::account(TrackerId::KITSU, crate::base::Field::Username),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::MemSecrets;

    fn token(created: i64, expires: i64) -> OAuth {
        OAuth {
            access_token: "a".into(),
            token_type: "bearer".into(),
            created_at: created,
            expires_in: expires,
            refresh_token: Some("r".into()),
        }
    }

    #[test]
    fn t_30a_004_expiry_leaves_an_hour_of_margin() {
        let t = token(1_000, 7_200);
        // The end is 1000 + 7200 = 8200, minus the 3600 s margin: expired strictly after 4600.
        assert!(!t.is_expired(4_600));
        assert!(t.is_expired(4_601));
    }

    #[test]
    fn t_30a_004_unknown_keys_are_ignored() {
        let text = r#"{"access_token":"a","token_type":"bearer","created_at":1,"expires_in":2,"scope":"public"}"#;
        let t: OAuth = serde_json::from_str(text).unwrap();
        assert_eq!(t.refresh_token, None);
        assert_eq!(t.expires_in, 2);
    }

    #[test]
    fn t_30a_005_empty_null_and_malformed_restore_as_no_token() {
        assert_eq!(parse_stored(None), None);
        assert_eq!(parse_stored(Some("")), None);
        assert_eq!(parse_stored(Some("null")), None);
        assert_eq!(parse_stored(Some("{not json")), None);
    }

    #[test]
    fn t_30a_005_save_then_load_round_trips_and_clear_removes_the_entry() {
        let secrets: Arc<dyn SecretStore> = MemSecrets::new();
        let t = token(10, 20);
        save(secrets.as_ref(), &t).unwrap();
        assert_eq!(load(&secrets), Some(t));
        clear(secrets.as_ref()).unwrap();
        assert_eq!(load(&secrets), None);
        // Clearing again is fine.
        clear(secrets.as_ref()).unwrap();
    }

    #[test]
    fn t_30a_005_a_refresh_token_is_omitted_when_absent() {
        let mut t = token(1, 2);
        t.refresh_token = None;
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("null"));
        assert!(!json.contains("refresh_token"));
    }
}
