//! Tracker registry (`TrackerManager.kt`): the fixed set of trackers in registry order, the logged-in
//! view, and enhanced-tracker discovery.

use std::sync::Arc;

use tokio::sync::watch;

use crate::error::TrackError;
use crate::model::TrackerId;
use crate::tracker::Tracker;

/// The eleven tracker ids in registry order (`TrackerManager.kt:39-42`).
pub const REGISTRY_ORDER: [TrackerId; 11] = [
    TrackerId::MYANIMELIST,
    TrackerId::ANILIST,
    TrackerId::KITSU,
    TrackerId::SHIKIMORI,
    TrackerId::BANGUMI,
    TrackerId::KOMGA,
    TrackerId::MANGAUPDATES,
    TrackerId::KAVITA,
    TrackerId::SUWAYOMI,
    TrackerId::SIMKL,
    TrackerId::JELLYFIN,
];

/// The registry. Holds exactly the trackers of [`REGISTRY_ORDER`].
pub struct TrackerManager {
    trackers: Vec<Arc<dyn Tracker>>,
    logged_in: watch::Sender<Vec<TrackerId>>,
    /// Source identities `"{name}/{lang}"` installed now. The app publishes them with
    /// [`TrackerManager::set_installed_sources`] (T-27a-077).
    installed: watch::Sender<Vec<String>>,
}

/// One enhanced tracker and whether an installed source serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnhancedDiscovery {
    /// Trackers with an installed accepted source, in registry order.
    pub available: Vec<TrackerId>,
    /// Names of the other enhanced trackers, in registry order.
    pub not_installed: Vec<String>,
}

impl TrackerManager {
    /// Builds the registry. Fails unless `trackers` holds each id of [`REGISTRY_ORDER`] exactly once.
    /// The trackers are sorted into registry order. Starts a watcher task per tracker, so it must run
    /// inside a tokio runtime.
    pub fn new(mut trackers: Vec<Arc<dyn Tracker>>) -> Result<Self, TrackError> {
        let position = |t: &Arc<dyn Tracker>| REGISTRY_ORDER.iter().position(|id| *id == t.id());
        let mut slots: Vec<usize> = trackers.iter().filter_map(position).collect();
        slots.sort_unstable();
        let exact = trackers.len() == REGISTRY_ORDER.len()
            && slots == (0..REGISTRY_ORDER.len()).collect::<Vec<_>>();
        if !exact {
            return Err(TrackError::Message(
                "the tracker registry needs each of the 11 tracker ids once".into(),
            ));
        }
        trackers.sort_by_key(|t| position(t));

        let shared = Arc::new(trackers.clone());
        let (logged_in, _) = watch::channel(logged_in_ids(&shared));
        for tracker in &trackers {
            let mut rx = tracker.login_state();
            let tx = logged_in.clone();
            let all = Arc::clone(&shared);
            tokio::spawn(async move {
                while rx.changed().await.is_ok() {
                    tx.send_if_modified(|list| {
                        let next = logged_in_ids(&all);
                        if *list == next {
                            false
                        } else {
                            *list = next;
                            true
                        }
                    });
                }
            });
        }
        Ok(Self {
            trackers,
            logged_in,
            installed: watch::channel(Vec::new()).0,
        })
    }

    /// Every tracker in registry order.
    pub fn all(&self) -> &[Arc<dyn Tracker>] {
        &self.trackers
    }

    /// The tracker with `id`, or `None` for an unknown id.
    pub fn get(&self, id: TrackerId) -> Option<Arc<dyn Tracker>> {
        self.trackers.iter().find(|t| t.id() == id).cloned()
    }

    /// The trackers that are logged in, in registry order.
    pub fn logged_in(&self) -> Vec<Arc<dyn Tracker>> {
        self.trackers
            .iter()
            .filter(|t| t.is_logged_in())
            .cloned()
            .collect()
    }

    /// The ids of the logged-in trackers. Emits a new list after each login or logout.
    pub fn logged_in_stream(&self) -> watch::Receiver<Vec<TrackerId>> {
        self.logged_in.subscribe()
    }

    /// Publishes the installed source identities. Subscribers of [`TrackerManager::installed_stream`] are told only when
    /// the list changes, so installing a source that an enhanced tracker accepts adds its row without a restart (T-27a-077).
    pub fn set_installed_sources(&self, ids: &[&str]) {
        self.installed.send_if_modified(|list| {
            let next: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
            if *list == next {
                false
            } else {
                *list = next;
                true
            }
        });
    }

    /// The installed source identities. Emits a new list after each change.
    pub fn installed_stream(&self) -> watch::Receiver<Vec<String>> {
        self.installed.subscribe()
    }

    /// [`TrackerManager::discover_enhanced`] over the sources last published with
    /// [`TrackerManager::set_installed_sources`].
    pub fn discover_installed(&self) -> EnhancedDiscovery {
        let ids = self.installed.borrow().clone();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        self.discover_enhanced(&refs)
    }

    /// Enhanced anime and manga trackers, matched against the qualified ids of installed sources.
    pub fn discover_enhanced(&self, installed_source_ids: &[&str]) -> EnhancedDiscovery {
        let mut available = Vec::new();
        let mut not_installed = Vec::new();
        for t in &self.trackers {
            let accepts = |src: &&str| {
                t.as_enhanced_anime().is_some_and(|e| e.accepts(src))
                    || t.as_enhanced_manga().is_some_and(|e| e.accepts(src))
            };
            let is_enhanced = t.as_enhanced_anime().is_some() || t.as_enhanced_manga().is_some();
            if !is_enhanced {
                continue;
            }
            if installed_source_ids.iter().any(accepts) {
                available.push(t.id());
            } else {
                not_installed.push(t.name().to_owned());
            }
        }
        EnhancedDiscovery {
            available,
            not_installed,
        }
    }
}

/// The info text of the enhanced group: the base sentence, then the names of the trackers that need a
/// source not installed, after two newlines. The names are joined with ", ".
pub fn not_installed_text(base: &str, names: &[String]) -> String {
    if names.is_empty() {
        base.to_owned()
    } else {
        format!("{base}\n\n{}", names.join(", "))
    }
}

fn logged_in_ids(trackers: &[Arc<dyn Tracker>]) -> Vec<TrackerId> {
    trackers
        .iter()
        .filter(|t| t.is_logged_in())
        .map(|t| t.id())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::fakes::{FakeAnime, FakeManga, MemSecrets, new_log};

    /// The eleven trackers of the registry, with the kind each one implements.
    fn registry(secrets: &Arc<MemSecrets>) -> Vec<Arc<dyn Tracker>> {
        let log = new_log();
        let anime_ids = [
            TrackerId::KITSU,
            TrackerId::SIMKL,
            TrackerId::JELLYFIN,
            TrackerId::ANILIST,
        ];
        REGISTRY_ORDER
            .iter()
            .rev()
            .map(|id| -> Arc<dyn Tracker> {
                if anime_ids.contains(id) {
                    Arc::new(FakeAnime::new(*id, "anime", secrets.clone(), log.clone()))
                } else {
                    Arc::new(FakeManga::new(*id, "manga", secrets.clone(), log.clone()))
                }
            })
            .collect()
    }

    #[tokio::test]
    async fn tr_27_002_registry_holds_the_eleven_ids_in_registry_order() {
        let secrets = MemSecrets::new();
        let m = TrackerManager::new(registry(&secrets)).unwrap();
        let ids: Vec<i64> = m.all().iter().map(|t| t.id().0).collect();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 101, 102]);
        assert!(m.get(TrackerId(999)).is_none());
        assert_eq!(m.get(TrackerId::ANILIST).unwrap().id(), TrackerId::ANILIST);
    }

    #[tokio::test]
    async fn tr_27_002_registry_rejects_a_wrong_set_of_trackers() {
        let secrets = MemSecrets::new();
        let mut list = registry(&secrets);
        list.pop();
        assert!(TrackerManager::new(list).is_err());
        let mut dup = registry(&secrets);
        dup[0] = dup[1].clone();
        assert!(TrackerManager::new(dup).is_err());
    }

    #[tokio::test]
    async fn tr_27_002_logged_in_keeps_registry_order_and_stream_follows_changes() {
        let secrets = MemSecrets::new();
        let m = TrackerManager::new(registry(&secrets)).unwrap();
        let mut stream = m.logged_in_stream();
        assert!(m.logged_in().is_empty());

        let kitsu = m.get(TrackerId::KITSU).unwrap();
        let anilist = m.get(TrackerId::ANILIST).unwrap();
        anilist.save_credentials("a", "b").unwrap();
        kitsu.save_credentials("k", "b").unwrap();
        let ids: Vec<TrackerId> = m.logged_in().iter().map(|t| t.id()).collect();
        assert_eq!(ids, vec![TrackerId::ANILIST, TrackerId::KITSU]);

        tokio::time::timeout(Duration::from_secs(2), stream.changed())
            .await
            .unwrap()
            .unwrap();
        assert!(stream.borrow_and_update().contains(&TrackerId::KITSU));

        kitsu.logout().unwrap();
        tokio::time::timeout(Duration::from_secs(2), stream.changed())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*stream.borrow_and_update(), vec![TrackerId::ANILIST]);
    }

    #[tokio::test]
    async fn tr_27_024_enhanced_discovery_and_info_text() {
        let secrets = MemSecrets::new();
        let log = new_log();
        let mut list = registry(&secrets);
        // Jellyfin (102) and Kavita (8) are enhanced; only Jellyfin's source is installed.
        let mut jelly = FakeAnime::new(
            TrackerId::JELLYFIN,
            "Jellyfin",
            secrets.clone(),
            log.clone(),
        );
        jelly.enhanced = true;
        jelly.accepted = &["jf.source"];
        let mut kavita = FakeManga::new(TrackerId::KAVITA, "Kavita", secrets.clone(), log.clone());
        kavita.enhanced = true;
        kavita.accepted = &["kv.source"];
        list.retain(|t| t.id() != TrackerId::JELLYFIN && t.id() != TrackerId::KAVITA);
        list.push(Arc::new(jelly));
        list.push(Arc::new(kavita));
        let m = TrackerManager::new(list).unwrap();

        let d = m.discover_enhanced(&["jf.source"]);
        assert_eq!(d.available, vec![TrackerId::JELLYFIN]);
        assert_eq!(d.not_installed, vec!["Kavita".to_string()]);

        let text = not_installed_text("Install a source:", &d.not_installed);
        assert_eq!(text, "Install a source:\n\nKavita");
        assert_eq!(not_installed_text("Base", &[]), "Base");
        assert_eq!(
            not_installed_text("Base", &["A".into(), "B".into()]),
            "Base\n\nA, B"
        );
    }

    #[tokio::test]
    async fn t_27a_077_installing_an_accepted_source_adds_its_row_without_a_restart() {
        let secrets = MemSecrets::new();
        let log = new_log();
        let mut list = registry(&secrets);
        let mut jelly = FakeAnime::new(TrackerId::JELLYFIN, "Jellyfin", secrets.clone(), log.clone());
        jelly.enhanced = true;
        jelly.accepted = &["jf.source"];
        list.retain(|t| t.id() != TrackerId::JELLYFIN);
        list.push(Arc::new(jelly));
        let m = TrackerManager::new(list).unwrap();
        let mut stream = m.installed_stream();

        assert!(m.discover_installed().available.is_empty());
        m.set_installed_sources(&["jf.source"]);
        stream.changed().await.unwrap();
        assert_eq!(m.discover_installed().available, vec![TrackerId::JELLYFIN]);
        // The same list again is not a change.
        m.set_installed_sources(&["jf.source"]);
        assert!(!stream.has_changed().unwrap());
    }
}

#[cfg(test)]
mod mal_handle_tests {
    use std::sync::Arc;

    use super::*;
    use crate::fakes::{FakeManga, MemSecrets, new_log};
    use crate::mal::store::{SecretTokenStore, TokenStore};
    use crate::mal::testkit::fresh_token;
    use crate::mal::tracker::MalTracker;

    #[tokio::test]
    async fn t_29a_059_every_reader_of_the_registry_shares_one_mal_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let secrets = MemSecrets::new();
        let tokens: Arc<dyn TokenStore> = Arc::new(SecretTokenStore::new(secrets.clone()));
        let mal =
            Arc::new(MalTracker::new(dir.path(), tokens, Some("cid".into())).expect("tracker"));
        let mut trackers: Vec<Arc<dyn Tracker>> = vec![mal.clone()];
        for id in REGISTRY_ORDER
            .into_iter()
            .filter(|id| *id != TrackerId::MYANIMELIST)
        {
            trackers.push(Arc::new(FakeManga::new(
                id,
                "Fake",
                secrets.clone(),
                new_log(),
            )));
        }
        let manager = TrackerManager::new(trackers).expect("registry");

        let first = manager
            .get(TrackerId::MYANIMELIST)
            .expect("MAL is registered");
        let second = manager
            .get(TrackerId::MYANIMELIST)
            .expect("MAL is registered");
        assert!(Arc::ptr_eq(&first, &second), "the same handle every time");

        let mut logged_in = manager.logged_in_stream();
        mal.settings_for_test()
            .set_username("ann")
            .expect("username");
        mal.auth().set_token(fresh_token("a")).expect("token");
        mal.sync_login_state();
        logged_in.changed().await.expect("a login is announced");
        assert_eq!(*logged_in.borrow(), vec![TrackerId::MYANIMELIST]);
        assert!(first.is_logged_in(), "the handle reads the same state");

        mal.logout().expect("logout");
        logged_in.changed().await.expect("a logout is announced");
        assert!(logged_in.borrow().is_empty());
        assert!(!second.is_logged_in());
    }
}
