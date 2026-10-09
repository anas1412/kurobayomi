//! Read-modify-write helpers on [`Pref`]. Each is a get then a set, not atomic across threads.

use std::collections::BTreeSet;

use crate::PrefError;
use crate::pref::Pref;
use crate::values::PrefType;

impl<T: PrefType> Pref<T> {
    /// Sets the value to `f` applied to the current one (Kotlin `getAndSet`).
    pub fn update(&self, f: impl FnOnce(T) -> T) -> Result<(), PrefError> {
        self.set(f(self.get()))
    }

    /// Deletes the key and returns the default (Kotlin `deleteAndGet`).
    pub fn delete_and_get(&self) -> Result<T, PrefError> {
        self.delete()?;
        Ok(self.get())
    }
}

impl Pref<bool> {
    /// Flips the value and returns the new one (Kotlin `toggle`).
    pub fn toggle(&self) -> Result<bool, PrefError> {
        self.set(!self.get())?;
        Ok(self.get())
    }
}

impl Pref<BTreeSet<String>> {
    /// Adds `item` (Kotlin `+=`).
    pub fn insert(&self, item: impl Into<String>) -> Result<(), PrefError> {
        let mut set = self.get();
        set.insert(item.into());
        self.set(set)
    }

    /// Adds every item (Kotlin `+= items`).
    pub fn extend(&self, items: impl IntoIterator<Item = String>) -> Result<(), PrefError> {
        let mut set = self.get();
        set.extend(items);
        self.set(set)
    }

    /// Removes `item` (Kotlin `-=`).
    pub fn remove(&self, item: &str) -> Result<(), PrefError> {
        let mut set = self.get();
        set.remove(item);
        self.set(set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PrefStore;

    #[test]
    fn update_applies_the_function_to_the_current_value() {
        let store = PrefStore::in_memory();
        let p = store.pref("n", 1i32);
        p.update(|v| v + 10).unwrap();
        assert_eq!(p.get(), 11);
    }

    #[test]
    fn delete_and_get_resets_to_the_default() {
        let store = PrefStore::in_memory();
        let p = store.pref("n", 1i32);
        p.set(5).unwrap();
        assert_eq!(p.delete_and_get().unwrap(), 1);
        assert!(!p.is_set());
    }

    #[test]
    fn toggle_flips_and_returns_the_new_value() {
        let store = PrefStore::in_memory();
        let p = store.pref("b", false);
        assert!(p.toggle().unwrap());
        assert!(!p.toggle().unwrap());
        assert!(!p.get());
    }

    #[test]
    fn insert_extend_and_remove_work_on_sets() {
        let store = PrefStore::in_memory();
        let p = store.pref("s", BTreeSet::<String>::new());
        p.insert("a").unwrap();
        p.extend(["b".to_owned(), "c".to_owned()]).unwrap();
        p.remove("a").unwrap();
        assert_eq!(p.get(), BTreeSet::from(["b".to_owned(), "c".to_owned()]));
    }

    #[test]
    fn an_equal_set_publishes_no_change() {
        let store = PrefStore::in_memory();
        let p = store.pref("s", BTreeSet::<String>::new());
        p.insert("a").unwrap();
        let rx = store.subscribe();
        p.insert("a").unwrap();
        assert!(rx.try_recv().is_err());
    }
}
