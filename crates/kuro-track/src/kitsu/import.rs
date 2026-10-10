//! What an Aniyomi backup brings for Kitsu (T-30a-032). The backup importer calls these helpers; it does not own the
//! Kitsu rules.
//!
//! Decisions: a row with a remote id was bound before the backup was made, so it is bound (remote-id lifecycle,
//! section 4). The backup's `password` slot holds a Kitsu user id, not a password, so no credential is imported and the
//! user logs in once after the import. The backup's `track_token_3` is accepted as the session token only when it is
//! valid token JSON and no token is stored yet.

use kuro_platform::secrets::SecretStore;

use super::auth::{self, OAuth};
use super::error::KitsuError;

/// The tracker id of Kitsu in Aniyomi backups.
pub const BACKUP_TRACKER_ID: i64 = 3;

/// The backup key of the Kitsu session token, used as a fallback when the secret store holds none.
pub const FALLBACK_TOKEN_KEY: &str = "track_token_3";

/// Whether an imported Kitsu row is bound: a row that has a remote id was bound before the backup was made.
pub fn imported_row_is_bound(remote_id: i64) -> bool {
    remote_id > 0
}

/// Stores the backup's `track_token_3` as the session token when it parses and no token is stored yet. Returns the
/// token that was stored, or `None` when nothing was imported.
pub fn import_fallback_token(
    secrets: &dyn SecretStore,
    backup_token: Option<&str>,
) -> Result<Option<OAuth>, KitsuError> {
    let stored = secrets
        .get(crate::base::SECRET_SERVICE, &auth::token_account())
        .ok()
        .flatten();
    if auth::parse_stored(stored.as_deref()).is_some() {
        return Ok(None);
    }
    let Some(token) = auth::parse_stored(backup_token) else {
        return Ok(None);
    };
    auth::save(secrets, &token)?;
    Ok(Some(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::MemSecrets;

    const TOKEN: &str = r#"{"access_token":"kitsu-token","token_type":"Bearer","created_at":1700000000,"expires_in":7200,"refresh_token":"r"}"#;

    #[test]
    fn t_30a_032_a_row_with_a_remote_id_is_bound_and_one_without_is_not() {
        assert!(imported_row_is_bound(9001));
        assert!(!imported_row_is_bound(0));
    }

    #[test]
    fn t_30a_032_a_valid_fallback_token_becomes_the_session_token() {
        let secrets = MemSecrets::new();
        let got = import_fallback_token(secrets.as_ref(), Some(TOKEN)).unwrap();
        assert_eq!(got.map(|t| t.access_token), Some("kitsu-token".to_owned()));
        assert!(auth::parse_stored(secrets.get(crate::base::SECRET_SERVICE, &auth::token_account()).unwrap().as_deref()).is_some());
    }

    #[test]
    fn t_30a_032_a_stored_token_or_a_bad_fallback_is_kept_and_nothing_is_imported() {
        let secrets = MemSecrets::new();
        assert_eq!(import_fallback_token(secrets.as_ref(), Some("not json")).unwrap(), None);
        assert_eq!(import_fallback_token(secrets.as_ref(), None).unwrap(), None);
        secrets
            .set(crate::base::SECRET_SERVICE, &auth::token_account(), TOKEN)
            .unwrap();
        let other = TOKEN.replace("kitsu-token", "other");
        assert_eq!(import_fallback_token(secrets.as_ref(), Some(&other)).unwrap(), None);
        let kept = secrets.get(crate::base::SECRET_SERVICE, &auth::token_account()).unwrap();
        assert!(kept.unwrap_or_default().contains("kitsu-token"));
    }
}
