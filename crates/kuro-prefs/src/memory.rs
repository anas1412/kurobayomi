//! In-memory backend for tests and previews. Clones share one state, so a test can keep a clone
//! and count the saves the store makes.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

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
