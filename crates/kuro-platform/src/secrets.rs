//! Credential storage. Tracker tokens and source credentials go through this trait only.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::paths::Paths;
use crate::secrets_file::EncryptedFileStore;

/// Failure from a [`SecretStore`] call. Callers match `Unavailable` to fall back to another store.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    /// No keyring service is reachable (no Secret Service daemon, no D-Bus session).
    #[error("secret store unavailable: {0}")]
    Unavailable(String),
    /// The store is reachable but the operation failed or the stored data is unusable.
    #[error("secret store operation failed: {0}")]
    Failed(String),
}

/// Stores secrets keyed by service and account name. Stores are shared across threads (tracker
/// work runs on tokio), so every implementation is `Send + Sync`.
pub trait SecretStore: Send + Sync {
    /// Returns the secret, or `Ok(None)` when no entry exists.
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError>;
    /// Creates or replaces the secret.
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretError>;
    /// Removes the secret. Deleting a missing entry is `Ok(())`.
    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError>;
}

/// Account probed when [`FallbackSecrets::open`] chooses a backend. Only read, never written.
const PROBE: (&str, &str) = ("kurobayomi", "probe");

/// How long the probe may take. An activatable Secret Service that is not running can block for
/// minutes on D-Bus activation; past this limit the primary counts as unavailable.
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Set by the first [`FallbackSecrets::notice_once`] that reports fallback mode.
static FALLBACK_NOTICE: AtomicBool = AtomicBool::new(false);

/// Which backend a [`FallbackSecrets`] uses for the whole process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretsMode {
    /// The primary store (the system keyring) is reachable.
    Primary,
    /// The primary store reported `Unavailable`; secrets live in the encrypted file.
    EncryptedFile,
}

/// The app's secret store: the system keyring when reachable, otherwise the encrypted file in the config directory.
/// A build without a keyring uses the file.
#[allow(unreachable_code)]
pub fn open(paths: &dyn Paths) -> FallbackSecrets {
    let file = Box::new(EncryptedFileStore::new(paths.config_dir()));
    #[cfg(feature = "keyring")]
    return FallbackSecrets::open(Box::new(crate::keyring_store::KeyringStore), file);
    FallbackSecrets { mode: SecretsMode::EncryptedFile, store: file }
}

/// Uses the primary store when it is reachable and the fallback otherwise.
///
/// The backend is chosen once in [`FallbackSecrets::open`] and kept for the process, so a secret
/// written to one backend is never looked up in the other.
pub struct FallbackSecrets {
    mode: SecretsMode,
    store: Box<dyn SecretStore>,
}

impl FallbackSecrets {
    /// Probes `primary` once, waiting at most [`PROBE_TIMEOUT`]. Only `Unavailable` (or no answer
    /// in time) selects `fallback`; a reachable store that fails the probe stays in use so its
    /// errors reach the caller.
    pub fn open(primary: Box<dyn SecretStore>, fallback: Box<dyn SecretStore>) -> Self {
        Self::open_within(primary, fallback, PROBE_TIMEOUT)
    }

    fn open_within(primary: Box<dyn SecretStore>, fallback: Box<dyn SecretStore>, limit: Duration) -> Self {
        let (service, account) = PROBE;
        let (tx, rx) = mpsc::channel();
        // The probe runs on its own thread so a stalled store cannot block startup. When it misses
        // the limit the thread is left to finish; its result is dropped with the receiver.
        std::thread::spawn(move || {
            let probe = primary.get(service, account);
            let _ = tx.send((primary, probe));
        });
        match rx.recv_timeout(limit) {
            Ok((primary, Ok(_) | Err(SecretError::Failed(_)))) => Self {
                mode: SecretsMode::Primary,
                store: primary,
            },
            Ok((_, Err(SecretError::Unavailable(_)))) | Err(_) => Self {
                mode: SecretsMode::EncryptedFile,
                store: fallback,
            },
        }
    }

    /// The backend in use.
    pub fn mode(&self) -> SecretsMode {
        self.mode
    }

    /// True exactly once per process, and only in fallback mode. The GUI shows its one-time notice
    /// when this returns true. In primary mode it always returns false.
    pub fn notice_once(&self) -> bool {
        self.mode == SecretsMode::EncryptedFile && !FALLBACK_NOTICE.swap(true, Ordering::AcqRel)
    }
}

impl SecretStore for FallbackSecrets {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError> {
        self.store.get(service, account)
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretError> {
        self.store.set(service, account, secret)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
        self.store.delete(service, account)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use super::*;

    /// A reachable in-memory store.
    #[derive(Default)]
    struct Memory(Mutex<BTreeMap<(String, String), String>>);

    impl SecretStore for Memory {
        fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(&(service.to_owned(), account.to_owned()))
                .cloned())
        }

        fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretError> {
            self.0
                .lock()
                .unwrap()
                .insert((service.to_owned(), account.to_owned()), secret.to_owned());
            Ok(())
        }

        fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
            self.0
                .lock()
                .unwrap()
                .remove(&(service.to_owned(), account.to_owned()));
            Ok(())
        }
    }

    /// A store whose every call fails with the given error.
    struct Failing(SecretError);

    impl SecretStore for Failing {
        fn get(&self, _: &str, _: &str) -> Result<Option<String>, SecretError> {
            Err(self.0.clone())
        }

        fn set(&self, _: &str, _: &str, _: &str) -> Result<(), SecretError> {
            Err(self.0.clone())
        }

        fn delete(&self, _: &str, _: &str) -> Result<(), SecretError> {
            Err(self.0.clone())
        }
    }

    /// A store whose probe answers only after `delay`, like an activatable service that is slow to start.
    struct Stalled(Duration);

    impl SecretStore for Stalled {
        fn get(&self, _: &str, _: &str) -> Result<Option<String>, SecretError> {
            std::thread::sleep(self.0);
            Ok(None)
        }

        fn set(&self, _: &str, _: &str, _: &str) -> Result<(), SecretError> {
            Ok(())
        }

        fn delete(&self, _: &str, _: &str) -> Result<(), SecretError> {
            Ok(())
        }
    }

    fn unavailable() -> SecretError {
        SecretError::Unavailable("no session bus".into())
    }

    #[test]
    fn stalled_probe_selects_the_fallback_within_the_limit() {
        let started = std::time::Instant::now();
        let secrets = FallbackSecrets::open_within(
            Box::new(Stalled(Duration::from_secs(3))),
            Box::new(Memory::default()),
            Duration::from_millis(100),
        );
        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
        assert_eq!(secrets.mode(), SecretsMode::EncryptedFile);
        secrets.set("svc", "acct", "token").unwrap();
        assert_eq!(secrets.get("svc", "acct").unwrap().as_deref(), Some("token"));
    }

    #[test]
    fn stores_can_be_shared_across_threads() {
        fn assert_send_sync<T: Send + Sync + ?Sized>() {}
        assert_send_sync::<dyn SecretStore>();
        assert_send_sync::<std::sync::Arc<dyn SecretStore>>();
        assert_send_sync::<FallbackSecrets>();
    }

    #[test]
    fn answering_probe_keeps_the_primary() {
        let secrets = FallbackSecrets::open_within(
            Box::new(Stalled(Duration::ZERO)),
            Box::new(Failing(unavailable())),
            Duration::from_secs(5),
        );
        assert_eq!(secrets.mode(), SecretsMode::Primary);
    }

    #[test]
    fn unreachable_primary_selects_the_fallback() {
        let secrets = FallbackSecrets::open(
            Box::new(Failing(unavailable())),
            Box::new(Memory::default()),
        );
        assert_eq!(secrets.mode(), SecretsMode::EncryptedFile);
        secrets.set("svc", "acct", "token").unwrap();
        assert_eq!(
            secrets.get("svc", "acct").unwrap().as_deref(),
            Some("token")
        );
    }

    #[test]
    fn reachable_or_failing_primary_is_kept_and_never_notices() {
        let reachable = FallbackSecrets::open(
            Box::new(Memory::default()),
            Box::new(Failing(unavailable())),
        );
        assert_eq!(reachable.mode(), SecretsMode::Primary);
        reachable.set("svc", "acct", "token").unwrap();
        assert_eq!(
            reachable.get("svc", "acct").unwrap().as_deref(),
            Some("token")
        );
        assert!(!reachable.notice_once());

        let locked = FallbackSecrets::open(
            Box::new(Failing(SecretError::Failed("collection locked".into()))),
            Box::new(Memory::default()),
        );
        assert_eq!(locked.mode(), SecretsMode::Primary);
        assert!(locked.get("svc", "acct").is_err());
    }

    #[test]
    fn fallback_notice_is_true_once_per_process() {
        let first = FallbackSecrets::open(
            Box::new(Failing(unavailable())),
            Box::new(Memory::default()),
        );
        assert!(first.notice_once());
        assert!(!first.notice_once());

        let second = FallbackSecrets::open(
            Box::new(Failing(unavailable())),
            Box::new(Memory::default()),
        );
        assert!(!second.notice_once());
    }

    #[test]
    fn unavailable_and_failed_are_distinct_and_display_their_reason() {
        let unavailable = SecretError::Unavailable("no bus".into());
        let failed = SecretError::Failed("no bus".into());
        assert_ne!(unavailable, failed);
        assert_eq!(unavailable.to_string(), "secret store unavailable: no bus");
        assert_eq!(failed.to_string(), "secret store operation failed: no bus");
    }
}
