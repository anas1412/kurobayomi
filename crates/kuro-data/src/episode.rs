//! Anime episode model and its partial update (`Episode.kt`, `EpisodeUpdate.kt`), and `EpisodeRepo`
//! (`EpisodeRepositoryImpl.kt`) over the shared `Item` and `ItemRepo`. The behaviour (`copy_from`, `to_update`,
//! recognition) lives in `kuro-domain::items`. Phase E deletes this file.

use kuro_db::schema_anime::Memo;
use kuro_db::statements_anime::EpisodeRow;
use kuro_db::subscribe::Subscription;
use kuro_platform::kind::Kind;

use crate::DataResult;
use crate::item::{Item, ItemKey, ItemUpdate};
use crate::legacy::{LegacyItem, LegacyItemRepo};
use crate::sanitize::sanitize_item_name;

/// One `episodes` row as the domain sees it. Positions and durations are whole seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Episode {
    pub id: i64,
    pub anime_id: i64,
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub last_second_seen: i64,
    pub total_seconds: i64,
    pub date_fetch: i64,
    pub source_order: i64,
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    /// `-1.0` when unrecognised, `-2.0` for the intentionally numberless sentinel.
    pub episode_number: f64,
    pub scanlator: Option<String>,
    pub summary: Option<String>,
    pub preview_url: Option<String>,
    pub last_modified_at: i64,
    pub version: i64,
    pub memo: serde_json::Map<String, serde_json::Value>,
}

impl Episode {
    /// `Episode.create()`: id and anime id -1, date upload -1, episode number -1.0, version 1, empty memo.
    pub fn create() -> Self {
        Self::from(Item::create(Kind::Anime))
    }
}

impl From<Item> for Episode {
    fn from(i: Item) -> Self {
        Self {
            total_seconds: i.total_seconds(),
            memo: i.memo(),
            id: i.id,
            anime_id: i.entry_id,
            seen: i.done,
            bookmark: i.bookmark,
            fillermark: i.fillermark,
            last_second_seen: i.progress,
            date_fetch: i.date_fetch,
            source_order: i.source_order,
            url: i.url,
            name: i.name,
            date_upload: i.date_upload,
            episode_number: i.number,
            scanlator: i.scanlator,
            summary: i.summary,
            preview_url: i.preview_url,
            last_modified_at: i.last_modified_at,
            version: i.version,
        }
    }
}

impl From<&Episode> for Item {
    fn from(e: &Episode) -> Self {
        Self {
            id: e.id,
            entry_id: e.anime_id,
            kind: Kind::Anime,
            url: e.url.clone(),
            name: e.name.clone(),
            scanlator: e.scanlator.clone(),
            done: e.seen,
            bookmark: e.bookmark,
            fillermark: e.fillermark,
            progress: e.last_second_seen,
            number: e.episode_number,
            source_order: e.source_order,
            date_fetch: e.date_fetch,
            date_upload: e.date_upload,
            last_modified_at: e.last_modified_at,
            version: e.version,
            total_seconds: Some(e.total_seconds),
            summary: e.summary.clone(),
            preview_url: e.preview_url.clone(),
            memo: Some(Memo(e.memo.clone())),
        }
    }
}

/// Partial episode update. `None` keeps the stored value; `id` is required.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EpisodeUpdate {
    pub id: i64,
    pub anime_id: Option<i64>,
    pub seen: Option<bool>,
    pub bookmark: Option<bool>,
    pub fillermark: Option<bool>,
    pub last_second_seen: Option<i64>,
    pub total_seconds: Option<i64>,
    pub date_fetch: Option<i64>,
    pub source_order: Option<i64>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub date_upload: Option<i64>,
    pub episode_number: Option<f64>,
    pub scanlator: Option<String>,
    pub summary: Option<String>,
    pub preview_url: Option<String>,
    pub version: Option<i64>,
    pub memo: Option<serde_json::Map<String, serde_json::Value>>,
}

impl EpisodeUpdate {
    pub fn new(id: i64) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }
}

impl From<&EpisodeUpdate> for ItemUpdate {
    fn from(u: &EpisodeUpdate) -> Self {
        Self {
            id: u.id,
            entry_id: u.anime_id,
            done: u.seen,
            bookmark: u.bookmark,
            fillermark: u.fillermark,
            progress: u.last_second_seen,
            total_seconds: u.total_seconds,
            date_fetch: u.date_fetch,
            source_order: u.source_order,
            url: u.url.clone(),
            name: u.name.clone(),
            date_upload: u.date_upload,
            number: u.episode_number,
            scanlator: u.scanlator.clone(),
            summary: u.summary.clone(),
            preview_url: u.preview_url.clone(),
            version: u.version,
            memo: u.memo.clone().map(Memo),
        }
    }
}

impl LegacyItem for Episode {
    const KIND: Kind = Kind::Anime;
    type Update = EpisodeUpdate;

    fn to_item(&self) -> Item {
        self.into()
    }

    fn to_update(u: &EpisodeUpdate) -> ItemUpdate {
        u.into()
    }
}

/// Identity of an episode as Kotlin's `EpisodeImpl.equals`/`hashCode` define it: the pair (id, url).
pub type EpisodeKey = ItemKey;

impl From<&Episode> for ItemKey {
    fn from(e: &Episode) -> Self {
        Self { id: e.id, url: e.url.clone() }
    }
}

/// Episode access over the anime items (`EpisodeRepositoryImpl.kt`).
pub type EpisodeRepo = LegacyItemRepo<Episode>;

impl EpisodeRepo {
    /// The lowest-id row with this url in the anime, or `None`. A duplicate url logs a warning.
    pub fn by_url_and_anime_id(&self, url: &str, anime_id: i64) -> DataResult<Option<Episode>> {
        Ok(self.items.by_url_and_entry_id(url, anime_id)?.map(Episode::from))
    }

    /// Every episode of an anime, with no scanlator filter.
    pub fn by_anime_id(&self, anime_id: i64) -> DataResult<Vec<Episode>> {
        Ok(Self::list(self.items.by_entry_id(anime_id, false)?))
    }

    /// The episode list of an anime, published again after a change to its rows.
    pub fn subscribe_by_anime_id(&self, anime_id: i64) -> Subscription<Vec<Episode>> {
        self.items.subscribe_by_entry_id_as(anime_id, false)
    }
}

/// One `episodes` row of `statements_anime` as the domain sees it; `is_syncing` is dropped.
pub fn map_episode(r: &EpisodeRow) -> Episode {
    Episode {
        id: r.id,
        anime_id: r.anime_id,
        seen: r.seen,
        bookmark: r.bookmark,
        fillermark: r.fillermark,
        last_second_seen: r.last_second_seen,
        total_seconds: r.total_seconds,
        date_fetch: r.date_fetch,
        source_order: r.source_order,
        url: r.url.clone(),
        name: r.name.clone(),
        date_upload: r.date_upload,
        episode_number: r.episode_number,
        scanlator: r.scanlator.clone(),
        summary: r.summary.clone(),
        preview_url: r.preview_url.clone(),
        last_modified_at: r.last_modified_at,
        version: r.version,
        memo: r.memo.0.clone(),
    }
}

/// `EpisodeSanitizer.sanitize`: the same algorithm as the chapter sanitizer, with the anime title as the prefix
/// (T-05a-016). Sync code calls this with the episode name from the source and the anime title.
pub fn sanitize_episode_name(name: &str, anime_title: &str) -> String {
    sanitize_item_name(name, anime_title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::{test_db, insert_anime, insert_episode};

    fn episode(anime_id: i64, url: &str) -> Episode {
        Episode { anime_id, url: url.to_owned(), name: url.to_owned(), ..Episode::create() }
    }

    #[test]
    fn add_all_binds_every_column_and_returns_ids_in_order() {
        let (_dir, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let repo = EpisodeRepo::new(db.clone());
        let mut first = episode(a, "e1");
        first.summary = Some("s".into());
        first.preview_url = Some("p".into());
        first.fillermark = true;
        first.memo.insert("k".into(), serde_json::json!(2));
        first.total_seconds = 1400;
        let added = repo.add_all(&[first.clone(), episode(a, "e2")]).unwrap();
        assert_eq!(added[0].url, "e1");
        assert!(added[0].id < added[1].id);
        let stored = repo.by_id(added[0].id).unwrap().unwrap();
        assert_eq!(stored.summary.as_deref(), Some("s"));
        assert_eq!(stored.preview_url.as_deref(), Some("p"));
        assert!(stored.fillermark);
        assert_eq!(stored.total_seconds, 1400);
        assert_eq!(stored.memo, first.memo);
    }

    #[test]
    fn add_all_with_a_failing_row_leaves_the_table_unchanged() {
        let (_dir, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let repo = EpisodeRepo::new(db.clone());
        assert!(repo.add_all(&[episode(a, "ok"), episode(999, "bad")]).is_err());
        assert!(repo.by_anime_id(a).unwrap().is_empty());
    }

    #[test]
    fn remove_with_ids_chunks_large_lists() {
        let (_dir, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let repo = EpisodeRepo::new(db.clone());
        let eps: Vec<Episode> = (0..1200).map(|i| episode(a, &format!("e{i}"))).collect();
        let ids: Vec<i64> = repo.add_all(&eps).unwrap().iter().map(|e| e.id).collect();
        repo.remove_with_ids(&ids).unwrap();
        assert!(repo.by_anime_id(a).unwrap().is_empty());
    }

    #[test]
    fn by_anime_id_has_no_scanlator_filter_and_bookmarked_lists_only_bookmarks() {
        let (_dir, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let repo = EpisodeRepo::new(db.clone());
        let mut scanned = episode(a, "x");
        scanned.scanlator = Some("G".into());
        let kept = repo.add_all(&[scanned, episode(a, "y")]).unwrap();
        repo.update(&EpisodeUpdate { bookmark: Some(true), ..EpisodeUpdate::new(kept[1].id) }).unwrap();
        assert_eq!(repo.by_anime_id(a).unwrap().len(), 2);
        let bookmarked = repo.bookmarked(a).unwrap();
        assert_eq!(bookmarked.iter().map(|e| e.url.as_str()).collect::<Vec<_>>(), ["y"]);
    }

    #[test]
    fn a_corrupt_memo_does_not_fail_the_list_query() {
        let (_dir, db) = test_db();
        let a = insert_anime(&db, "a", false);
        let id = insert_episode(&db, a, "e");
        db.write(|tx| Ok(tx.execute("UPDATE items SET memo = CAST('not json' AS BLOB) WHERE _id = ?1", [id]).map(drop)?))
            .unwrap();
        let listed = EpisodeRepo::new(db.clone()).by_anime_id(a).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].memo.is_empty());
    }

    #[test]
    fn create_has_version_one_and_date_upload_minus_one() {
        let e = Episode::create();
        assert_eq!((e.version, e.date_upload, e.anime_id), (1, -1, -1));
    }

    #[test]
    fn episode_sanitizer_entry_point_uses_the_anime_title_as_prefix() {
        assert_eq!(sanitize_episode_name("Show 3", "Show"), "3");
        assert_eq!(sanitize_episode_name("Show", "Show"), "");
        assert_eq!(sanitize_episode_name("  Show - Ep 4 ", "Show"), "Ep 4");
    }
}
