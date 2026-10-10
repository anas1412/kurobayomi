//! The MangaUpdates session token, held in memory for the authenticated calls (`MangaUpdatesInterceptor.kt`). The
//! token is read from the secret store at start and written by login. It is behind a std lock that is never held
//! across an await, so `logout` clears it from any thread (fix D-MU-1).

use std::sync::{PoisonError, RwLock};

use super::error::MuError;

#[derive(Debug, Default)]
pub struct MuAuth {
    token: RwLock<Option<String>>,
}

impl MuAuth {
    pub fn new(token: Option<String>) -> Self {
        let auth = Self::default();
        auth.set(token);
        auth
    }

    /// Replaces the token. A blank token is no token (`ifBlank { null }`, B-2.6).
    pub fn set(&self, token: Option<String>) {
        let token = token.filter(|t| !t.trim().is_empty());
        *self.token.write().unwrap_or_else(PoisonError::into_inner) = token;
    }

    /// The bearer token, or `NotAuthenticated` before any request is made.
    pub fn bearer(&self) -> Result<String, MuError> {
        let guard = self.token.read().unwrap_or_else(PoisonError::into_inner);
        (*guard).clone().ok_or(MuError::NotAuthenticated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_32a_011_a_set_token_is_the_bearer_and_clearing_it_refuses() {
        let auth = MuAuth::new(None);
        assert_eq!(auth.bearer(), Err(MuError::NotAuthenticated));
        auth.set(Some("T".into()));
        assert_eq!(auth.bearer().unwrap(), "T");
        auth.set(None);
        assert_eq!(auth.bearer(), Err(MuError::NotAuthenticated));
    }

    #[test]
    fn t_32a_014_a_blank_restored_token_is_no_token() {
        assert_eq!(
            MuAuth::new(Some("  ".into())).bearer(),
            Err(MuError::NotAuthenticated)
        );
    }
}
