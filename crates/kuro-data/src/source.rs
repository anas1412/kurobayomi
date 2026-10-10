//! Stub source rows (`MangaStubSourceRepositoryImpl.kt`, `AnimeStubSourceRepositoryImpl.kt`, `sources.sq`,
//! `animesources.sq`): the id, language and name of a source whose extension is gone, so its library entries
//! still show the source name. Behaviour that needs domain errors (the failing remote calls) is in `kuro-domain`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_anime as sa;
use kuro_db::statements_manga::{self as sm, StubSourceRow};
use kuro_db::subscribe::Subscription;

use crate::{DataResult, ENTRY_DEBOUNCE};

/// A persisted stub source (`StubMangaSource`, `StubAnimeSource`). Both kinds share this record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubSource {
    pub id: i64,
    pub lang: String,
    pub name: String,
}

/// `StubSource.toString()`: the id, whatever the name and language are.
impl std::fmt::Display for StubSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl StubSource {
    /// `name.isBlank() || lang.isBlank()`.
    pub fn is_invalid(&self) -> bool {
        self.name.trim().is_empty() || self.lang.trim().is_empty()
    }

    /// `"{name} ({LANG})"` when neither name nor language is blank, else the numeric id (Q-SRC-1).
    pub fn display_name(&self) -> String {
        if self.is_invalid() { self.id.to_string() } else { format!("{} ({})", self.name, self.lang.to_uppercase()) }
    }

    /// The placeholder for an id with no stored row: empty name and language, shown as the id (`StubSource.fallback`,
    /// E-SRC-05). Never persisted.
    pub fn fallback(id: i64) -> Self {
        Self { id, lang: String::new(), name: String::new() }
    }
}

/// Stub rows of manga sources, over the `sources` table.
#[derive(Clone)]
pub struct MangaStubSourceRepo {
    db: Arc<Db>,
}

impl MangaStubSourceRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Inserts the stub, or replaces `lang` and `name` of the row with the same id.
    pub fn upsert(&self, id: i64, lang: &str, name: &str) -> DataResult<()> {
        let row = StubSourceRow { id, lang: lang.to_owned(), name: name.to_owned() };
        self.db.write(|tx| Ok(sm::sources::upsert(tx, &row).map(drop)?))
    }

    /// The stub with `id`, or `None` for an unknown id.
    pub fn get(&self, id: i64) -> DataResult<Option<StubSource>> {
        let row = self.db.read(|c| Ok(sm::sources::find_one(c, id)?))?;
        Ok(row.map(map_manga))
    }

    /// Every stub, in table order.
    pub fn all(&self) -> DataResult<Vec<StubSource>> {
        let rows = self.db.read(|c| Ok(sm::sources::find_all(c)?))?;
        Ok(rows.into_iter().map(map_manga).collect())
    }

    /// Every stub, published again after each upsert.
    pub fn watch_all(&self) -> Subscription<Vec<StubSource>> {
        self.db.subscribe(&["sources"], ENTRY_DEBOUNCE, |c| {
            Ok(sm::sources::find_all(c)?.into_iter().map(map_manga).collect::<Vec<_>>())
        })
    }
}

/// Stub rows of anime sources, over the `animesources` table.
#[derive(Clone)]
pub struct AnimeStubSourceRepo {
    db: Arc<Db>,
}

impl AnimeStubSourceRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Inserts the stub, or replaces `lang` and `name` of the row with the same id.
    pub fn upsert(&self, id: i64, lang: &str, name: &str) -> DataResult<()> {
        self.db.write(|tx| Ok(sa::upsert_stub_source(tx, id, lang, name).map(drop)?))
    }

    /// The stub with `id`, or `None` for an unknown id.
    pub fn get(&self, id: i64) -> DataResult<Option<StubSource>> {
        let row = self.db.read(|c| Ok(sa::stub_source(c, id)?))?;
        Ok(row.map(map_anime))
    }

    /// Every stub, in table order.
    pub fn all(&self) -> DataResult<Vec<StubSource>> {
        let rows = self.db.read(|c| Ok(sa::stub_sources(c)?))?;
        Ok(rows.into_iter().map(map_anime).collect())
    }

    /// Every stub, published again after each upsert.
    pub fn watch_all(&self) -> Subscription<Vec<StubSource>> {
        self.db.subscribe(&["animesources"], ENTRY_DEBOUNCE, |c| {
            Ok(sa::stub_sources(c)?.into_iter().map(map_anime).collect::<Vec<_>>())
        })
    }
}

fn map_manga(r: StubSourceRow) -> StubSource {
    StubSource { id: r.id, lang: r.lang, name: r.name }
}

fn map_anime(r: sa::StubSource) -> StubSource {
    StubSource { id: r.id, lang: r.lang, name: r.name }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::tests::{anime_db, manga_db};

    #[test]
    fn a_blank_stub_is_representable_and_prints_its_id() {
        let blank = StubSource { id: 42, lang: String::new(), name: String::new() };
        assert!(blank.is_invalid());
        assert_eq!(blank.to_string(), "42");
    }

    #[test]
    fn display_name_uses_the_upper_case_language_or_the_id_when_blank() {
        let s = |id, lang: &str, name: &str| StubSource { id, lang: lang.into(), name: name.into() };
        assert_eq!(s(77, "en", "Foo").display_name(), "Foo (EN)", "TC-SRC-03");
        assert_eq!(s(77, "", "Foo").display_name(), "77", "blank language: id (Q-SRC-1)");
        assert_eq!(s(77, "en", "   ").display_name(), "77", "blank name: id");
        assert_eq!(s(78, "", "").display_name(), "78", "S7/S8: an empty name and an empty language show the id");
        assert!(s(1, " ", "x").is_invalid());
    }

    #[test]
    fn manga_stub_upsert_reads_back_and_unknown_ids_are_none() {
        let (_d, db) = manga_db();
        let repo = MangaStubSourceRepo::new(db);
        assert_eq!(repo.get(5).expect("get"), None);
        repo.upsert(5, "en", "Old").expect("upsert");
        repo.upsert(5, "fr", "New").expect("upsert again");
        assert_eq!(repo.get(5).expect("get"), Some(StubSource { id: 5, lang: "fr".into(), name: "New".into() }));
    }

    #[test]
    fn anime_stub_upsert_reads_back_and_unknown_ids_are_none() {
        let (_d, db) = anime_db();
        let repo = AnimeStubSourceRepo::new(db);
        assert_eq!(repo.get(9).expect("get"), None);
        repo.upsert(9, "ja", "Alpha").expect("upsert");
        repo.upsert(9, "ja", "Beta").expect("upsert again");
        assert_eq!(repo.get(9).expect("get"), Some(StubSource { id: 9, lang: "ja".into(), name: "Beta".into() }));
    }

    #[test]
    fn a_stub_row_survives_reopening_the_database_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("tachiyomi.db");
        {
            let writer = kuro_db::schema_manga::open(&path).expect("schema");
            let db = Arc::new(kuro_db::handle::Db::new(&path, writer).expect("handle"));
            MangaStubSourceRepo::new(db).upsert(3, "en", "Kept").expect("upsert");
        }
        let writer = kuro_db::schema_manga::open(&path).expect("reopen");
        let db = Arc::new(kuro_db::handle::Db::new(&path, writer).expect("handle"));
        let got = MangaStubSourceRepo::new(db).get(3).expect("get");
        assert_eq!(got, Some(StubSource { id: 3, lang: "en".into(), name: "Kept".into() }));
    }

    async fn wait_until<T: PartialEq + std::fmt::Debug>(sub: &mut Subscription<Vec<T>>, want: &[T]) {
        loop {
            if sub.current().and_then(Result::ok).as_deref().map(Vec::as_slice) == Some(want) {
                return;
            }
            tokio::time::timeout(Duration::from_secs(5), sub.changed())
                .await
                .expect("a publication")
                .expect("alive")
                .expect("the query succeeded");
        }
    }

    #[tokio::test]
    async fn watch_all_emits_after_each_upsert() {
        let (_d, db) = manga_db();
        let repo = MangaStubSourceRepo::new(db);
        let mut sub = repo.watch_all();
        wait_until(&mut sub, &[]).await;
        repo.upsert(1, "en", "One").expect("upsert");
        wait_until(&mut sub, &[StubSource { id: 1, lang: "en".into(), name: "One".into() }]).await;
    }

    #[tokio::test]
    async fn anime_watch_all_emits_after_each_upsert() {
        let (_d, db) = anime_db();
        let repo = AnimeStubSourceRepo::new(db);
        let mut sub = repo.watch_all();
        wait_until(&mut sub, &[]).await;
        repo.upsert(2, "en", "Two").expect("upsert");
        wait_until(&mut sub, &[StubSource { id: 2, lang: "en".into(), name: "Two".into() }]).await;
    }
}
