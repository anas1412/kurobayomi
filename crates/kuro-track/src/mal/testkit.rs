//! Test helpers: an in-memory secret store and a MAL tracker pointed at a local wiremock server.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use kuro_platform::secrets::{SecretError, SecretStore};
use wiremock::MockServer;

use super::dates::now_secs;
use super::store::{SecretTokenStore, TOKEN_ACCOUNT, TOKEN_SERVICE, TokenStore};
use super::token::MalToken;
use super::tracker::MalTracker;

/// A reachable secret store that keeps everything in a map.
#[derive(Default)]
pub struct MemorySecrets(pub Mutex<HashMap<(String, String), String>>);

impl MemorySecrets {
    pub fn token_text(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&(TOKEN_SERVICE.to_owned(), TOKEN_ACCOUNT.to_owned()))
            .cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty()
    }
}

impl SecretStore for MemorySecrets {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&(service.to_owned(), account.to_owned()))
            .cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert((service.to_owned(), account.to_owned()), secret.to_owned());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

/// A tracker whose endpoints are the mock server, with the secrets map shared with the test.
pub fn tracker_for(
    server: &MockServer,
    config_dir: &Path,
    secrets: Arc<MemorySecrets>,
) -> MalTracker {
    let tokens: Arc<dyn TokenStore> = Arc::new(SecretTokenStore::new(secrets));
    MalTracker::with_endpoints(
        config_dir,
        tokens,
        Some("test-client".to_owned()),
        &format!("{}/v1/oauth2", server.uri()),
        &format!("{}/v2", server.uri()),
    )
    .expect("tracker builds")
}

/// A token valid for a day, created now.
pub fn fresh_token(access: &str) -> MalToken {
    MalToken {
        token_type: "Bearer".into(),
        refresh_token: format!("{access}-refresh"),
        access_token: access.into(),
        expires_in: 2_592_000,
        created_at: now_secs(),
    }
}

/// A token that expired long ago.
pub fn expired_token(access: &str) -> MalToken {
    MalToken {
        created_at: 0,
        expires_in: 100,
        ..fresh_token(access)
    }
}
