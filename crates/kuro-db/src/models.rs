//! Storage row and patch structs of the manga database. Rows map by column name; a patch field of
//! `None` leaves the column alone (`coalesce(:x, col)` in the update statements).

use rusqlite::{Result, Row};

use crate::codec::{GenreList, UpdateStrategy};

/// Row of `mangas`. Field order follows the table; mapping is by column name.
#[derive(Clone, Debug, PartialEq)]
pub struct MangaRow {
    pub id: i64,
    pub source: i64,
    pub url: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<GenreList>,
    pub title: String,
    pub status: i64,
    pub thumbnail_url: Option<String>,
    pub favorite: bool,
    pub last_update: Option<i64>,
    pub next_update: Option<i64>,
    pub initialized: bool,
    pub viewer: i64,
    pub chapter_flags: i64,
    pub cover_last_modified: i64,
    pub date_added: i64,
    pub update_strategy: UpdateStrategy,
    pub calculate_interval: i64,
    pub last_modified_at: i64,
    pub favorite_modified_at: Option<i64>,
    pub version: i64,
    pub is_syncing: bool,
}

impl MangaRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: r.get("_id")?,
            source: r.get("source")?,
            url: r.get("url")?,
            artist: r.get("artist")?,
            author: r.get("author")?,
            description: r.get("description")?,
            genre: r.get("genre")?,
            title: r.get("title")?,
            status: r.get("status")?,
            thumbnail_url: r.get("thumbnail_url")?,
            favorite: r.get("favorite")?,
            last_update: r.get("last_update")?,
            next_update: r.get("next_update")?,
            initialized: r.get("initialized")?,
            viewer: r.get("viewer")?,
            chapter_flags: r.get("chapter_flags")?,
            cover_last_modified: r.get("cover_last_modified")?,
            date_added: r.get("date_added")?,
            update_strategy: r.get("update_strategy")?,
            calculate_interval: r.get("calculate_interval")?,
            last_modified_at: r.get("last_modified_at")?,
            favorite_modified_at: r.get("favorite_modified_at")?,
            version: r.get("version")?,
            is_syncing: r.get("is_syncing")?,
        })
    }
}

/// Partial update of a manga. `None` leaves the column unchanged (`coalesce`), so a column cannot
/// be cleared to NULL through this type.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MangaPatch {
    pub id: i64,
    pub source: Option<i64>,
    pub url: Option<String>,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<GenreList>,
    pub title: Option<String>,
    pub status: Option<i64>,
    pub thumbnail_url: Option<String>,
    pub favorite: Option<bool>,
    pub last_update: Option<i64>,
    pub next_update: Option<i64>,
    pub initialized: Option<bool>,
    pub viewer: Option<i64>,
    pub chapter_flags: Option<i64>,
    pub cover_last_modified: Option<i64>,
    pub date_added: Option<i64>,
    pub update_strategy: Option<UpdateStrategy>,
    pub calculate_interval: Option<i64>,
    pub version: Option<i64>,
    pub is_syncing: Option<bool>,
}

impl MangaPatch {
    /// True when no column is set: `mangas::update` then runs no statement at all.
    pub fn is_empty(&self) -> bool {
        self.source.is_none()
            && self.url.is_none()
            && self.artist.is_none()
            && self.author.is_none()
            && self.description.is_none()
            && self.genre.is_none()
            && self.title.is_none()
            && self.status.is_none()
            && self.thumbnail_url.is_none()
            && self.favorite.is_none()
            && self.last_update.is_none()
            && self.next_update.is_none()
            && self.initialized.is_none()
            && self.viewer.is_none()
            && self.chapter_flags.is_none()
            && self.cover_last_modified.is_none()
            && self.date_added.is_none()
            && self.update_strategy.is_none()
            && self.calculate_interval.is_none()
            && self.version.is_none()
            && self.is_syncing.is_none()
    }
}

/// Row of `chapters`.
#[derive(Clone, Debug, PartialEq)]
pub struct ChapterRow {
    pub id: i64,
    pub manga_id: i64,
    pub url: String,
    pub name: String,
    pub scanlator: Option<String>,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,
    pub chapter_number: f64,
    pub source_order: i64,
    pub date_fetch: i64,
    pub date_upload: i64,
    pub last_modified_at: i64,
    pub version: i64,
    pub is_syncing: bool,
}

impl ChapterRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: r.get("_id")?,
            manga_id: r.get("manga_id")?,
            url: r.get("url")?,
            name: r.get("name")?,
            scanlator: r.get("scanlator")?,
            read: r.get("read")?,
            bookmark: r.get("bookmark")?,
            last_page_read: r.get("last_page_read")?,
            chapter_number: r.get("chapter_number")?,
            source_order: r.get("source_order")?,
            date_fetch: r.get("date_fetch")?,
            date_upload: r.get("date_upload")?,
            last_modified_at: r.get("last_modified_at")?,
            version: r.get("version")?,
            is_syncing: r.get("is_syncing")?,
        })
    }
}

/// Partial update of a chapter; `None` leaves the column unchanged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChapterPatch {
    pub id: i64,
    pub manga_id: Option<i64>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub scanlator: Option<String>,
    pub read: Option<bool>,
    pub bookmark: Option<bool>,
    pub last_page_read: Option<i64>,
    pub chapter_number: Option<f64>,
    pub source_order: Option<i64>,
    pub date_fetch: Option<i64>,
    pub date_upload: Option<i64>,
    pub version: Option<i64>,
    pub is_syncing: Option<bool>,
}

impl ChapterPatch {
    /// True when no column is set: `chapters::update` then runs no statement at all.
    pub fn is_empty(&self) -> bool {
        self.manga_id.is_none()
            && self.url.is_none()
            && self.name.is_none()
            && self.scanlator.is_none()
            && self.read.is_none()
            && self.bookmark.is_none()
            && self.last_page_read.is_none()
            && self.chapter_number.is_none()
            && self.source_order.is_none()
            && self.date_fetch.is_none()
            && self.date_upload.is_none()
            && self.version.is_none()
            && self.is_syncing.is_none()
    }
}

/// Row of `categories`, read positionally as `(id, name, order, flags, hidden)`: the aliased
/// queries and `SELECT *` both produce this order.
#[derive(Clone, Debug, PartialEq)]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub order: i64,
    pub flags: i64,
    pub hidden: bool,
}

impl CategoryRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            name: r.get(1)?,
            order: r.get(2)?,
            flags: r.get(3)?,
            hidden: r.get(4)?,
        })
    }
}

/// Partial update of a category; `None` leaves the column unchanged.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CategoryPatch {
    pub id: i64,
    pub name: Option<String>,
    pub order: Option<i64>,
    pub flags: Option<i64>,
    pub hidden: Option<bool>,
}

/// Row of `history`.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryRow {
    pub id: i64,
    pub chapter_id: i64,
    pub last_read: i64,
    pub time_read: i64,
}

impl HistoryRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: r.get("_id")?,
            chapter_id: r.get("chapter_id")?,
            last_read: r.get("last_read")?,
            time_read: r.get("time_read")?,
        })
    }
}

/// Row of `manga_sync` (tracker binding). `sync_id` is the tracker id, see [`manga_sync`].
#[derive(Clone, Debug, PartialEq)]
pub struct TrackRow {
    pub id: i64,
    pub manga_id: i64,
    pub sync_id: i64,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_chapter_read: f64,
    pub total_chapters: i64,
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,
    pub finish_date: i64,
    pub private: bool,
}

impl TrackRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: r.get("_id")?,
            manga_id: r.get("manga_id")?,
            sync_id: r.get("sync_id")?,
            remote_id: r.get("remote_id")?,
            library_id: r.get("library_id")?,
            title: r.get("title")?,
            last_chapter_read: r.get("last_chapter_read")?,
            total_chapters: r.get("total_chapters")?,
            status: r.get("status")?,
            score: r.get("score")?,
            remote_url: r.get("remote_url")?,
            start_date: r.get("start_date")?,
            finish_date: r.get("finish_date")?,
            private: r.get("private")?,
        })
    }
}

/// Partial update of a track; `None` leaves the column unchanged, so `library_id` cannot be cleared.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackPatch {
    pub id: i64,
    pub manga_id: Option<i64>,
    pub sync_id: Option<i64>,
    pub remote_id: Option<i64>,
    pub library_id: Option<i64>,
    pub title: Option<String>,
    pub last_chapter_read: Option<f64>,
    pub total_chapters: Option<i64>,
    pub status: Option<i64>,
    pub score: Option<f64>,
    pub remote_url: Option<String>,
    pub start_date: Option<i64>,
    pub finish_date: Option<i64>,
    pub private: Option<bool>,
}

/// Row of `sources` (stub source: a source whose extension is no longer installed).
#[derive(Clone, Debug, PartialEq)]
pub struct StubSourceRow {
    pub id: i64,
    pub lang: String,
    pub name: String,
}

impl StubSourceRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self { id: r.get("_id")?, lang: r.get("lang")?, name: r.get("name")? })
    }

    /// `"<name> (<LANG>)"` when name and lang are both non-blank, else the numeric id
    /// (`StubMangaSource.toString`).
    pub fn visual_name(&self) -> String {
        if self.name.trim().is_empty() || self.lang.trim().is_empty() {
            return self.id.to_string();
        }
        format!("{} ({})", self.name, self.lang.to_uppercase())
    }
}

/// Row of `libraryView`: every manga column plus the aggregates.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryRow {
    pub manga: MangaRow,
    pub total_count: i64,
    pub read_count: i64,
    pub latest_upload: i64,
    pub chapter_fetched_at: i64,
    pub last_read: i64,
    pub bookmark_count: i64,
    pub category: i64,
}

impl LibraryRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            manga: MangaRow::from_row(r)?,
            total_count: r.get("totalCount")?,
            read_count: r.get("readCount")?,
            latest_upload: r.get("latestUpload")?,
            chapter_fetched_at: r.get("chapterFetchedAt")?,
            last_read: r.get("lastRead")?,
            bookmark_count: r.get("bookmarkCount")?,
            category: r.get("category")?,
        })
    }
}

/// Row of `historyView`.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryViewRow {
    pub id: i64,
    pub manga_id: i64,
    pub chapter_id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    pub source: i64,
    pub favorite: bool,
    pub cover_last_modified: i64,
    pub chapter_number: f64,
    pub read_at: i64,
    pub read_duration: i64,
}

impl HistoryViewRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: r.get("id")?,
            manga_id: r.get("mangaId")?,
            chapter_id: r.get("chapterId")?,
            title: r.get("title")?,
            thumbnail_url: r.get("thumbnailUrl")?,
            source: r.get("source")?,
            favorite: r.get("favorite")?,
            cover_last_modified: r.get("cover_last_modified")?,
            chapter_number: r.get("chapterNumber")?,
            read_at: r.get("readAt")?,
            read_duration: r.get("readDuration")?,
        })
    }
}

/// Row of `updatesView`. The view names its fetch date `datefetch`.
#[derive(Clone, Debug, PartialEq)]
pub struct UpdateViewRow {
    pub manga_id: i64,
    pub manga_title: String,
    pub chapter_id: i64,
    pub chapter_name: String,
    pub scanlator: Option<String>,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,
    pub source: i64,
    pub favorite: bool,
    pub thumbnail_url: Option<String>,
    pub cover_last_modified: i64,
    pub date_upload: i64,
    pub date_fetch: i64,
}

impl UpdateViewRow {
    pub fn from_row(r: &Row<'_>) -> Result<Self> {
        Ok(Self {
            manga_id: r.get("mangaId")?,
            manga_title: r.get("mangaTitle")?,
            chapter_id: r.get("chapterId")?,
            chapter_name: r.get("chapterName")?,
            scanlator: r.get("scanlator")?,
            read: r.get("read")?,
            bookmark: r.get("bookmark")?,
            last_page_read: r.get("last_page_read")?,
            source: r.get("source")?,
            favorite: r.get("favorite")?,
            thumbnail_url: r.get("thumbnailUrl")?,
            cover_last_modified: r.get("coverLastModified")?,
            date_upload: r.get("dateUpload")?,
            date_fetch: r.get("datefetch")?,
        })
    }
}
