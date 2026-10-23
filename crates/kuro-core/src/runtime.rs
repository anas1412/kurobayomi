//! Tokio runtime for background work and the dispatch mapping of the Kotlin coroutine contexts
//! (`CoroutinesExtensions.kt`): `withIOContext` is [`spawn_io`], network work is [`spawn_net`],
//! `withUIContext` is [`on_ui`], `launchNonCancellable` is [`NonCancellable`].
//!
//! This crate has no glib. The GUI crate connects the main loop once at startup:
//! `mark_main_thread()` and `set_ui_poster(|f| glib::MainContext::default().invoke(f))`.

use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::thread::ThreadId;
use std::time::Duration;
use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::{JoinHandle, JoinSet};

pub const THREAD_NAME: &str = "kuro-rt";
/// Upper bound for waiting on blocking tasks at shutdown.
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// Worker count: available parallelism clamped to 2..=8.
pub fn worker_threads() -> usize {
    std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(2, 8)
}

/// Multi-thread runtime whose worker threads are named `kuro-rt`.
pub fn build() -> std::io::Result<Runtime> {
    Builder::new_multi_thread().worker_threads(worker_threads()).thread_name(THREAD_NAME).enable_all().build()
}

/// Stops the runtime, waiting at most `SHUTDOWN_TIMEOUT` for blocking tasks. Panics from here on are the app
/// quitting (see [`crate::crash::mark_shutting_down`]).
pub fn shutdown(rt: Runtime) {
    crate::crash::mark_shutting_down();
    rt.shutdown_timeout(SHUTDOWN_TIMEOUT);
}

/// Runs blocking disk or CPU work on the tokio blocking pool. Call inside the runtime context.
pub fn spawn_io<F, T>(f: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
}

/// Runs network or timer work on the tokio runtime. Call inside the runtime context.
pub fn spawn_net<Fut>(fut: Fut) -> JoinHandle<Fut::Output>
where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    tokio::spawn(fut)
}

type UiPoster = Box<dyn Fn(Box<dyn FnOnce() + Send>) + Send + Sync>;

static UI_POSTER: OnceLock<UiPoster> = OnceLock::new();
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

/// Installs the function that runs a closure on the UI main loop. The GUI crate passes
/// `|f| glib::MainContext::default().invoke(f)`. Returns false if one is already installed.
pub fn set_ui_poster(post: impl Fn(Box<dyn FnOnce() + Send>) + Send + Sync + 'static) -> bool {
    UI_POSTER.set(Box::new(post)).is_ok()
}

/// Runs `f` on the UI main loop; call it from worker threads.
///
/// # Panics
/// When the GUI has not installed [`set_ui_poster`], which is a startup-order bug.
pub fn on_ui(f: impl FnOnce() + Send + 'static) {
    let post = UI_POSTER.get().expect("on_ui called before set_ui_poster");
    post(Box::new(f));
}

/// Records the calling thread as the UI main thread. Call once, from the GUI thread.
pub fn mark_main_thread() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
}

/// True on the thread passed to [`mark_main_thread`]; false elsewhere or before it was called.
pub fn is_main_thread() -> bool {
    MAIN_THREAD.get() == Some(&std::thread::current().id())
}

/// Debug-build check placed before GTK calls: panics off the main thread. Free in release builds.
pub fn assert_main_thread() {
    debug_assert!(is_main_thread(), "GTK call off the main thread");
}

/// Work that must finish even when its starter goes away (`launchNonCancellable`). Tasks are
/// tracked in a `JoinSet` so shutdown can wait for them.
#[derive(Clone)]
pub struct NonCancellable {
    handle: Handle,
    tasks: Arc<Mutex<JoinSet<()>>>,
}

impl NonCancellable {
    pub fn new(handle: Handle) -> Self {
        Self { handle, tasks: Arc::default() }
    }

    pub fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) {
        let mut tasks = self.tasks.lock().unwrap_or_else(PoisonError::into_inner);
        // Reap finished tasks so the set does not grow for the life of the app.
        while tasks.try_join_next().is_some() {}
        tasks.spawn_on(fut, &self.handle);
    }

    /// Waits until every tracked task, including ones spawned meanwhile, has finished.
    pub async fn drain(&self) {
        loop {
            let mut tasks = std::mem::take(&mut *self.tasks.lock().unwrap_or_else(PoisonError::into_inner));
            if tasks.is_empty() {
                return;
            }
            while let Some(result) = tasks.join_next().await {
                if let Err(e) = result {
                    tracing::error!("non-cancellable task failed: {e}");
                }
            }
        }
    }
}

/// Waits up to `SHUTDOWN_TIMEOUT` for tracked non-cancellable work, then stops the runtime.
pub fn shutdown_draining(rt: Runtime, tasks: &NonCancellable) {
    if rt.block_on(async { tokio::time::timeout(SHUTDOWN_TIMEOUT, tasks.drain()).await }).is_err() {
        tracing::warn!("non-cancellable work still running after {SHUTDOWN_TIMEOUT:?}");
    }
    shutdown(rt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;

    #[test]
    fn spawn_io_runs_on_the_blocking_pool_and_spawn_net_on_a_worker() {
        let rt = build().expect("runtime");
        rt.block_on(async {
            // `Handle::block_on` is allowed on a blocking-pool thread and panics on a worker.
            let io = spawn_io(|| Handle::current().block_on(async { 7 })).await;
            assert_eq!(io.expect("blocking closure ran off the workers"), 7);
            let net = spawn_net(async { std::thread::current().name().map(str::to_owned) }).await.expect("net");
            assert_eq!(net.as_deref(), Some(THREAD_NAME));
        });
        shutdown(rt);
    }

    #[test]
    fn on_ui_runs_on_the_marked_main_thread_and_assert_rejects_others() {
        let (tx, rx) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        let (main_id_tx, main_id_rx) = mpsc::channel();
        let main = std::thread::spawn(move || {
            mark_main_thread();
            main_id_tx.send(std::thread::current().id()).unwrap();
            // Stand-in for the glib main loop: runs each posted closure on this thread.
            while let Ok(job) = rx.recv() {
                job();
            }
        });
        let main_id = main_id_rx.recv().unwrap();
        let tx = Mutex::new(tx);
        assert!(set_ui_poster(move |f| tx.lock().unwrap().send(f).unwrap()));
        assert!(!is_main_thread());

        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let off_thread_panics = std::panic::catch_unwind(assert_main_thread).is_err();
            on_ui(move || {
                assert_main_thread();
                done_tx.send((std::thread::current().id(), off_thread_panics)).unwrap();
            });
        });
        let (ran_on, off_thread_panics) = done_rx.recv_timeout(Duration::from_secs(5)).expect("on_ui ran");
        assert_eq!(ran_on, main_id);
        assert_eq!(off_thread_panics, cfg!(debug_assertions));
        drop(main); // the loop thread ends when the poster is dropped at process exit
    }

    #[test]
    fn shutdown_waits_for_a_non_cancellable_task() {
        let rt = build().expect("runtime");
        let tasks = NonCancellable::new(rt.handle().clone());
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = done.clone();
        // The clock starts before the spawn: the task's sleep begins at the spawn, not at the shutdown call.
        let start = Instant::now();
        tasks.spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        shutdown_draining(rt, &tasks);
        assert!(done.load(std::sync::atomic::Ordering::SeqCst));
        assert!(start.elapsed() >= Duration::from_millis(190));
    }

    #[test]
    fn spawned_task_completes_on_named_worker() {
        let rt = build().expect("runtime");
        let (value, thread) = rt.block_on(async {
            tokio::spawn(async { (6 * 7, std::thread::current().name().map(str::to_owned)) }).await.expect("join")
        });
        assert_eq!(value, 42);
        assert_eq!(thread.as_deref(), Some(THREAD_NAME));
        shutdown(rt);
    }

    #[test]
    fn worker_count_is_clamped() {
        assert!((2..=8).contains(&worker_threads()));
    }
}
