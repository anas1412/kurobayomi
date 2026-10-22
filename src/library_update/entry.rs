//! The per-entry flow of a pass (behaviour 6.2): favourite gate, fetch, empty check, favourite re-read, sync, sort,
//! download hand-off. Manga and anime share it; season children (anime, `parent_id` set) skip the favourite gate.

use std::sync::atomic::{AtomicBool, Ordering};

use kuro_platform::kind::Kind;

use crate::library_update::error::UpdateError;
use crate::library_update::fetch_interval::Window;
use crate::library_update::pass::{Host, ItemSync, PassSettings};
use crate::library_update::queue::LibraryEntry;
use crate::library_update::source::{SourceItem, SourceRegistry};
use crate::library_update::store::LibraryStore;

/// The collaborators of a pass. Shared by every entry task.
pub struct Deps<R, S, Y, H> {
    pub registry: R,
    pub store: S,
    pub sync: Y,
    pub host: H,
}

/// What one entry produced: the new items (newest source position first) and how many were queued to download.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct EntryOutcome {
    pub new_items: Vec<SourceItem>,
    pub downloads: usize,
}

pub struct EntryContext<'a> {
    pub settings: &'a PassSettings,
    pub window: Window,
    pub now_ms: i64,
    pub cancel: &'a AtomicBool,
}

pub async fn update_entry<R, S, Y, H>(
    deps: &Deps<R, S, Y, H>,
    cx: &EntryContext<'_>,
    entry: &LibraryEntry,
) -> Result<EntryOutcome, UpdateError>
where
    R: SourceRegistry,
    S: LibraryStore,
    Y: ItemSync,
    H: Host,
{
    let kind = entry.kind;
    // Season children are not library favourites themselves; their parent is checked elsewhere.
    let top_level = entry.parent_id.is_none();
    if top_level && !deps.store.is_favorite(entry.id, kind).await? {
        return Ok(EntryOutcome::default());
    }

    let slot = deps.registry.get_or_stub(kind, entry.source);
    // a local title's details are its own files (details.json, ComicInfo.xml, cover), read on every pass so edits show up
    let fetched = slot.fetch(kind, &entry.url, &entry.title, cx.settings.auto_update_metadata || slot.is_local()).await?;
    if fetched.items.is_empty() && !slot.is_local() {
        return Err(match kind {
            Kind::Manga => UpdateError::NoChapters,
            Kind::Anime => UpdateError::NoEpisodes,
        });
    }
    check_cancelled(cx.cancel)?;

    // Removed from the library while the source was answering (P-04, R-03): write nothing.
    if top_level && !deps.store.is_favorite(entry.id, kind).await? {
        return Ok(EntryOutcome::default());
    }

    let mut new_items = deps.sync.sync(entry, &fetched, cx.window, cx.now_ms).await?;
    new_items.sort_by(|a, b| b.source_order.cmp(&a.source_order));
    // a local title's files are on disk already: nothing to download
    let downloads = if slot.is_local() { 0 } else { queue_downloads(deps, cx, entry, &new_items).await? };
    Ok(EntryOutcome { new_items, downloads })
}

fn check_cancelled(cancel: &AtomicBool) -> Result<(), UpdateError> {
    if cancel.load(Ordering::SeqCst) {
        Err(UpdateError::Cancelled)
    } else {
        Ok(())
    }
}

/// Queues the new items the download policy selects, with autostart off. Returns how many were queued.
async fn queue_downloads<R, S, Y, H>(
    deps: &Deps<R, S, Y, H>,
    cx: &EntryContext<'_>,
    entry: &LibraryEntry,
    new_items: &[SourceItem],
) -> Result<usize, UpdateError>
where
    S: LibraryStore,
    H: Host,
{
    let policy = &cx.settings.download;
    if new_items.is_empty() || !policy.enabled {
        return Ok(0);
    }
    let categories = deps.store.categories(entry.id, entry.kind).await?;
    if !policy.wants(entry.favorite, &categories) {
        return Ok(0);
    }
    let read = if policy.unread_only {
        deps.store.read_numbers(entry.id, entry.kind).await?
    } else {
        Vec::new()
    };
    let selected = policy.select(new_items, &read);
    if selected.is_empty() {
        return Ok(0);
    }
    deps.host.enqueue_downloads(entry.kind, entry.id, &selected);
    Ok(selected.len())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use kuro_platform::kind::Kind;

    use super::*;
    use crate::library_update::fake::{FakeHost, FakeStore, FakeSync, entry, item};
    use crate::library_update::fetch_interval::Window;
    use crate::library_update::queue::LibraryEntry;
    use crate::library_update::source::fake::{FakeRegistry, FakeSource};

    type TestDeps = Deps<FakeRegistry, FakeStore, FakeSync, FakeHost>;

    fn window() -> Window {
        Window { lower_ms: 0, upper_ms: 0 }
    }

    fn deps(store: FakeStore, source_id: i64, source: FakeSource) -> TestDeps {
        let mut registry = FakeRegistry::default();
        registry.sources.insert(source_id, source);
        Deps { registry, store, sync: FakeSync::default(), host: FakeHost::default() }
    }

    async fn run(deps: &TestDeps, settings: &PassSettings, entry: &LibraryEntry, cancel: bool) -> Result<EntryOutcome, UpdateError> {
        let flag = AtomicBool::new(cancel);
        let cx = EntryContext { settings, window: window(), now_ms: 1_000, cancel: &flag };
        update_entry(deps, &cx, entry).await
    }

    #[tokio::test]
    async fn a_removed_favourite_is_skipped_before_any_source_call() {
        // P-03: the favourite gate runs first, so the source is never asked.
        let store = FakeStore::with(vec![]);
        let source = FakeSource::ok(vec![item(1.0, 0)]);
        let calls = Arc::clone(&source.calls);
        let deps = deps(store, 1001, source);
        let settings = PassSettings::new(Kind::Manga);
        let outcome = run(&deps, &settings, &entry(1, 1001, "A"), false).await.expect("skipped, not failed");
        assert_eq!(outcome, EntryOutcome::default());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn an_entry_whose_source_is_missing_fails_with_source_not_installed() {
        // P-05
        let deps = Deps {
            registry: FakeRegistry::default(),
            store: FakeStore::with(vec![entry(1, 7, "A")]),
            sync: FakeSync::default(),
            host: FakeHost::default(),
        };
        let settings = PassSettings::new(Kind::Manga);
        let result = run(&deps, &settings, &entry(1, 7, "A"), false).await;
        assert_eq!(result, Err(UpdateError::SourceNotInstalled));
    }

    #[tokio::test]
    async fn an_empty_list_from_a_remote_source_is_no_chapters_and_from_a_local_one_is_fine() {
        // S-01, S-02
        let settings = PassSettings::new(Kind::Manga);
        let remote = deps(FakeStore::with(vec![entry(1, 1, "A")]), 1, FakeSource::ok(vec![]));
        assert_eq!(run(&remote, &settings, &entry(1, 1, "A"), false).await, Err(UpdateError::NoChapters));
        let mut local_source = FakeSource::ok(vec![]);
        local_source.local = true;
        let local = deps(FakeStore::with(vec![entry(1, 1, "A")]), 1, local_source);
        assert_eq!(run(&local, &settings, &entry(1, 1, "A"), false).await, Ok(EntryOutcome::default()));
    }

    #[tokio::test]
    async fn an_entry_removed_while_the_source_answered_writes_nothing() {
        // P-04: the favourite check passes at the gate and fails at the re-read.
        let store = FakeStore { revoke_after_checks: Some(1), ..FakeStore::with(vec![entry(1, 1, "A")]) };
        let deps = deps(store, 1, FakeSource::ok(vec![item(1.0, 0)]));
        let settings = PassSettings::new(Kind::Manga);
        let outcome = run(&deps, &settings, &entry(1, 1, "A"), false).await.expect("no error");
        assert!(outcome.new_items.is_empty());
        assert_eq!(deps.sync.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_season_child_does_not_need_to_be_a_favourite_itself() {
        // Season children skip the favourite gate (parent_id set).
        let mut child = entry(5, 1, "S1");
        child.parent_id = Some(9);
        let deps = deps(FakeStore::with(vec![]), 1, FakeSource::ok(vec![item(1.0, 0)]));
        let settings = PassSettings::new(Kind::Anime);
        let outcome = run(&deps, &settings, &child, false).await.expect("child is synced");
        assert_eq!(outcome.new_items.len(), 1);
        assert_eq!(deps.sync.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn new_items_come_back_newest_source_position_first() {
        let deps = deps(FakeStore::with(vec![entry(1, 1, "A")]), 1, FakeSource::ok(vec![item(1.0, 1), item(3.0, 3), item(2.0, 2)]));
        let settings = PassSettings::new(Kind::Manga);
        let outcome = run(&deps, &settings, &entry(1, 1, "A"), false).await.expect("ok");
        let order: Vec<i64> = outcome.new_items.iter().map(|i| i.source_order).collect();
        assert_eq!(order, vec![3, 2, 1]);
    }

    #[tokio::test]
    async fn selected_downloads_are_queued_with_autostart_off_once_per_entry() {
        // P-13 (per entry): the policy selects, the host enqueues.
        let deps = deps(FakeStore::with(vec![entry(1, 1, "A")]), 1, FakeSource::ok(vec![item(1.0, 0), item(2.0, 1)]));
        let mut settings = PassSettings::new(Kind::Anime);
        settings.download.enabled = true;
        let outcome = run(&deps, &settings, &entry(1, 1, "A"), false).await.expect("ok");
        assert_eq!(outcome.downloads, 2);
        assert_eq!(deps.host.enqueued.lock().unwrap().as_slice(), &[(1, 2)]);
    }

    #[tokio::test]
    async fn downloads_stay_off_when_the_policy_is_disabled() {
        let deps = deps(FakeStore::with(vec![entry(1, 1, "A")]), 1, FakeSource::ok(vec![item(1.0, 0)]));
        let settings = PassSettings::new(Kind::Manga);
        let outcome = run(&deps, &settings, &entry(1, 1, "A"), false).await.expect("ok");
        assert_eq!(outcome.downloads, 0);
        assert!(deps.host.enqueued.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_cancel_seen_before_the_sync_is_a_cancellation_not_an_error() {
        // P-08 / T-26a-021
        let deps = deps(FakeStore::with(vec![entry(1, 1, "A")]), 1, FakeSource::ok(vec![item(1.0, 0)]));
        let settings = PassSettings::new(Kind::Manga);
        let result = run(&deps, &settings, &entry(1, 1, "A"), true).await;
        assert_eq!(result, Err(UpdateError::Cancelled));
        assert_eq!(deps.sync.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_source_error_comes_back_as_its_message() {
        // P-06 (per entry): the message is kept verbatim.
        let deps = deps(FakeStore::with(vec![entry(1, 1, "A")]), 1, FakeSource::err(UpdateError::Other("timeout".into())));
        let settings = PassSettings::new(Kind::Manga);
        let result = run(&deps, &settings, &entry(1, 1, "A"), false).await;
        assert_eq!(result.map_err(|e| e.user_message()), Err("timeout".to_owned()));
    }
}
