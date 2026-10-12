//! Stub source rows (`sources.sq`, `animesources.sq`): the id, language and name of a source whose extension is gone, so its library entries
//! still show the source name. Behaviour that needs domain errors (the failing remote calls) is in `kuro-domain`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_db::tables::stub_sources as t;
use kuro_db::tables::StubSourceRow;
use kuro_platform::kind::Kind;

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

/// Stub rows of one kind, over the `stub_sources` table.
#[derive(Clone)]
pub struct StubSourceRepo {
    db: Arc<Db>,
    kind: Kind,
}

impl StubSourceRepo {
    pub fn new(db: Arc<Db>, kind: Kind) -> Self {
        Self { db, kind }
    }

    /// Inserts the stub, or replaces `lang` and `name` of the row with the same id.
    pub fn upsert(&self, id: i64, lang: &str, name: &str) -> DataResult<()> {
        let kind = self.kind;
        self.db.write(|tx| Ok(t::upsert(tx, kind, id, lang, name).map(drop)?))
    }

    /// The stub with `id`, or `None` for an unknown id.
    pub fn get(&self, id: i64) -> DataResult<Option<StubSource>> {
        let kind = self.kind;
        Ok(self.db.read(|c| Ok(t::find_one(c, kind, id)?))?.map(StubSource::from))
    }

    /// Every stub, in table order.
    pub fn all(&self) -> DataResult<Vec<StubSource>> {
        let kind = self.kind;
        Ok(self.db.read(|c| Ok(t::find_all(c, kind)?))?.into_iter().map(StubSource::from).collect())
    }

    /// Every stub, published again after each upsert.
    pub fn watch_all(&self) -> Subscription<Vec<StubSource>> {
        let kind = self.kind;
        self.db.subscribe(&["stub_sources"], ENTRY_DEBOUNCE, move |c| {
            Ok(t::find_all(c, kind)?.into_iter().map(StubSource::from).collect::<Vec<_>>())
        })
    }
}

impl From<StubSourceRow> for StubSource {
    fn from(r: StubSourceRow) -> Self {
        Self { id: r.id, lang: r.lang, name: r.name }
    }
}

/// A legacy per-kind name for [`StubSourceRepo`] (`new(db)` fixes the kind); phase E deletes them.
macro_rules! legacy_stub_repo {
    ($name:ident, $kind:expr) => {
        #[derive(Clone)]
        pub struct $name(StubSourceRepo);

        impl $name {
            pub fn new(db: Arc<Db>) -> Self {
                Self(StubSourceRepo::new(db, $kind))
            }

            pub fn upsert(&self, id: i64, lang: &str, name: &str) -> DataResult<()> {
                self.0.upsert(id, lang, name)
            }

            pub fn get(&self, id: i64) -> DataResult<Option<StubSource>> {
                self.0.get(id)
            }

            pub fn all(&self) -> DataResult<Vec<StubSource>> {
                self.0.all()
            }

            pub fn watch_all(&self) -> Subscription<Vec<StubSource>> {
                self.0.watch_all()
            }
        }
    };
}

legacy_stub_repo!(MangaStubSourceRepo, Kind::Manga);
legacy_stub_repo!(AnimeStubSourceRepo, Kind::Anime);

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::tests::test_db;

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
        let (_d, db) = test_db();
        let repo = MangaStubSourceRepo::new(db);
        assert_eq!(repo.get(5).expect("get"), None);
        repo.upsert(5, "en", "Old").expect("upsert");
        repo.upsert(5, "fr", "New").expect("upsert again");
        assert_eq!(repo.get(5).expect("get"), Some(StubSource { id: 5, lang: "fr".into(), name: "New".into() }));
    }

    #[test]
    fn anime_stub_upsert_reads_back_and_unknown_ids_are_none() {
        let (_d, db) = test_db();
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
            let writer = kuro_db::schema_unified::open_file(&path).expect("schema");
            let db = Arc::new(kuro_db::handle::Db::new(&path, writer).expect("handle"));
            MangaStubSourceRepo::new(db).upsert(3, "en", "Kept").expect("upsert");
        }
        let writer = kuro_db::schema_unified::open_file(&path).expect("reopen");
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
        let (_d, db) = test_db();
        let repo = MangaStubSourceRepo::new(db);
        let mut sub = repo.watch_all();
        wait_until(&mut sub, &[]).await;
        repo.upsert(1, "en", "One").expect("upsert");
        wait_until(&mut sub, &[StubSource { id: 1, lang: "en".into(), name: "One".into() }]).await;
    }

    #[tokio::test]
    async fn anime_watch_all_emits_after_each_upsert() {
        let (_d, db) = test_db();
        let repo = AnimeStubSourceRepo::new(db);
        let mut sub = repo.watch_all();
        wait_until(&mut sub, &[]).await;
        repo.upsert(2, "en", "Two").expect("upsert");
        wait_until(&mut sub, &[StubSource { id: 2, lang: "en".into(), name: "Two".into() }]).await;
    }
}
