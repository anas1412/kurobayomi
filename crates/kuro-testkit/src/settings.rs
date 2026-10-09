//! In-memory settings for tests and previews. It is a `kuro-prefs` store over `MemoryBackend`, so
//! it keeps sets, publishes change events and writes nothing to disk. It replaces Kotlin's
//! `InMemoryPreferenceStore`, whose `changes()` never emitted and whose `getStringSet` was not
//! implemented.

use std::ops::Deref;

use kuro_prefs::store::PrefStore;

/// Values in memory with change notification. It derefs to [`PrefStore`] for the whole API.
#[derive(Clone)]
pub struct InMemorySettings(PrefStore);

impl InMemorySettings {
    pub fn new() -> Self {
        Self(PrefStore::in_memory())
    }
}

impl Default for InMemorySettings {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for InMemorySettings {
    type Target = PrefStore;

    fn deref(&self) -> &PrefStore {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use kuro_prefs::store::Changed;
    use kuro_prefs::values::PrefValue;

    use super::*;

    #[test]
    fn strings_and_string_sets_round_trip() {
        let s = InMemorySettings::new();
        let tags: BTreeSet<String> = ["a", "b"].into_iter().map(str::to_owned).collect();
        s.set_raw("name", PrefValue::Str("kuro".into()));
        s.set_raw("tags", PrefValue::Set(tags.clone()));
        assert_eq!(s.get_raw("name"), Some(PrefValue::Str("kuro".into())));
        assert_eq!(s.get_raw("tags"), Some(PrefValue::Set(tags)));
        assert_eq!(s.get_raw("missing"), None);
    }

    #[test]
    fn a_change_is_published_once_and_an_equal_set_is_silent() {
        let s = InMemorySettings::new();
        let rx = s.subscribe();

        s.set_raw("k", PrefValue::Str("v".into()));
        assert_eq!(
            rx.try_recv(),
            Ok(Changed {
                key: Some("k".into())
            })
        );

        s.set_raw("k", PrefValue::Str("v".into()));
        assert!(rx.try_recv().is_err());

        s.remove("k");
        assert_eq!(
            rx.try_recv(),
            Ok(Changed {
                key: Some("k".into())
            })
        );
    }

    #[test]
    fn a_batch_is_one_change_with_no_key() {
        let s = InMemorySettings::new();
        let rx = s.subscribe();
        s.replace_many(vec![
            ("a".into(), PrefValue::Str("1".into())),
            ("b".into(), PrefValue::Str("2".into())),
        ]);
        assert_eq!(rx.try_recv(), Ok(Changed { key: None }));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn clones_share_one_store() {
        let s = InMemorySettings::new();
        let other = s.clone();
        s.set_raw("k", PrefValue::Str("shared".into()));
        assert_eq!(other.get_raw("k"), Some(PrefValue::Str("shared".into())));
    }
}
