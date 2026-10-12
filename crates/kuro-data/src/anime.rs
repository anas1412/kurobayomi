//! Anime entries in the legacy shape: `Anime`, `AnimeUpdate`, the library and season rows, the deletable parents and
//! `AnimeRepo` (AnimeRepositoryImpl.kt) over the shared `Entry` and `EntryRepo`. Phase E deletes this file.

use kuro_db::codecs::UpdateStrategy;
use kuro_db::schema_anime::{FetchType, Memo};
use kuro_db::subscribe::Subscription;
use kuro_platform::kind::Kind;

use crate::DataResult;
use crate::entry::{self, Entry, EntryUpdate, LibraryEntry, SeasonEntry};
use crate::legacy::{LegacyEntry, LegacyRepo};

/// Upcoming-tab statuses for anime: ONGOING (1), PUBLISHING_FINISHED (4) and ONGOING_ANNOUNCED (7),
/// as `GetUpcomingAnime` passes them.
pub const UPCOMING_STATUSES: [i64; 3] = {
    let statuses = Kind::Anime.upcoming_statuses();
    [statuses[0], statuses[1], statuses[2]]
};

/// One `animes` row as the domain sees it (`Anime.kt`). `is_syncing` is not exposed.
#[derive(Clone, Debug, PartialEq)]
pub struct Anime {
    pub id: i64,
    pub source: i64,
    pub url: String,
    pub title: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: i64,
    pub thumbnail_url: Option<String>,
    pub favorite: bool,
    pub last_update: i64,
    pub next_update: i64,
    pub fetch_interval: i32,
    pub date_added: i64,
    pub viewer_flags: i64,
    pub episode_flags: i64,
    pub cover_last_modified: i64,
    pub update_strategy: UpdateStrategy,
    pub initialized: bool,
    pub last_modified_at: i64,
    pub favorite_modified_at: Option<i64>,
    pub version: i64,
    pub background_last_modified: i64,
    pub background_url: Option<String>,
    pub fetch_type: FetchType,
    pub parent_id: Option<i64>,
    pub season_flags: i64,
    pub season_number: f64,
    pub season_source_order: i64,
    pub memo: serde_json::Map<String, serde_json::Value>,
}

impl Anime {
    /// `Anime.create()`: id -1, source -1, `Episodes`, season number -1.0, empty memo, version 0.
    pub fn create() -> Self {
        Self::from(Entry::create(Kind::Anime))
    }
}

impl From<Entry> for Anime {
    fn from(e: Entry) -> Self {
        Self {
            fetch_type: e.fetch_type(),
            season_flags: e.season_flags(),
            season_number: e.season_number(),
            season_source_order: e.season_source_order(),
            background_last_modified: e.background_last_modified(),
            memo: e.memo(),
            id: e.id,
            source: e.source,
            url: e.url,
            title: e.title,
            artist: e.artist,
            author: e.author,
            description: e.description,
            genre: e.genre,
            status: e.status,
            thumbnail_url: e.thumbnail_url,
            favorite: e.favorite,
            last_update: e.last_update,
            next_update: e.next_update,
            fetch_interval: e.fetch_interval,
            date_added: e.date_added,
            viewer_flags: e.viewer_flags,
            episode_flags: e.item_flags,
            cover_last_modified: e.cover_last_modified,
            update_strategy: e.update_strategy,
            initialized: e.initialized,
            last_modified_at: e.last_modified_at,
            favorite_modified_at: e.favorite_modified_at,
            version: e.version,
            background_url: e.background_url,
            parent_id: e.parent_id,
        }
    }
}

impl From<&Anime> for Entry {
    fn from(a: &Anime) -> Self {
        Self {
            id: a.id,
            kind: Kind::Anime,
            source: a.source,
            url: a.url.clone(),
            title: a.title.clone(),
            artist: a.artist.clone(),
            author: a.author.clone(),
            description: a.description.clone(),
            genre: a.genre.clone(),
            status: a.status,
            thumbnail_url: a.thumbnail_url.clone(),
            favorite: a.favorite,
            last_update: a.last_update,
            next_update: a.next_update,
            fetch_interval: a.fetch_interval,
            date_added: a.date_added,
            viewer_flags: a.viewer_flags,
            item_flags: a.episode_flags,
            cover_last_modified: a.cover_last_modified,
            update_strategy: a.update_strategy,
            initialized: a.initialized,
            last_modified_at: a.last_modified_at,
            favorite_modified_at: a.favorite_modified_at,
            version: a.version,
            fetch_type: Some(a.fetch_type),
            parent_id: a.parent_id,
            season_flags: Some(a.season_flags),
            season_number: Some(a.season_number),
            season_source_order: Some(a.season_source_order),
            background_url: a.background_url.clone(),
            background_last_modified: Some(a.background_last_modified),
            memo: Some(Memo(a.memo.clone())),
        }
    }
}

/// Partial update (`AnimeUpdate.kt`). `None` keeps the stored value. `id` is required: use
/// [`AnimeUpdate::new`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimeUpdate {
    pub id: i64,
    pub source: Option<i64>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub favorite: Option<bool>,
    pub last_update: Option<i64>,
    pub next_update: Option<i64>,
    pub fetch_interval: Option<i32>,
    pub date_added: Option<i64>,
    pub viewer_flags: Option<i64>,
    pub episode_flags: Option<i64>,
    pub cover_last_modified: Option<i64>,
    pub update_strategy: Option<UpdateStrategy>,
    pub initialized: Option<bool>,
    pub version: Option<i64>,
    pub background_last_modified: Option<i64>,
    pub background_url: Option<String>,
    pub fetch_type: Option<FetchType>,
    pub parent_id: Option<i64>,
    pub season_flags: Option<i64>,
    pub season_number: Option<f64>,
    pub season_source_order: Option<i64>,
    pub memo: Option<serde_json::Map<String, serde_json::Value>>,
}

impl AnimeUpdate {
    pub fn new(id: i64) -> Self {
        Self { id, ..Default::default() }
    }
}

impl From<&AnimeUpdate> for EntryUpdate {
    fn from(u: &AnimeUpdate) -> Self {
        Self {
            id: u.id,
            source: u.source,
            url: u.url.clone(),
            title: u.title.clone(),
            artist: u.artist.clone(),
            author: u.author.clone(),
            description: u.description.clone(),
            genre: u.genre.clone(),
            status: u.status,
            thumbnail_url: u.thumbnail_url.clone(),
            favorite: u.favorite,
            last_update: u.last_update,
            next_update: u.next_update,
            fetch_interval: u.fetch_interval,
            date_added: u.date_added,
            viewer_flags: u.viewer_flags,
            item_flags: u.episode_flags,
            cover_last_modified: u.cover_last_modified,
            update_strategy: u.update_strategy,
            initialized: u.initialized,
            version: u.version,
            fetch_type: u.fetch_type,
            parent_id: u.parent_id,
            season_flags: u.season_flags,
            season_number: u.season_number,
            season_source_order: u.season_source_order,
            background_url: u.background_url.clone(),
            background_last_modified: u.background_last_modified,
            memo: u.memo.clone().map(Memo),
        }
    }
}

/// One `libraryView` row (`LibraryAnime.kt`): the anime, its category link and the aggregates
/// (episode counts for `Episodes`, season aggregates for `Seasons`).
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryAnime {
    pub anime: Anime,
    pub category: i64,
    pub total_count: i64,
    pub seen_count: i64,
    pub bookmark_count: i64,
    pub fillermark_count: i64,
    pub latest_upload: i64,
    pub episode_fetched_at: i64,
    pub last_seen: i64,
}

impl LibraryAnime {
    pub fn id(&self) -> i64 {
        self.anime.id
    }

    pub fn unseen_count(&self) -> i64 {
        self.total_count - self.seen_count
    }

    pub fn has_bookmarks(&self) -> bool {
        self.bookmark_count > 0
    }

    pub fn has_started(&self) -> bool {
        self.seen_count > 0
    }
}

impl From<LibraryEntry> for LibraryAnime {
    fn from(r: LibraryEntry) -> Self {
        Self {
            anime: r.entry.into(),
            category: r.category,
            total_count: r.total,
            seen_count: r.done,
            bookmark_count: r.bookmarks,
            fillermark_count: r.fillermarks,
            latest_upload: r.latest_upload,
            episode_fetched_at: r.item_fetched_at,
            last_seen: r.last_activity,
        }
    }
}

/// One season row (`SeasonAnime.kt`): a child of a `Seasons` parent with its aggregates.
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonAnime {
    pub anime: Anime,
    pub total_count: i64,
    pub seen_count: i64,
    pub bookmark_count: i64,
    pub fillermark_count: i64,
    pub latest_upload: i64,
    pub fetched_at: i64,
    pub last_seen: i64,
}

impl SeasonAnime {
    pub fn id(&self) -> i64 {
        self.anime.id
    }

    pub fn seen(&self) -> bool {
        self.total_count == self.seen_count
    }

    pub fn unseen_count(&self) -> i64 {
        self.total_count - self.seen_count
    }

    pub fn has_started(&self) -> bool {
        self.seen_count > 0
    }

    pub fn has_bookmarks(&self) -> bool {
        self.bookmark_count > 0
    }

    pub fn has_fillermarks(&self) -> bool {
        self.fillermark_count > 0
    }

    /// `toLibraryAnime()`: category -1 and the fetch date carried over.
    pub fn to_library_anime(&self) -> LibraryAnime {
        LibraryAnime {
            anime: self.anime.clone(),
            category: -1,
            total_count: self.total_count,
            seen_count: self.seen_count,
            bookmark_count: self.bookmark_count,
            fillermark_count: self.fillermark_count,
            latest_upload: self.latest_upload,
            episode_fetched_at: self.fetched_at,
            last_seen: self.last_seen,
        }
    }
}

impl From<SeasonEntry> for SeasonAnime {
    fn from(r: SeasonEntry) -> Self {
        Self {
            anime: r.entry.into(),
            total_count: r.total,
            seen_count: r.done,
            bookmark_count: r.bookmarks,
            fillermark_count: r.fillermarks,
            latest_upload: r.latest_upload,
            fetched_at: r.fetched_at,
            last_seen: r.last_activity,
        }
    }
}

/// One `animedeletableView` row (`DeletableAnime.kt`): a non-favourite top-level anime.
#[derive(Clone, Debug, PartialEq)]
pub struct DeletableEntry {
    pub anime_id: i64,
    pub source_id: i64,
    pub fetch_type: FetchType,
}

impl From<entry::DeletableEntry> for DeletableEntry {
    fn from(d: entry::DeletableEntry) -> Self {
        Self { anime_id: d.entry_id, source_id: d.source_id, fetch_type: d.fetch_type }
    }
}

/// A named group of related anime (`AnimeRelationGroup.kt`).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimeRelationGroup {
    pub name: String,
    pub anime: Vec<Anime>,
}

impl LegacyEntry for Anime {
    const KIND: Kind = Kind::Anime;
    type Update = AnimeUpdate;
    type Library = LibraryAnime;

    fn to_entry(&self) -> Entry {
        self.into()
    }

    fn to_update(u: &AnimeUpdate) -> EntryUpdate {
        u.into()
    }
}

/// Typed access to the anime rows of `entries`.
pub type AnimeRepo = LegacyRepo<Anime>;

impl AnimeRepo {
    /// Non-favourites with a seen episode or some progress.
    pub fn watched_not_in_library(&self) -> DataResult<Vec<Anime>> {
        Ok(self.entries.watched_not_in_library(Kind::Anime)?.into_iter().map(Anime::from).collect())
    }

    /// Sets the `mask` bits of `episode_flags` to those of `flag` in one UPDATE. Returns whether the row changed.
    pub fn set_episode_flags_masked(&self, anime_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.entries.set_item_flags_masked(anime_id, flag, mask)
    }

    /// Reads `episode_flags`, applies `change` and stores the result in the writer transaction.
    pub fn update_episode_flags(&self, anime_id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        self.entries.update_item_flags(anime_id, change)
    }

    /// Sets the `mask` bits of `season_flags` to those of `flag` in one UPDATE. Returns whether the row changed.
    pub fn set_season_flags_masked(&self, anime_id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.entries.set_season_flags_masked(anime_id, flag, mask)
    }

    /// Reads `season_flags`, applies `change` and stores the result in the writer transaction.
    pub fn update_season_flags(&self, anime_id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        self.entries.update_season_flags(anime_id, change)
    }

    /// Season rows of one `Seasons` parent. Includes non-favourite children.
    pub fn seasons_by_parent(&self, parent_id: i64) -> DataResult<Vec<SeasonAnime>> {
        Ok(self.entries.seasons_by_parent(parent_id)?.into_iter().map(SeasonAnime::from).collect())
    }

    /// Season rows of one parent, published again after a change to any table they read.
    pub fn subscribe_seasons_by_parent(&self, parent_id: i64) -> Subscription<Vec<SeasonAnime>> {
        self.entries.subscribe_seasons_by_parent_as(parent_id)
    }

    /// Every row whose `parent_id` is `parent_id`, favourite or not.
    pub fn children_by_parent(&self, parent_id: i64) -> DataResult<Vec<Anime>> {
        Ok(self.entries.children_by_parent(parent_id)?.into_iter().map(Anime::from).collect())
    }

    /// Clears `parent_id` on the rows named by `ids` (their own `_id`s). Used after a parent is deleted, because
    /// `parent_id` has no foreign key.
    pub fn remove_parent_ids(&self, ids: &[i64]) -> DataResult<()> {
        self.entries.remove_parent_ids(ids)
    }

    /// Non-favourite top-level anime, the candidates of the clear-database screen.
    pub fn deletable_parents(&self) -> DataResult<Vec<DeletableEntry>> {
        Ok(self.entries.deletable_parents(Kind::Anime)?.into_iter().map(DeletableEntry::from).collect())
    }

    /// `deletable_parents`, published again after a change to an anime, its episodes or its history.
    pub fn subscribe_deletable_parents(&self) -> Subscription<Vec<DeletableEntry>> {
        self.entries.subscribe_deletable_parents_as(Kind::Anime)
    }

    /// `(source, anime id)` rows of the non-library anime and of the orphaned children, published again after a change
    /// to an anime, its episodes or its history.
    pub fn subscribe_non_library_tree(&self) -> Subscription<(Vec<(i64, i64)>, Vec<(i64, i64)>)> {
        self.entries.subscribe_non_library_tree(Kind::Anime)
    }

    /// Deletes the non-favourite anime named by `ids`, then the history rows left reset. A favourite is never deleted,
    /// even when its id is passed. Episodes, category links and relations go with the row (foreign keys cascade).
    pub fn delete_non_library_by_ids(&self, ids: &[i64]) -> DataResult<()> {
        self.entries.clear_non_library_ids(ids, &[])
    }
}
