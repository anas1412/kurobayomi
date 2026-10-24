//! The system keyring through the `keyring` crate: the Secret Service (libsecret-compatible) on Linux, the
//! Credential Manager on Windows.

use keyring::{Entry, Error as KeyringError, Result as KeyringResult};

use crate::secrets::{SecretError, SecretStore};

/// System keyring backend. Each call opens an entry; the keyring crate keeps the connection.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError> {
        let entry = Entry::new(service, account)?;
        read_password(entry.get_password())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretError> {
        let entry = Entry::new(service, account)?;
        Ok(entry.set_password(secret)?)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
        let entry = Entry::new(service, account)?;
        remove_result(entry.delete_credential())
    }
}

impl From<KeyringError> for SecretError {
    fn from(err: KeyringError) -> Self {
        match err {
            KeyringError::NoStorageAccess(_) | KeyringError::NoDefaultStore => {
                SecretError::Unavailable(err.to_string())
            }
            other => SecretError::Failed(other.to_string()),
        }
    }
}

/// Maps a read result: a missing entry is `Ok(None)`, not an error.
fn read_password(result: KeyringResult<String>) -> Result<Option<String>, SecretError> {
    match result {
        Ok(secret) => Ok(Some(secret)),
        Err(KeyringError::NoEntry) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Maps a delete result: deleting a missing entry succeeds.
fn remove_result(result: KeyringResult<()>) -> Result<(), SecretError> {
    match result {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    fn unavailable() -> KeyringError {
        KeyringError::NoStorageAccess(Box::new(io::Error::other("no session bus")))
    }

    #[test]
    fn no_storage_access_maps_to_unavailable() {
        assert!(matches!(
            SecretError::from(unavailable()),
            SecretError::Unavailable(_)
        ));
        assert!(matches!(
            SecretError::from(KeyringError::NoDefaultStore),
            SecretError::Unavailable(_)
        ));
    }

    #[test]
    fn other_errors_map_to_failed() {
        let platform = KeyringError::PlatformFailure(Box::new(io::Error::other("boom")));
        assert!(matches!(
            SecretError::from(platform),
            SecretError::Failed(_)
        ));
        assert!(matches!(
            SecretError::from(KeyringError::BadEncoding(vec![0xff])),
            SecretError::Failed(_)
        ));
    }

    #[test]
    fn missing_entry_reads_as_none_and_deletes_cleanly() {
        assert_eq!(read_password(Err(KeyringError::NoEntry)), Ok(None));
        assert_eq!(read_password(Ok("token".into())), Ok(Some("token".into())));
        assert_eq!(remove_result(Err(KeyringError::NoEntry)), Ok(()));
    }

    #[test]
    fn unavailable_store_is_reported_on_read() {
        assert!(matches!(
            read_password(Err(unavailable())),
            Err(SecretError::Unavailable(_))
        ));
    }
}
