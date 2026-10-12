//! Which trackers are logged in (T-37a-056): the view menu's Tracked check, the tracker filters and the tracker-mean
//! sort only see a tracker the user is logged in to. A login is a non-empty username and password in the secret
//! store, the rule of `kuro_track::base`.
//!
//! The secret store can take seconds to answer (the first open probes it), so a read never runs on the GTK thread:
//! [`Logins::get`] returns the last known set at once and refreshes it on a worker thread, which calls `on_change`
//! on the main context when the set differs.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use kuro_domain::kind::EntryKind;
use kuro_platform::secrets::SecretStore;
use kuro_track::base::{Field, SECRET_SERVICE, account};
use kuro_track::model::TrackerId;

/// The trackers each library can show, in the order of Aniyomi's tracker manager.
const MANGA: [(TrackerId, &str); 9] = [
    (TrackerId::MYANIMELIST, "MyAnimeList"),
    (TrackerId::ANILIST, "AniList"),
    (TrackerId::KITSU, "Kitsu"),
    (TrackerId::MANGAUPDATES, "MangaUpdates"),
    (TrackerId::SHIKIMORI, "Shikimori"),
    (TrackerId::BANGUMI, "Bangumi"),
    (TrackerId::KOMGA, "Komga"),
    (TrackerId::KAVITA, "Kavita"),
    (TrackerId::SUWAYOMI, "Suwayomi"),
];
const ANIME: [(TrackerId, &str); 7] = [
    (TrackerId::MYANIMELIST, "MyAnimeList"),
    (TrackerId::ANILIST, "AniList"),
    (TrackerId::KITSU, "Kitsu"),
    (TrackerId::SHIKIMORI, "Shikimori"),
    (TrackerId::BANGUMI, "Bangumi"),
    (TrackerId::SIMKL, "Simkl"),
    (TrackerId::JELLYFIN, "Jellyfin"),
];

fn candidates(kind: EntryKind) -> &'static [(TrackerId, &'static str)] {
    match kind {
        EntryKind::Manga => &MANGA,
        EntryKind::Anime => &ANIME,
    }
}

fn read(secrets: &dyn SecretStore, tracker: TrackerId, field: Field) -> bool {
    secrets.get(SECRET_SERVICE, &account(tracker, field)).ok().flatten().is_some_and(|s| !s.is_empty())
}

/// The ids of the trackers of `kind` that are logged in. An unreadable store reads as logged out.
pub fn logged_in(secrets: &dyn SecretStore, kind: EntryKind) -> BTreeSet<i64> {
    candidates(kind)
        .iter()
        .filter(|(id, _)| read(secrets, *id, Field::Username) && read(secrets, *id, Field::Password))
        .map(|(id, _)| id.0)
        .collect()
}

/// The last known logins of one kind and the refresh that updates them.
pub struct Logins {
    kind: EntryKind,
    open: Arc<dyn Fn() -> Arc<dyn SecretStore> + Send + Sync>,
    known: Arc<Mutex<BTreeSet<i64>>>,
    refreshing: Arc<AtomicBool>,
    /// Runs on the GTK main context after the set changed.
    on_change: Arc<dyn Fn() + Send + Sync>,
}

impl Logins {
    /// `open` builds the secret store on the worker thread (it may block); `on_change` is called on the main context.
    pub fn new(
        kind: EntryKind,
        open: impl Fn() -> Arc<dyn SecretStore> + Send + Sync + 'static,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            open: Arc::new(open),
            known: Arc::default(),
            refreshing: Arc::default(),
            on_change: Arc::new(on_change),
        }
    }

    /// The last known set. Starts a refresh on a worker thread unless one is running.
    pub fn get(&self) -> BTreeSet<i64> {
        self.refresh();
        self.known.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    fn refresh(&self) {
        if self.refreshing.swap(true, Ordering::SeqCst) {
            return;
        }
        let (kind, open, known, refreshing, on_change) =
            (self.kind, self.open.clone(), self.known.clone(), self.refreshing.clone(), self.on_change.clone());
        let work = move || {
            let now = logged_in(open().as_ref(), kind);
            let changed = {
                let mut known = known.lock().unwrap_or_else(PoisonError::into_inner);
                let changed = *known != now;
                *known = now;
                changed
            };
            refreshing.store(false, Ordering::SeqCst);
            if changed {
                on_change();
            }
        };
        if std::thread::Builder::new().name("library-logins".into()).spawn(work).is_err() {
            self.refreshing.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::mpsc;
    use std::time::Duration;

    use kuro_platform::secrets::SecretError;

    use super::*;

    #[derive(Default)]
    struct Mem(Mutex<HashMap<String, String>>);

    impl Mem {
        fn login(&self, tracker: TrackerId) {
            for field in [Field::Username, Field::Password] {
                self.set(SECRET_SERVICE, &account(tracker, field), "x").unwrap();
            }
        }
    }

    impl SecretStore for Mem {
        fn get(&self, _service: &str, account: &str) -> Result<Option<String>, SecretError> {
            Ok(self.0.lock().unwrap().get(account).cloned())
        }
        fn set(&self, _service: &str, account: &str, secret: &str) -> Result<(), SecretError> {
            self.0.lock().unwrap().insert(account.into(), secret.into());
            Ok(())
        }
        fn delete(&self, _service: &str, account: &str) -> Result<(), SecretError> {
            self.0.lock().unwrap().remove(account);
            Ok(())
        }
    }

    #[test]
    fn a_tracker_needs_both_a_username_and_a_password_to_count() {
        let store = Mem::default();
        store.set(SECRET_SERVICE, &account(TrackerId::ANILIST, Field::Username), "me").unwrap();
        assert!(logged_in(&store, EntryKind::Manga).is_empty(), "a username alone is not a login");
        store.login(TrackerId::ANILIST);
        store.login(TrackerId::KITSU);
        assert_eq!(logged_in(&store, EntryKind::Manga), BTreeSet::from([2, 3]));
    }

    #[test]
    fn each_library_only_sees_its_own_trackers() {
        let store = Mem::default();
        store.login(TrackerId::KOMGA);
        store.login(TrackerId::SIMKL);
        assert_eq!(logged_in(&store, EntryKind::Manga), BTreeSet::from([6]));
        assert_eq!(logged_in(&store, EntryKind::Anime), BTreeSet::from([101]));
    }

    #[test]
    fn the_first_get_is_empty_and_the_refresh_reports_the_change_once() {
        let store = Arc::new(Mem::default());
        store.login(TrackerId::ANILIST);
        let (tx, rx) = mpsc::channel();
        let shared = store.clone();
        let tx = Mutex::new(tx);
        let logins = Logins::new(
            EntryKind::Manga,
            move || shared.clone() as Arc<dyn SecretStore>,
            move || {
                let _ = tx.lock().unwrap().send(());
            },
        );
        assert!(logins.get().is_empty(), "the secret store is never read on the caller's thread");
        rx.recv_timeout(Duration::from_secs(5)).expect("the change is announced");
        assert_eq!(logins.get(), BTreeSet::from([2]));
        // a second refresh finds the same set and announces nothing
        std::thread::sleep(Duration::from_millis(200));
        assert!(rx.try_recv().is_err());
    }
}
