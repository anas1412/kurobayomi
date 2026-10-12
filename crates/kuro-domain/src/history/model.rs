//! History rows of both kinds, the cover key shown next to a row, and the upsert payload.

use kuro_platform::kind::Kind;

/// The cover of a history row's entry (`MangaCover`): where the image comes from and whether it is cached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverKey {
    pub entry_id: i64,
    pub source_id: i64,
    pub favorite: bool,
    pub url: Option<String>,
    pub cover_last_modified: i64,
}

impl CoverKey {
    /// Defaults of `HistoryWithRelations.create()`: ids -1, no URL.
    pub fn create() -> Self {
        Self { entry_id: -1, source_id: -1, favorite: false, url: None, cover_last_modified: 0 }
    }
}

/// One history row joined with what the list shows: the item, its entry, and the time of the read.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryWithRelations {
    pub id: i64,
    /// The chapter or episode id (`chapter_id` / `episode_id`).
    pub item_id: i64,
    pub entry_id: i64,
    pub title: String,
    /// Chapter or episode number, as the row text shows it.
    pub item_number: f64,
    /// `last_read` / `last_seen` in epoch ms; the list only holds rows where this is above 0.
    pub at: i64,
    /// `time_read` in ms. Anime has no duration and reports 0.
    pub duration_ms: i64,
    pub cover: CoverKey,
}

impl HistoryWithRelations {
    /// Defaults of `HistoryWithRelations.create()`: ids -1, duration -1.
    pub fn create() -> Self {
        Self {
            id: -1,
            item_id: -1,
            entry_id: -1,
            title: String::new(),
            item_number: 0.0,
            at: 0,
            duration_ms: -1,
            cover: CoverKey::create(),
        }
    }
}

/// A plain history row as the repository stores it (`MangaHistory` / `AnimeHistory`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryRow {
    pub id: i64,
    pub item_id: i64,
    /// 0 after a reset (the row stays but leaves the list).
    pub at: i64,
    pub time_read: i64,
}

/// One recording: the item was read or watched at `at`, for `session_ms` (0 for anime).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryUpdate {
    pub kind: Kind,
    pub item_id: i64,
    pub at: i64,
    pub session_ms: i64,
}
