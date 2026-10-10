//! Shikimori token state (T-31a-005). The token is one JSON secret kept by [`crate::base::BaseTracker`]. This module
//! keeps the in-memory copy, a generation counter for logout (a refresh that finishes after logout is discarded) and
//! the lock that makes a refresh single flight. The network side is in [`super::api`].

use std::sync::{Mutex, MutexGuard, PoisonError};

use super::dto::SmOAuth;

/// Refresh starts once less than an hour remains (`SMOAuth.isExpired`, D-behaviour 2.6). Strict: a token is expired
/// only when `now` is past `created_at + expires_in - 3600`.
pub fn is_expired(token: &SmOAuth, now_secs: i64) -> bool {
    now_secs > token.created_at + token.expires_in - 3600
}

#[derive(Default)]
struct Slot {
    token: Option<SmOAuth>,
    /// Bumped on every logout and login.
    generation: u64,
}

/// The in-memory token. Never held across an `await`.
#[derive(Default)]
pub struct TokenSlot {
    slot: Mutex<Slot>,
    /// Held for the whole refresh, so concurrent callers refresh once (single flight, T-31a-005).
    pub(super) refresh_lock: tokio::sync::Mutex<()>,
}

impl TokenSlot {
    pub fn with_token(token: Option<SmOAuth>) -> Self {
        Self {
            slot: Mutex::new(Slot {
                token,
                generation: 0,
            }),
            refresh_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Slot> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The token and the generation it was read under.
    pub fn snapshot(&self) -> Option<(SmOAuth, u64)> {
        let slot = self.lock();
        slot.token.clone().map(|t| (t, slot.generation))
    }

    pub fn has_token(&self) -> bool {
        self.lock().token.is_some()
    }

    /// Stores `token` when no logout happened since `generation`. Returns false when the result is discarded.
    pub fn replace_if_current(&self, token: SmOAuth, generation: u64) -> bool {
        let mut slot = self.lock();
        if slot.generation != generation {
            return false;
        }
        slot.token = Some(token);
        true
    }

    /// Sets a new login token (after a successful code exchange) and starts a new generation.
    pub fn login(&self, token: SmOAuth) {
        let mut slot = self.lock();
        slot.token = Some(token);
        slot.generation += 1;
    }

    /// Forgets the token (logout). A refresh still in flight is discarded.
    pub fn clear(&self) {
        let mut slot = self.lock();
        slot.token = None;
        slot.generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(created: i64, expires: i64) -> SmOAuth {
        SmOAuth {
            access_token: "a".into(),
            token_type: "Bearer".into(),
            created_at: created,
            expires_in: expires,
            refresh_token: Some("r".into()),
        }
    }

    #[test]
    fn t_31a_005_expiry_is_strict_and_leaves_an_hour() {
        let t = token(1_000, 86_400);
        // The end is 1000 + 86400 = 87400, minus 3600: expired strictly after 83800.
        assert!(!is_expired(&t, 83_800));
        assert!(is_expired(&t, 83_801));
    }

    #[test]
    fn t_31a_005_a_refresh_is_discarded_after_logout() {
        let slot = TokenSlot::with_token(Some(token(0, 1)));
        let (_, generation) = slot.snapshot().unwrap();
        slot.clear();
        assert!(!slot.replace_if_current(token(5, 1), generation));
        assert!(!slot.has_token());
    }

    #[test]
    fn t_31a_005_a_refresh_is_kept_when_nothing_changed() {
        let slot = TokenSlot::with_token(Some(token(0, 1)));
        let (_, generation) = slot.snapshot().unwrap();
        assert!(slot.replace_if_current(token(5, 1), generation));
        assert_eq!(slot.snapshot().unwrap().0.created_at, 5);
    }
}
