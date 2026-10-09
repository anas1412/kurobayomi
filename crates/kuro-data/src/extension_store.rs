//! Anime extension stores (`AnimeExtensionStoreRepositoryImpl.kt`, `extension_store.sq`): the persisted index
//! URLs with their metadata, and their refresh. Fetching a store's index is the store service's job
//! (T-06a-051/052), so `refresh_all` takes the fetch as a parameter.

use std::sync::Arc;
use std::time::Duration;

use kuro_db::handle::Db;
use kuro_db::statements_anime::{
    ExtensionStore, delete_extension_store, extension_store, extension_store_count,
    extension_stores, insert_extension_store_from_preference, upsert_extension_store,
};
use kuro_db::subscribe::Subscription;

use crate::{DataError, DataResult};

/// Tables a store listing reads.
const STORE_TABLES: &[&str] = &["extension_store"];

/// Quiet time before a live store query re-runs after a write.
const STORE_DEBOUNCE: Duration = Duration::from_millis(50);

/// Why [`ExtensionStoreRepo::add_from_url`] failed: the fetch, or the save of the fetched row.
#[derive(Debug)]
pub enum AddStoreError<E> {
    Fetch(E),
    Save(DataError),
}

/// Repository over the `extension_store` table of the anime database.
#[derive(Clone)]
pub struct ExtensionStoreRepo {
    db: Arc<Db>,
}

impl ExtensionStoreRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The stored row for `index_url`, if any.
    pub fn get(&self, index_url: &str) -> DataResult<Option<ExtensionStore>> {
        self.db.read(|c| Ok(extension_store(c, index_url)?))
    }

    /// Every store, in insertion order (`ORDER BY rowid`).
    pub fn all(&self) -> DataResult<Vec<ExtensionStore>> {
        self.db.read(|c| Ok(extension_stores(c)?))
    }

    pub fn count(&self) -> DataResult<i64> {
        self.db.read(|c| Ok(extension_store_count(c)?))
    }

    /// Persists a fetched store. Adding the same index URL again overwrites the row (E19).
    pub fn add(&self, store: &ExtensionStore) -> DataResult<()> {
        self.db
            .write(|tx| Ok(upsert_extension_store(tx, store).map(drop)?))
    }

    /// Fetches the store at `index_url` with `fetch`, then saves the fetched row (`AnimeExtensionStoreRepositoryImpl.insert`).
    /// A fetch failure is returned as [`AddStoreError::Fetch`] and saves nothing.
    pub fn add_from_url<E>(
        &self,
        index_url: &str,
        fetch: impl FnOnce(&str) -> Result<ExtensionStore, E>,
    ) -> Result<(), AddStoreError<E>> {
        let store = fetch(index_url).map_err(AddStoreError::Fetch)?;
        self.add(&store).map_err(AddStoreError::Save)
    }

    /// Removes the store with `index_url`. A missing row is not an error.
    pub fn remove(&self, index_url: &str) -> DataResult<()> {
        self.db
            .write(|tx| Ok(delete_extension_store(tx, index_url).map(drop)?))
    }

    /// Stores a legacy repo URL from the backup import of an old `anime_extension_repos` value
    /// (`signing_key = NO_SIGNING_KEY`, T-19a-022).
    pub fn insert_from_legacy(&self, index_url: &str, name: &str) -> DataResult<()> {
        self.db
            .write(|tx| Ok(insert_extension_store_from_preference(tx, index_url, name).map(drop)?))
    }

    /// Re-fetches every store, one after another. A failing store keeps its old row and logs
    /// `Failed to refresh extension store '<name> (<index_url>)'` (E20). A changed signing key logs a warning.
    /// A store whose index moved gets its new row and loses the old one in one transaction.
    pub fn refresh_all<E: std::fmt::Display>(
        &self,
        mut fetch: impl FnMut(&ExtensionStore) -> Result<ExtensionStore, E>,
    ) -> DataResult<()> {
        for old in self.all()? {
            match fetch(&old) {
                Ok(new) => {
                    if new.signing_key != old.signing_key {
                        tracing::warn!(
                            store = %old.index_url,
                            "extension store signing key changed from '{}' to '{}'",
                            old.signing_key,
                            new.signing_key
                        );
                    }
                    if new.index_url == old.index_url {
                        self.add(&new)?;
                    } else {
                        self.db.write(|tx| {
                            upsert_extension_store(tx, &new)?;
                            delete_extension_store(tx, &old.index_url)?;
                            Ok(())
                        })?;
                    }
                }
                Err(e) => tracing::error!(
                    error = %e,
                    "Failed to refresh extension store '{} ({})'",
                    old.name,
                    old.index_url
                ),
            }
        }
        Ok(())
    }

    /// Live list of every store, re-emitted after each write to the table.
    pub fn watch_all(&self) -> Subscription<Vec<ExtensionStore>> {
        self.db
            .subscribe(STORE_TABLES, STORE_DEBOUNCE, |c| Ok(extension_stores(c)?))
    }

    /// Live count of stores.
    pub fn watch_count(&self) -> Subscription<i64> {
        self.db.subscribe(STORE_TABLES, STORE_DEBOUNCE, |c| {
            Ok(extension_store_count(c)?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::anime_db;
    use std::time::Duration as StdDuration;

    fn store(url: &str, name: &str, key: &str) -> ExtensionStore {
        ExtensionStore {
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

    fn repo() -> (tempfile::TempDir, ExtensionStoreRepo) {
        let (dir, db) = anime_db();
        (dir, ExtensionStoreRepo::new(db))
    }

    #[test]
    fn adding_the_same_index_twice_leaves_one_row() {
        let (_dir, repo) = repo();
        repo.add(&store("https://a.test/index.min.json", "A", "k1"))
            .expect("add");
        repo.add(&store("https://a.test/index.min.json", "A2", "k2"))
            .expect("add again");
        assert_eq!(repo.count().expect("count"), 1);
        assert_eq!(
            repo.get("https://a.test/index.min.json")
                .expect("get")
                .expect("row")
                .name,
            "A2"
        );
    }

    #[test]
    fn adding_the_same_index_url_twice_through_a_fetch_leaves_one_row() {
        let (_dir, repo) = repo();
        repo.add_from_url("https://a.test/index.min.json", |u| Ok::<_, String>(store(u, "A", "k")))
            .expect("first add");
        repo.add_from_url("https://a.test/index.min.json", |u| Ok::<_, String>(store(u, "A2", "k")))
            .expect("second add");
        assert_eq!(repo.count().expect("count"), 1);
        assert_eq!(repo.get("https://a.test/index.min.json").expect("get").expect("row").name, "A2");
    }

    #[test]
    fn a_failed_fetch_saves_nothing() {
        let (_dir, repo) = repo();
        let result = repo.add_from_url("https://down.test/index.min.json", |_| Err::<ExtensionStore, _>("offline"));
        assert!(matches!(result, Err(AddStoreError::Fetch("offline"))));
        assert_eq!(repo.count().expect("count"), 0);
    }

    #[test]
    fn remove_deletes_and_a_missing_row_is_fine() {
        let (_dir, repo) = repo();
        repo.add(&store("https://a.test/index.min.json", "A", "k"))
            .expect("add");
        repo.remove("https://a.test/index.min.json")
            .expect("remove");
        repo.remove("https://a.test/index.min.json")
            .expect("second remove is a no-op");
        assert_eq!(repo.count().expect("count"), 0);
    }

    #[test]
    fn failing_refresh_keeps_the_old_row() {
        let (_dir, repo) = repo();
        repo.add(&store("https://a.test/index.min.json", "A", "k"))
            .expect("add");
        repo.refresh_all(|_| Err::<ExtensionStore, _>("network down"))
            .expect("refresh");
        assert_eq!(
            repo.get("https://a.test/index.min.json")
                .expect("get")
                .expect("row")
                .name,
            "A"
        );
    }

    #[test]
    fn refresh_with_a_new_key_updates_the_row() {
        let (_dir, repo) = repo();
        repo.add(&store("https://a.test/index.min.json", "A", "old"))
            .expect("add");
        repo.refresh_all(|old| {
            Ok::<_, String>(ExtensionStore {
                signing_key: "new".into(),
                ..old.clone()
            })
        })
        .expect("refresh");
        assert_eq!(
            repo.get("https://a.test/index.min.json")
                .expect("get")
                .expect("row")
                .signing_key,
            "new"
        );
    }

    #[test]
    fn refresh_with_a_moved_index_swaps_the_row_in_one_step() {
        let (_dir, repo) = repo();
        repo.add(&store("https://old.test/index.min.json", "A", "k"))
            .expect("add");
        repo.refresh_all(|old| {
            Ok::<_, String>(ExtensionStore {
                index_url: "https://new.test/index.min.json".into(),
                ..old.clone()
            })
        })
        .expect("refresh");
        assert_eq!(repo.count().expect("count"), 1, "no duplicate row");
        assert!(
            repo.get("https://old.test/index.min.json")
                .expect("get")
                .is_none()
        );
        assert!(
            repo.get("https://new.test/index.min.json")
                .expect("get")
                .is_some()
        );
    }

    #[test]
    fn refresh_continues_after_a_failing_store() {
        let (_dir, repo) = repo();
        repo.add(&store("https://bad.test/index.min.json", "Bad", "k"))
            .expect("add");
        repo.add(&store("https://good.test/index.min.json", "Good", "k"))
            .expect("add");
        let mut seen = Vec::new();
        repo.refresh_all(|old| {
            seen.push(old.name.clone());
            if old.name == "Bad" {
                Err("boom".to_owned())
            } else {
                Ok(ExtensionStore {
                    badge_label: "New".into(),
                    ..old.clone()
                })
            }
        })
        .expect("refresh");
        assert_eq!(seen, ["Bad", "Good"], "sequential, in store order");
        assert_eq!(
            repo.get("https://good.test/index.min.json")
                .expect("get")
                .expect("row")
                .badge_label,
            "New"
        );
    }

    #[test]
    fn preference_import_row_has_no_signing_key() {
        let (_dir, repo) = repo();
        repo.insert_from_legacy("https://pref.test/index.min.json", "Pref")
            .expect("insert");
        let row = repo
            .get("https://pref.test/index.min.json")
            .expect("get")
            .expect("row");
        assert_eq!(row.signing_key, "NO_SIGNING_KEY");
        assert_eq!(row.contact_website, "https://pref.test/index.min.json");
        assert!(!row.is_legacy);
        assert_eq!(row.extension_list_url, None);
    }

    #[test]
    fn listing_is_in_insertion_order() {
        let (_dir, repo) = repo();
        for name in ["c", "a", "b"] {
            repo.add(&store(
                &format!("https://{name}.test/index.min.json"),
                name,
                "k",
            ))
            .expect("add");
        }
        let names: Vec<String> = repo
            .all()
            .expect("all")
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, ["c", "a", "b"]);
    }

    /// Waits until the live count shows `want`, skipping intermediate publications.
    async fn wait_until(sub: &mut Subscription<i64>, want: i64) {
        loop {
            if sub.current().and_then(Result::ok).as_deref() == Some(&want) {
                return;
            }
            tokio::time::timeout(StdDuration::from_secs(5), sub.changed())
                .await
                .expect("the count reaches the expected value");
        }
    }

    #[tokio::test]
    async fn watch_count_reemits_on_insert_and_delete() {
        let (_dir, repo) = repo();
        let mut sub = repo.watch_count();
        wait_until(&mut sub, 0).await;
        repo.add(&store("https://w.test/index.min.json", "W", "k"))
            .expect("add");
        wait_until(&mut sub, 1).await;
        repo.remove("https://w.test/index.min.json")
            .expect("remove");
        wait_until(&mut sub, 0).await;
    }
}
