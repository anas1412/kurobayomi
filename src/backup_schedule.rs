//! The automatic backup schedule of the running app (T-02a-024). The loop of `kuro_backup::auto::run_loop` runs on the
//! tokio runtime; this module feeds it the interval. A change of `backup_interval` reaches the loop on the main loop,
//! which wakes it at once.

use std::cell::Cell;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Local;
use kuro_backup::auto::{AutoConfig, AutoHost, PowerState, run_loop};
use kuro_backup::error::BackupError;
use kuro_backup::settings::{automatic_dir_for, enabled_at_on_change, normalize_interval};
use kuro_platform::paths::Paths as _;
use kuro_prefs::backup::{BackupPrefs, StoragePrefs};
use tokio::sync::watch;

use crate::backup_run::{CliHost, ENABLED_AT_KEY, Library, power_state};

/// [`CliHost`] with the power state read fresh on every tick.
struct GuiHost(CliHost);

impl AutoHost for GuiHost {
    fn now_ms(&self) -> i64 {
        self.0.now_ms()
    }
    fn last_auto_ms(&self) -> i64 {
        self.0.last_auto_ms()
    }
    fn restore_running(&self) -> bool {
        // the restore state belongs to the GTK thread and this runs on a worker; a backup only reads the databases, which
        // SQLite lets run beside the restore's own connection
        false
    }
    fn power(&self) -> PowerState {
        power_state()
    }
    fn run_backup(&self) -> Result<PathBuf, BackupError> {
        self.0.run_backup()
    }
    fn save_last_auto_ms(&self, ms: i64) {
        self.0.save_last_auto_ms(ms);
    }
    fn report_failure(&self, streak: u32) {
        tracing::warn!("the automatic backup failed {streak} time(s) in a row");
    }
}

/// Sends the scheduler the config for the stored `interval`. Turning the schedule on from Off stamps the moment it
/// was enabled, which the first run counts one interval from. `seen` is the interval the scheduler ran on before.
fn reschedule(tx: &watch::Sender<AutoConfig>, host: &CliHost, seen: &Cell<u32>, stored: i32) {
    let now = host.now_ms();
    let interval = normalize_interval(i64::from(stored));
    if let Some(at) = enabled_at_on_change(seen.get(), interval, now) {
        if let Err(e) = host.library.prefs.pref(ENABLED_AT_KEY.to_owned(), 0_i64).set(at) {
            tracing::warn!("could not store when the automatic backup was turned on: {e}");
        }
    }
    seen.set(interval);
    tx.send_replace(host.config());
}

/// Starts the loop over the open databases and follows `backup_interval` from now on. Call once, on the GTK thread,
/// inside the runtime.
pub fn start(app: &gtk::Application) {
    let Ok(paths) = kuro_platform::linux::LinuxPaths::system() else { return };
    let state = crate::data::app();
    let store = state.base.store().clone();
    let (backup, storage) = (BackupPrefs::new(store.clone()), StoragePrefs::new(store.clone()));
    let base = crate::settings_ui::storage::decode(&storage.base_storage_directory.get());
    let library = Library { prefs: store, manga: state.manga_db.clone(), anime: state.anime_db.clone(), extensions_dir: paths.data_dir().join("extensions") };
    let host = CliHost {
        library,
        auto_dir: automatic_dir_for(base.as_deref(), &paths),
        now_ms: Box::new(|| Local::now().timestamp_millis()),
        power: power_state(),
    };
    let (tx, rx) = watch::channel(host.config());
    let seen = Cell::new(normalize_interval(i64::from(backup.backup_interval.get())));
    let host = Arc::new(GuiHost(host));
    let notifier = Arc::new(crate::gio_notifier::GioNotifier::new(app, Vec::new()));
    tokio::spawn(run_loop(rx, crate::pref_model::backup_pages::job_slot().clone(), Arc::clone(&host), notifier));
    crate::pref_model::bind::follow(&backup.backup_interval, || true, move |stored| reschedule(&tx, &host.0, &seen, stored));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_db::handle::Db;
    use kuro_db::{schema_anime, schema_manga};
    use kuro_prefs::store::PrefStore;

    use super::*;

    fn host(dir: &std::path::Path, now: i64) -> CliHost {
        let (manga_path, anime_path) = (dir.join("manga.db"), dir.join("anime.db"));
        let manga = Arc::new(Db::new(&manga_path, schema_manga::open(&manga_path).unwrap()).unwrap());
        let anime = Arc::new(Db::new(&anime_path, schema_anime::open(&anime_path).unwrap()).unwrap());
        CliHost {
            library: Library { prefs: PrefStore::in_memory(), manga, anime, extensions_dir: dir.join("extensions") },
            auto_dir: dir.join("autobackup"),
            now_ms: Box::new(move || now),
            power: PowerState { on_battery: false, percent: None },
        }
    }

    #[test]
    fn a_new_interval_reaches_the_scheduler_at_once_and_turning_it_on_stamps_the_moment() {
        let dir = tempfile::tempdir().unwrap();
        let host = host(dir.path(), 5_000);
        let backup = BackupPrefs::new(host.library.prefs.clone());
        backup.backup_interval.set(0).unwrap();
        let (tx, mut rx) = watch::channel(host.config());
        assert_eq!(rx.borrow().interval_hours, 0);
        let seen = Cell::new(0);
        backup.backup_interval.set(24).unwrap();
        reschedule(&tx, &host, &seen, 24);
        assert!(rx.has_changed().unwrap());
        let cfg = rx.borrow_and_update().clone();
        assert_eq!((cfg.interval_hours, cfg.enabled_at_epoch_ms), (24, 5_000));
        // a change from one interval to another keeps the first enabled moment
        backup.backup_interval.set(48).unwrap();
        reschedule(&tx, &host, &seen, 48);
        let cfg = rx.borrow_and_update().clone();
        assert_eq!((cfg.interval_hours, cfg.enabled_at_epoch_ms), (48, 5_000));
    }

    #[test]
    fn a_stored_value_outside_the_list_schedules_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let host = host(dir.path(), 1);
        let (tx, rx) = watch::channel(host.config());
        reschedule(&tx, &host, &Cell::new(12), 7);
        assert_eq!(rx.borrow().interval_hours, 12);
    }
}
