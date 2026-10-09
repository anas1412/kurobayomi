//! Live queries (T-05a-002), the replacement for Kotlin `Flow`. A subscription runs its query at once,
//! then again after each committed write that touches one of its tables, and publishes the result only
//! when it differs from the last one. A failing query publishes its error in the same way (T-08a-032).
//! A paused subscription runs nothing until it is resumed, then runs once. Must be created inside a
//! tokio runtime.
//!
//! UI debounce: the library screens (area:37-ui-library) add a 300 ms trailing debounce on top of
//! `debounce` here, and screens they hide call `pause()`, so the 250 ms library query does not re-run
//! while nobody sees it. The SQL is not changed for either.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::time::Duration;

use rusqlite::Connection;
use tokio::sync::{broadcast, watch};

use crate::error::DbResult;
use crate::notify::{Changed, Tables};
use crate::runtime::Db;

/// What a live query publishes: its result, or the error text of the query that failed.
pub type Published<T> = Result<Arc<T>, String>;

/// The receiving end of a live query. Dropping it stops the task behind it.
pub struct Subscription<T> {
    rx: watch::Receiver<Option<Published<T>>>,
    paused: watch::Sender<bool>,
}

impl<T> Subscription<T> {
    /// A subscription over a channel the caller feeds, for readers that run without a database (fakes in tests). The
    /// last value sent stays readable after the sender is dropped.
    pub fn over(rx: watch::Receiver<Option<Published<T>>>) -> Self {
        Self { rx, paused: watch::channel(false).0 }
    }

    /// Stops running the query until [`Subscription::resume`]. The last result stays readable.
    pub fn pause(&self) {
        self.paused.send_replace(true);
    }

    /// Lets the query run again. It runs once straight away, so the result is fresh on return.
    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    pub fn is_paused(&self) -> bool {
        *self.paused.borrow()
    }

    /// Waits for the next published result or error. `None` once the database handle is dropped.
    pub async fn changed(&mut self) -> Option<Published<T>> {
        self.rx.changed().await.ok()?;
        self.rx.borrow_and_update().clone()
    }

    /// The last published result or error, `None` until the first query has finished.
    pub fn current(&self) -> Option<Published<T>> {
        self.rx.borrow().clone()
    }
}

/// How long until the next run of a clock-driven query, measured from the end of the last run.
type WakeFn = Arc<dyn Fn() -> Duration + Send + Sync>;

impl Db {
    /// Starts a live query over `tables`. `debounce` is the quiet time after a notification before the
    /// query runs again, so a burst of writes costs one query. A failing query publishes its error, and the
    /// watcher keeps running: the next change runs the query again.
    pub fn subscribe<T, F>(
        self: &Arc<Self>,
        tables: &'static [&'static str],
        debounce: Duration,
        query: F,
    ) -> Subscription<T>
    where
        T: PartialEq + Send + Sync + 'static,
        F: Fn(&Connection) -> DbResult<T> + Send + Sync + 'static,
    {
        self.spawn_subscription(tables.to_vec(), debounce, None, query)
    }

    /// Like [`Db::subscribe`], over the manga tables of a [`Tables`] set (the dependency constants of
    /// `notify`, such as `LIBRARY_DEPS`).
    pub fn subscribe_tables<T, F>(self: &Arc<Self>, deps: Tables, debounce: Duration, query: F) -> Subscription<T>
    where
        T: PartialEq + Send + Sync + 'static,
        F: Fn(&Connection) -> DbResult<T> + Send + Sync + 'static,
    {
        self.spawn_subscription(deps.names(), debounce, None, query)
    }

    /// Like [`Db::subscribe`], and also runs the query again `next_wake()` after each run, with no write
    /// needed. For results that depend on the clock, such as an upcoming list cut off at local midnight.
    pub fn subscribe_timed<T, F>(
        self: &Arc<Self>,
        tables: &'static [&'static str],
        debounce: Duration,
        next_wake: impl Fn() -> Duration + Send + Sync + 'static,
        query: F,
    ) -> Subscription<T>
    where
        T: PartialEq + Send + Sync + 'static,
        F: Fn(&Connection) -> DbResult<T> + Send + Sync + 'static,
    {
        self.spawn_subscription(tables.to_vec(), debounce, Some(Arc::new(next_wake)), query)
    }

    fn spawn_subscription<T, F>(
        self: &Arc<Self>,
        tables: Vec<&'static str>,
        debounce: Duration,
        next_wake: Option<WakeFn>,
        query: F,
    ) -> Subscription<T>
    where
        T: PartialEq + Send + Sync + 'static,
        F: Fn(&Connection) -> DbResult<T> + Send + Sync + 'static,
    {
        let (tx, rx) = watch::channel(None);
        let (paused, paused_rx) = watch::channel(false);
        let changes = self.changes.subscribe();
        tokio::spawn(run(
            Arc::downgrade(self),
            changes,
            tables,
            debounce,
            next_wake,
            Arc::new(query),
            tx,
            paused_rx,
        ));
        Subscription { rx, paused }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run<T, F>(
    db: Weak<Db>,
    mut changes: broadcast::Receiver<Changed>,
    tables: Vec<&'static str>,
    debounce: Duration,
    next_wake: Option<WakeFn>,
    query: Arc<F>,
    tx: watch::Sender<Option<Published<T>>>,
    mut paused: watch::Receiver<bool>,
) where
    T: PartialEq + Send + Sync + 'static,
    F: Fn(&Connection) -> DbResult<T> + Send + Sync + 'static,
{
    let mut last: Option<Published<T>> = None;
    loop {
        // Paused: run nothing until resumed. The query then runs once, so the result is fresh.
        if *paused.borrow() && paused.wait_for(|p| !*p).await.is_err() {
            return;
        }
        let Some(handle) = db.upgrade() else { return };
        let q = Arc::clone(&query);
        // The query runs on a blocking thread, so a slow read never stalls the async workers.
        let outcome = tokio::task::spawn_blocking(move || handle.read(|c| q(c))).await;
        let published: Published<T> = match outcome {
            Ok(Ok(value)) => Ok(Arc::new(value)),
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "subscription query failed; publishing the error");
                Err(e.to_string())
            }
            Err(e) => {
                tracing::warn!(error = %e, "subscription query task failed; publishing the error");
                Err(e.to_string())
            }
        };
        // The first outcome is always published, even an error; after that only changes are.
        if last.as_ref() != Some(&published) {
            last = Some(published.clone());
            if tx.send(Some(published)).is_err() {
                return;
            }
        }
        let mut timer: Pin<Box<dyn Future<Output = ()> + Send>> = match &next_wake {
            Some(wake) => Box::pin(tokio::time::sleep(wake())),
            None => Box::pin(std::future::pending()),
        };
        loop {
            tokio::select! {
                _ = tx.closed() => return,
                _ = paused.wait_for(|p| *p) => break,
                _ = &mut timer => break,
                received = changes.recv() => match received {
                    Ok(changed) if tables.iter().any(|t| changed.contains(*t)) => break,
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => break,
                    Err(broadcast::error::RecvError::Closed) => return,
                },
            }
        }
        if *paused.borrow() {
            continue;
        }
        tokio::time::sleep(debounce).await;
        // Notifications that arrived before this point are covered by the query that follows: its
        // reader starts after their commits. Anything that commits later leaves a new notification.
        while let Ok(_) | Err(broadcast::error::TryRecvError::Lagged(_)) = changes.try_recv() {}
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::open::DbError;
    use crate::schema_manga;
    use crate::statements_manga::{CategoryPatch, categories};

    const CATEGORIES: &[&str] = &["categories"];
    const HISTORY: &[&str] = &["history"];
    /// How long a test waits for a publication that should not come.
    const QUIET: Duration = Duration::from_millis(400);

    fn fixture() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manga.db");
        let writer = schema_manga::open(&path).unwrap();
        (dir, Arc::new(Db::new(&path, writer).unwrap()))
    }

    fn names(c: &Connection) -> DbResult<Vec<String>> {
        Ok(categories::get_categories(c)?.into_iter().map(|r| r.name).collect())
    }

    /// The next published outcome, or `None` when nothing is published within [`QUIET`].
    async fn next_published<T>(sub: &mut Subscription<T>) -> Option<Published<T>> {
        tokio::time::timeout(QUIET, sub.changed()).await.ok().flatten()
    }

    /// The next published result, which must be a success.
    async fn next<T>(sub: &mut Subscription<T>) -> Option<Arc<T>> {
        next_published(sub).await.map(|r| r.expect("the query succeeded"))
    }

    #[tokio::test]
    async fn a_timed_subscription_runs_again_when_its_timer_fires_with_no_write() {
        let (_d, db) = fixture();
        let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let mut sub = db.subscribe_timed(
            CATEGORIES,
            Duration::ZERO,
            || Duration::from_millis(20),
            move |_| Ok(counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst)),
        );
        assert_eq!(next(&mut sub).await.as_deref(), Some(&0));
        assert_eq!(next(&mut sub).await.as_deref(), Some(&1), "the timer re-runs the query");
    }

    #[tokio::test]
    async fn an_untimed_subscription_does_not_run_without_a_write() {
        let (_d, db) = fixture();
        let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, move |_| {
            Ok(counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
        });
        assert_eq!(next(&mut sub).await.as_deref(), Some(&0));
        assert!(next(&mut sub).await.is_none(), "no timer, no write, no publication");
        assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_first_publication_is_the_current_result() {
        let (_d, db) = fixture();
        let expected = db.read(names).unwrap();
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, names);
        assert_eq!(next(&mut sub).await.as_deref(), Some(&expected));
        assert_eq!(sub.current().unwrap().unwrap().as_ref(), &expected);
    }

    #[tokio::test]
    async fn a_commit_publishes_and_a_rolled_back_write_does_not() {
        let (_d, db) = fixture();
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, names);
        next(&mut sub).await.expect("first publication");
        let failed: DbResult<()> = db.write(|tx| {
            categories::insert(tx, "Ghost", 1, 0)?;
            Err(DbError::NotFound)
        });
        assert!(failed.is_err());
        assert!(next(&mut sub).await.is_none(), "a rolled-back write publishes nothing");
        db.write(|tx| Ok(categories::insert(tx, "Reading", 1, 0)?)).unwrap();
        let after = next(&mut sub).await.expect("the commit publishes");
        assert!(after.contains(&"Reading".to_owned()));
        assert!(!after.contains(&"Ghost".to_owned()));
    }

    #[tokio::test]
    async fn a_failing_query_publishes_its_error_and_the_watcher_recovers() {
        let (_d, db) = fixture();
        let healthy = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&healthy);
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, move |c| {
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                names(c)
            } else {
                Err(DbError::Constraint("broken".into()))
            }
        });
        let first = next_published(&mut sub).await.expect("a failing first query is still published");
        assert!(first.unwrap_err().contains("broken"));
        assert!(next_published(&mut sub).await.is_none(), "the same error is not published twice");
        healthy.store(true, std::sync::atomic::Ordering::SeqCst);
        db.write(|tx| Ok(categories::insert(tx, "Reading", 1, 0)?)).unwrap();
        let recovered = next(&mut sub).await.expect("the next change runs the query again");
        assert!(recovered.contains(&"Reading".to_owned()));
    }

    #[tokio::test]
    async fn writes_to_other_tables_do_not_publish() {
        let (_d, db) = fixture();
        let mut sub = db.subscribe(HISTORY, Duration::ZERO, |c| {
            Ok(c.query_row("SELECT count(*) FROM history", [], |r| r.get::<_, i64>(0))?)
        });
        next(&mut sub).await.expect("first publication");
        db.write(|tx| Ok(categories::insert(tx, "Reading", 1, 0)?)).unwrap();
        assert!(next(&mut sub).await.is_none());
    }

    #[tokio::test]
    async fn a_burst_inside_the_debounce_window_costs_one_publication() {
        let (_d, db) = fixture();
        let mut sub = db.subscribe(CATEGORIES, Duration::from_millis(200), names);
        let before = next(&mut sub).await.expect("first publication").len();
        for i in 0..5 {
            db.write(|tx| Ok(categories::insert(tx, &format!("C{i}"), i, 0)?)).unwrap();
        }
        let burst = next(&mut sub).await.expect("one publication after the burst");
        assert_eq!(burst.len(), before + 5);
        assert!(next(&mut sub).await.is_none(), "the burst is not published again");
    }

    #[tokio::test]
    async fn an_unchanged_result_is_not_published_again() {
        let (_d, db) = fixture();
        let id = db.write(|tx| Ok(categories::insert(tx, "Same", 1, 0)?)).unwrap();
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, names);
        next(&mut sub).await.expect("first publication");
        db.write(|tx| Ok(categories::update(tx, &CategoryPatch { id, flags: Some(4), ..Default::default() })?)).unwrap();
        assert!(next(&mut sub).await.is_none(), "a write that leaves the result equal publishes nothing");
    }

    #[tokio::test]
    async fn dropping_subscriptions_stops_their_tasks() {
        let (_d, db) = fixture();
        let metrics = tokio::runtime::Handle::current().metrics();
        let baseline = metrics.num_alive_tasks();
        for _ in 0..1_000 {
            drop(db.subscribe(CATEGORIES, Duration::ZERO, names));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while metrics.num_alive_tasks() > baseline && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(metrics.num_alive_tasks(), baseline, "every task ended after its subscription was dropped");
    }

    #[tokio::test]
    async fn a_dropped_database_ends_the_subscription() {
        let (_d, db) = fixture();
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, names);
        next(&mut sub).await.expect("first publication");
        drop(db);
        assert!(tokio::time::timeout(QUIET, sub.changed()).await.expect("ends in time").is_none());
    }


    #[tokio::test]
    async fn a_paused_subscription_runs_nothing_until_it_is_resumed() {
        let (_d, db) = fixture();
        let runs = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, move |c| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            names(c)
        });
        next(&mut sub).await.expect("first publication");
        sub.pause();
        assert!(sub.is_paused());
        db.write(|tx| Ok(categories::insert(tx, "Paused", 1, 0)?)).unwrap();
        assert!(next_published(&mut sub).await.is_none(), "nothing is published while paused");
        assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 1, "no query ran while paused");
        sub.resume();
        let got = next(&mut sub).await.expect("runs once on resume");
        assert!(got.contains(&"Paused".to_owned()));
        assert_eq!(runs.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_commit_during_a_run_is_picked_up_by_the_next_run() {
        // TC-N04: the first run reads the table, then waits while another connection commits. Its
        // result is stale, so the notification of that commit must make the subscription run again.
        let (_d, db) = fixture();
        let slow_first = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag = Arc::clone(&slow_first);
        let mut sub = db.subscribe(CATEGORIES, Duration::ZERO, move |c| {
            let before = names(c)?.len();
            if flag.swap(false, std::sync::atomic::Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(200));
            }
            Ok(before)
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        db.write(|tx| Ok(categories::insert(tx, "Late", 1, 0)?)).unwrap();
        let first = next(&mut sub).await.expect("first, stale result");
        assert_eq!(*first, 1, "the seed row only");
        let second = next(&mut sub).await.expect("the commit makes a second run");
        assert_eq!(*second, 2, "the second run sees the commit");
    }

    #[tokio::test]
    async fn subscribe_tables_fires_for_its_tables_only() {
        let (_d, db) = fixture();
        let mut sub = db.subscribe_tables(crate::notify::CATEGORIES_DEPS, Duration::ZERO, names);
        next(&mut sub).await.expect("first publication");
        db.write(|tx| Ok(crate::statements_manga::mangas::insert(tx, &crate::statements_manga::fixtures::manga("/a"))?))
            .unwrap();
        assert!(next_published(&mut sub).await.is_none(), "a manga write is not a category change");
        db.write(|tx| Ok(categories::insert(tx, "Yes", 1, 0)?)).unwrap();
        assert!(next(&mut sub).await.expect("category change").contains(&"Yes".to_owned()));
    }
}
