//! The shell object (T-36a-001): one per process on the GTK thread. It owns the tokio runtime handle that `main`
//! creates once, the queue of launch requests that arrive before the window exists, the home tab selection that
//! survives layout changes, and the download index the banners and the reader read.

use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use kuro_downloads::download_prefs::DownloadPrefs;
use kuro_downloads::index::DownloadIndex;
use kuro_platform::paths::Paths;
use kuro_prefs::ui::{NavStyle, StartScreen};

use super::argv::LaunchRequest;
use super::bootstrap::Bootstrap;
use super::lifecycle::Lifecycle;
use super::tabs::HomeTabs;

pub struct Shell {
    #[allow(dead_code)] // read through `runtime()` by the background tasks that start from the shell
    runtime: tokio::runtime::Handle,
    boot: RefCell<Bootstrap<LaunchRequest>>,
    home: RefCell<Option<Rc<RefCell<HomeTabs>>>>,
    downloads: DownloadIndex,
    /// The running manga download manager, once [`Shell::start_downloads`] has started it.
    downloads_started: RefCell<Option<crate::downloads::Started>>,
}

thread_local! {
    static SHELL: OnceCell<Rc<Shell>> = const { OnceCell::new() };
}

impl Shell {
    /// Creates the shell. Call once from `main` on the GTK thread, right after the runtime is built. A second call
    /// returns the first shell.
    pub fn install(runtime: tokio::runtime::Handle, paths: &dyn Paths) -> Rc<Shell> {
        SHELL.with(|cell| {
            cell.get_or_init(|| {
                let cache = paths.download_index_cache_dir();
                if let Err(e) = std::fs::create_dir_all(&cache) {
                    tracing::warn!("the download index snapshot folder was not created: {e}");
                }
                let downloads = DownloadIndex::open(paths.default_downloads_dir(), &cache);
                Rc::new(Shell {
                    runtime,
                    boot: RefCell::new(Bootstrap::new()),
                    home: RefCell::new(None),
                    downloads,
                    downloads_started: RefCell::new(None),
                })
            })
            .clone()
        })
    }

    /// The window's first step (Kotlin `BaseActivity.onCreate`): runs `apply_theme`, then records it, so the first
    /// widget is built under the theme. The returned [`Lifecycle`] refuses `build_widget` if the theme did not run.
    pub fn build(apply_theme: impl FnOnce()) -> Lifecycle {
        let mut life = Lifecycle::new();
        apply_theme();
        life.apply_theme();
        life
    }

    /// The installed shell, or `None` before [`Shell::install`] (unit tests).
    pub fn get() -> Option<Rc<Shell>> {
        SHELL.with(|cell| cell.get().cloned())
    }

    /// The runtime every background task of the app runs on.
    #[allow(dead_code)]
    pub fn runtime(&self) -> &tokio::runtime::Handle {
        &self.runtime
    }

    /// The download index, for the indexing banner and the downloaded-only filters.
    pub fn downloads(&self) -> &DownloadIndex {
        &self.downloads
    }

    /// A launch request arrived. `Some` means the window exists and the caller applies it now; `None` means it is
    /// queued until [`Shell::window_built`].
    pub fn submit(&self, request: LaunchRequest) -> Option<LaunchRequest> {
        self.boot.borrow_mut().request(request)
    }

    /// The window is built: the queued requests, in arrival order.
    pub fn window_built(&self) -> Vec<LaunchRequest> {
        self.boot.borrow_mut().window_built()
    }

    /// The migrations finished: the library page may leave its loading state (T-53a-069). The window is built only
    /// after the data opens, so `main` calls this when it starts building it.
    pub fn migration_done(&self) {
        self.boot.borrow_mut().migration_done();
    }

    /// What the library page shows now.
    #[allow(dead_code)]
    pub fn library_state(&self) -> super::bootstrap::LibraryState {
        self.boot.borrow().library_state()
    }

    /// Starts the manga download manager on the data the app has opened (call once, after the migrations). A failure is
    /// logged and the app runs without downloads. The first call wins.
    pub fn start_downloads(&self, paths: &dyn Paths, prefs: DownloadPrefs) {
        if self.downloads_started.borrow().is_some() {
            return;
        }
        let _entered = self.runtime.enter();
        match crate::downloads::start_manga(paths, self.downloads.clone(), prefs) {
            Ok(started) => *self.downloads_started.borrow_mut() = Some(started),
            Err(e) => tracing::error!("the manga download manager did not start: {e}"),
        }
    }

    /// The home tab selection. It is built once, the first time this runs, so a wide or narrow switch keeps it.
    pub fn home_tabs(&self, start: StartScreen, style: NavStyle) -> Rc<RefCell<HomeTabs>> {
        self.home.borrow_mut().get_or_insert_with(|| Rc::new(RefCell::new(HomeTabs::new(start, style)))).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::ui::Tab;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(1).build().unwrap()
    }

    fn shell(rt: &tokio::runtime::Runtime) -> Shell {
        let dir = tempfile::tempdir().unwrap();
        Shell {
            runtime: rt.handle().clone(),
            boot: RefCell::new(Bootstrap::new()),
            home: RefCell::new(None),
            downloads: DownloadIndex::open(dir.path(), dir.path()),
            downloads_started: RefCell::new(None),
        }
    }

    #[test]
    fn requests_before_the_window_wait_and_after_it_apply_at_once() {
        let rt = rt();
        let shell = shell(&rt);
        let search = LaunchRequest::OpenUri("a://b".into());
        assert_eq!(shell.submit(search.clone()), None);
        assert_eq!(shell.window_built(), vec![search]);
        let later = LaunchRequest::OpenUri("c://d".into());
        assert_eq!(shell.submit(later.clone()), Some(later));
    }

    #[test]
    fn the_library_loads_until_the_migrations_are_done() {
        let rt = rt();
        let shell = shell(&rt);
        assert_eq!(shell.library_state(), super::super::bootstrap::LibraryState::Loading);
        shell.migration_done();
        assert_eq!(shell.library_state(), super::super::bootstrap::LibraryState::Ready);
    }

    #[test]
    fn one_home_selection_serves_every_layout() {
        let rt = rt();
        let shell = shell(&rt);
        let first = shell.home_tabs(StartScreen::ANIME, NavStyle::MOVE_HISTORY_TO_MORE);
        first.borrow_mut().select(Tab::Browse);
        // a later layout asks again, even with other prefs: it gets the same selection
        let again = shell.home_tabs(StartScreen::MANGA, NavStyle::MOVE_BROWSE_TO_MORE);
        assert!(Rc::ptr_eq(&first, &again));
        assert_eq!(again.borrow().current(), Tab::Browse);
    }

    #[test]
    fn build_applies_the_theme_before_a_widget_may_be_built() {
        let ran = std::cell::Cell::new(false);
        let life = Shell::build(|| ran.set(true));
        assert!(ran.get());
        assert_eq!(life.build_widget(), Ok(()));
    }

    #[test]
    fn the_runtime_handle_runs_tasks() {
        let rt = rt();
        let shell = shell(&rt);
        assert_eq!(shell.runtime().block_on(async { 7 }), 7);
    }
}
