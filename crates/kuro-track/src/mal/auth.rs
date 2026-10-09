//! Token state, refresh with single flight, and the auth-expired flag (`p1-behavior.md` sections 4.7 to 4.10,
//! `p1-rust-design.md` section 3.5). The state lock is a `std` mutex that is never held across an await. The
//! refresh gate is a `tokio` mutex, so only one refresh is in flight.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use reqwest::StatusCode;
use url::Url;

use super::dates::now_secs;
use super::error::MalError;
use super::store::{MalSettings, TokenStore};
use super::token::MalToken;

#[derive(Default)]
struct AuthState {
    token: Option<MalToken>,
    /// Bumped by logout. A refresh that started before a logout does not store its result.
    epoch: u64,
}

pub struct MalAuth {
    tokens: Arc<dyn TokenStore>,
    settings: Arc<MalSettings>,
    state: Mutex<AuthState>,
    refresh_gate: tokio::sync::Mutex<()>,
    refresh_url: Url,
    client_id: String,
}

impl MalAuth {
    /// Loads the saved token. A corrupt token is treated as logged out by the store.
    pub fn new(
        tokens: Arc<dyn TokenStore>,
        settings: Arc<MalSettings>,
        refresh_url: Url,
        client_id: String,
    ) -> Result<Self, MalError> {
        let token = tokens.load()?;
        Ok(Self {
            tokens,
            settings,
            state: Mutex::new(AuthState { token, epoch: 0 }),
            refresh_gate: tokio::sync::Mutex::new(()),
            refresh_url,
            client_id,
        })
    }

    pub fn has_token(&self) -> bool {
        self.lock().token.is_some()
    }

    pub fn token(&self) -> Option<MalToken> {
        self.lock().token.clone()
    }

    /// The persisted auth-expired flag (`TrackPreferences.kt:24-27`).
    pub fn expired(&self) -> bool {
        self.settings.auth_expired()
    }

    pub fn set_expired(&self, value: bool) -> Result<(), MalError> {
        self.settings.set_auth_expired(value)
    }

    /// Stores a token (`setAuth`). The token is written to the store before memory changes.
    pub fn set_token(&self, token: MalToken) -> Result<(), MalError> {
        let mut state = self.lock();
        self.tokens.save(&token)?;
        state.token = Some(token);
        Ok(())
    }

    /// Removes the token from memory and from the store, and invalidates any refresh in flight.
    pub fn clear_token(&self) -> Result<(), MalError> {
        let mut state = self.lock();
        state.epoch += 1;
        state.token = None;
        self.tokens.clear()
    }

    /// The Bearer value for one request, refreshing first when the token is past its margin.
    pub async fn bearer(&self, http: &reqwest::Client) -> Result<String, MalError> {
        if self.expired() {
            return Err(MalError::LoginExpired);
        }
        let token = self.token().ok_or(MalError::NotAuthenticated)?;
        if !token.is_expired(now_secs()) {
            return Ok(token.access_token);
        }

        let _gate = self.refresh_gate.lock().await;
        // Re-read under the gate: a caller that waited may find a fresh token already stored.
        let (token, epoch) = {
            let state = self.lock();
            (
                state.token.clone().ok_or(MalError::NotAuthenticated)?,
                state.epoch,
            )
        };
        if !token.is_expired(now_secs()) {
            return Ok(token.access_token);
        }
        if self.expired() {
            return Err(MalError::LoginExpired);
        }
        self.refresh(http, &token, epoch).await
    }

    async fn refresh(
        &self,
        http: &reqwest::Client,
        old: &MalToken,
        epoch: u64,
    ) -> Result<String, MalError> {
        let response = http
            .post(self.refresh_url.clone())
            .bearer_auth(&old.access_token)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("refresh_token", old.refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "MAL token refresh did not complete");
                MalError::RefreshFailed
            })?;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED {
            self.set_expired(true)?;
            return Err(MalError::LoginExpired);
        }
        if !status.is_success() {
            tracing::warn!(status = status.as_u16(), "MAL token refresh was refused");
            return Err(MalError::RefreshFailed);
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| MalError::RefreshFailed)?;
        let fresh: MalToken =
            serde_json::from_slice(&bytes).map_err(|_| MalError::RefreshFailed)?;

        let mut state = self.lock();
        if state.epoch != epoch {
            // Logout ran while the refresh was pending: the result must not bring the token back.
            return Err(MalError::NotAuthenticated);
        }
        self.tokens.save(&fresh)?;
        let access = fresh.access_token.clone();
        state.token = Some(fresh);
        Ok(access)
    }

    fn lock(&self) -> MutexGuard<'_, AuthState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
