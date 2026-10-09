//! In-memory backend for tests and previews. Clones share one state, so a test can keep a clone
//! and count the saves the store makes.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kuro_platform::secrets::{SecretError, SecretStore};

use crate::PrefError;
use crate::lock;
use crate::store::Backend;
use crate::values::PrefValue;

#[derive(Clone, Default)]
pub struct MemoryBackend {
    state: Arc<Mutex<MemState>>,
}

#[derive(Default)]
struct MemState {
    data: BTreeMap<String, PrefValue>,
    saves: usize,
}

impl MemoryBackend {
    /// How many times the store has saved through this backend.
    pub fn saves(&self) -> usize {
        lock(&self.state).saves
    }

    /// The values from the most recent save.
    pub fn saved(&self) -> BTreeMap<String, PrefValue> {
        lock(&self.state).data.clone()
    }
}

impl Backend for MemoryBackend {
    fn flushes(&self) -> bool {
        false
    }

    fn load(&self) -> Result<BTreeMap<String, PrefValue>, PrefError> {
        Ok(lock(&self.state).data.clone())
    }

    fn save(&self, all: &BTreeMap<String, PrefValue>) -> Result<(), PrefError> {
        let mut state = lock(&self.state);
        state.data = all.clone();
        state.saves += 1;
        Ok(())
    }
}

/// An in-memory [`SecretStore`] for tests and previews: credentials stay in the process.
#[derive(Default)]
pub struct MemorySecrets(Mutex<BTreeMap<(String, String), String>>);

impl SecretStore for MemorySecrets {
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecretError> {
        let map = lock_map(&self.0);
        Ok(map.get(&(service.to_owned(), account.to_owned())).cloned())
    }

    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecretError> {
        lock_map(&self.0).insert((service.to_owned(), account.to_owned()), secret.to_owned());
        Ok(())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecretError> {
        lock_map(&self.0).remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

fn lock_map(
    m: &Mutex<BTreeMap<(String, String), String>>,
) -> std::sync::MutexGuard<'_, BTreeMap<(String, String), String>> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
