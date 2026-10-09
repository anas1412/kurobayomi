//! Manga extension repos (`MangaExtensionRepoRepositoryImpl.kt`, `extension_repos.sq`): the repos a user trusts,
//! keyed by `base_url`, with the signing-key fingerprint as a second unique key. Fetching a repo's details and
//! deciding what a failed insert means are the interactors' job in `kuro-domain`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_manga::{ExtensionRepoRow, extension_repos};
use kuro_db::subscribe::Subscription;

use crate::{DataError, DataResult, ENTRY_DEBOUNCE};

/// Tables a repo listing reads.
const REPO_TABLES: &[&str] = &["extension_repos"];

/// One trusted manga repo (`ExtensionRepo.kt`). `base_url` is the primary key.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionRepo {
    pub base_url: String,
    pub name: String,
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String,
}

/// Why an insert failed, as `CreateMangaExtensionRepo.handleInsertionError` decides it.
#[derive(Clone, Debug, PartialEq)]
pub enum InsertConflict {
    /// A row with the same `base_url` exists.
    RepoAlreadyExists,
    /// Another row owns the fingerprint; the payload is that row.
    DuplicateFingerprint(ExtensionRepo),
    /// Neither key is taken, so the failure has another cause.
    Other,
}

/// `SaveExtensionRepoException`: an insert failed. Kotlin wraps every SQLite error of an insert this way, and so does
/// this type; the interactor classifies the conflict from the database afterwards.
#[derive(Debug)]
pub struct SaveError(pub DataError);

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error Saving Repository to Database")
    }
}

impl std::error::Error for SaveError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Repository over `extension_repos` in the manga database.
#[derive(Clone)]
pub struct ExtensionRepoRepo {
    db: Arc<Db>,
}

impl ExtensionRepoRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// The repo with `base_url`, or `None`.
    pub fn get(&self, base_url: &str) -> DataResult<Option<ExtensionRepo>> {
        let row = self.db.read(|c| Ok(extension_repos::find_one(c, base_url)?))?;
        Ok(row.map(map_repo))
    }

    /// The repo whose signing-key fingerprint is `fingerprint`, or `None`.
    pub fn get_by_fingerprint(&self, fingerprint: &str) -> DataResult<Option<ExtensionRepo>> {
        let row = self.db.read(|c| Ok(extension_repos::find_one_by_signing_key_fingerprint(c, fingerprint)?))?;
        Ok(row.map(map_repo))
    }

    /// Every repo, in rowid order.
    pub fn all(&self) -> DataResult<Vec<ExtensionRepo>> {
        let rows = self.db.read(|c| Ok(extension_repos::find_all(c)?))?;
        Ok(rows.into_iter().map(map_repo).collect())
    }

    /// Plain insert. Any database error is returned as [`SaveError`]; the caller classifies it with
    /// [`ExtensionRepoRepo::classify_conflict`].
    pub fn insert(&self, r: &ExtensionRepo) -> Result<(), SaveError> {
        let row = to_row(r);
        self.db
            .write(|tx| extension_repos::insert(tx, &row).map(drop))
            .map_err(SaveError)
    }

    /// Inserts, or updates the row with the same `base_url`. A fingerprint held by another row is an `Err`.
    pub fn upsert(&self, r: &ExtensionRepo) -> DataResult<()> {
        let row = to_row(r);
        self.db.write(|tx| extension_repos::upsert(tx, &row).map(drop))
    }

    /// Inserts, or on a fingerprint match rewrites the row that owns it, `base_url` included. A `base_url` held
    /// by a third row is an `Err`.
    pub fn replace(&self, r: &ExtensionRepo) -> DataResult<()> {
        let row = to_row(r);
        self.db.write(|tx| extension_repos::replace(tx, &row).map(drop))
    }

    /// Deletes the repo with `base_url`. A missing row is not an error.
    pub fn delete(&self, base_url: &str) -> DataResult<()> {
        self.db.write(|tx| Ok(extension_repos::delete(tx, base_url).map(drop)?))
    }

    /// Classifies a failed insert of `base_url` with `fingerprint`: an existing `base_url` wins, then an existing
    /// fingerprint (returned), else [`InsertConflict::Other`].
    pub fn classify_conflict(&self, base_url: &str, fingerprint: &str) -> DataResult<InsertConflict> {
        let conflict = self.db.read(|c| Ok(extension_repos::classify_conflict(c, base_url, fingerprint)?))?;
        Ok(match conflict {
            extension_repos::RepoConflict::RepoAlreadyExists => InsertConflict::RepoAlreadyExists,
            extension_repos::RepoConflict::DuplicateFingerprint(row) => {
                InsertConflict::DuplicateFingerprint(map_repo(row))
            }
            extension_repos::RepoConflict::Other => InsertConflict::Other,
        })
    }

    /// Live list of every repo, re-emitted after each write to the table.
    pub fn watch_all(&self) -> Subscription<Vec<ExtensionRepo>> {
        self.db.subscribe(REPO_TABLES, ENTRY_DEBOUNCE, |c| {
            Ok(extension_repos::find_all(c)?.into_iter().map(map_repo).collect::<Vec<_>>())
        })
    }

    /// Live count of repos.
    pub fn watch_count(&self) -> Subscription<i64> {
        self.db.subscribe(REPO_TABLES, ENTRY_DEBOUNCE, |c| Ok(extension_repos::count(c)?))
    }
}

fn map_repo(r: ExtensionRepoRow) -> ExtensionRepo {
    ExtensionRepo {
        base_url: r.base_url,
        name: r.name,
        short_name: r.short_name,
        website: r.website,
        signing_key_fingerprint: r.signing_key_fingerprint,
    }
}

fn to_row(r: &ExtensionRepo) -> ExtensionRepoRow {
    ExtensionRepoRow {
        base_url: r.base_url.clone(),
        name: r.name.clone(),
        short_name: r.short_name.clone(),
        website: r.website.clone(),
        signing_key_fingerprint: r.signing_key_fingerprint.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::manga_db;
    use std::time::Duration;

    fn repo_at(base: &str, name: &str, fingerprint: &str) -> ExtensionRepo {
        ExtensionRepo {
            base_url: base.into(),
            name: name.into(),
            short_name: Some("S".into()),
            website: "https://site.test".into(),
            signing_key_fingerprint: fingerprint.into(),
        }
    }

    fn repos() -> (tempfile::TempDir, ExtensionRepoRepo) {
        let (dir, db) = manga_db();
        (dir, ExtensionRepoRepo::new(db))
    }

    #[test]
    fn insert_then_read_back_by_url_fingerprint_and_listing() {
        let (_d, repo) = repos();
        let a = repo_at("https://a.test", "A", "fp-a");
        repo.insert(&a).expect("insert");
        assert_eq!(repo.get("https://a.test").expect("get"), Some(a.clone()));
        assert_eq!(repo.get_by_fingerprint("fp-a").expect("by fp"), Some(a.clone()));
        assert_eq!(repo.get("https://missing.test").expect("get"), None);
        assert_eq!(repo.all().expect("all"), vec![a]);
    }

    #[test]
    fn insert_fails_on_a_taken_url_or_fingerprint() {
        let (_d, repo) = repos();
        repo.insert(&repo_at("https://a.test", "A", "fp-a")).expect("insert");
        assert!(repo.insert(&repo_at("https://a.test", "again", "fp-x")).is_err(), "same base_url");
        assert!(repo.insert(&repo_at("https://b.test", "B", "fp-a")).is_err(), "same fingerprint");
    }

    #[test]
    fn upsert_updates_by_base_url_and_fails_on_another_rows_fingerprint() {
        let (_d, repo) = repos();
        repo.insert(&repo_at("https://a.test", "A", "fp-a")).expect("insert");
        repo.insert(&repo_at("https://b.test", "B", "fp-b")).expect("insert");
        repo.upsert(&repo_at("https://a.test", "A2", "fp-a")).expect("upsert same row");
        assert_eq!(repo.get("https://a.test").expect("get").expect("row").name, "A2");
        assert!(repo.upsert(&repo_at("https://a.test", "A3", "fp-b")).is_err(), "fp-b belongs to B");
    }

    #[test]
    fn replace_rewrites_the_row_that_owns_the_fingerprint_including_its_base_url() {
        let (_d, repo) = repos();
        repo.insert(&repo_at("https://old.test", "Old", "fp")).expect("insert");
        repo.replace(&repo_at("https://new.test", "New", "fp")).expect("replace");
        assert_eq!(repo.all().expect("all"), vec![repo_at("https://new.test", "New", "fp")]);
    }

    #[test]
    fn replace_onto_a_third_repos_base_url_is_an_error_not_a_panic() {
        let (_d, repo) = repos();
        repo.insert(&repo_at("https://a.test", "A", "fp-a")).expect("insert");
        repo.insert(&repo_at("https://c.test", "C", "fp-c")).expect("insert");
        assert!(repo.replace(&repo_at("https://c.test", "A", "fp-a")).is_err());
        assert_eq!(repo.all().expect("all").len(), 2, "nothing changed");
    }

    #[test]
    fn classify_conflict_names_the_repo_that_collided() {
        let (_d, repo) = repos();
        repo.insert(&repo_at("https://a.test", "A", "fp-a")).expect("insert");
        assert_eq!(
            repo.classify_conflict("https://a.test", "fp-new").expect("classify"),
            InsertConflict::RepoAlreadyExists
        );
        assert_eq!(
            repo.classify_conflict("https://b.test", "fp-a").expect("classify"),
            InsertConflict::DuplicateFingerprint(repo_at("https://a.test", "A", "fp-a"))
        );
        assert_eq!(repo.classify_conflict("https://b.test", "fp-b").expect("classify"), InsertConflict::Other);
    }

    #[test]
    fn delete_removes_and_a_missing_row_is_fine() {
        let (_d, repo) = repos();
        repo.insert(&repo_at("https://a.test", "A", "fp-a")).expect("insert");
        repo.delete("https://a.test").expect("delete");
        repo.delete("https://a.test").expect("second delete is a no-op");
        assert!(repo.all().expect("all").is_empty());
    }

    /// Waits until the live count shows `want`, skipping intermediate publications.
    async fn wait_for(sub: &mut Subscription<i64>, want: i64) {
        loop {
            if sub.current().and_then(Result::ok).as_deref() == Some(&want) {
                return;
            }
            tokio::time::timeout(Duration::from_secs(5), sub.changed())
                .await
                .expect("a publication")
                .expect("alive")
                .expect("the count query succeeded");
        }
    }

    #[tokio::test]
    async fn watch_count_reemits_on_insert_and_delete() {
        let (_d, repo) = repos();
        let mut sub = repo.watch_count();
        wait_for(&mut sub, 0).await;
        repo.insert(&repo_at("https://w.test", "W", "fp-w")).expect("insert");
        wait_for(&mut sub, 1).await;
        repo.delete("https://w.test").expect("delete");
        wait_for(&mut sub, 0).await;
    }

    /// Waits until the published list has `want` rows.
    async fn wait_len(sub: &mut Subscription<Vec<ExtensionRepo>>, want: usize) {
        while sub.current().and_then(Result::ok).map(|v| v.len()) != Some(want) {
            tokio::time::timeout(Duration::from_secs(5), sub.changed())
                .await
                .expect("a publication")
                .expect("alive")
                .expect("query ok");
        }
    }

    #[tokio::test]
    async fn watch_all_reemits_after_each_write() {
        let (_d, repo) = repos();
        let mut sub = repo.watch_all();
        wait_len(&mut sub, 0).await;
        repo.insert(&repo_at("https://w.test", "W", "fp-w")).expect("insert");
        wait_len(&mut sub, 1).await;
        repo.upsert(&repo_at("https://w.test", "W2", "fp-w")).expect("upsert");
        while sub.current().and_then(Result::ok).map(|v| v[0].name.clone()).as_deref() != Some("W2") {
            tokio::time::timeout(Duration::from_secs(5), sub.changed())
                .await
                .expect("a publication")
                .expect("alive")
                .expect("query ok");
        }
        repo.delete("https://w.test").expect("delete");
        wait_len(&mut sub, 0).await;
    }
}
