//! Encrypted-file fallback for [`SecretStore`], used when no Secret Service is reachable.
//!
//! Two files sit in the config directory. `secrets.key` holds a random 32-byte ChaCha20-Poly1305
//! key and is created with mode 0600. `secrets.bin` holds a random 12-byte nonce followed by the
//! ciphertext of a JSON object `{service: {account: secret}}`. Writes replace `secrets.bin` through
//! a temp file and a rename, so a crash leaves the old file or the new one, never a partial file.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};

use crate::secrets::{SecretError, SecretStore};

const DATA_FILE: &str = "secrets.bin";
const KEY_FILE: &str = "secrets.key";
const NONCE_LEN: usize = 12;

/// `{service: {account: secret}}`. Sorted maps keep the plaintext stable.
type Vault = BTreeMap<String, BTreeMap<String, String>>;

/// Secrets encrypted into a file under the config directory.
#[derive(Debug)]
pub struct EncryptedFileStore {
    dir: PathBuf,
    /// Serialises read-modify-write cycles within this process.
    lock: Mutex<()>,
}

impl EncryptedFileStore {
    /// Stores secrets under `config_dir`. Nothing is created until a secret is written.
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            dir: config_dir,
            lock: Mutex::new(()),
        }
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        self.lock.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn data_path(&self) -> PathBuf {
        self.dir.join(DATA_FILE)
    }

    fn key_path(&self) -> PathBuf {
        self.dir.join(KEY_FILE)
    }

    /// Decrypts the vault with its key. `Ok(None)` when no secrets file exists yet.
    fn read_vault(&self) -> Result<Option<(Key, Vault)>, SecretError> {
        let blob = match fs::read(self.data_path()) {
            Ok(blob) => blob,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(failed(err)),
        };
        let key = self.key(false)?;
        let (nonce, ciphertext) = blob.split_at_checked(NONCE_LEN).ok_or_else(corrupt)?;
        let plaintext = ChaCha20Poly1305::new(&key)
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| {
                SecretError::Failed(
                    "secrets file cannot be decrypted; the key does not match".into(),
                )
            })?;
        let vault: Vault = serde_json::from_slice(&plaintext).map_err(|_| corrupt())?;
        Ok(Some((key, vault)))
    }

    fn write_vault(&self, key: &Key, vault: &Vault) -> Result<(), SecretError> {
        let plaintext =
            serde_json::to_vec(vault).map_err(|err| SecretError::Failed(err.to_string()))?;
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = ChaCha20Poly1305::new(key)
            .encrypt(&nonce, plaintext.as_slice())
            .map_err(|_| SecretError::Failed("secrets could not be encrypted".into()))?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ciphertext);
        write_atomic(&self.data_path(), &blob).map_err(failed)
    }

    /// The key in `secrets.key`. With `create`, a missing key file is generated first.
    fn key(&self, create: bool) -> Result<Key, SecretError> {
        match fs::read(self.key_path()) {
            Ok(bytes) => {
                let array: [u8; 32] = bytes.try_into().map_err(|_| corrupt())?;
                Ok(Key::from(array))
            }
            Err(err) if err.kind() == ErrorKind::NotFound && create => {
                create_key(&self.key_path()).map_err(failed)
            }
            Err(err) if err.kind() == ErrorKind::NotFound => Err(SecretError::Failed(
                "secrets key file is missing; stored secrets cannot be read".into(),
            )),
            Err(err) => Err(failed(err)),
        }
    }
}

impl SecretStore for EncryptedFileStore {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError> {
        let _guard = self.guard();
        Ok(self
            .read_vault()?
            .and_then(|(_, vault)| vault.get(service)?.get(account).cloned()))
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretError> {
        let _guard = self.guard();
        fs::create_dir_all(&self.dir).map_err(failed)?;
        let (key, mut vault) = match self.read_vault()? {
            Some(found) => found,
            None => (self.key(true)?, Vault::new()),
        };
        vault
            .entry(service.to_owned())
            .or_default()
            .insert(account.to_owned(), secret.to_owned());
        self.write_vault(&key, &vault)
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
        let _guard = self.guard();
        let Some((key, mut vault)) = self.read_vault()? else {
            return Ok(());
        };
        if let Some(accounts) = vault.get_mut(service) {
            accounts.remove(account);
            if accounts.is_empty() {
                vault.remove(service);
            }
        }
        self.write_vault(&key, &vault)
    }
}

/// Generates a key and writes its file atomically with mode 0600. A crash mid-write leaves no
/// partial key behind, which would otherwise make every later write fail.
fn create_key(path: &Path) -> io::Result<Key> {
    let key = ChaCha20Poly1305::generate_key(&mut OsRng);
    write_atomic(path, &key)?;
    Ok(key)
}

/// Writes `bytes` to `path` through a 0600 temp file and a rename.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    let _ = fs::remove_file(&tmp);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

fn failed(err: io::Error) -> SecretError {
    SecretError::Failed(err.to_string())
}

fn corrupt() -> SecretError {
    SecretError::Failed("secrets file is corrupt".into())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    const SECRET: &[u8] = b"plaintext-secret-7f3a";

    fn store(tmp: &tempfile::TempDir) -> EncryptedFileStore {
        EncryptedFileStore::new(tmp.path().join("config"))
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn round_trip_replace_delete_and_reopen() {
        let tmp = tempfile::tempdir().unwrap();
        let s = store(&tmp);
        assert_eq!(s.get("svc", "acct").unwrap(), None);
        s.set("svc", "acct", "first").unwrap();
        s.set("svc", "other", "second").unwrap();
        s.set("svc", "acct", "replaced").unwrap();

        let reopened = store(&tmp);
        assert_eq!(
            reopened.get("svc", "acct").unwrap().as_deref(),
            Some("replaced")
        );
        assert_eq!(
            reopened.get("svc", "other").unwrap().as_deref(),
            Some("second")
        );

        s.delete("svc", "acct").unwrap();
        assert_eq!(s.get("svc", "acct").unwrap(), None);
        assert_eq!(s.get("svc", "other").unwrap().as_deref(), Some("second"));
        s.delete("svc", "acct").unwrap();
        s.delete("never", "existed").unwrap();
    }

    #[test]
    fn wrong_key_fails_and_leaves_the_file_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        let s = store(&tmp);
        s.set("svc", "acct", "value").unwrap();
        let before = fs::read(config.join(DATA_FILE)).unwrap();

        fs::write(config.join(KEY_FILE), [7u8; 32]).unwrap();
        assert!(matches!(s.get("svc", "acct"), Err(SecretError::Failed(_))));
        assert!(matches!(
            s.set("svc", "new", "x"),
            Err(SecretError::Failed(_))
        ));
        assert_eq!(fs::read(config.join(DATA_FILE)).unwrap(), before);
    }

    #[test]
    fn missing_key_with_existing_secrets_is_an_error_not_a_reset() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        let s = store(&tmp);
        s.set("svc", "acct", "value").unwrap();
        fs::remove_file(config.join(KEY_FILE)).unwrap();

        assert!(matches!(s.get("svc", "acct"), Err(SecretError::Failed(_))));
        assert!(matches!(
            s.set("svc", "new", "x"),
            Err(SecretError::Failed(_))
        ));
        assert!(!config.join(KEY_FILE).exists());
    }

    #[test]
    fn key_and_secrets_files_are_mode_0600() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        let s = store(&tmp);
        s.set("svc", "acct", "first").unwrap();
        s.set("svc", "acct", "second").unwrap();
        for name in [KEY_FILE, DATA_FILE] {
            let mode = fs::metadata(config.join(name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{name}");
        }
        assert!(!config.join("secrets.tmp").exists());
    }

    #[test]
    fn leftover_temp_file_from_a_crash_does_not_block_writes() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("secrets.tmp"), b"partial").unwrap();
        let s = store(&tmp);
        s.set("svc", "acct", "value").unwrap();
        assert_eq!(s.get("svc", "acct").unwrap().as_deref(), Some("value"));
        assert!(!config.join("secrets.tmp").exists());
    }

    #[test]
    fn no_plaintext_secret_or_account_name_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        let s = store(&tmp);
        s.set(
            "tracker",
            "account-name",
            std::str::from_utf8(SECRET).unwrap(),
        )
        .unwrap();

        let files: Vec<PathBuf> = fs::read_dir(&config)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(files.len(), 2);
        for path in files {
            let bytes = fs::read(&path).unwrap();
            assert!(!contains(&bytes, SECRET), "{}", path.display());
            assert!(!contains(&bytes, b"account-name"), "{}", path.display());
        }
    }
}
