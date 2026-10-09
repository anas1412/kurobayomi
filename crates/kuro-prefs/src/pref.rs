//! Typed handle for one preference key.

use std::borrow::Cow;
use std::sync::mpsc::Receiver;

use crate::store::{Changed, PrefStore};
use crate::values::{Decoded, PrefType};
use crate::PrefError;

/// One key of a store, read and written as `T`. Clones are cheap.
#[derive(Clone)]
pub struct Pref<T: PrefType> {
    store: PrefStore,
    key: Cow<'static, str>,
    default: T,
}

/// A subscription to one [`Pref`]. Changes queue up until [`Watch::latest`] drains them.
pub struct Watch<T: PrefType> {
    pref: Pref<T>,
    events: Receiver<Changed>,
}

impl<T: PrefType> Watch<T> {
    /// The current value. Every queued change is drained first, so many writes cost one read.
    pub fn latest(&self) -> T {
        while self.events.try_recv().is_ok() {}
        self.pref.get()
    }

    /// Like [`Watch::changed`], but never blocks: `Some(value)` when this key changed since the last call.
    /// Use it from a main-loop timer or idle callback.
    pub fn try_changed(&self) -> Option<T> {
        let mut changed = false;
        while let Ok(Changed { key }) = self.events.try_recv() {
            changed |= key.as_deref().is_none_or(|k| k == self.pref.key());
        }
        changed.then(|| self.pref.get())
    }

    /// Blocks until this key changes (or every key does), then returns the new value. `None` once the store is gone.
    /// Changes to other keys are skipped.
    pub fn changed(&self) -> Option<T> {
        loop {
            match self.events.recv() {
                Ok(Changed { key }) if key.as_deref().is_none_or(|k| k == self.pref.key()) => {
                    return Some(self.pref.get());
                }
                Ok(_) => {}
                Err(_) => return None,
            }
        }
    }
}

impl<T: PrefType> Pref<T> {
    pub fn new(store: PrefStore, key: impl Into<Cow<'static, str>>, default: T) -> Self {
        Self {
            store,
            key: key.into(),
            default,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn default_value(&self) -> T {
        self.default.clone()
    }

    pub fn is_set(&self) -> bool {
        self.store.get_raw(&self.key).is_some()
    }

    /// The stored value, or the default. A value of the wrong kind is deleted and the default
    /// returned. An invalid value (an unknown enum name) gives the default and stays stored.
    pub fn get(&self) -> T {
        let Some(raw) = self.store.get_raw(&self.key) else {
            return self.default.clone();
        };
        match T::decode(&raw) {
            Decoded::Ok(v) => v,
            Decoded::Invalid => self.default.clone(),
            Decoded::WrongType => {
                tracing::warn!("Invalid value for {}; deleting", self.key);
                self.store.remove(&self.key);
                self.default.clone()
            }
        }
    }

    /// Stores `v` in memory and queues the save. `Err` when the store's last save failed: the value
    /// is kept and retried, but is not on disk yet.
    pub fn set(&self, v: T) -> Result<(), PrefError> {
        self.store.set_raw(&self.key, v.encode());
        self.store.status()
    }

    /// Startup repair: a stored value that does not decode (an unknown enum name, a sort without a
    /// direction) is replaced by the default and logged once. Wrong-kind values are left to [`Pref::get`].
    pub fn repair(&self) -> Result<(), PrefError> {
        let Some(raw) = self.store.get_raw(&self.key) else {
            return Ok(());
        };
        if matches!(T::decode(&raw), Decoded::Invalid) {
            tracing::warn!("Invalid value for {}; resetting to default", self.key);
            return self.set(self.default.clone());
        }
        Ok(())
    }

    /// Removes the key. `Err` as for [`Pref::set`].
    pub fn delete(&self) -> Result<(), PrefError> {
        self.store.remove(&self.key);
        self.store.status()
    }

    /// A subscription to this key. Its first [`Watch::latest`] returns the current value.
    pub fn watch(&self) -> Watch<T> {
        Watch {
            pref: self.clone(),
            events: self.store.subscribe(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::pref_enum;
    use crate::values::PrefValue;

    pref_enum! {
        pub enum ProxyMode { None, Peers }
    }

    #[test]
    fn default_is_returned_until_a_value_is_set() {
        let store = PrefStore::in_memory();
        let p = store.pref("n", 7i32);
        assert_eq!(p.get(), 7);
        assert!(!p.is_set());
        p.set(9).unwrap();
        assert!(p.is_set());
        assert_eq!(p.get(), 9);
        p.delete().unwrap();
        assert_eq!(p.get(), 7);
        assert_eq!(p.key(), "n");
        assert_eq!(p.default_value(), 7);
    }

    #[test]
    fn wrong_kind_is_deleted_and_default_returned() {
        let store = PrefStore::in_memory();
        store.set_raw("k", PrefValue::Str("abc".into()));
        let p = store.pref("k", 5i64);
        assert_eq!(p.get(), 5);
        assert!(!p.is_set());
    }

    #[test]
    fn undecodable_enum_returns_default_and_keeps_the_value() {
        let store = PrefStore::in_memory();
        store.set_raw("mode", PrefValue::Str("Peers".into()));
        let p = store.pref("mode", ProxyMode::None);
        assert_eq!(p.get(), ProxyMode::Peers);
        store.set_raw("mode", PrefValue::Str("Nope".into()));
        assert_eq!(p.get(), ProxyMode::None);
        assert_eq!(ProxyMode::ALL.len(), 2);
        assert!(p.is_set());
    }

    #[test]
    fn watch_reads_the_current_value_and_conflates_many_writes() {
        let store = PrefStore::in_memory();
        let p = store.pref("n", 7i32);
        let w = p.watch();
        assert_eq!(w.latest(), 7);
        for i in 0..1000 {
            p.set(i).unwrap();
        }
        assert_eq!(w.latest(), 999);
        assert!(w.events.try_recv().is_err());
    }

    #[test]
    fn set_stores_the_encoded_value_for_sets() {
        let store = PrefStore::in_memory();
        let p = store.pref("s", BTreeSet::<String>::new());
        p.set(BTreeSet::from(["a".to_owned()])).unwrap();
        assert_eq!(
            store.get_raw("s"),
            Some(PrefValue::Set(BTreeSet::from(["a".to_owned()])))
        );
    }

    #[test]
    fn try_changed_reports_this_keys_changes_without_blocking() {
        let store = PrefStore::in_memory();
        let p = store.pref("n", 7i32);
        let other = store.pref("other", 0i32);
        let w = p.watch();
        assert_eq!(w.try_changed(), None);
        other.set(1).unwrap();
        assert_eq!(w.try_changed(), None);
        p.set(9).unwrap();
        p.set(10).unwrap();
        assert_eq!(w.try_changed(), Some(10));
        assert_eq!(w.try_changed(), None);
        store.replace_many(vec![("n".to_owned(), PrefValue::Int(11))]);
        assert_eq!(w.try_changed(), Some(11));
    }

    #[test]
    fn repair_resets_an_invalid_value_and_leaves_a_wrong_kind_alone() {
        let store = PrefStore::in_memory();
        store.set_raw("mode", PrefValue::Str("Nope".into()));
        store.pref("mode", ProxyMode::None).repair().unwrap();
        assert_eq!(store.get_raw("mode"), Some(PrefValue::Str("None".into())));
        store.set_raw("n", PrefValue::Str("abc".into()));
        store.pref("n", 5i32).repair().unwrap();
        assert_eq!(store.get_raw("n"), Some(PrefValue::Str("abc".into())));
    }
}
