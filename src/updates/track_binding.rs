//! Tracker binding of an add from History (T-42a-019, EC-42-H-14, EC-42-H-17b): `bindEnhancedTrackers` runs once the
//! favourite is written, on the add path and on the confirm path alike, because both reach `after_add`.
//!
//! The tracker services are owned by the tracking layer. It hands its [`TrackCtx`] to [`install`] when it starts them;
//! until then a binder finds no context and binds nothing, which is also what a user with no logged-in tracker gets.

use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

use kuro_domain::kind::EntryKind;
use kuro_track::ctx::TrackCtx;
use tokio::sync::watch;

use crate::browse::entries::{CategoryChoice, FavoriteStore, LocalEntry, TrackerBinder};
use crate::browse::favorite::FavoriteWrite;

static CTX: RwLock<Option<Arc<TrackCtx>>> = RwLock::new(None);

/// Registers the tracking context the binders use. The last call wins.
pub fn install(ctx: Arc<TrackCtx>) {
    *CTX.write().unwrap_or_else(|e| e.into_inner()) = Some(ctx);
}

fn current() -> Option<Arc<TrackCtx>> {
    CTX.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The binder of one kind: every logged-in enhanced tracker that accepts the entry's source is matched and bound, on the
/// shell's runtime and detached, so the add does not wait (B§5.2). Built on the GTK thread, where the shell is found.
pub fn binder(kind: EntryKind) -> TrackerBinder {
    let runtime = crate::shell::host::Shell::get().map(|shell| shell.runtime().clone()).or_else(|| tokio::runtime::Handle::try_current().ok());
    Arc::new(move |id, title, source| {
        let (Some(ctx), Some(runtime)) = (current(), runtime.as_ref()) else { return };
        let (title, source) = (title.to_owned(), source.to_string());
        runtime.spawn(async move {
            let failures = match kind {
                EntryKind::Anime => kuro_track::bind::bind_enhanced_anime(&ctx, id, &title, &source).await,
                EntryKind::Manga => kuro_track::bind::bind_enhanced_manga(&ctx, id, &title, &source).await,
            };
            for (tracker, error) in failures {
                tracing::warn!(entry = id, tracker = tracker.0, %error, "an enhanced tracker could not be bound");
            }
        });
    })
}

/// Looks up `(title, source id)` of an entry by id.
pub type EntryLookup = Arc<dyn Fn(i64) -> Result<(String, i64), String> + Send + Sync>;

/// A store whose `after_add` also binds the enhanced trackers. The manga store has no such step of its own, so this
/// wraps it; every other call goes to the wrapped store unchanged.
pub struct BindingStore {
    inner: Arc<dyn FavoriteStore>,
    lookup: EntryLookup,
    binder: TrackerBinder,
}

impl BindingStore {
    pub fn new(inner: Arc<dyn FavoriteStore>, lookup: EntryLookup, binder: TrackerBinder) -> Self {
        Self { inner, lookup, binder }
    }
}

impl FavoriteStore for BindingStore {
    fn local_entry(&self, source_id: i64, url: &str, title: &str, cover: Option<&str>) -> Result<LocalEntry, String> {
        self.inner.local_entry(source_id, url, title, cover)
    }

    fn duplicates(&self, id: i64, title: &str) -> Result<Vec<i64>, String> {
        self.inner.duplicates(id, title)
    }

    fn categories(&self) -> Result<Vec<CategoryChoice>, String> {
        self.inner.categories()
    }

    fn default_category(&self) -> i64 {
        self.inner.default_category()
    }

    fn write_favorite(&self, id: i64, write: FavoriteWrite) -> Result<(), String> {
        self.inner.write_favorite(id, write)
    }

    fn set_categories(&self, id: i64, ids: &[i64]) -> Result<(), String> {
        self.inner.set_categories(id, ids)
    }

    fn entry_categories(&self, id: i64) -> Result<Vec<i64>, String> {
        self.inner.entry_categories(id)
    }

    fn clear_cover(&self, id: i64) -> usize {
        self.inner.clear_cover(id)
    }

    fn stamp_cover(&self, id: i64, now_ms: i64) -> Result<(), String> {
        self.inner.stamp_cover(id, now_ms)
    }

    fn clear_background(&self, id: i64) -> usize {
        self.inner.clear_background(id)
    }

    fn stamp_background(&self, id: i64, now_ms: i64) -> Result<(), String> {
        self.inner.stamp_background(id, now_ms)
    }

    fn after_add(&self, id: i64) -> Result<(), String> {
        let wrapped = self.inner.after_add(id);
        // the trackers are asked even when the wrapped step failed: they do not depend on each other
        let (title, source) = (self.lookup)(id)?;
        (self.binder)(id, &title, source);
        wrapped
    }

    fn favorite_urls(&self, source_id: i64) -> watch::Receiver<BTreeSet<String>> {
        self.inner.favorite_urls(source_id)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::browse::entries::add_entry;
    use crate::browse::entries::fake::FakeStore;

    fn recording() -> (TrackerBinder, Arc<Mutex<Vec<(i64, String, i64)>>>) {
        let seen: Arc<Mutex<Vec<(i64, String, i64)>>> = Arc::default();
        let log = Arc::clone(&seen);
        (Arc::new(move |id, title, source| log.lock().expect("lock").push((id, title.to_owned(), source))), seen)
    }

    fn wrapped(binder: TrackerBinder) -> (BindingStore, Arc<FakeStore>) {
        let fake = Arc::new(FakeStore::default());
        let lookup: EntryLookup = Arc::new(|id| Ok((format!("title {id}"), 9)));
        (BindingStore::new(fake.clone(), lookup, binder), fake)
    }

    #[test]
    fn an_add_binds_the_trackers_after_the_wrapped_after_add() {
        let (binder, seen) = recording();
        let (store, fake) = wrapped(binder);
        *fake.default_category.lock().expect("lock") = 0;
        add_entry(&store, 3, 100);
        assert_eq!(fake.added.lock().expect("lock").as_slice(), [3], "the wrapped after_add ran");
        assert_eq!(seen.lock().expect("lock").as_slice(), [(3, "title 3".to_owned(), 9)], "the binder got id, title and source");
    }

    #[test]
    fn a_chooser_that_has_not_confirmed_binds_nothing() {
        let (binder, seen) = recording();
        let (store, fake) = wrapped(binder);
        *fake.default_category.lock().expect("lock") = -1;
        *fake.categories.lock().expect("lock") = vec![CategoryChoice { id: 1, name: "one".into() }];
        add_entry(&store, 3, 100);
        assert!(seen.lock().expect("lock").is_empty(), "the add asks first; the confirm path binds");
        crate::browse::entries::add_entry_to(&store, 3, 100, vec![1]);
        assert_eq!(seen.lock().expect("lock").len(), 1, "the confirm path reaches after_add through the same wrapper");
    }

    #[test]
    fn without_a_tracking_context_the_binder_does_nothing() {
        // no context is installed in a unit test: the call must return without a runtime or a panic
        binder(EntryKind::Anime)(1, "T", 2);
        binder(EntryKind::Manga)(1, "T", 2);
    }
}
