//! Credential state shared by every tracker (`BaseTracker.kt`), and the secret-store layout of the tracker secrets
//! (`p2-topic-oauth` and `p2-behavior`, behaviour T-27b-027). Every tracker secret lives in the platform secret store,
//! never in preferences or logs.

use std::sync::Arc;

use kuro_platform::secrets::SecretStore;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::watch;

use crate::error::TrackError;
use crate::events::{EventBus, TrackEvent};
use crate::model::TrackerId;

/// Secret Service service name of every tracker secret.
pub const SECRET_SERVICE: &str = "kurobayomi.track";

/// One secret of a tracker. The account is `tracker/<id>/<suffix>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// The username (`account`).
    Username,
    /// The password, or the access token of a browser login (`password`). Credential trackers store their secret here.
    Password,
    /// The OAuth token JSON (`oauth`).
    Token,
    /// The auth-expired flag (`expired`), stored as `"true"` while set.
    Expired,
}

impl Field {
    fn suffix(self) -> &'static str {
        match self {
            Field::Username => "account",
            Field::Password => "password",
            Field::Token => "oauth",
            Field::Expired => "expired",
        }
    }
}

/// The secret-store account of `field` for `tracker`.
pub fn account(tracker: TrackerId, field: Field) -> String {
    format!("tracker/{}/{}", tracker.0, field.suffix())
}

/// Reads one secret. A missing or unreadable secret counts as empty, so an unreadable store reads as logged out
/// instead of failing every status check.
fn read(secrets: &dyn SecretStore, tracker: TrackerId, field: Field) -> String {
    secrets
        .get(SECRET_SERVICE, &account(tracker, field))
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Logged in when username and password are both non-empty (`BaseTracker.kt:32-34`).
fn logged_in(secrets: &dyn SecretStore, tracker: TrackerId) -> bool {
    !read(secrets, tracker, Field::Username).is_empty()
        && !read(secrets, tracker, Field::Password).is_empty()
}

/// Credential state of one tracker. The login state is recomputed from the secret store on every write, and published
/// only when it changes. Every committed write also sends a [`TrackEvent::LoginChanged`].
pub struct BaseTracker {
    id: TrackerId,
    name: &'static str,
    secrets: Arc<dyn SecretStore>,
    events: EventBus,
    login_tx: watch::Sender<bool>,
}

impl BaseTracker {
    pub fn new(
        id: TrackerId,
        name: &'static str,
        secrets: Arc<dyn SecretStore>,
        events: EventBus,
    ) -> Self {
        let (login_tx, _) = watch::channel(logged_in(secrets.as_ref(), id));
        Self {
            id,
            name,
            secrets,
            events,
            login_tx,
        }
    }

    pub fn id(&self) -> TrackerId {
        self.id
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn username(&self) -> String {
        read(self.secrets.as_ref(), self.id, Field::Username)
    }

    pub fn password(&self) -> String {
        read(self.secrets.as_ref(), self.id, Field::Password)
    }

    pub fn is_logged_in(&self) -> bool {
        logged_in(self.secrets.as_ref(), self.id)
    }

    pub fn login_state(&self) -> watch::Receiver<bool> {
        self.login_tx.subscribe()
    }

    /// Whether the server refused the stored login (the auth-expired flag).
    pub fn auth_expired(&self) -> bool {
        read(self.secrets.as_ref(), self.id, Field::Expired) == "true"
    }

    /// Sets or clears the auth-expired flag. Clearing deletes the entry, so the flag is absent after a login or logout.
    pub fn set_auth_expired(&self, expired: bool) -> Result<(), TrackError> {
        let account = account(self.id, Field::Expired);
        if expired {
            self.secrets.set(SECRET_SERVICE, &account, "true")?;
        } else {
            self.secrets.delete(SECRET_SERVICE, &account)?;
        }
        self.events.send(TrackEvent::LoginChanged {
            tracker_id: self.id,
        });
        Ok(())
    }

    /// Deletes the old password, writes username, then password, then clears auth-expired, then publishes the login
    /// state. Deleting first means an interruption between the two writes never leaves (new username, old password).
    /// When the password write fails, the username is cleared again so the tracker stays logged out.
    pub fn save_credentials(&self, username: &str, password: &str) -> Result<(), TrackError> {
        self.secrets
            .delete(SECRET_SERVICE, &account(self.id, Field::Password))?;
        self.secrets
            .set(SECRET_SERVICE, &account(self.id, Field::Username), username)?;
        if let Err(e) =
            self.secrets
                .set(SECRET_SERVICE, &account(self.id, Field::Password), password)
        {
            let _ = self
                .secrets
                .set(SECRET_SERVICE, &account(self.id, Field::Username), "");
            self.publish();
            return Err(e.into());
        }
        let cleared = self
            .secrets
            .delete(SECRET_SERVICE, &account(self.id, Field::Expired));
        self.publish();
        self.events.send(TrackEvent::LoginChanged {
            tracker_id: self.id,
        });
        cleared.map_err(TrackError::from)
    }

    /// Deletes username, password, token and the auth-expired flag, then publishes `false`. Local track rows are not
    /// touched. Idempotent: deleting a missing entry is not an error.
    pub fn logout(&self) -> Result<(), TrackError> {
        let result = self.clear();
        self.publish();
        self.events.send(TrackEvent::LoginChanged {
            tracker_id: self.id,
        });
        result
    }

    fn clear(&self) -> Result<(), TrackError> {
        for field in [
            Field::Username,
            Field::Password,
            Field::Token,
            Field::Expired,
        ] {
            self.secrets
                .delete(SECRET_SERVICE, &account(self.id, field))?;
        }
        Ok(())
    }

    /// Completes a browser login (T-27b-027): stores the token JSON under `oauth`, then the username and the access
    /// token in the username and password slots, which is what [`BaseTracker::is_logged_in`] reads. The auth-expired
    /// flag is cleared, the login state is published and [`TrackEvent::LoginChanged`] is sent. A refreshed token
    /// goes through here too, with the stored username. A failed write leaves no token behind.
    pub fn save_oauth<T: Serialize>(
        &self,
        username: &str,
        access_token: &str,
        token: &T,
    ) -> Result<(), TrackError> {
        self.save_token(token)?;
        if let Err(e) = self.save_credentials(username, access_token) {
            let _ = self
                .secrets
                .delete(SECRET_SERVICE, &account(self.id, Field::Token));
            return Err(e);
        }
        Ok(())
    }

    /// Stores the token as JSON under the `oauth` account. This alone does not log in: use [`BaseTracker::save_oauth`].
    pub fn save_token<T: Serialize>(&self, token: &T) -> Result<(), TrackError> {
        let json = serde_json::to_string(token)?;
        self.secrets
            .set(SECRET_SERVICE, &account(self.id, Field::Token), &json)?;
        Ok(())
    }

    /// The stored token, or `None` when there is none.
    pub fn load_token<T: DeserializeOwned>(&self) -> Result<Option<T>, TrackError> {
        match self
            .secrets
            .get(SECRET_SERVICE, &account(self.id, Field::Token))?
        {
            Some(json) if !json.is_empty() => Ok(Some(serde_json::from_str(&json)?)),
            _ => Ok(None),
        }
    }

    fn publish(&self) {
        let now = logged_in(self.secrets.as_ref(), self.id);
        self.login_tx.send_if_modified(|v| {
            if *v == now {
                false
            } else {
                *v = now;
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_platform::secrets::SecretError;

    use super::*;
    use crate::fakes::MemSecrets;

    fn base(secrets: &Arc<MemSecrets>) -> BaseTracker {
        BaseTracker::new(
            TrackerId::KITSU,
            "Kitsu",
            secrets.clone(),
            EventBus::default(),
        )
    }

    fn kitsu(field: Field) -> String {
        account(TrackerId::KITSU, field)
    }

    #[test]
    fn tr_27_003_logged_in_needs_both_username_and_password() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        assert!(!b.is_logged_in());
        secrets
            .set(SECRET_SERVICE, &kitsu(Field::Username), "someone")
            .unwrap();
        assert!(!b.is_logged_in(), "username alone is not a login");
        secrets
            .set(SECRET_SERVICE, &kitsu(Field::Password), "pw")
            .unwrap();
        assert!(b.is_logged_in());
    }

    #[test]
    fn tr_27b_027_an_oauth_login_fills_the_password_slot_and_reports_logged_in() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        let rx = b.login_state();
        b.save_oauth("someone", "access-1", &serde_json::json!({"access": "access-1"}))
            .unwrap();
        assert!(b.is_logged_in());
        assert!(rx.has_changed().unwrap(), "the login state was published");
        assert_eq!(
            secrets
                .get(SECRET_SERVICE, &kitsu(Field::Password))
                .unwrap()
                .as_deref(),
            Some("access-1")
        );
        assert!(
            secrets
                .get(SECRET_SERVICE, &kitsu(Field::Token))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn tr_27b_027_an_oauth_login_without_a_secret_service_reports_it_and_stores_nothing() {
        let secrets = MemSecrets::new();
        secrets.fail_set_for(&kitsu(Field::Token));
        secrets.fail_with_unavailable();
        let err = base(&secrets)
            .save_oauth("someone", "access-1", &serde_json::json!({"access": "access-1"}))
            .unwrap_err();
        assert!(matches!(err, TrackError::SecretsUnavailable));
        assert!(secrets.all_values().is_empty());
    }

    #[test]
    fn tr_27_003_save_then_logout_publishes_each_change_once() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        let mut rx = b.login_state();
        b.save_credentials("u", "p").unwrap();
        assert!(rx.has_changed().unwrap());
        assert!(*rx.borrow_and_update());
        b.save_credentials("u", "p").unwrap();
        assert!(
            !rx.has_changed().unwrap(),
            "same state is not published again"
        );
        b.logout().unwrap();
        assert!(!*rx.borrow_and_update());
        assert!(!b.is_logged_in());
        assert_eq!(b.username(), "");
        assert_eq!(b.password(), "");
    }

    #[test]
    fn tr_27b_028_logout_deletes_the_credentials_token_and_expired_flag() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        b.save_credentials("u", "p").unwrap();
        b.save_token(&serde_json::json!({"access": "abc"})).unwrap();
        b.set_auth_expired(true).unwrap();
        b.logout().unwrap();
        for field in [
            Field::Username,
            Field::Password,
            Field::Token,
            Field::Expired,
        ] {
            assert!(secrets.raw(&kitsu(field)).is_none(), "{field:?} is deleted");
        }
        assert!(!b.auth_expired());
    }

    #[test]
    fn tr_27b_028_logout_is_idempotent_and_keeps_the_login_state_false() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        b.logout().unwrap();
        b.logout().unwrap();
        assert!(!b.is_logged_in());
        assert!(!b.auth_expired());
    }

    #[test]
    fn tr_27b_027_the_expired_flag_is_a_secret_not_a_preference() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        b.set_auth_expired(true).unwrap();
        assert_eq!(
            secrets
                .raw(&account(TrackerId::KITSU, Field::Expired))
                .as_deref(),
            Some("true")
        );
        assert!(b.auth_expired());
        b.save_credentials("u", "p").unwrap();
        assert!(!b.auth_expired(), "a new login clears the flag");
    }

    #[test]
    fn tr_27_003_failed_password_write_leaves_the_tracker_logged_out() {
        let secrets = MemSecrets::new();
        secrets.fail_set_for(&kitsu(Field::Password));
        let b = base(&secrets);
        let rx = b.login_state();
        assert!(b.save_credentials("u", "p").is_err());
        assert!(!b.is_logged_in());
        assert!(!*rx.borrow(), "nothing is published as logged in");
        let restarted = base(&secrets);
        assert!(!restarted.is_logged_in(), "the next start reads logged out");
    }

    #[test]
    fn tr_27_003_failed_password_write_while_logged_in_leaves_the_tracker_logged_out() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        b.save_credentials("u1", "p1").unwrap();
        secrets.fail_set_for(&kitsu(Field::Password));
        assert!(b.save_credentials("u2", "p2").is_err());
        assert!(
            !base(&secrets).is_logged_in(),
            "the next start reads logged out"
        );
        assert_eq!(secrets.raw(&kitsu(Field::Password)).unwrap_or_default(), "");
    }

    #[test]
    fn tr_27_004_login_state_receiver_reflects_the_saved_state() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        let mut rx = b.login_state();
        assert!(!*rx.borrow());
        b.save_credentials("user", "token").unwrap();
        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), b.is_logged_in());
    }

    #[test]
    fn tr_27b_027_secrets_use_the_tracker_account_layout() {
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        b.save_credentials("name", "pw").unwrap();
        assert_eq!(secrets.raw("tracker/3/account").as_deref(), Some("name"));
        assert_eq!(secrets.raw("tracker/3/password").as_deref(), Some("pw"));
        assert_eq!(SECRET_SERVICE, "kurobayomi.track");
        assert_eq!(account(TrackerId::SIMKL, Field::Token), "tracker/101/oauth");
        assert_eq!(
            account(TrackerId::ANILIST, Field::Expired),
            "tracker/2/expired"
        );
    }

    #[test]
    fn tr_27b_027_an_unavailable_store_reports_secret_storage_unavailable() {
        let secrets = MemSecrets::new();
        secrets.fail_set_for(&kitsu(Field::Username));
        secrets.fail_with_unavailable();
        let b = base(&secrets);
        let err = b.save_credentials("u", "p").unwrap_err();
        assert_eq!(err.to_string(), "Secret storage is unavailable");
        assert!(
            !b.is_logged_in(),
            "nothing is stored and the state stays logged out"
        );
    }

    #[test]
    fn tr_27_020_token_round_trips_as_json_and_never_reaches_prefs() {
        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Tok {
            access: String,
        }
        let secrets = MemSecrets::new();
        let b = base(&secrets);
        assert_eq!(b.load_token::<Tok>().unwrap(), None);
        b.save_token(&Tok {
            access: "secret-token-value".into(),
        })
        .unwrap();
        assert_eq!(
            b.load_token::<Tok>().unwrap(),
            Some(Tok {
                access: "secret-token-value".into()
            })
        );
        let stored = secrets.all_values().join("\n");
        assert!(
            stored.contains("secret-token-value"),
            "token is in the secret store"
        );
    }

    #[test]
    fn tr_27b_040_every_committed_login_write_sends_login_changed() {
        let secrets = MemSecrets::new();
        let events = EventBus::default();
        let mut rx = events.subscribe();
        let b = BaseTracker::new(TrackerId::KITSU, "Kitsu", secrets.clone(), events);
        b.save_credentials("u", "p").unwrap();
        b.logout().unwrap();
        let expected = TrackEvent::LoginChanged {
            tracker_id: TrackerId::KITSU,
        };
        assert_eq!(rx.try_recv().unwrap(), expected);
        assert_eq!(rx.try_recv().unwrap(), expected);
    }

    #[test]
    fn tr_27b_027_secret_error_kinds_stay_distinct() {
        let unavailable: TrackError = SecretError::Unavailable("no bus".into()).into();
        let failed: TrackError = SecretError::Failed("disk".into()).into();
        assert!(matches!(unavailable, TrackError::SecretsUnavailable));
        assert!(matches!(failed, TrackError::Secret(_)));
    }
}
