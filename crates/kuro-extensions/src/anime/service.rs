//! The owner of the worker runtime (T-19a-059, `p1-rust-design.md` section 10). The main thread builds the runtime with
//! [`build_runtime`](crate::anime::runtime::build_runtime), keeps it alive, and gives [`AnimeExtensions`] its handle and
//! the sender of the event channel. Every background job of the area (scan, refresh, scheduled checks, install,
//! uninstall) is started through it, so all of them run on the two worker threads and report back as
//! [`Event`]s. The main thread feeds the receiver to [`pump`] and applies each event to its `ExtensionState`.
//! No job touches the state, and no method here waits for a job.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_channel::{Receiver, Sender};
use kuro_prefs::pref::Pref;
use kuro_prefs::source::SourcePrefs;
use tokio::runtime::{Handle, Runtime};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::anime::icons::IconCache;
use crate::anime::install::{InstallDirs, InstallError, Ticket, install};
use crate::anime::loader::PackageDir;
use crate::anime::manager::{Event, spawn_scan, spawn_uninstall};
use crate::anime::model::{Available, StoreRow};
use crate::anime::runtime::build_runtime;
use crate::anime::trust::TrustStore;
use crate::anime::update::{refresh_once, run_checks, unix_now};

/// The service handle of the anime extensions. It is `Send`, but the state it feeds lives on the main thread.
pub struct AnimeExtensions {
    runtime: Handle,
    events: Sender<Event>,
    /// One install at a time writes the package folder (section 6.2).
    placement: Arc<tokio::sync::Mutex<()>>,
    /// Counts the manual refreshes. A refresh whose number is no longer the latest drops its result (race R-03).
    refresh_generation: Arc<AtomicU64>,
}

impl AnimeExtensions {
    /// Builds the two-thread runtime, the event channel and the service over them. The caller keeps the returned
    /// `Runtime` alive for the life of the process and runs [`pump`] on the receiver in the main thread.
    pub fn start() -> std::io::Result<(Runtime, Self, Receiver<Event>)> {
        let runtime = build_runtime()?;
        let (events, inbox) = async_channel::unbounded();
        let service = Self::new(runtime.handle().clone(), events);
        Ok((runtime, service, inbox))
    }

    pub fn new(runtime: Handle, events: Sender<Event>) -> Self {
        Self {
            runtime,
            events,
            placement: Arc::new(tokio::sync::Mutex::new(())),
            refresh_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// The startup scan of the package folder, on the blocking pool (T-19a-024). The trust set is read by the worker.
    /// `show_nsfw_source` is read from `prefs` on every call, so a change between two scans changes the result
    /// (T-19a-007). Stale `*.kuroext.part` files in the package folder and in `download_dir` are deleted before the
    /// scan (T-19a-005). Package icons are extracted into `icons` after the scan (T-19a-046). Sends [`Event::Loaded`].
    pub fn scan(
        &self,
        dir: PackageDir,
        download_dir: PathBuf,
        trust: TrustStore,
        stores: Vec<StoreRow>,
        prefs: &SourcePrefs,
        icons: IconCache,
    ) -> JoinHandle<()> {
        let nsfw_allowed = prefs.show_nsfw_source.get();
        spawn_scan(
            &self.runtime,
            move || {
                dir.cleanup_stale(&download_dir);
                let mut results = dir.scan(&crate::anime::loader::LoadContext {
                    trust: &trust,
                    stores: &stores,
                    nsfw_allowed,
                });
                icons.fill(&dir, &mut results);
                results
            },
            self.events.clone(),
        )
    }

    /// Deletes a package's files (T-19a-036). Sends [`Event::Removed`] or [`Event::RemoveFailed`].
    pub fn uninstall(&self, package_dir: PathBuf, pkg: String) -> JoinHandle<()> {
        spawn_uninstall(&self.runtime, package_dir, pkg, self.events.clone())
    }

    /// One refresh of every store (T-19a-052). It sends [`Event::Refreshed`] and resolves to true when the list was
    /// sent. This is the job of the Refresh button.
    pub fn refresh(
        &self,
        client: reqwest::Client,
        index_urls: Vec<String>,
        cancel: CancellationToken,
    ) -> JoinHandle<bool> {
        let events = self.events.clone();
        let generation = self.refresh_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = Arc::clone(&self.refresh_generation);
        self.runtime.spawn(async move {
            let is_latest = move || latest.load(Ordering::SeqCst) == generation;
            refresh_once(&client, &index_urls, &cancel, &events, &is_latest).await
        })
    }

    /// The scheduled daily check (T-19a-052). `index_urls` is read at each check. It ends when `cancel` fires or the
    /// receiver is gone.
    pub fn schedule_checks(
        &self,
        client: reqwest::Client,
        index_urls: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
        last_check: Pref<i64>,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let events = self.events.clone();
        self.runtime.spawn(async move {
            run_checks(&client, &*index_urls, &last_check, &unix_now, &events, &cancel).await;
        })
    }

    /// Installs one listed package (T-19a-032). `ticket` comes from `InstallTracker::begin` on the main thread. Every
    /// step is sent as [`Event::Step`]; the last one is the outcome. After `Installed` the main thread rescans, so the
    /// loader checks trust for the new file.
    pub fn install(
        &self,
        client: reqwest::Client,
        ticket: Ticket,
        dirs: (PathBuf, PathBuf),
        available: Available,
        store_signing_key: String,
    ) -> JoinHandle<Result<PathBuf, InstallError>> {
        let events = self.events.clone();
        let placement = Arc::clone(&self.placement);
        self.runtime.spawn(async move {
            let (package_dir, download_dir) = dirs;
            let dirs = InstallDirs { package_dir: &package_dir, download_dir: &download_dir };
            let mut on_step = |step| {
                // The channel is unbounded, so this never waits. A dropped receiver only loses the step.
                let _ = events.try_send(Event::Step { ticket: ticket.clone(), step });
            };
            install(&client, dirs, &available, &store_signing_key, &placement, &ticket.cancel, &mut on_step).await
        })
    }
}

/// The receiver loop of the main thread: it applies each event in arrival order until every sender is gone. Spawn it
/// on the GTK main context, with `apply` calling `ExtensionState::apply`.
pub async fn pump(inbox: Receiver<Event>, mut apply: impl FnMut(Event)) {
    while let Ok(event) = inbox.recv().await {
        apply(event);
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use kuro_prefs::store::PrefStore;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::anime::install::Begin;
    use crate::anime::manager::ExtensionState;
    use crate::anime::model::{InstallStep, StoreRef};
    use crate::anime::store::map::available;
    use crate::anime::store::wire::ExtensionJson;
    use crate::anime::testkit::{
        MANIFEST, PKG, fixture_key, kuroext_block, signed_package, signer_hex, store_bytes, store_document,
        store_extension, test_client, trust_with_entries,
    };

    fn setup() -> (Runtime, AnimeExtensions, Receiver<Event>) {
        AnimeExtensions::start().expect("the runtime builds")
    }

    /// The events already sent, applied to `state` in arrival order.
    fn apply_pending(inbox: &Receiver<Event>, state: &mut ExtensionState) {
        while let Ok(event) = inbox.try_recv() {
            state.apply(event);
        }
    }

    #[test]
    fn tst_19_201_a_scan_runs_on_a_worker_thread_and_reports_loaded() {
        let (runtime, service, inbox) = setup();
        let root = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        std::fs::write(root.path().join(format!("{PKG}.kuroext")), &fixture.bytes).unwrap();
        std::fs::write(root.path().join(format!("{PKG}.signer")), signer_hex(&fixture_key())).unwrap();
        let handle = service.scan(
            PackageDir::new(root.path()),
            root.path().join("dl"),
            trust_with_entries(&[format!("{PKG}:7:{}", signer_hex(&fixture_key()))]),
            Vec::new(),
            &SourcePrefs::new(PrefStore::in_memory()),
            IconCache::new(root.path().join("ext-icons")),
        );
        runtime.block_on(handle).expect("the scan task ends");
        let mut state = ExtensionState::new();
        apply_pending(&inbox, &mut state);
        assert!(state.initialized());
        assert!(state.installed().contains_key(PKG), "{:?}", state.installed());

        // A job started through the service runs on one of the runtime's own threads.
        let (name_tx, name_rx) = std::sync::mpsc::channel();
        let handle = spawn_scan(
            runtime.handle(),
            move || {
                name_tx.send(thread::current().name().map(str::to_owned)).unwrap();
                Vec::new()
            },
            service.events.clone(),
        );
        runtime.block_on(handle).unwrap();
        assert_eq!(name_rx.recv().unwrap().as_deref(), Some("kuro-extension-worker"));
    }

    #[test]
    fn install_refresh_and_uninstall_run_on_the_runtime_and_reach_the_state_as_events() {
        let (runtime, service, inbox) = setup();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let server = runtime.block_on(MockServer::start());
        runtime.block_on(async {
            Mock::given(method("GET"))
                .and(path("/pkg.kuroext"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(fixture.bytes.clone()))
                .mount(&server)
                .await;
            Mock::given(method("GET"))
                .and(path("/index.json"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(store_bytes(&store_document(
                    "S",
                    &signer_hex(&fixture_key()),
                    vec![store_extension(PKG, 7, None)],
                ))))
                .mount(&server)
                .await;
        });
        let work = tempfile::tempdir().unwrap();
        let (package_dir, download_dir) = (work.path().join("anime"), work.path().join("dl"));
        let mut state = ExtensionState::new();

        // Install: the main thread takes the ticket, the runtime downloads and places, the steps arrive as events.
        let entry: ExtensionJson = serde_json::from_value(store_extension(
            PKG,
            7,
            Some(kuroext_block(&fixture, &format!("{}/pkg.kuroext", server.uri()))),
        ))
        .unwrap();
        let listed = available(
            &entry,
            &StoreRef { index_url: format!("{}/index.json", server.uri()), name: "S".into() },
        )
        .unwrap();
        let Begin::Started(ticket) = state.installs_mut().begin(PKG) else { panic!("a fresh install starts") };
        let job = service.install(
            test_client(),
            ticket,
            (package_dir.clone(), download_dir),
            listed,
            signer_hex(&fixture_key()),
        );
        let placed = runtime.block_on(job).expect("the install task ends").expect("the install succeeds");
        assert!(placed.exists());
        apply_pending(&inbox, &mut state);
        assert_eq!(state.installs().step(PKG), Some(InstallStep::Installed));

        // Refresh: the store list arrives as one event.
        let urls = vec![format!("{}/index.json", server.uri())];
        let refreshed = runtime.block_on(service.refresh(test_client(), urls, CancellationToken::new()));
        assert!(refreshed.unwrap());
        assert!(matches!(inbox.try_recv(), Ok(Event::Refreshed(aggregate)) if aggregate.available.len() == 1));

        // Uninstall: the files go and the event clears the step.
        runtime.block_on(service.uninstall(package_dir.clone(), PKG.to_owned())).unwrap();
        apply_pending(&inbox, &mut state);
        assert!(!placed.exists());
        assert_eq!(state.installs().step(PKG), None);
    }

    #[test]
    fn the_scheduled_check_refreshes_at_once_and_ends_on_cancel() {
        let (runtime, service, inbox) = setup();
        let server = runtime.block_on(MockServer::start());
        runtime.block_on(
            Mock::given(method("GET"))
                .and(path("/index.json"))
                .respond_with(ResponseTemplate::new(200).set_body_bytes(store_bytes(&store_document(
                    "S",
                    "k",
                    vec![store_extension("p.a", 1, None)],
                ))))
                .mount(&server),
        );
        let url = format!("{}/index.json", server.uri());
        let cancel = CancellationToken::new();
        let last = PrefStore::in_memory().pref("anime_ext_last_check", 0_i64);
        let job = service.schedule_checks(test_client(), Arc::new(move || vec![url.clone()]), last.clone(), cancel.clone());
        let event = runtime.block_on(inbox.recv()).expect("the first check runs at once");
        assert!(matches!(event, Event::Refreshed(_)));
        runtime.block_on(async {
            crate::anime::testkit::wait_until(|| last.is_set()).await;
        });
        cancel.cancel();
        runtime.block_on(job).expect("the check loop ends on cancel");
    }

    #[test]
    fn the_pump_applies_events_in_order_until_the_senders_are_gone() {
        let (runtime, service, inbox) = setup();
        let root = tempfile::tempdir().unwrap();
        runtime.block_on(service.uninstall(root.path().to_path_buf(), "../bad".into())).unwrap();
        runtime.block_on(service.uninstall(root.path().to_path_buf(), PKG.to_owned())).unwrap();
        drop(service);
        let mut seen = Vec::new();
        runtime.block_on(pump(inbox, |event| seen.push(event)));
        assert!(matches!(seen[0], Event::RemoveFailed { .. }));
        assert!(matches!(seen[1], Event::Removed(_)));
        assert_eq!(seen.len(), 2);
    }
}
