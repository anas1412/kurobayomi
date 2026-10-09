//! The store runner: one tokio task consumes `StoreCmd`, and every result goes back as a `StoreEvent` through an
//! `async-channel`. The GTK side drains the channel with [`collect`], which keeps only the newest snapshot
//! (`LatestWins`). `Shutdown` (the page was popped) aborts the subscriptions and cancels the page's
//! `CancellationToken`, which ends a refresh that is still fetching. Write commands run in their own task and on the
//! blocking pool and never look at the token, so a pop does not cancel them (`launchNonCancellable`). Nothing here imports GTK.
//! Upstream: `MangaScreenModel.kt:173-256` (init), `:362-420` (fetchAllFromSource), `CoroutinesExtensions.kt`.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use async_channel::{Receiver, Sender};
use kuro_data::manga::Manga;
use tokio::runtime::Handle;
use tokio::task::AbortHandle;
use tokio_util::sync::CancellationToken;

use crate::entry_core::backend::Backend;
use crate::entry_core::chapter::ChapterRow;
use crate::entry_core::detail::{
    LatestWins, LoadContext, LoadState, MangaRow, StoreCmd, StoreEvent, finish_load,
};
use crate::entry_core::errors::DetailError;
use crate::entry_core::store::{DbStore, chapter_rows, manga_row};

/// The newest published rows of the page.
pub type Snapshot = (Box<MangaRow>, Vec<ChapterRow>);

/// The page's handle on the runner. Dropping it stops the runner and the subscriptions.
pub struct Runner {
    cmds: Sender<StoreCmd>,
    events: Receiver<StoreEvent>,
    cancel: CancellationToken,
}

impl Runner {
    /// Queues a command. The queue is unbounded, so this never blocks the GTK thread.
    pub fn send(&self, cmd: StoreCmd) {
        let _ = self.cmds.try_send(cmd);
    }

    pub fn events(&self) -> Receiver<StoreEvent> {
        self.events.clone()
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        self.cancel.cancel();
        let _ = self.cmds.try_send(StoreCmd::Shutdown);
    }
}

/// Starts the runner on `handle`.
pub fn spawn(handle: &Handle, store: DbStore, backend: Arc<dyn Backend>, ctx: LoadContext) -> Runner {
    let (cmd_tx, cmd_rx) = async_channel::unbounded();
    let (event_tx, event_rx) = async_channel::unbounded();
    let cancel = CancellationToken::new();
    let shared = Arc::new(Shared {
        store,
        backend,
        ctx,
        events: event_tx,
        cancel: cancel.clone(),
        subscriptions: Mutex::new(Vec::new()),
        refreshing: Mutex::new(HashSet::new()),
    });
    handle.spawn(run(shared, cmd_rx));
    Runner {
        cmds: cmd_tx,
        events: event_rx,
        cancel,
    }
}

/// Drains the channel after `first` arrived. Snapshots go through `latest`, so a burst becomes one value, and it
/// comes last. Every other event keeps its order.
pub fn collect(first: StoreEvent, rest: &Receiver<StoreEvent>, latest: &LatestWins<Snapshot>) -> Vec<StoreEvent> {
    let mut out = Vec::new();
    let mut route = |event: StoreEvent| match event {
        StoreEvent::Snapshot { manga, chapters } => latest.put((manga, chapters)),
        other => out.push(other),
    };
    route(first);
    while let Ok(event) = rest.try_recv() {
        route(event);
    }
    if let Some((manga, chapters)) = latest.take() {
        out.push(StoreEvent::Snapshot { manga, chapters });
    }
    out
}

struct Shared {
    store: DbStore,
    backend: Arc<dyn Backend>,
    ctx: LoadContext,
    events: Sender<StoreEvent>,
    /// The page's scope: cancelled when the page is disposed. Only refreshes watch it.
    cancel: CancellationToken,
    subscriptions: Mutex<Vec<AbortHandle>>,
    refreshing: Mutex<HashSet<i64>>,
}

async fn run(shared: Arc<Shared>, cmds: Receiver<StoreCmd>) {
    while let Ok(cmd) = cmds.recv().await {
        match cmd {
            StoreCmd::Shutdown => {
                shared.cancel.cancel();
                break;
            }
            StoreCmd::Load { manga_id } => shared.clone().load(manga_id).await,
            StoreCmd::FetchAll { manga_id, manual } => {
                tokio::spawn(shared.clone().fetch_all(manga_id, manual));
            }
            write => {
                tokio::spawn(shared.clone().write(write));
            }
        }
    }
    shared.drop_subscriptions();
}

/// Removes the manga from the refreshing set on every exit path, a panic included.
struct RefreshGuard<'a>(&'a Shared, i64);

impl Drop for RefreshGuard<'_> {
    fn drop(&mut self) {
        self.0.refreshing.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.1);
    }
}

impl Shared {
    fn emit(&self, event: StoreEvent) {
        let _ = self.events.try_send(event);
    }

    fn drop_subscriptions(&self) {
        for handle in self.subscriptions.lock().unwrap_or_else(PoisonError::into_inner).drain(..) {
            handle.abort();
        }
    }

    /// Runs blocking store work on the blocking pool. The work keeps running if the awaiting task is aborted.
    async fn blocking<T: Send + 'static>(&self, work: impl FnOnce(DbStore, Arc<dyn Backend>) -> T + Send + 'static) -> T {
        let (store, backend) = (self.store.clone(), Arc::clone(&self.backend));
        tokio::task::spawn_blocking(move || work(store, backend)).await.expect("store work does not panic")
    }

    fn fail(&self, what: &str, e: impl std::fmt::Display) {
        self.emit(StoreEvent::Failed(DetailError::Other {
            class: what.to_owned(),
            message: Some(e.to_string()),
        }));
    }

    /// Load order: the manga row, the library defaults for a non-favorite, the chapters, then `Ready`.
    async fn load(self: Arc<Self>, manga_id: i64) {
        let defaults = self.ctx.library_defaults;
        let read = self
            .blocking(move |store, backend| {
                let Some(manga) = store.manga(manga_id)? else {
                    return Ok((None, Vec::new(), Default::default(), Default::default(), Default::default()));
                };
                if !manga.favorite {
                    store.write_defaults(manga_id, defaults)?;
                }
                let downloads = backend.downloads(manga_id);
                let (manga, chapters) = store.read_page(manga_id, &downloads)?;
                let source = manga.as_ref().map(|m| backend.source_info(m.source)).unwrap_or_default();
                let scanlators = store.scanlators(manga_id)?;
                let categories = store.categories(manga_id)?;
                kuro_data::DataResult::Ok((manga, chapters, source, scanlators, categories))
            })
            .await;
        let (manga, chapters, source, scanlators, (all, membership)) = match read {
            Ok(read) => read,
            Err(e) => {
                self.fail("SQLiteException", e);
                self.emit(StoreEvent::Loaded(Box::new(finish_load(None, vec![], Default::default(), &self.ctx))));
                return;
            }
        };
        let mut loaded = finish_load(manga, chapters, source, &self.ctx);
        if let LoadState::Ready(page) = &mut loaded.state {
            page.available_scanlators = scanlators.available;
            page.excluded_scanlators = scanlators.excluded;
            page.categories = all;
            page.category_membership = membership;
        }
        let ready = matches!(loaded.state, LoadState::Ready(_));
        let fetch = loaded.needs_info || loaded.needs_chapters;
        self.emit(StoreEvent::Loaded(Box::new(loaded)));
        if ready {
            self.subscribe(manga_id);
            if fetch {
                tokio::spawn(self.clone().fetch_all(manga_id, false));
            }
        }
    }

    /// Two tasks, one per table, each publishing a snapshot of the pair when either side changes.
    fn subscribe(self: &Arc<Self>, manga_id: i64) {
        let latest: Arc<Mutex<(Option<Manga>, Option<Vec<kuro_data::chapter::Chapter>>)>> = Arc::default();
        let mut manga_sub = self.store.watch_manga(manga_id);
        let mut chapter_sub = self.store.watch_chapters(manga_id);
        let publish = {
            let (shared, latest) = (Arc::clone(self), Arc::clone(&latest));
            move || {
                let guard = latest.lock().unwrap_or_else(PoisonError::into_inner);
                if let (Some(m), Some(c)) = (&guard.0, &guard.1) {
                    let downloads = shared.backend.downloads(manga_id);
                    shared.emit(StoreEvent::Snapshot {
                        manga: Box::new(manga_row(m)),
                        chapters: chapter_rows(c, &downloads),
                    });
                }
            }
        };
        let manga_task = tokio::spawn({
            let (latest, publish) = (Arc::clone(&latest), publish.clone());
            async move {
                while let Some(published) = manga_sub.changed().await {
                    if let Ok(Some(m)) = published.map(|p| p.as_ref().clone()) {
                        latest.lock().unwrap_or_else(PoisonError::into_inner).0 = Some(m);
                        publish();
                    }
                }
            }
        });
        let chapter_task = tokio::spawn(async move {
            while let Some(published) = chapter_sub.changed().await {
                if let Ok(list) = published {
                    latest.lock().unwrap_or_else(PoisonError::into_inner).1 = Some(list.as_ref().clone());
                    publish();
                }
            }
        });
        let mut subs = self.subscriptions.lock().unwrap_or_else(PoisonError::into_inner);
        subs.push(manga_task.abort_handle());
        subs.push(chapter_task.abort_handle());
    }

    /// `fetchAllFromSource`: details and chapters run at once. One refresh per manga: a second call while one runs
    /// does nothing. A chapter failure re-reads the manga from the store.
    async fn fetch_all(self: Arc<Self>, manga_id: i64, _manual: bool) {
        if !self.refreshing.lock().unwrap_or_else(PoisonError::into_inner).insert(manga_id) {
            return;
        }
        let _guard = RefreshGuard(&self, manga_id);
        self.emit(StoreEvent::Refreshing(true));
        let fetched = tokio::select! {
            () = self.cancel.cancelled() => None,
            both = async { tokio::join!(self.backend.fetch_details(manga_id), self.backend.fetch_chapters(manga_id)) } => Some(both),
        };
        let Some((details, chapters)) = fetched else {
            // the page is gone: nobody reads this event, and the guard frees the manga
            self.emit(StoreEvent::Refreshing(false));
            return;
        };
        if let Err(e) = details
            && !e.is_ignored()
        {
            self.emit(StoreEvent::Failed(e));
        }
        if let Err(e) = chapters {
            self.emit(StoreEvent::ChapterFetchFailed(e));
            let reread = self
                .blocking(move |store, backend| store.read_page(manga_id, &backend.downloads(manga_id)))
                .await;
            if let Ok((Some(manga), chapters)) = reread {
                self.emit(StoreEvent::Snapshot {
                    manga: Box::new(manga),
                    chapters,
                });
            }
        }
        self.emit(StoreEvent::Refreshing(false));
    }

    /// One write command. Runs in its own task.
    async fn write(self: Arc<Self>, cmd: StoreCmd) {
        match cmd {
            StoreCmd::SetFlagBits { manga_id, flag, mask } => {
                self.write_result(move |s, _| s.set_flag_bits(manga_id, flag, mask).map(drop)).await;
            }
            StoreCmd::SortBy { manga_id, mode } => {
                self.write_result(move |s, _| s.sort_by(manga_id, mode).map(drop)).await;
            }
            StoreCmd::MarkRead {
                manga_id,
                chapter_ids,
                read,
                remove_after_read,
            } => {
                let plan = self.blocking(move |s, _| s.mark_read(manga_id, &chapter_ids, read, remove_after_read)).await;
                match plan {
                    Ok(Some(plan)) if !plan.delete_downloads.is_empty() => {
                        let ids = plan.delete_downloads;
                        let removed = ids.clone();
                        self.blocking(move |_, backend| backend.delete_downloads(manga_id, &ids)).await;
                        self.emit(StoreEvent::DownloadsDeleted(removed));
                    }
                    Ok(_) => {}
                    Err(e) => self.fail("SQLiteException", e),
                }
            }
            StoreCmd::SetBookmark {
                manga_id,
                chapter_ids,
                bookmark,
            } => {
                self.write_result(move |s, _| s.set_bookmark(manga_id, &chapter_ids, bookmark).map(drop)).await;
            }
            StoreCmd::AddToLibrary { manga_id, category_ids } => {
                let now = chrono::Utc::now().timestamp_millis();
                self.write_result(move |s, _| s.add_to_library(manga_id, &category_ids, now)).await;
                self.reload_categories(manga_id).await;
            }
            StoreCmd::SetCategories { manga_id, category_ids } => {
                self.write_result(move |s, _| s.set_categories(manga_id, &category_ids)).await;
                self.reload_categories(manga_id).await;
            }
            StoreCmd::Load { .. } | StoreCmd::FetchAll { .. } | StoreCmd::Shutdown => {}
        }
    }

    async fn write_result(&self, work: impl FnOnce(&DbStore, &Arc<dyn Backend>) -> kuro_data::DataResult<()> + Send + 'static) {
        if let Err(e) = self.blocking(move |store, backend| work(&store, &backend)).await {
            self.fail("SQLiteException", e);
        }
    }

    async fn reload_categories(&self, manga_id: i64) {
        match self.blocking(move |store, _| store.categories(manga_id)).await {
            Ok((all, membership)) => self.emit(StoreEvent::Categories { all, membership }),
            Err(e) => self.fail("SQLiteException", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use kuro_db::flags::chapter as c;

    use super::*;
    use crate::entry_core::backend::Fetch;
    use crate::entry_core::detail::SourceInfo;
    use crate::entry_core::store::fixture::store;

    /// A backend that counts fetches, can fail either one, and can hold them open.
    struct Fake {
        details: AtomicUsize,
        chapters: AtomicUsize,
        chapter_error: Option<DetailError>,
        detail_error: Option<DetailError>,
        hold_ms: u64,
        /// Fetches that ran to the end. A cancelled fetch is dropped before it counts.
        finished: Arc<AtomicUsize>,
    }

    impl Fake {
        fn ok() -> Arc<Self> {
            Arc::new(Self {
                details: AtomicUsize::new(0),
                chapters: AtomicUsize::new(0),
                chapter_error: None,
                detail_error: None,
                hold_ms: 0,
                finished: Arc::default(),
            })
        }
    }

    impl Backend for Fake {
        fn source_info(&self, _: i64) -> SourceInfo {
            SourceInfo {
                name: "Fake".into(),
                stub: false,
            }
        }

        fn fetch_details(&self, _: i64) -> Fetch {
            self.details.fetch_add(1, Ordering::SeqCst);
            let (hold, error, finished) = (self.hold_ms, self.detail_error.clone(), Arc::clone(&self.finished));
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(hold)).await;
                finished.fetch_add(1, Ordering::SeqCst);
                error.map_or(Ok(()), Err)
            })
        }

        fn fetch_chapters(&self, _: i64) -> Fetch {
            self.chapters.fetch_add(1, Ordering::SeqCst);
            let (hold, error, finished) = (self.hold_ms, self.chapter_error.clone(), Arc::clone(&self.finished));
            Box::pin(async move {
                tokio::time::sleep(Duration::from_millis(hold)).await;
                finished.fetch_add(1, Ordering::SeqCst);
                error.map_or(Ok(()), Err)
            })
        }
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
    }

    async fn next(rx: &Receiver<StoreEvent>) -> StoreEvent {
        tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.expect("an event in time").expect("open")
    }

    async fn until(rx: &Receiver<StoreEvent>, mut pick: impl FnMut(&StoreEvent) -> bool) -> StoreEvent {
        loop {
            let event = next(rx).await;
            if pick(&event) {
                return event;
            }
        }
    }

    fn ctx() -> LoadContext {
        LoadContext::default()
    }

    #[test]
    fn load_reads_manga_then_chapters_then_ready_and_starts_no_fetch_when_complete() {
        let rt = rt();
        let (_dir, store, id) = store(true, 2);
        // an initialized manga with chapters needs no fetch
        store.mark_initialized_for_test(id);
        let backend = Fake::ok();
        rt.block_on(async {
            let runner = spawn(rt.handle(), store, backend.clone(), ctx());
            runner.send(StoreCmd::Load { manga_id: id });
            let rx = runner.events();
            let StoreEvent::Loaded(loaded) = until(&rx, |e| matches!(e, StoreEvent::Loaded(_))).await else {
                unreachable!()
            };
            let LoadState::Ready(page) = loaded.state else { panic!("ready") };
            assert_eq!(page.chapters.len(), 2);
            assert_eq!(page.source.name, "Fake");
            assert!(!loaded.needs_info && !loaded.needs_chapters);
            tokio::time::sleep(Duration::from_millis(100)).await;
            assert_eq!(backend.details.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn a_missing_row_loads_not_found_instead_of_staying_loading() {
        let rt = rt();
        let (_dir, store, _id) = store(true, 0);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store, Fake::ok(), ctx());
            runner.send(StoreCmd::Load { manga_id: 4242 });
            let StoreEvent::Loaded(loaded) = next(&runner.events()).await else { panic!("loaded first") };
            assert_eq!(loaded.state, LoadState::NotFound);
        });
    }

    #[test]
    fn missing_info_starts_one_concurrent_fetch_with_refreshing_true_then_false() {
        let rt = rt();
        let (_dir, store, id) = store(true, 0);
        let backend = Arc::new(Fake {
            hold_ms: 150,
            ..Arc::into_inner(Fake::ok()).unwrap()
        });
        rt.block_on(async {
            let runner = spawn(rt.handle(), store, backend.clone(), ctx());
            runner.send(StoreCmd::Load { manga_id: id });
            // a manual refresh while the first one runs is dropped
            runner.send(StoreCmd::FetchAll { manga_id: id, manual: true });
            let rx = runner.events();
            until(&rx, |e| matches!(e, StoreEvent::Refreshing(true))).await;
            let started = std::time::Instant::now();
            until(&rx, |e| matches!(e, StoreEvent::Refreshing(false))).await;
            assert!(
                started.elapsed() < Duration::from_millis(290),
                "details and chapters ran together, not one after the other"
            );
            assert_eq!(backend.details.load(Ordering::SeqCst), 1, "one refresh per manga");
            assert_eq!(backend.chapters.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn chapter_failure_is_reported_and_rereads_the_manga_and_ignored_103_is_silent() {
        let rt = rt();
        let (_dir, store, id) = store(true, 1);
        let backend = Arc::new(Fake {
            chapter_error: Some(DetailError::NoChapters),
            detail_error: Some(DetailError::Http(103)),
            ..Arc::into_inner(Fake::ok()).unwrap()
        });
        rt.block_on(async {
            let runner = spawn(rt.handle(), store, backend, ctx());
            runner.send(StoreCmd::FetchAll { manga_id: id, manual: true });
            let rx = runner.events();
            let mut seen = Vec::new();
            loop {
                let event = next(&rx).await;
                let done = matches!(event, StoreEvent::Refreshing(false));
                seen.push(event);
                if done {
                    break;
                }
            }
            assert!(!seen.iter().any(|e| matches!(e, StoreEvent::Failed(_))), "HTTP 103 is ignored");
            let kinds: Vec<&str> = seen
                .iter()
                .map(|e| match e {
                    StoreEvent::Refreshing(true) => "start",
                    StoreEvent::ChapterFetchFailed(DetailError::NoChapters) => "chapter-error",
                    StoreEvent::Snapshot { .. } => "reread",
                    StoreEvent::Refreshing(false) => "stop",
                    _ => "other",
                })
                .collect();
            assert_eq!(kinds, ["start", "chapter-error", "reread", "stop"]);
        });
    }

    #[test]
    fn flag_writes_reach_the_page_as_a_snapshot_from_the_subscription() {
        let rt = rt();
        let (_dir, store, id) = store(true, 1);
        store.mark_initialized_for_test(id);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), Fake::ok(), ctx());
            runner.send(StoreCmd::Load { manga_id: id });
            let rx = runner.events();
            until(&rx, |e| matches!(e, StoreEvent::Loaded(_))).await;
            runner.send(StoreCmd::SetFlagBits {
                manga_id: id,
                flag: c::SHOW_UNREAD,
                mask: c::UNREAD_MASK,
            });
            let event = until(&rx, |e| matches!(e, StoreEvent::Snapshot { manga, .. } if manga.flags.0 & c::UNREAD_MASK == c::SHOW_UNREAD)).await;
            let StoreEvent::Snapshot { manga, .. } = event else { unreachable!() };
            assert_eq!(manga.flags.0 & c::UNREAD_MASK, c::SHOW_UNREAD, "the view reads the store, not a local copy");
        });
    }

    #[test]
    fn shutdown_drops_the_subscriptions_but_a_write_sent_before_it_still_lands() {
        let rt = rt();
        let (_dir, store, id) = store(true, 1);
        store.mark_initialized_for_test(id);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), Fake::ok(), ctx());
            runner.send(StoreCmd::Load { manga_id: id });
            let rx = runner.events();
            until(&rx, |e| matches!(e, StoreEvent::Snapshot { .. })).await;
            runner.send(StoreCmd::SetFlagBits {
                manga_id: id,
                flag: c::SHOW_BOOKMARKED,
                mask: c::BOOKMARKED_MASK,
            });
            drop(runner); // the page was popped
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                let word = store.manga(id).unwrap().unwrap().flags.0;
                if word & c::BOOKMARKED_MASK == c::SHOW_BOOKMARKED {
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "the detached write never landed");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            // the runner is gone: its event channel closes once every sender is dropped
            let closed = tokio::time::timeout(Duration::from_secs(5), async {
                while rx.recv().await.is_ok() {}
            })
            .await;
            assert!(closed.is_ok(), "subscription tasks were aborted, so the channel closes");
        });
    }

    #[test]
    fn dispose_during_a_fetch_cancels_the_fetch_and_a_queued_write_still_lands() {
        let rt = rt();
        let (_dir, store, id) = store(true, 1);
        let backend = Arc::new(Fake {
            hold_ms: 400,
            ..Arc::into_inner(Fake::ok()).unwrap()
        });
        let finished = Arc::clone(&backend.finished);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), backend.clone(), ctx());
            let rx = runner.events();
            runner.send(StoreCmd::FetchAll { manga_id: id, manual: true });
            until(&rx, |e| matches!(e, StoreEvent::Refreshing(true))).await;
            runner.send(StoreCmd::SetFlagBits {
                manga_id: id,
                flag: c::SHOW_BOOKMARKED,
                mask: c::BOOKMARKED_MASK,
            });
            drop(runner); // dispose while both fetches are still held open
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while store.manga(id).unwrap().unwrap().flags.0 & c::BOOKMARKED_MASK != c::SHOW_BOOKMARKED {
                assert!(std::time::Instant::now() < deadline, "the queued write never landed");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            tokio::time::sleep(Duration::from_millis(700)).await;
            assert_eq!(backend.details.load(Ordering::SeqCst), 1, "the fetch had started");
            assert_eq!(finished.load(Ordering::SeqCst), 0, "dispose cancelled the fetch before it finished");
        });
    }

    #[test]
    fn category_commands_run_and_reload_the_categories() {
        let rt = rt();
        let (_dir, store, id) = store(false, 0);
        rt.block_on(async {
            let runner = spawn(rt.handle(), store.clone(), Fake::ok(), ctx());
            runner.send(StoreCmd::AddToLibrary {
                manga_id: id,
                category_ids: vec![],
            });
            let rx = runner.events();
            let StoreEvent::Categories { membership, .. } = until(&rx, |e| matches!(e, StoreEvent::Categories { .. })).await else {
                unreachable!()
            };
            assert!(membership.is_empty());
            assert!(store.manga(id).unwrap().unwrap().favorite);
        });
    }

    #[test]
    fn a_burst_of_snapshots_collapses_to_the_newest_and_keeps_other_events_in_order() {
        let (tx, rx) = async_channel::unbounded();
        let manga = |title: &str| {
            Box::new(MangaRow {
                id: 1,
                source: 1,
                url: String::new(),
                title: title.into(),
                author: None,
                artist: None,
                description: None,
                genres: vec![],
                status: 0,
                thumbnail_url: None,
                initialized: true,
                favorite: true,
                local: false,
                flags: Default::default(),
            })
        };
        for event in [
            StoreEvent::Snapshot { manga: manga("old"), chapters: vec![] },
            StoreEvent::Refreshing(true),
            StoreEvent::Snapshot { manga: manga("new"), chapters: vec![] },
            StoreEvent::Refreshing(false),
        ] {
            tx.try_send(event).unwrap();
        }
        let first = rx.try_recv().unwrap();
        let out = collect(first, &rx, &LatestWins::new());
        assert_eq!(out.len(), 3);
        assert_eq!(out[0], StoreEvent::Refreshing(true));
        assert_eq!(out[1], StoreEvent::Refreshing(false));
        let StoreEvent::Snapshot { manga, .. } = &out[2] else { panic!("snapshot last") };
        assert_eq!(manga.title, "new");
    }
}
