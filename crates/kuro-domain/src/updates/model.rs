//! The update feed row of both kinds: one chapter or episode of a favourite entry, fetched recently.

use kuro_platform::kind::Kind;

/// One line of the updates feed (`MangaUpdatesWithRelations` / `AnimeUpdatesWithRelations`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateRow {
    pub kind: Kind,
    pub entry_id: i64,
    pub entry_title: String,
    pub item_id: i64,
    pub item_name: String,
    pub scanlator: Option<String>,
    /// `read` for manga, `seen` for anime.
    pub done: bool,
    pub bookmark: bool,
    /// Anime only; manga is always false.
    pub fillermark: bool,
    /// `last_page_read` (manga) or `last_second_seen` (anime), in pages or seconds.
    pub progress: i64,
    /// Anime only: `total_seconds`. Manga is 0.
    pub total_seconds: i64,
    pub source_id: i64,
    pub date_fetch: i64,
    pub date_upload: i64,
    pub thumbnail_url: Option<String>,
    pub cover_last_modified: i64,
}
