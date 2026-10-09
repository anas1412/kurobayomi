//! Operations over the store table (T-09a-031 to T-09a-034): add, add from a preference entry, refresh, the parallel
//! extension fetch, and the live lists. The repository is `kuro-data`'s `ExtensionStoreRepo`.

use std::sync::Arc;
use std::time::Duration;

use kuro_data::extension_store::ExtensionStoreRepo;
use kuro_db::handle::Db;
use kuro_db::statements_anime::{extension_store_count, extension_stores};
use kuro_db::subscribe::Subscription;

use super::fetch::{StoreHttp, fetch_store};
use super::model::{AnimeExtensionStore, NO_SIGNING_KEY};
use crate::error::DomainError;

/// Quiet time before a live store query re-runs after a write.
const STORE_DEBOUNCE: Duration = Duration::from_millis(50);
/// Tables a store listing reads.
const STORE_TABLES: &[&str] = &["extension_store"];

/// `AddAnimeExtensionStore`: fetches the store at `index_url` and saves it under that key. A failed fetch saves nothing.
pub fn add_store(
    repo: &ExtensionStoreRepo,
    http: &dyn StoreHttp,
    index_url: &str,
) -> Result<(), DomainError> {
    let store = fetch_store(http, index_url)?;
    repo.add(&(&store).into())?;
    Ok(())
}

/// `insert_from_legacy`: a store from an old preference entry, with placeholder metadata and no network call
/// (TC-STO-07). A row that already has a real signing key is kept as it is (E-STO-08).
pub fn insert_from_legacy(
    repo: &ExtensionStoreRepo,
    index_url: &str,
    name: &str,
) -> Result<(), DomainError> {
    if let Some(existing) = repo.get(index_url)? {
        if existing.signing_key != NO_SIGNING_KEY {
            return Ok(());
        }
    }
    repo.insert_from_legacy(index_url, name)?;
    Ok(())
}

/// `refresh_all`: re-fetches every store one after another. A failing store keeps its row and logs
/// `Failed to refresh extension store '<name> (<index_url>)'`, and the loop goes on (TC-STO-05). A store removed during
/// its own refresh is not written back (E-STO-09). An empty table is `Ok` with no call.
pub fn refresh_all(repo: &ExtensionStoreRepo, http: &dyn StoreHttp) -> Result<(), DomainError> {
    for old in repo.all()? {
        let fetched = match fetch_store(http, &old.index_url) {
            Ok(store) => store,
            Err(e) => {
                tracing::error!(error = %e, "Failed to refresh extension store '{} ({})'", old.name, old.index_url);
                continue;
            }
        };
        if fetched.signing_key != old.signing_key {
            tracing::warn!(store = %old.index_url, "extension store signing key changed");
        }
        if repo.get(&old.index_url)?.is_some() {
            repo.add(&(&fetched).into())?;
        }
    }
    Ok(())
}

/// Every store's extensions, fetched in parallel: one thread per store. A store whose fetch fails or panics contributes
/// nothing and is logged; the others still count (TC-STO-06). With no stores, `fetch` is never called (E-STO-06).
/// The fetch is the caller's, because the extension-list shape belongs to area 19 (see [`super::list_source`]).
pub fn fetch_extensions<T, E, F>(repo: &ExtensionStoreRepo, fetch: F) -> Vec<T>
where
    T: Send,
    E: std::fmt::Display + Send,
    F: Fn(&AnimeExtensionStore) -> Result<Vec<T>, E> + Sync,
{
    let stores: Vec<AnimeExtensionStore> = match repo.all() {
        Ok(rows) => rows.into_iter().map(Into::into).collect(),
        Err(e) => {
            tracing::error!(error = %e, "cannot list extension stores");
            return Vec::new();
        }
    };
    if stores.is_empty() {
        return Vec::new();
    }
    // ponytail: fixed batches of eight stores, not a rolling window; the slowest store holds up its batch.
    const FETCH_CONCURRENCY: usize = 8;
    stores
        .chunks(FETCH_CONCURRENCY)
        .flat_map(|batch| std::thread::scope(|scope| {
        let handles: Vec<_> = batch
            .iter()
            .map(|store| (store, scope.spawn(|| fetch(store))))
            .collect();
        handles
            .into_iter()
            .flat_map(|(store, handle)| match handle.join() {
                Ok(Ok(items)) => items,
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "Failed to get extensions of store '{}'", store.index_url);
                    Vec::new()
                }
                Err(_) => {
                    tracing::error!("fetching the extensions of store '{}' panicked", store.index_url);
                    Vec::new()
                }
            })
            .collect::<Vec<T>>()
    }))
        .collect()
}

/// Live list of stores, as domain models, re-emitted after each write to the table (`subscribe_all`).
pub fn subscribe_all(db: &Arc<Db>) -> Subscription<Vec<AnimeExtensionStore>> {
    db.subscribe(STORE_TABLES, STORE_DEBOUNCE, |c| {
        Ok(extension_stores(c)?
            .into_iter()
            .map(AnimeExtensionStore::from)
            .collect::<Vec<_>>())
    })
}

/// Live count of stores (`count`).
pub fn subscribe_count(db: &Arc<Db>) -> Subscription<i64> {
    db.subscribe(STORE_TABLES, STORE_DEBOUNCE, |c| {
        Ok(extension_store_count(c)?)
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use kuro_data::extension_store::ExtensionStoreRepo;
    use kuro_db::statements_anime::ExtensionStore as Row;

    use super::*;
    use crate::extension_store::fetch::tests::FakeHttp;
    use crate::testdb::anime_db;

    const CURRENT: &[u8] = include_bytes!("../../tests/fixtures/store_current.json");
    const LEGACY_LIST: &[u8] = include_bytes!("../../tests/fixtures/store_legacy_list.json");

    fn repo() -> (tempfile::TempDir, Arc<Db>, ExtensionStoreRepo) {
        let (dir, db) = anime_db();
        (dir, Arc::clone(&db), ExtensionStoreRepo::new(db))
    }

    fn row(url: &str, name: &str, key: &str) -> Row {
        Row {
            index_url: url.into(),
            name: name.into(),
            badge_label: "B".into(),
            signing_key: key.into(),
            contact_website: "https://site.test".into(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
            resolved_index_url: url.into(),
        }
    }

    fn legacy_repo_body() -> &'static [u8] {
        br#"{"meta":{"name":"Legacy Store","shortName":"LS","website":"https://legacy.test","signingKeyFingerprint":"ff00ee11"}}"#
    }

    #[test]
    fn adding_a_legacy_index_keys_one_row_by_the_entered_url() {
        let (_dir, _db, repo) = repo();
        let http = FakeHttp::default()
            .route("https://s.test/index.min.json", LEGACY_LIST)
            .route("https://s.test/repo.json", legacy_repo_body());
        add_store(&repo, &http, "https://s.test/index.min.json").expect("add");
        add_store(&repo, &http, "https://s.test/index.min.json").expect("add again");
        assert_eq!(
            repo.count().expect("count"),
            1,
            "Q-STORE-1: no second row under repo.json"
        );
        assert!(
            repo.get("https://s.test/index.min.json")
                .expect("get")
                .is_some()
        );
        assert!(repo.get("https://s.test/repo.json").expect("get").is_none());
    }

    #[test]
    fn a_followed_store_is_saved_with_the_document_its_list_comes_from() {
        use crate::extension_store::fetch::{ListSource, list_source};

        let (_dir, _db, repo) = repo();
        let pointer = br#"{"index_v2":"https://v2.test/index.min.json","meta":{"name":"A","website":"w","signingKeyFingerprint":"k"}}"#;
        let followed = br#"{"name":"Followed","badgeLabel":"F","signingKey":"k","contact":{"website":"https://f.test"}}"#;
        let http = FakeHttp::default()
            .route("https://s.test/a.json", pointer)
            .route("https://v2.test/index.min.json", followed);
        add_store(&repo, &http, "https://s.test/a.json").expect("add");
        let stored = AnimeExtensionStore::from(
            repo.get("https://s.test/a.json")
                .expect("get")
                .expect("row"),
        );
        assert_eq!(
            stored.index_url, "https://s.test/a.json",
            "Q-STORE-1: one row under the entered url"
        );
        assert_eq!(repo.count().expect("count"), 1);
        assert_eq!(stored.resolved_index_url, "https://v2.test/index.min.json");
        assert_eq!(
            list_source(&stored),
            ListSource::Index("https://v2.test/index.min.json".into())
        );
    }

    #[test]
    fn a_failed_add_saves_nothing() {
        let (_dir, _db, repo) = repo();
        let http = FakeHttp::default().fail("https://down.test/index.min.json");
        assert!(add_store(&repo, &http, "https://down.test/index.min.json").is_err());
        assert_eq!(repo.count().expect("count"), 0);
    }

    #[test]
    fn a_refresh_keeps_going_past_a_failing_store_and_logs_its_name_and_url() {
        let (_dir, _db, repo) = repo();
        repo.add(&row("https://bad.test/index.min.json", "Bad", "k"))
            .expect("bad");
        repo.add(&row("https://good.test/index.min.json", "Good", "old"))
            .expect("good");
        let http = FakeHttp::default()
            .fail("https://bad.test/index.min.json")
            .route("https://good.test/index.min.json", CURRENT);

        let log = Arc::new(Mutex::new(Vec::<u8>::new()));
        let subscriber = tracing_subscriber::fmt()
            .with_writer(CapturedLog(Arc::clone(&log)))
            .with_ansi(false)
            .without_time()
            .finish();
        tracing::subscriber::with_default(subscriber, || refresh_all(&repo, &http))
            .expect("refresh");

        let calls = http.calls();
        assert!(calls.contains(&"https://bad.test/index.min.json".to_owned()));
        assert!(
            calls.contains(&"https://good.test/index.min.json".to_owned()),
            "both attempted"
        );
        assert_eq!(
            repo.get("https://bad.test/index.min.json")
                .expect("get")
                .expect("kept")
                .name,
            "Bad"
        );
        assert_eq!(
            repo.get("https://good.test/index.min.json")
                .expect("get")
                .expect("row")
                .signing_key,
            "a1b2c3d4"
        );
        let text = String::from_utf8(log.lock().unwrap().clone()).expect("utf8");
        assert!(
            text.contains(
                "Failed to refresh extension store 'Bad (https://bad.test/index.min.json)'"
            ),
            "log was: {text}"
        );
    }

    #[test]
    fn a_store_removed_during_its_refresh_is_not_written_back() {
        struct RemovingHttp {
            repo: ExtensionStoreRepo,
        }
        impl StoreHttp for RemovingHttp {
            fn get(&self, _url: &str) -> Result<Vec<u8>, String> {
                self.repo
                    .remove("https://gone.test/index.min.json")
                    .expect("remove");
                Ok(CURRENT.to_vec())
            }
        }
        let (_dir, _db, repo) = repo();
        repo.add(&row("https://gone.test/index.min.json", "Gone", "k"))
            .expect("add");
        refresh_all(&repo, &RemovingHttp { repo: repo.clone() }).expect("refresh");
        assert_eq!(
            repo.count().expect("count"),
            0,
            "E-STO-09: the refresh does not re-create the row"
        );
    }

    #[test]
    fn a_preference_entry_gets_placeholders_and_does_not_overwrite_a_real_key() {
        let (_dir, _db, repo) = repo();
        insert_from_legacy(&repo, "https://p.test/index.min.json", "Pref").expect("insert");
        let stored = repo
            .get("https://p.test/index.min.json")
            .expect("get")
            .expect("row");
        assert_eq!(stored.badge_label, "Pref");
        assert_eq!(stored.signing_key, NO_SIGNING_KEY);
        assert_eq!(stored.contact_website, "https://p.test/index.min.json");
        assert_eq!(stored.contact_discord, None);
        assert!(!stored.is_legacy);
        assert_eq!(stored.extension_list_url, None);

        repo.add(&row("https://real.test/index.min.json", "Real", "realkey"))
            .expect("real");
        insert_from_legacy(&repo, "https://real.test/index.min.json", "Other").expect("no-op");
        let kept = repo
            .get("https://real.test/index.min.json")
            .expect("get")
            .expect("row");
        assert_eq!(kept.signing_key, "realkey");
        assert_eq!(kept.name, "Real");
    }

    #[test]
    fn extensions_are_the_union_of_the_good_stores_and_no_store_means_no_call() {
        let (_dir, _db, repo) = repo();
        let calls = AtomicUsize::new(0);
        assert!(
            fetch_extensions(&repo, |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok::<Vec<u8>, String>(vec![1])
            })
            .is_empty()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0, "E-STO-06");

        repo.add(&row("https://a.test/index.min.json", "A", "k"))
            .expect("a");
        repo.add(&row("https://b.test/index.min.json", "B", "k"))
            .expect("b");
        repo.add(&row("https://c.test/index.min.json", "C", "k"))
            .expect("c");
        let mut names: Vec<String> = fetch_extensions(&repo, |store| match store.name.as_str() {
            "B" => Err("offline".to_owned()),
            "C" => Ok(vec![format!("{}-ext", store.name)]),
            _ => Ok(vec![format!("{}-ext", store.name), "extra".to_owned()]),
        });
        names.sort();
        assert_eq!(
            names,
            ["A-ext", "C-ext", "extra"],
            "TC-STO-06: failing store contributes nothing"
        );
    }

    #[test]
    fn a_panicking_store_fetch_is_isolated() {
        let (_dir, _db, repo) = repo();
        repo.add(&row("https://a.test/index.min.json", "A", "k"))
            .expect("a");
        repo.add(&row("https://b.test/index.min.json", "B", "k"))
            .expect("b");
        let items: Vec<u8> = fetch_extensions(&repo, |store| -> Result<Vec<u8>, String> {
            if store.name == "A" {
                panic!("store A is broken");
            }
            Ok(vec![7])
        });
        assert_eq!(items, vec![7]);
    }

    #[tokio::test]
    async fn the_live_list_and_count_emit_after_insert_and_remove() {
        let (_dir, db, repo) = repo();
        let mut list = subscribe_all(&db);
        let mut count = subscribe_count(&db);
        let timeout = Duration::from_secs(5);

        let first = tokio::time::timeout(timeout, list.changed())
            .await
            .expect("list")
            .expect("open");
        assert!(first.expect("query").is_empty());
        assert_eq!(
            *tokio::time::timeout(timeout, count.changed())
                .await
                .expect("count")
                .expect("open")
                .expect("query"),
            0
        );

        repo.add(&row("https://a.test/index.min.json", "A", "k"))
            .expect("add");
        let added = tokio::time::timeout(timeout, list.changed())
            .await
            .expect("list after insert")
            .expect("open");
        assert_eq!(added.expect("query").len(), 1);
        assert_eq!(
            *tokio::time::timeout(timeout, count.changed())
                .await
                .expect("count")
                .expect("open")
                .expect("query"),
            1
        );

        repo.remove("https://a.test/index.min.json")
            .expect("remove");
        let removed = tokio::time::timeout(timeout, list.changed())
            .await
            .expect("list after remove")
            .expect("open");
        assert!(removed.expect("query").is_empty());
        assert_eq!(
            *tokio::time::timeout(timeout, count.changed())
                .await
                .expect("count")
                .expect("open")
                .expect("query"),
            0
        );
    }

    /// A `tracing` writer that appends to a shared buffer.
    #[derive(Clone)]
    struct CapturedLog(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedLog {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CapturedLog {
        type Writer = CapturedLog;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }
}
