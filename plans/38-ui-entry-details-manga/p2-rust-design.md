# 38 part 2: Rust design for the manga details model, chapter engine, and tracking sheets

Constraints from the project: Rust stable, gtk4-rs 0.11 (feature `v4_16`, no libadwaita), tokio, rusqlite, reqwest, serde. Cross-platform addendum (`plans/addenda/requirements-cross-platform.md`) applies: no `cfg(target_os)` outside `kuro-platform`, no path literals, OS services only through traits, DI through an app context.

Current mockup: single binary crate at `/home/blackbox/kurobayomi` (`Cargo.toml`: `gtk = { package = "gtk4", version = "0.11.5", features = ["v4_16"] }`, serde, serde_json). Modules: `src/main.rs`, `src/entry.rs` (585 lines; manga and anime details, tracking sheet, tracker search, migrate screens), `src/screens.rs`, `src/ui.rs`, `src/spec.rs`, `src/settings.rs`, `src/player.rs`, `src/clamp.rs`. Spec: `spec/entries.json` (sheets, dialogs, notes), `spec/SCHEMA.md`.

---

## 1. Crate and module layout

Target workspace (the mockup becomes the `app` member; no behavior moves into GTK code):

```
/home/blackbox/kurobayomi
  Cargo.toml                       [workspace] members = crates/* + app
  crates/kuro-platform/            (owned by T-req-platform-001; this part only uses the traits)
    src/lib.rs                     traits: Paths, SecretStore, Opener, Notifier
  crates/kuro-chapters/            NEW: pure logic, no GTK, no tokio, no rusqlite
    src/lib.rs
    src/flags.rs                   ChapterFlags bit layout, Sorting, TriState
    src/sort.rs                    comparator (getChapterSort), collator trait
    src/filter.rs                  applyFilters pipeline, downloaded-only filter
    src/missing.rs                 missing_chapters_count, calculate_chapter_gap, separator generator
    src/selection.rs               Selection engine (set + anchors)
    src/next_unread.rs             next unread (view order)
    src/dedupe.rs                  remove_duplicates for reader lists
    src/download_pick.rs           getUnreadChapters, getUnreadChaptersSorted, NEXT_N, UNVIEWED
    src/tests/                     port of MissingItemsTest + derived vectors (see p2-edge-cases-and-tests.md)
  crates/kuro-core/                NEW: state machines and use-cases (no GTK). Depends on kuro-chapters, traits from kuro-platform
    src/lib.rs
    src/ports.rs                   traits the app implements: MangaStore, ChapterStore, Downloads, Trackers, Sources, Clock, Prefs
    src/detail/mod.rs              DetailModel (reducer)
    src/detail/msg.rs              Msg, Cmd, Effect enums
    src/detail/fetch.rs            fetch-from-source use-cases (manga + chapters)
    src/detail/favorite.rs         toggle favorite, duplicate routing, categories
    src/detail/read.rs             mark read/unread, tracker prompt decision
    src/detail/download.rs         download intents (START, START_NOW, CANCEL, DELETE, NEXT_N)
    src/detail/defaults.rs         chapter defaults, set-as-default, reset
    src/tracking/mod.rs            MangaTrackSheet model (home list)
    src/tracking/pickers.rs        StatusPicker, ChapterPicker, ScorePicker, DatePicker (+ constraints)
    src/tracking/search.rs         TrackSearch model
    src/tracking/remove.rs         unlink, remote delete
    src/errors.rs                  DetailError and user-message keys
    src/strings.rs                 message keys (MR/AYMR names) with English text
  crates/kuro-db/                  (owned by area:03/05) rusqlite repos implementing ports::MangaStore/ChapterStore
  app/                             current mockup, moved; GTK views only
    src/main.rs                    builds AppContext (DI), starts tokio runtime, GTK app
    src/ui/detail_manga.rs         replaces src/entry.rs manga branch (chapter list, sheets, dialogs)
    src/ui/chapter_row.rs          glib::Object subclass ChapterRowObject for ListView
    src/ui/track_sheet.rs          tracking dialogs
    src/ui/snackbar.rs             in-window banner (replaces Compose SnackbarHostState and Android Toast)
    src/ui/pickers.rs              date/score/status/chapter picker widgets
    src/entry.rs                   keep for anime and migrate screens until their parts are ported
    src/spec.rs                    keeps static labels and ordering from spec/*.json
```

Rule: `kuro-chapters` and `kuro-core` never import `gtk`, `glib`, or `tokio::fs`. They are testable with `cargo test` and no display.

---

## 2. Ports (traits the core needs; implemented by kuro-db, kuro-platform, and the download/track areas)

Use the `async-trait` crate for dyn-safe async traits (the core holds `Arc<dyn ...>`).

```rust
// crates/kuro-core/src/ports.rs
use async_trait::async_trait;
use kuro_chapters::flags::ChapterFlags;
use std::sync::Arc;
use tokio::sync::broadcast;

pub type Id = i64;

#[derive(Clone, Debug, PartialEq)]
pub struct Manga {
    pub id: Id,
    pub source: Id,
    pub favorite: bool,
    pub title: String,
    pub url: String,
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: i64,
    pub thumbnail_url: Option<String>,
    pub initialized: bool,
    pub fetch_interval: i32,         // negative = custom (stored as -days), 0 = automatic
    pub next_update: i64,
    pub chapter_flags: ChapterFlags,
    pub cover_last_modified: i64,
    pub date_added: i64,
    pub update_strategy: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    pub id: Id,
    pub manga_id: Id,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,
    pub source_order: i64,
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    pub chapter_number: f64,         // -1.0 = unrecognised
    pub scanlator: Option<String>,   // blank is stored as None
}

impl Chapter {
    pub fn is_recognized(&self) -> bool { self.chapter_number >= 0.0 }
}

#[derive(Default, Clone, Debug)]
pub struct MangaUpdate {
    pub id: Id,
    pub favorite: Option<bool>,
    pub date_added: Option<i64>,
    pub chapter_flags: Option<ChapterFlags>,
    pub fetch_interval: Option<i32>,
    pub next_update: Option<i64>,
    pub cover_last_modified: Option<i64>,
    pub initialized: Option<bool>,
    // fields from a source refresh are grouped in SourceRefresh (see fetch.rs)
}

#[derive(Clone, Debug)]
pub struct ChapterUpdate {
    pub id: Id,
    pub read: Option<bool>,
    pub bookmark: Option<bool>,
    pub last_page_read: Option<i64>,
}

#[async_trait]
pub trait MangaStore: Send + Sync {
    async fn get(&self, id: Id) -> Result<Option<Manga>, StoreError>;
    /// Emits the manga row on every change. Must emit the current value first.
    fn watch(&self, id: Id) -> broadcast::Receiver<Manga>;
    async fn update(&self, u: MangaUpdate) -> Result<bool, StoreError>;
    /// Atomic masked write: new = (old & !mask) | (flag & mask), done in SQL (fixes E-FLAG-1).
    async fn set_flag_bits(&self, id: Id, flag: u64, mask: u64) -> Result<bool, StoreError>;
    async fn duplicate_library_titles(&self, id: Id, title_lower: &str) -> Result<Vec<Manga>, StoreError>;
    async fn set_categories(&self, manga_id: Id, category_ids: &[Id]) -> Result<(), StoreError>;
    /// Set favorite=true AND categories in ONE transaction (fixes the race in behavior 4.4).
    async fn add_to_library(&self, manga_id: Id, category_ids: &[Id]) -> Result<(), StoreError>;
    async fn excluded_scanlators(&self, manga_id: Id) -> Result<Vec<String>, StoreError>;
    async fn set_excluded_scanlators(&self, manga_id: Id, set: &[String]) -> Result<(), StoreError>;
}

#[async_trait]
pub trait ChapterStore: Send + Sync {
    /// Rows for the manga with excluded scanlators removed (applyScanlatorFilter = true).
    async fn list(&self, manga_id: Id, apply_scanlator_filter: bool) -> Result<Vec<Chapter>, StoreError>;
    fn watch(&self, manga_id: Id) -> broadcast::Receiver<Vec<Chapter>>;
    async fn update_many(&self, updates: Vec<ChapterUpdate>) -> Result<(), StoreError>;
    async fn available_scanlators(&self, manga_id: Id) -> Result<Vec<String>, StoreError>; // raw, unfiltered
}

#[derive(Clone, Debug)]
pub struct DownloadEvent {
    pub manga_id: Id,
    pub chapter_id: Id,
    pub status: DownloadState,
    pub progress: u8,              // 0..=100
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState { NotDownloaded = 0, Queue = 1, Downloading = 2, Downloaded = 3, Error = 4 }

pub trait Downloads: Send + Sync {
    fn events(&self) -> broadcast::Receiver<DownloadEvent>;
    /// Current queue entry for a chapter, if any (Kotlin: getQueuedDownloadOrNull).
    fn queued(&self, chapter_id: Id) -> Option<(DownloadState, u8)>;
    /// Cache-backed check (Kotlin: isChapterDownloaded with skipCache = false).
    fn is_chapter_downloaded(&self, name: &str, scanlator: Option<&str>, manga_title: &str, source_id: Id) -> bool;
    fn download_count(&self, manga_id: Id) -> usize;
    fn enqueue(&self, manga: &Manga, chapters: Vec<Chapter>, auto_start: bool);
    fn start_now(&self, chapter_id: Id);
    fn start_all(&self);
    fn cancel(&self, chapter_ids: &[Id]);
    fn delete_chapters(&self, manga: &Manga, chapters: Vec<Chapter>);
    fn delete_manga(&self, manga: &Manga, remove_queued: bool);
}

#[async_trait]
pub trait Sources: Send + Sync {
    /// Returns Err(SourceError::NotInstalled) when the source is missing.
    async fn manga_details(&self, manga: &Manga) -> Result<SourceManga, SourceError>;
    async fn chapter_list(&self, manga: &Manga) -> Result<Vec<SourceChapter>, SourceError>;
    fn is_local(&self, source_id: Id) -> bool;
}

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
    fn today(&self) -> chrono::NaiveDate;   // system local zone
}

pub trait Prefs: Send + Sync {
    fn chapter_filter_read(&self) -> u64;           // default 0
    fn chapter_filter_downloaded(&self) -> u64;     // default 0
    fn chapter_filter_bookmarked(&self) -> u64;     // default 0
    fn chapter_sort_mode(&self) -> u64;             // default 0 (source)
    fn chapter_display_mode(&self) -> u64;          // default 0 (name)
    fn chapter_sort_desc_pref(&self) -> bool;       // default true (CHAPTER_SORT_DESC = 0)
    fn default_category(&self) -> i64;              // default -1
    fn skip_filtered(&self) -> bool;                // default true
    fn downloaded_only(&self) -> bool;              // default false
    fn track_on_add(&self) -> bool;                 // default true
    fn auto_track_on_read(&self) -> AutoTrackState; // default Always
    fn remove_download_after_read(&self) -> bool;   // default false
    fn set_chapter_defaults(&self, d: &ChapterDefaults);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoTrackState { Always, Ask, Never }

/// The six keys written by LibraryPreferences.setChapterSettingsDefault (LibraryPreferences.kt:249-258).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChapterDefaults {
    pub unread: u64,        // unreadFilterRaw (0, 0x2 or 0x4)
    pub downloaded: u64,    // downloadedFilterRaw (0, 0x8 or 0x10)
    pub bookmarked: u64,    // bookmarkedFilterRaw (0, 0x20 or 0x40)
    pub sorting: u64,       // sorting (0, 0x100, 0x200 or 0x300)
    pub display: u64,       // displayMode (0 or 0x100000)
    pub sort_desc: bool,    // true -> CHAPTER_SORT_DESC (0), false -> CHAPTER_SORT_ASC (1)
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Db(String),
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source not installed")]
    NotInstalled,
    #[error("http {code}")]
    Http { code: u16 },
    #[error("offline")]
    Offline,
    #[error("unknown host: {0}")]
    UnknownHost(String),
    #[error("{0}")]
    Other(String),
}

/// Mirrors eu.kanade.tachiyomi.source.model.SManga as used by toSManga() (app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:38-48)
/// and by UpdateManga.awaitUpdateFromSource (UpdateManga.kt:29-77).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceManga {
    pub url: String,
    pub title: Option<String>,        // Kotlin lateinit: absent = ""
    pub artist: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,   // None = source did not send genres (copyFrom keeps the local value); parsed by SManga.getGenres() (separator owned by area:15)
    pub status: i32,
    pub thumbnail_url: Option<String>,
    pub initialized: bool,
    pub update_strategy: i64,
}

/// Mirrors SChapter as used by Chapter.copyFromSChapter (app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt:19-26).
#[derive(Clone, Debug, PartialEq)]
pub struct SourceChapter {
    pub url: String,
    pub name: String,
    pub date_upload: i64,
    pub chapter_number: f64,          // -1.0 when the source did not provide one
    pub scanlator: Option<String>,    // blank becomes None (ifBlank { null }, then trim)
}

/// Everything the details model needs, injected once.
#[derive(Clone)]
pub struct Services {
    pub mangas: Arc<dyn MangaStore>,
    pub chapters: Arc<dyn ChapterStore>,
    pub downloads: Arc<dyn Downloads>,
    pub sources: Arc<dyn Sources>,
    pub trackers: Arc<dyn Trackers>,
    pub clock: Arc<dyn Clock>,
    pub prefs: Arc<dyn Prefs>,
    pub categories: Arc<dyn Categories>,
}
```

Trackers and categories are separate traits (area:27 and area:08 implement them):

```rust
#[async_trait]
pub trait Trackers: Send + Sync {
    fn logged_in_manga_trackers(&self) -> Vec<Arc<dyn MangaTracker>>;   // MangaTracker only (MTID:269-271)
    fn get(&self, tracker_id: Id) -> Option<Arc<dyn MangaTracker>>;
    fn tracks_for(&self, manga_id: Id) -> Vec<MangaTrack>;             // watch via tokio broadcast in impl
    fn watch_tracks(&self, manga_id: Id) -> broadcast::Receiver<Vec<MangaTrack>>;
    /// RefreshMangaTracks: returns failures only (service id, error text)
    async fn refresh_all(&self, manga_id: Id) -> Vec<(Id, String)>;
    /// TrackChapter: push lastChapterRead to every logged-in track that is behind
    async fn push_last_chapter_read(&self, manga_id: Id, chapter_number: f64) -> Result<(), String>;
    async fn bind_enhanced(&self, manga: &Manga, source_id: Id) -> Result<(), String>;
}

#[async_trait]
pub trait MangaTracker: Send + Sync {
    fn id(&self) -> Id;
    fn name(&self) -> &str;
    fn status_list(&self) -> Vec<i64>;                 // getStatusListManga, order preserved
    fn status_label(&self, status: i64) -> Option<String>;
    fn score_list(&self) -> Vec<String>;
    fn display_score(&self, track: &MangaTrack) -> String;
    fn supports_private(&self) -> bool;
    fn accepts_source(&self, source_id: Id) -> bool;   // EnhancedMangaTracker.accept, default true
    fn enhanced(&self) -> Option<&dyn EnhancedMangaTracker> { None }
    fn deletable(&self) -> Option<&dyn DeletableMangaTracker> { None }
    async fn search(&self, query: &str) -> Result<Vec<TrackSearchResult>, String>;
    async fn register(&self, item: TrackSearchResult, manga_id: Id) -> Result<(), String>;
    async fn set_status(&self, track: &MangaTrack, status: i64) -> Result<(), String>;
    async fn set_last_chapter_read(&self, track: &MangaTrack, chapter: i32) -> Result<(), String>;
    async fn set_score(&self, track: &MangaTrack, score: &str) -> Result<(), String>;
    async fn set_start_date(&self, track: &MangaTrack, epoch_ms: i64) -> Result<(), String>; // 0 = remove
    async fn set_finish_date(&self, track: &MangaTrack, epoch_ms: i64) -> Result<(), String>;
    async fn set_private(&self, track: &MangaTrack, private: bool) -> Result<(), String>;
}

#[async_trait]
pub trait EnhancedMangaTracker: Send + Sync {
    async fn match_manga(&self, manga: &Manga) -> Result<Option<TrackSearchResult>, String>;
}

#[async_trait]
pub trait DeletableMangaTracker: Send + Sync {
    async fn delete_remote(&self, track: &MangaTrack) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TrackSearchResult {
    pub tracker_id: Id,
    pub remote_id: i64,            // identity together with tracker_id (equals() in MangaTrackSearch)
    pub title: String,
    pub tracking_url: String,
    pub cover_url: String,
    pub summary: String,
    pub authors: Vec<String>,
    pub artists: Vec<String>,
    pub total_chapters: i64,
    pub private: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MangaTrack {
    pub id: Id,
    pub manga_id: Id,
    pub tracker_id: Id,
    pub remote_id: i64,
    pub library_id: Option<i64>,
    pub title: String,
    pub last_chapter_read: f64,
    pub total_chapters: i64,
    pub status: i64,
    pub score: f64,
    pub remote_url: String,
    pub start_date: i64,           // epoch ms, 0 = unset
    pub finish_date: i64,
    pub private: bool,
}

#[async_trait]
pub trait Categories: Send + Sync {
    /// User categories only: id != 0 (Category.isSystemCategory)
    async fn user_categories(&self) -> Result<Vec<Category>, StoreError>;
    async fn categories_of(&self, manga_id: Id) -> Result<Vec<Id>, StoreError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category { pub id: Id, pub name: String, pub order: i64, pub flags: i64, pub hidden: bool }
```

---

## 3. Chapter engine (pure, `kuro-chapters`)

### 3.1 Flags

```rust
// crates/kuro-chapters/src/flags.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ChapterFlags(pub u64);

pub mod bits {
    pub const SHOW_ALL: u64 = 0x0000_0000;
    pub const SORT_DESC: u64 = 0x0000_0000;
    pub const SORT_ASC: u64 = 0x0000_0001;
    pub const SORT_DIR_MASK: u64 = 0x0000_0001;
    pub const SHOW_UNREAD: u64 = 0x0000_0002;
    pub const SHOW_READ: u64 = 0x0000_0004;
    pub const UNREAD_MASK: u64 = 0x0000_0006;
    pub const SHOW_DOWNLOADED: u64 = 0x0000_0008;
    pub const SHOW_NOT_DOWNLOADED: u64 = 0x0000_0010;
    pub const DOWNLOADED_MASK: u64 = 0x0000_0018;
    pub const SHOW_BOOKMARKED: u64 = 0x0000_0020;
    pub const SHOW_NOT_BOOKMARKED: u64 = 0x0000_0040;
    pub const BOOKMARKED_MASK: u64 = 0x0000_0060;
    pub const SORTING_SOURCE: u64 = 0x0000_0000;
    pub const SORTING_NUMBER: u64 = 0x0000_0100;
    pub const SORTING_UPLOAD_DATE: u64 = 0x0000_0200;
    pub const SORTING_ALPHABET: u64 = 0x0000_0300;
    pub const SORTING_MASK: u64 = 0x0000_0300;
    pub const DISPLAY_NAME: u64 = 0x0000_0000;
    pub const DISPLAY_NUMBER: u64 = 0x0010_0000;
    pub const DISPLAY_MASK: u64 = 0x0010_0000;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState { Disabled, EnabledIs, EnabledNot }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sorting { Source, Number, UploadDate, Alphabet }

impl ChapterFlags {
    /// Kotlin: (old and mask.inv()) or (flag and mask)
    #[must_use]
    pub const fn with_masked(self, flag: u64, mask: u64) -> Self {
        ChapterFlags((self.0 & !mask) | (flag & mask))
    }
    pub const fn sorting(self) -> Sorting {
        match self.0 & bits::SORTING_MASK {
            bits::SORTING_NUMBER => Sorting::Number,
            bits::SORTING_UPLOAD_DATE => Sorting::UploadDate,
            bits::SORTING_ALPHABET => Sorting::Alphabet,
            _ => Sorting::Source,           // 0x000; unknown values are rejected upstream (NotImplementedError)
        }
    }
    pub const fn sort_descending(self) -> bool { self.0 & bits::SORT_DIR_MASK == bits::SORT_DESC }
    pub const fn unread(self) -> TriState {
        match self.0 & bits::UNREAD_MASK {
            bits::SHOW_UNREAD => TriState::EnabledIs,
            bits::SHOW_READ => TriState::EnabledNot,
            _ => TriState::Disabled,
        }
    }
    pub const fn bookmarked(self) -> TriState {
        match self.0 & bits::BOOKMARKED_MASK {
            bits::SHOW_BOOKMARKED => TriState::EnabledIs,
            bits::SHOW_NOT_BOOKMARKED => TriState::EnabledNot,
            _ => TriState::Disabled,
        }
    }
    /// downloadedFilter; `downloaded_only` forces EnabledIs (Manga.kt:25)
    pub const fn downloaded(self, downloaded_only: bool) -> TriState {
        if downloaded_only { return TriState::EnabledIs; }
        match self.0 & bits::DOWNLOADED_MASK {
            bits::SHOW_DOWNLOADED => TriState::EnabledIs,
            bits::SHOW_NOT_DOWNLOADED => TriState::EnabledNot,
            _ => TriState::Disabled,
        }
    }
    pub const fn filter_active(self, downloaded_only: bool) -> bool {
        !matches!(self.unread(), TriState::Disabled)
            || !matches!(self.downloaded(downloaded_only), TriState::Disabled)
            || !matches!(self.bookmarked(), TriState::Disabled)
    }
}

// Setter helpers mirroring SetMangaChapterFlags
pub fn unread_flag(s: TriState) -> u64 {
    match s { TriState::Disabled => bits::SHOW_ALL, TriState::EnabledIs => bits::SHOW_UNREAD, TriState::EnabledNot => bits::SHOW_READ }
}
pub fn downloaded_flag(s: TriState) -> u64 {
    match s { TriState::Disabled => bits::SHOW_ALL, TriState::EnabledIs => bits::SHOW_DOWNLOADED, TriState::EnabledNot => bits::SHOW_NOT_DOWNLOADED }
}
pub fn bookmarked_flag(s: TriState) -> u64 {
    match s { TriState::Disabled => bits::SHOW_ALL, TriState::EnabledIs => bits::SHOW_BOOKMARKED, TriState::EnabledNot => bits::SHOW_NOT_BOOKMARKED }
}
```

Unit-tested invariant: `ChapterFlags(0x1A).with_masked(0x8, 0x18).0 == 0x0A` (behavior section 11).

### 3.2 Sort comparator

```rust
// crates/kuro-chapters/src/sort.rs
use std::cmp::Ordering;

pub trait Collate {
    /// Primary-strength comparison (case- and accent-insensitive), Java Collator PRIMARY equivalent
    fn cmp_primary(&self, a: &str, b: &str) -> Ordering;
}

/// Mirrors getChapterSort(manga, sortDescending). NOTE the SOURCE branch is intentionally
/// inverted: with descending = true it sorts sourceOrder ASCENDING (newest first, because
/// sourceOrder 0 is the first item the source lists).
pub fn compare(
    sorting: Sorting,
    descending: bool,
    a: &Chapter,
    b: &Chapter,
    collate: &dyn Collate,
) -> Ordering {
    use Ordering::*;
    match (sorting, descending) {
        (Sorting::Source, true) => a.source_order.cmp(&b.source_order),
        (Sorting::Source, false) => b.source_order.cmp(&a.source_order),
        (Sorting::Number, true) => b.chapter_number.total_cmp(&a.chapter_number),
        (Sorting::Number, false) => a.chapter_number.total_cmp(&b.chapter_number),
        (Sorting::UploadDate, true) => b.date_upload.cmp(&a.date_upload),
        (Sorting::UploadDate, false) => a.date_upload.cmp(&b.date_upload),
        (Sorting::Alphabet, true) => collate.cmp_primary(&b.name, &a.name),
        (Sorting::Alphabet, false) => collate.cmp_primary(&a.name, &b.name),
    }
}

/// Stable sort (Kotlin sortedWith is stable).
pub fn sort_chapters(v: &mut [Chapter], sorting: Sorting, descending: bool, collate: &dyn Collate) {
    v.sort_by(|a, b| compare(sorting, descending, a, b, collate));
}
```

Collation implementation (in `kuro-chapters` behind a feature, default on): `icu_collator` with `CollatorOptions { strength: Some(Strength::Primary), .. }` and locale from `sys-locale::get_locale()`, fallback `und`. Port test: ICU PRIMARY and Java PRIMARY agree on ASCII letters and common Latin accents; they can disagree on some punctuation and CJK ordering. Record the differences in the test fixtures (E-SORT-1).

### 3.3 View filter pipeline

```rust
// crates/kuro-chapters/src/filter.rs
pub struct ChapterRow {
    pub chapter: Chapter,
    pub download: DownloadState,     // see kuro-core ports
    pub progress: u8,
    pub selected: bool,
}

impl ChapterRow {
    pub fn is_downloaded(&self) -> bool { self.download == DownloadState::Downloaded }
}

/// Order matters only for performance; the result is the same set.
/// 1) unread  2) bookmarked  3) downloaded (is_downloaded || is_local)  4) sort
pub fn apply_view(rows: &[ChapterRow], flags: ChapterFlags, is_local: bool, downloaded_only: bool, collate: &dyn Collate) -> Vec<usize> {
    // returns indices into `rows` in view order (indices keep selection and id lookups cheap)
    let unread = flags.unread();
    let bm = flags.bookmarked();
    let dl = flags.downloaded(downloaded_only);
    let mut idx: Vec<usize> = (0..rows.len())
        .filter(|&i| keep(unread, !rows[i].chapter.read))
        .filter(|&i| keep(bm, rows[i].chapter.bookmark))
        .filter(|&i| keep(dl, rows[i].is_downloaded() || is_local))
        .collect();
    idx.sort_by(|&a, &b| compare(flags.sorting(), flags.sort_descending(), &rows[a].chapter, &rows[b].chapter, collate));
    idx
}

/// applyFilter(TriState) { predicate }  (TriState.kt:5-9)
fn keep(t: TriState, pred: bool) -> bool {
    match t { TriState::Disabled => true, TriState::EnabledIs => pred, TriState::EnabledNot => !pred }
}

/// filterDownloadedChapters (ChapterFilterDownloaded.kt): returns input unchanged for local manga
pub fn downloaded_only_list<'a>(chapters: &'a [Chapter], is_local: bool, is_downloaded: impl Fn(&Chapter) -> bool) -> Vec<&'a Chapter> {
    if is_local { return chapters.iter().collect(); }
    chapters.iter().filter(|c| is_downloaded(c)).collect()
}
```

Note: `apply_view` returns indices. The view is recomputed when (a) the chapter list changes, (b) a filter/sort flag changes, (c) a download state changes that affects `is_downloaded` (only when the downloaded filter is active or the sort is affected; otherwise only the row is updated in place). Cache the last view and invalidate by a generation counter.

### 3.4 Missing chapters

```rust
// crates/kuro-chapters/src/missing.rs
/// MissingChapters.kt:46-49
pub fn gap(higher: f64, lower: f64) -> i64 {
    if higher < 0.0 || lower < 0.0 { return 0; }
    higher.floor() as i64 - lower.floor() as i64 - 1
}
/// calculateChapterGap(Chapter?, Chapter?) : 0 if either missing or unrecognised (MissingChapters.kt:40-44)
pub fn gap_chapters(higher: Option<&Chapter>, lower: Option<&Chapter>) -> i64 {
    match (higher, lower) {
        (Some(h), Some(l)) if h.is_recognized() && l.is_recognized() => gap(h.chapter_number, l.chapter_number),
        _ => 0,
    }
}
/// missingChaptersCount (MissingChapters.kt:6-38), port for the test suite
pub fn missing_count(numbers: &[f64]) -> i64 {
    let mut items: Vec<i64> = numbers.iter().filter(|&&n| n != -1.0).map(|&n| n as i64).collect();
    items.sort_unstable();
    items.dedup();
    let mut missing = 0;
    let mut prev = 0i64;
    for cur in items {
        if cur > prev + 1 { missing += cur - prev - 1; }
        prev = cur;
    }
    missing
}

#[derive(Clone, Debug, PartialEq)]
pub enum Separator {
    Missing { after: Option<Id>, before: Id, count: u32 },
}
// stable id used by the list: format!("{}-{}", after.map_or("null".into(), |v| v.to_string()), before)
```

Separator generator (behavior 6.7), port in `missing.rs`:

```rust
/// rows: view-ordered chapters. Emits Separator before index i (or at end) exactly as insertSeparators does.
pub fn separators(view: &[&Chapter], descending: bool) -> Vec<(usize, Separator)> {
    // i runs from -1..len; (before, after) = (view[i-1], view[i]); swap for descending
    let mut out = Vec::new();
    let n = view.len() as isize;
    for i in -1..n {
        let before = if i >= 0 { Some(view[i as usize]) } else { None };
        let after = if i + 1 < n { Some(view[(i + 1) as usize]) } else { None };
        let (lower, higher) = if descending { (after, before) } else { (before, after) };
        let Some(higher) = higher else { continue };
        let count = match lower {
            None => ((higher.chapter_number.floor() as i64) - 1).max(0),
            Some(l) => gap_chapters(Some(higher), Some(l)),
        };
        if count > 0 {
            out.push(((i + 1) as usize, Separator::Missing {
                after: lower.map(|l| l.id),
                before: higher.id,
                count: count as u32,
            }));
        }
    }
    out
}
```

Caveat: `higher.chapter_number.floor() as i64` for unrecognised `-1.0` yields `-1 - 1 = -2`, clamped to 0. Matches Kotlin (`coerceAtLeast(0)`).

### 3.5 Selection engine

```rust
// crates/kuro-chapters/src/selection.rs
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Programmatic,           // userSelected = false
    Tap,                    // userSelected = true, fromLongPress = false
    LongPress,              // userSelected = true, fromLongPress = true
}

/// Selection is a set of chapter ids plus range anchors stored as CHAPTER IDS (not indices).
/// Anchors are resolved against the current view on each use, which fixes stale-index bugs
/// that come from filters changing the view.
#[derive(Default, Clone, Debug)]
pub struct Selection {
    pub ids: HashSet<Id>,
    pub first_anchor: Option<Id>,
    pub last_anchor: Option<Id>,
}

impl Selection {
    /// view: current processed order (ids). Returns true when the selection changed.
    pub fn toggle(&mut self, view: &[Id], chapter_id: Id, selected: bool, gesture: Gesture) -> bool {
        let Some(pos) = view.iter().position(|&i| i == chapter_id) else { return false };
        let currently = self.ids.contains(&chapter_id);
        if currently == selected { return false; }
        let first_selection = !view.iter().any(|i| self.ids.contains(i));
        if selected { self.ids.insert(chapter_id) } else { self.ids.remove(&chapter_id) };
        match gesture {
            Gesture::LongPress if selected => {
                if first_selection {
                    self.first_anchor = Some(chapter_id);
                    self.last_anchor = Some(chapter_id);
                } else {
                    let lo = self.first_anchor.and_then(|a| view.iter().position(|&i| i == a));
                    let hi = self.last_anchor.and_then(|a| view.iter().position(|&i| i == a));
                    let (range, new_lo, new_hi) = match (lo, hi) {
                        (Some(lo), Some(hi)) if pos < lo => (pos + 1..lo, pos, hi),
                        (Some(lo), Some(hi)) if pos > hi => (hi + 1..pos, lo, pos),
                        _ => (0..0, lo.unwrap_or(pos), hi.unwrap_or(pos)),
                    };
                    self.first_anchor = Some(view[new_lo]);
                    self.last_anchor = Some(view[new_hi]);
                    for &id in &view[range] { self.ids.insert(id); }
                }
            }
            Gesture::Tap => {
                // Mirror Kotlin: on deselect at an anchor, move the anchor to the first/last selected item
                if !selected {
                    if self.first_anchor == Some(chapter_id) {
                        self.first_anchor = view.iter().find(|i| self.ids.contains(i)).copied();
                    } else if self.last_anchor == Some(chapter_id) {
                        self.last_anchor = view.iter().rev().find(|i| self.ids.contains(i)).copied();
                    }
                } else {
                    // extend the anchors outward only (upstream: idx < first or idx > last)
                    let lo = self.first_anchor.and_then(|a| view.iter().position(|&i| i == a));
                    let hi = self.last_anchor.and_then(|a| view.iter().position(|&i| i == a));
                    if lo.map_or(true, |l| pos < l) { self.first_anchor = Some(chapter_id); }
                    if hi.map_or(true, |h| pos > h) { self.last_anchor = Some(chapter_id); }
                }
            }
            _ => {}
        }
        true
    }

    /// toggleAllSelection / invertSelection apply to the RAW list (behavior 6.8); callers pass raw ids.
    pub fn set_all(&mut self, raw_ids: &[Id], selected: bool) {
        for &id in raw_ids { if selected { self.ids.insert(id); } else { self.ids.remove(&id); } }
        self.first_anchor = None;
        self.last_anchor = None;
    }
    pub fn invert(&mut self, raw_ids: &[Id]) {
        for &id in raw_ids {
            if self.ids.contains(&id) { self.ids.remove(&id); } else { self.ids.insert(id); }
        }
        self.first_anchor = None;
        self.last_anchor = None;
    }
}
```

Note: the Tap "extend outward" branch deviates from upstream when an anchor is `-1` (upstream never sets `[0] = idx` if `[0] == -1`). Proposal: anchors are always set on first tap (documented change, E-SEL-3). Long-press range semantics otherwise match upstream.

### 3.6 Next unread, dedupe, download pick

```rust
// crates/kuro-chapters/src/next_unread.rs
/// view: rows in view order. Kotlin: sortDescending ? findLast : find (ChapterGetNextUnread.kt)
pub fn next_unread(view: &[&Chapter], descending: bool) -> Option<Id> {
    if descending { view.iter().rev().find(|c| !c.read).map(|c| c.id) }
    else { view.iter().find(|c| !c.read).map(|c| c.id) }
}
```

```rust
// crates/kuro-chapters/src/dedupe.rs  (ChapterRemoveDuplicates.kt)
/// Groups by chapter_number in first-seen order; per group: id == current, else same scanlator, else first.
pub fn remove_duplicates(chapters: &[Chapter], current: &Chapter) -> Vec<Chapter> {
    let mut order: Vec<f64> = Vec::new();
    let mut groups: std::collections::HashMap<u64, Vec<&Chapter>> = Default::default();
    for c in chapters {
        let key = c.chapter_number.to_bits();
        if !groups.contains_key(&key) { order.push(c.chapter_number); }
        groups.entry(key).or_default().push(c);
    }
    order.iter().map(|n| {
        let g = &groups[&n.to_bits()];
        (*g.iter().find(|c| c.id == current.id)
            .or_else(|| g.iter().find(|c| c.scanlator == current.scanlator))
            .unwrap_or(&g[0])).clone()
    }).collect()
}
```

Note: `to_bits` grouping distinguishes `0.0` and `-0.0`; Kotlin `groupBy` uses `Double.equals` which also distinguishes them (boxed `Double.equals`). Matching behavior.

```rust
// crates/kuro-chapters/src/download_pick.rs
/// getUnreadChapters: keep !read && download == NotDownloaded (exact; ERROR/QUEUE/DOWNLOADING excluded)
pub fn unread_not_downloaded<'a>(source: &'a [ChapterRow]) -> Vec<&'a ChapterRow> {
    source.iter().filter(|r| !r.chapter.read && r.download == DownloadState::NotDownloaded).collect()
}
/// getUnreadChaptersSorted: sort with the same comparator then REVERSE when descending (oldest first)
pub fn unread_sorted_oldest_first(rows: &[&ChapterRow], flags: ChapterFlags, collate: &dyn Collate) -> Vec<Id> {
    let mut v: Vec<&Chapter> = rows.iter().map(|r| &r.chapter).collect();
    v.sort_by(|a, b| compare(flags.sorting(), flags.sort_descending(), a, b, collate));
    if flags.sort_descending() { v.reverse(); }
    v.into_iter().map(|c| c.id).collect()
}
pub fn take_n(ids: &[Id], n: usize) -> Vec<Id> { ids.iter().take(n).copied().collect() }
```

`DownloadAction` mapping (`NEXT_1_ITEM=1`, `NEXT_5_ITEMS=5`, `NEXT_10_ITEMS=10`, `NEXT_25_ITEMS=25`; `UNVIEWED_ITEMS` = `unread_not_downloaded` in view order, not sorted) lives in `kuro-core::detail::download`.

---

## 4. The details model (`kuro-core::detail`)

### 4.1 Pattern: reducer + commands

The Kotlin ScreenModel mixes state mutation with IO launched from inside. The port separates them:

- `DetailModel::update(&mut self, msg: Msg) -> Vec<Cmd>`: pure state transition (no IO, no await). Synchronous. Runs only on the GTK main thread.
- `Cmd`: an async job description. The app's executor runs it on the tokio runtime; results come back as `Msg`.
- The UI renders from `DetailModel` after each `update` (GTK main thread).

```rust
// crates/kuro-core/src/detail/msg.rs
#[derive(Debug)]
pub enum Msg {
    // loading
    MangaLoaded(Manga),
    ChaptersLoaded(Vec<Chapter>),
    ScanlatorsLoaded { available: Vec<String>, excluded: Vec<String> },
    DownloadEvent(DownloadEvent),
    TrackingCounts { tracking_count: usize, has_logged_in: bool },
    InitialLoadDone,
    // fetch
    RefreshRequested { manual: bool },
    SourceFetchFinished { manga_part: Result<(), SourceError>, chapter_part: Result<Vec<Chapter>, SourceError> },
    // user intents
    ToggleFavorite,
    FavoriteToggled { removed: bool },              // result of the add/remove command
    DuplicateFound(Option<Manga>),
    CategoriesConfirmed(Vec<Id>),
    DialogDismissed,
    FilterUnread(TriState), FilterDownloaded(TriState), FilterBookmarked(TriState),
    SetSortMode(Sorting), SetDisplayMode(u64),
    ResetDefaults, SaveDefaults { apply_to_existing: bool },
    Select { chapter_id: Id, selected: bool, gesture: Gesture },
    SelectAll(bool), InvertSelection,
    Swipe { chapter_id: Id, action: SwipeAction },
    Download(DownloadAction), ChapterDownload { action: ChapterDownloadAction, chapter_ids: Vec<Id> },
    MarkRead { chapter_ids: Vec<Id>, read: bool },
    MarkPreviousRead { pointer: Id },
    Bookmark { chapter_ids: Vec<Id>, bookmarked: bool },
    DeleteChapters(Vec<Id>),
    OpenDialog(Dialog),
    SetFetchInterval(i32),
    SetExcludedScanlators(Vec<String>),
    CommandFailed { what: &'static str, error: String },
}

#[derive(Debug)]
pub enum Cmd {
    LoadManga(Id),
    WatchChapters(Id),
    FetchFromSource { manual: bool },
    AddToLibrary { manga_id: Id, categories: Vec<Id> },   // one transaction
    RemoveFromLibrary { manga_id: Id },
    CheckDuplicate { manga: Manga },
    SetFilterFlags { manga_id: Id, flag: u64, mask: u64 },
    WriteChapterUpdates(Vec<ChapterUpdate>),
    Track { chapters_read_to: f64 },
    RefreshTrackers,
    ToastOrSnack(UserMessage),
    StartDownloads(Vec<Id>),
    DeleteDownloads { manga_id: Id, chapter_ids: Vec<Id> },
    SaveChapterDefaults(ChapterDefaults, bool),
    OpenInBrowser(String),
    CopyToClipboard(String),          // executed in kuro-ui (gdk::Clipboard), not in core
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadAction { Next1, Next5, Next10, Next25, UnviewedItems }   // EntryScreenConstants.kt DownloadAction

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChapterDownloadAction { Start, StartNow, Cancel, Delete }        // ChapterDownloadIndicator.kt

/// Banner content. `action` is None for messages without a button.
#[derive(Debug)]
pub struct UserMessage {
    pub text: String,
    pub action: Option<(String, Box<Msg>)>,   // label + message sent on click (Msg: Debug)
    pub dismissible: bool,                    // withDismissAction
    pub duration: BannerDuration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerDuration { Short, Indefinite }   // Short = 4000 ms (proposal, behavior 9.1)
```

### 4.2 Executor and threads

```rust
// app/src/main.rs (sketch)
let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build()?;
let (tx, rx) = async_channel::unbounded::<Msg>();       // Msg: Send
// GTK side: drain rx on the main context
glib::spawn_future_local(clone!(@strong model => async move {
    while let Ok(msg) = rx.recv().await {
        let cmds = model.borrow_mut().update(msg);
        for cmd in cmds { executor.spawn(cmd, tx.clone()); }
        render(&model.borrow());
    }
}));
```

`executor.spawn(cmd, tx)` runs the async job on `rt.handle()` and sends the result back through `tx`. Commands carry a `CancellationToken` for the page lifetime, except commands marked "non-cancellable" (writes), which are spawned on a detached task (equivalent of `launchNonCancellable`, behavior 10). Page dispose cancels the token; detached writes still complete.

Rules:
1. `update` never blocks. DB reads/writes are in `Cmd`s.
2. Only the GTK main thread touches `DetailModel` and widgets.
3. Subscriptions (`MangaStore::watch`, `ChapterStore::watch`, `Downloads::events`, `Trackers::watch_tracks`) are forwarded as `Msg`s by spawned tasks that hold the cancellation token.
4. Download progress is throttled to 10 Hz per manga in the forwarder (upstream debounce is 50 ms for progress, MangaDownload.kt:59). Status changes are not throttled.

### 4.3 State

```rust
pub enum LoadState { Loading, Ready(Box<Ready>) }

pub struct Ready {
    pub manga: Manga,
    pub source_is_local: bool,
    pub chapters: Vec<ChapterRow>,        // raw list (allChapters); download state + selected filled in
    pub view: Vec<usize>,                 // indices into chapters, view order (processedChapters)
    pub view_generation: u64,
    pub available_scanlators: BTreeSet<String>,   // sorted for stable UI
    pub excluded_scanlators: BTreeSet<String>,
    pub selection: Selection,
    pub tracking_count: usize,
    pub has_logged_in_trackers: bool,
    pub is_refreshing: bool,
    pub dialog: Option<Dialog>,
    pub has_prompted_add: bool,           // in-memory only (hasPromptedToAddBefore)
    pub pending_after_category: Option<PendingAfterCategory>, // replaces isFromChangeCategory
    pub auto_track: AutoTrackState,       // captured at open (autoTrackState var)
    pub swipe_start: SwipeAction,         // captured at open (chapterSwipeStartAction)
    pub swipe_end: SwipeAction,
    pub skip_filtered: bool,
}

#[derive(Clone, Debug)]
pub enum Dialog {
    ChangeCategory { manga_id: Id, selection: Vec<(Category, CheckState)> },
    DeleteChapters(Vec<Id>),
    DuplicateManga { manga: Manga, duplicate: Manga },
    Migrate { new_manga: Manga, old_manga: Manga },
    SetFetchInterval { manga_id: Id },
    ChapterSettings,       // SettingsSheet
    Tracking,              // TrackSheet
    FullCover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAfterCategory { OpenTracking }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeAction { ToggleRead, ToggleBookmark, Download, Disabled }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckState { Unchecked, Checked }  // CheckboxState.State (two-state) for ChangeCategory
```

Derived values (computed in `Ready`, cached by `view_generation`):
- `filter_active() = scanlator_filter_active || manga.chapter_flags.filter_active(downloaded_only)`, `scanlator_filter_active = !excluded ∩ available is empty`.
- `tracking_available() = tracking_count > 0`.
- `auto_open_tracking() = tracking_available() && prefs.track_on_add()` (reads the current pref at call time, like upstream `autoOpenTrack`).
- `is_any_selected() = chapters.iter().any(|c| c.selected)`.
- `chapter_list_items()` = `separators(view_rows, descending)` merged into the view (section 3.4). The list widget uses this as the model.

### 4.4 Key reducer rules (behavior parity)

Loading and init (behavior 3.1):
- `MangaLoaded` + `ChaptersLoaded` set `Ready`. Before `Ready`, download events and tracking counts are DROPPED (same as upstream). After `Ready`, `chapters` is rebuilt from the latest DB rows on every `ChaptersLoaded`, with `download`/`progress` taken from `Downloads::queued` and `Downloads::is_chapter_downloaded(skip_cache = false)`, and `selected` from `selection.ids` (behavior 6.1).
- If `!manga.favorite`: `Cmd::SaveChapterDefaults`-like "apply defaults to this manga" (`SetMangaDefaultChapterFlags`), before `Ready` (behavior 3.1 step 5, 7.4).
- `needs_info = !manga.initialized`, `needs_chapters = chapters.is_empty()`: emit `FetchFromSource { manual: false }` for each needed part.

Selection (behavior 6.8), use `Selection::toggle` with `view` = ids of the view (processed) list. Fix E-SEL-1: `Selection` never writes the raw list.

Filters (behavior 6.5): `FilterUnread(s)` emits `SetFilterFlags { flag: unread_flag(s), mask: UNREAD_MASK }` AND does NOT update `manga.chapter_flags` in memory until the DB watch returns; instead it updates an optimistic `pending_flags` value so two quick changes compose correctly (E-FLAG-1). The DB write itself is `set_flag_bits` (atomic masked SQL).

Mark read (behavior 6.9.9): `MarkRead` -> `selection.set_all(raw, false)` first (same ordering as upstream: clears selection even if the list is empty), then `WriteChapterUpdates` (filtering: read=true only where !read; read=false only where read || last_page_read > 0; lastPageRead = 0 when read=false, untouched when true), then if `read && auto_track != Never && has_logged_in_trackers`: `RefreshTrackers`, then compute `max_number` over the PASSED chapters; if any track is behind: `Always` -> `Track{..}` + `UserMessage::TrackersUpdatedTo(n)`; `Ask` -> show snackbar `confirm_tracker_update` with action OK, and only on action `Track{..}`.

Download intents (behavior 6.9.2-6.9.5):
- `Download(NEXT_N)`: `download::unread_sorted_oldest_first(unread_not_downloaded(...))`, take N, `StartDownloads`. `UNVIEWED`: unsorted view-order list.
- `ChapterDownload { START_NOW, [id] }` runs `Downloads::start_now`. Multi-id START_NOW is ignored (same as upstream).
- `CANCEL` with exactly one id only (upstream), otherwise no-op.
- After enqueue: if `!favorite && !has_prompted_add`: set `has_prompted_add = true` and show the add-to-library banner (behavior 6.9.3 step 3). Action -> `ToggleFavorite`.

Favorite routing (behavior 4.2): implemented as one pure function returning a Cmd list:

```rust
pub enum FavoriteRoute { Remove, CheckDuplicateThenAdd, AddDefaultCategory(Option<Id>) , ChooseCategory }

pub fn route_add(default_category: i64, user_categories: &[Category]) -> FavoriteRoute {
    if let Some(c) = user_categories.iter().find(|c| c.id == default_category as Id) {
        return FavoriteRoute::AddDefaultCategory(Some(c.id));
    }
    if default_category == 0 || user_categories.is_empty() {
        return FavoriteRoute::AddDefaultCategory(None);
    }
    FavoriteRoute::ChooseCategory
}
```

Note `default_category == -1` with categories present -> `ChooseCategory` (upstream branch C).

Categories confirmation (behavior 4.4): `CategoriesConfirmed(ids)` -> `Cmd::AddToLibrary { manga_id, categories: ids }` as ONE transaction (fix for the unordered two-write race). If already favorite: `set_categories` only.

Removal (behavior 4.1): `RemoveFromLibrary` -> on success, `removeCovers` side effect (cover cache delete, area:34), and if `download_count(manga_id) > 0` show banner `delete_downloads_for_manga` with action `action_delete`; action -> `DeleteDownloads`.

Fetch (behavior 3.2-3.4): `FetchFromSource { manual }`:
- details part: `Sources::manga_details` then `MangaStore::update` with the mapping of `UpdateManga.awaitUpdateFromSource` (area:07).
- chapter part: `Sources::chapter_list` -> sync (area:07) -> returns new chapters; if `manual` then `FilterChaptersForDownload` (behavior 6.12) and enqueue.
- Error mapping: `SourceError::NotInstalled` -> `source_not_installed`; empty list for a non-local source -> `no_chapters_error` text; HTTP code 103 is ignored; others -> `formatted`.

Tracking sheet state (behavior 5.1): `TrackingCounts` from a watch: `tracking_count = tracks where tracker in supported`, `has_logged_in = supported non-empty`.

---

## 5. Tracking sheet models (`kuro-core::tracking`)

### 5.1 Home list

```rust
pub struct TrackItem { pub tracker: Arc<dyn MangaTracker>, pub track: Option<MangaTrack> }

pub fn map_track_items(logged_in: &[Arc<dyn MangaTracker>], tracks: &[MangaTrack], source_id: Id) -> Vec<TrackItem> {
    logged_in.iter()
        .filter(|t| t.accepts_source(source_id))
        .map(|t| TrackItem { track: tracks.iter().find(|x| x.tracker_id == t.id()).cloned(), tracker: t.clone() })
        .collect()
}
```

Per-row action availability (fixes E-TRK-1):
- status, chapter, score, start/end date, remove: only when `track.is_some()`.
- "new search": when `track.is_none()` or re-searching.
- "toggle private": only when `track.is_some()` AND `tracker.supports_private()`.
- "open in browser", "copy link": only when `track.remote_url` is non-blank.

Enhanced tracker "new search" = `register_enhanced(item)` (no UI): `match_manga`, on `None` or error show `error_no_match`. Error text is intentionally the same as upstream for parity; the log gets the real error (E-TRK-2).

### 5.2 Pickers and constraints

```rust
// status: options = tracker.status_list(); labels = status_label; initial = track.status
pub struct StatusPicker { pub options: Vec<(i64, Option<String>)>, pub selected: i64 }

// chapter: range 0..=max where max = track.total_chapters if > 0 else 10000 (MTID:376-383)
pub struct ChapterPicker { pub min: i32, pub max: i32, pub selected: i32 }
impl ChapterPicker {
    pub fn new(track: &MangaTrack) -> Self {
        let max = if track.total_chapters > 0 { track.total_chapters as i32 } else { 10_000 };
        Self { min: 0, max, selected: track.last_chapter_read as i32 }   // Kotlin Double.toInt() truncates
    }
}

// score: options = tracker.score_list(); initial = tracker.display_score(track) (a string)
pub struct ScorePicker { pub options: Vec<String>, pub selected: String }

// dates: MTID:465-520. All comparisons on local calendar dates (chrono::Local).
pub fn date_selectable(start: bool, candidate: chrono::NaiveDate, today: chrono::NaiveDate, track: &MangaTrack, zone: &impl chrono::TimeZone) -> bool {
    if candidate > today { return false; }
    if start && track.finish_date > 0 {
        let finished = to_local_date(track.finish_date, zone);
        candidate <= finished
    } else if !start && track.start_date > 0 {
        let started = to_local_date(track.start_date, zone);
        candidate >= started
    } else { true }
}
pub fn year_selectable(start: bool, year: i32, today_year: i32, track: &MangaTrack, zone: &impl chrono::TimeZone) -> bool {
    if year > today_year { return false; }
    if start && track.finish_date > 0 { return year <= to_local_date(track.finish_date, zone).year(); }
    if !start && track.start_date > 0 { return year >= to_local_date(track.start_date, zone).year(); }
    true
}
pub fn initial_date_ms(start: bool, track: &MangaTrack, now_ms: i64) -> i64 {
    let v = if start { track.start_date } else { track.finish_date };
    if v != 0 { v } else { now_ms }
}
pub fn can_remove_date(start: bool, track: &MangaTrack) -> bool {
    if start { track.start_date > 0 } else { track.finish_date > 0 }
}
```

Date conversion: picker output is a local date at local midnight. Upstream stores `convertEpochMillisZone(UTC -> system)`: Rust stores `local_date.and_hms(0,0,0)` in the local zone as epoch ms. Document: stored epoch ms are local-midnight values (same as upstream). Caveat for DST: use `chrono::Local.from_local_datetime(...).earliest()`.

Remove date: `set_start_date(track, 0)` / `set_finish_date(track, 0)`, then close the remover and the picker (upstream `popUntil`).

### 5.3 Search

```rust
pub struct TrackSearch {
    pub tracker: Arc<dyn MangaTracker>,
    pub query: String,
    pub state: SearchState,
    pub current_url: Option<String>,
}
pub enum SearchState {
    Idle,
    Loading,
    Done { results: Vec<TrackSearchResult>, selected: Option<TrackSearchResult> },
    Failed(String),
}
```

- On open with a non-blank initial query: `Loading` and `Cmd::TrackSearch` (behavior 8.7).
- On result: `selected = results.find(|r| Some(r.tracking_url) == current_url)`.
- `Confirm { private }`: if `selected` is `Some`, set `private` on the value (upstream mutates `selected.private = private`) and `register(item, manga_id)` as a detached command, then close.
- `private` option shown only when `tracker.supports_private()`.

### 5.4 Removal

`TrackerMangaRemoveScreen` -> dialog with checkbox `track_delete_remote_text` only when `tracker.deletable().is_some()`. OK -> `Cmd::UnlinkTrack { manga_id, tracker_id }` (detached, the local delete must complete) then `Cmd::DeleteRemote(track)` if checked (detached; error only logged).

---

## 6. Error model (`kuro-core::errors`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum DetailError {
    #[error("no chapters found")] NoChapters,
    #[error("http {0}")] Http(u16),
    #[error("offline")] Offline,
    #[error("unknown host {0}")] UnknownHost(String),
    #[error("source not installed")] SourceNotInstalled,
    #[error("{class}: {msg}")] Other { class: String, msg: String },
}

impl DetailError {
    /// User text, same shape as ExceptionFormatter.kt:16-38 (keys in strings.rs)
    pub fn user_message(&self, s: &Strings) -> String {
        match self {
            DetailError::NoChapters => s.no_results_found(),
            DetailError::Http(code) => s.exception_http(*code),
            DetailError::Offline => s.exception_offline(),
            DetailError::UnknownHost(m) => s.exception_unknown_host(m),
            DetailError::SourceNotInstalled => s.loader_not_implemented_error(),
            DetailError::Other { class, msg } if class == "Exception" || class == "IOException" => msg.clone(),
            DetailError::Other { class, msg } => format!("{class}: {msg}"),
        }
    }
}
```

Chapter-fetch special case: `NoChapters` from the fetch path uses `no_chapters_error` ("No chapters found") rather than `no_results_found` (behavior 3.4).

Background failures (download enqueue, read write, tracker push) are logged with `tracing::error!` and surface as a banner only where upstream shows one.

---

## 7. Strings (`kuro-core::strings`)

Keys are the upstream names so the Kotlin base strings can be checked mechanically (`i18n/src/commonMain/moko-resources/base/strings.xml`). Each key is a `const` pair with English text and `%d`/`%s` placeholders replaced by typed arguments (no printf at runtime).

```rust
pub struct Strings;
impl Strings {
    pub const DELETE_DOWNLOADS_FOR_MANGA: &'static str = "Delete downloaded chapters?";
    pub const ACTION_DELETE: &'static str = "Delete";
    pub const ACTION_ADD: &'static str = "Add";
    pub const ACTION_OK: &'static str = "OK";
    pub const ACTION_CANCEL: &'static str = "Cancel";
    pub const ACTION_REMOVE: &'static str = "Remove";
    pub fn confirm_tracker_update(chapter: i64) -> String { format!("Update trackers to chapter {chapter}?") }
    pub const NO_CHAPTERS_ERROR: &'static str = "No chapters found";
    pub const CHAPTER_SETTINGS_UPDATED: &'static str = "Updated default chapter settings";
    pub fn track_error(service: &str, msg: &str) -> String { format!("{service} error: {msg}") }
    pub const ERROR_NO_MATCH: &'static str = "No match found";
    pub const TRACK_STARTED_READING_DATE: &'static str = "Start date";
    pub const TRACK_FINISHED_READING_DATE: &'static str = "Finish date";
    pub const TRACK_REMOVE_DATE_CONF_TITLE: &'static str = "Remove date?";
    pub fn track_remove_start_date_conf_text(service: &str) -> String { format!("This will remove your previously selected start date from {service}") }
    pub fn track_remove_finish_date_conf_text(service: &str) -> String { format!("This will remove your previously selected finish date from {service}") }
    pub fn track_delete_title(service: &str) -> String { format!("Remove {service} tracking?") }
    pub const TRACK_DELETE_TEXT: &'static str = "This will remove the tracking locally.";
    pub fn track_delete_remote_text(service: &str) -> String { format!("Also remove from {service}") }
    pub const SNACK_ADD_TO_MANGA_LIBRARY: &'static str = "Add manga to library?";   // AYMR
    pub fn trackers_updated_summary_manga(chapter: i64) -> String { format!("Trackers updated to chapter {chapter}") } // AYMR
    pub const LOCK_ALWAYS: &'static str = "Always";
    pub const DEFAULT_CATEGORY_SUMMARY: &'static str = "Always ask";
    pub const LOCK_NEVER: &'static str = "Never";
}
```

Other keys used by formatter (`exception_http`, `exception_offline`, `exception_unknown_host`, `no_results_found`, `loader_not_implemented_error`) are defined in area:36's strings module; this part references them.

---

## 8. Platform traits used here (from `kuro-platform`)

| Need | Trait | Linux impl | Used by |
|---|---|---|---|
| open tracker URL | `Opener::open_url` | `open` crate (xdg-open / gio) | MTID open-in-browser |
| copy tracker URL | UI layer (GTK clipboard), not a platform trait in core | `gdk::Display::clipboard().set_text` | MTID copy link |
| tracker tokens | `SecretStore` (through trackers) | Secret Service via `keyring` | tracker services (area:27) |
| covers folder | `Paths::covers_dir` | `directories` crate | cover removal on unfavorite |
| downloads folder | `Paths::default_downloads_dir` | `directories` crate | downloads (area:23) |

Nothing in this part calls `xdg-open` or D-Bus directly.

---

## 9. Integrating with the existing mockup

| Mockup today | Change |
|---|---|
| `src/entry.rs::entry(title, anime)` builds static rows, `open_entry(title, anime)` from `screens.rs` | Manga branch takes `manga_id: Id` and builds `ui::detail_manga::DetailPage`. Anime branch unchanged until its own part. |
| `entry.rs` chapter rows use `Chapter N` text and `"Scanlator"` literal (lines 170, 287, 363) | Rows come from `ChapterRow`; literal scanlator text removed. |
| `entry.rs` filter button opens `sheet_of(id, sh)` from `spec/entries.json` | Keep the sheet for layout; bind its options to `Ready.flags` (unread/downloaded/bookmark tristates, sort, display). |
| `entry.rs:226-230` `"Migrate"` opens `dialog_of(id, ...)` | Opens `Dialog::Migrate` from the model (behavior 6.14). |
| `entry.rs` tracking sheet (lines 496-537) static | Bound to `TrackItem` list; rows follow 5.1 action availability. |
| `entry.rs` tracker search (539-570) | Bound to `TrackSearch` state machine (5.3). |
| `ui.rs::show_sheet`, `show_dialog` (lines 391, 406) | Reused for presentation; dialog content supplied by the model instead of spec text. |
| `spec.rs` `Dialog.buttons`, `Sheet.tabs` | Keep for labels and ordering of static controls. Behavioral text comes from `strings.rs`. |
| `spec/entries.json` manga screens | Keep `notes` as layout reference only; do not treat the mockup's sample rows as data. (Matches the project rule "no invented scaffolding": the mockup's chapter names are sample data to be replaced.) |

---

## 10. GTK specifics for this part

- Chapter list: `gtk::ListView` + `gio::ListStore<ChapterRowObject>` + `gtk::SignalListItemFactory`. The store contains chapter rows and separator rows (`MissingRowObject`). Do not use `ListBox` for >200 rows; the separators are list items, so keep one model.
- Updating one row (download progress): `ListStore::items_changed(index, 1, 1)` on the single changed row, not a rebuild. Keep `HashMap<chapter_id, index>` maintained on each view rebuild.
- Multi-select: `gtk::MultiSelection` is NOT used. Selection is the model's `Selection` (behavior 6.8). The list mirrors it. Long-press range and tap rules are gesture logic in the widget, which calls `Msg::Select { gesture }`.
- Snackbar replacement: `gtk::Overlay` at the window level with a `gtk::Revealer` (bottom-center) holding label + optional action button + dismiss button; a queue of at most one visible banner, a second one replaces it after its timeout. Durations: Short 4000 ms; Indefinite until dismissed (behavior 9.1 proposal).
- Count spin for chapter picker: `gtk::SpinButton` with `adjustment(0..=max, step 1)`, not a list of 10000 rows.
- Date picker: `gtk::Calendar` has no min/max. Implement the constraint in the handler: when the chosen day is not selectable, show a one-line error under the calendar and disable the OK button; do not silently clamp.
- Category checkboxes: plain two-state `gtk::CheckButton`. A click flips Unchecked and Checked, as `CheckboxState.State.next()` does. No inconsistent state.
- Dialogs: transient `gtk::Window` modal (no libadwaita). Use `gtk::Window::set_modal(true)` + `set_transient_for`.

---

## 11. Pitfalls

1. `toggle` must be a pure function of (view ids, selection). Upstream runs it inside a `MutableStateFlow.update` lambda (may re-run). The Rust reducer runs once per message; keep it that way.
2. Chapter ids are unique but `Chapter` value equality (used by upstream `indexOf(pointer)`) is full-field equality. Compare by id in Rust (E-PREV-1).
3. `chapter_number` as `f64` keys: use `total_cmp` for ordering, `to_bits` only for grouping, and never use `==` for dedupe of computed values.
4. Mixed-case titles: duplicate detection compares `to_lowercase()` on both sides in Rust (E-DUP-1).
5. Monotonic ids: rows must be keyed by `chapter.id` not by index, because the view changes with filters.
6. Time: `Clock` is injected so that date constraints can be tested (today's date changes at midnight; keep the value per action).
7. Locale: collation depends on `sys-locale`; tests must set a fixed locale (E-SORT-1).
8. The `autoOpenTrack` check reads `tracking_count` which may still be stale right after the add; accept and document (behavior 4.2 step 7).
9. Tracker `refresh_all` failure list uses the service name at display time; capture the name when the failure is produced (the service may be logged out before the toast renders).
10. Do not call `Downloads::is_chapter_downloaded(skip_cache = true)` on the UI path (it walks the directory tree). Use the cache (upstream default).
