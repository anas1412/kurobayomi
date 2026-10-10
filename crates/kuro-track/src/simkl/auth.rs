//! The Simkl access token, held in memory for the authenticated calls (`SimklInterceptor.kt`). It is restored from the
//! secret store at start (see `SimklTracker::with_base_url`) and written by a login. A std lock, never held across an
//! await, so logout works from any thread.

use std::sync::{PoisonError, RwLock};

use super::dto::SimklOAuth;
use super::error::SimklError;

#[derive(Debug, Default)]
pub struct SimklAuth {
    token: RwLock<Option<SimklOAuth>>,
}

impl SimklAuth {
    pub fn new(token: Option<SimklOAuth>) -> Self {
        Self {
            token: RwLock::new(token),
        }
    }

    pub fn set(&self, token: Option<SimklOAuth>) {
        *self.token.write().unwrap_or_else(PoisonError::into_inner) = token;
    }

    /// The access token for `Authorization: Bearer`. `NotAuthenticated` before any request when there is none.
    pub fn bearer(&self) -> Result<String, SimklError> {
        let guard = self.token.read().unwrap_or_else(PoisonError::into_inner);
        (*guard)
            .as_ref()
            .map(|t| t.access_token.clone())
            .ok_or(SimklError::NotAuthenticated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_32a_035_no_token_means_not_authenticated_and_set_replaces_it() {
        let auth = SimklAuth::new(None);
        assert_eq!(auth.bearer(), Err(SimklError::NotAuthenticated));
        auth.set(Some(SimklOAuth {
            access_token: "A".into(),
        }));
        assert_eq!(auth.bearer().unwrap(), "A");
        auth.set(None);
        assert_eq!(auth.bearer(), Err(SimklError::NotAuthenticated));
    }
}
