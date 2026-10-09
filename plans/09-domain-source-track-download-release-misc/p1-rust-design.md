# Area 09 — Rust design

Scope: source, track, download policy, release check, storage, backup preferences, custom buttons, extension repos and stores (domain layer only). Target: crate `kuro-domain` (no GTK, no libadwaita, no Android types). Companion: `p1-behavior.md` (algorithms), `p1-data-and-config.md` (schema, keys, endpoints), `p1-tasks.md` (task ids).

## 1. Crate and module layout

Workspace member `crates/kuro-domain` (created by T-09a-001 on top of the workspace skeleton from area 01 and the `kuro-domain` crate of area 08). Sibling crates used here: `kuro-prefs` (typed preference store, area 02), `kuro-db` (rusqlite handle and migrations, areas 03/04), `kuro-net` (reqwest client, area 12), `kuro-core` (ids, errors, tracing setup, area 01).

```
crates/kuro-domain/src/
  lib.rs
  error.rs                 // DomainError, RepoStoreError, CustomButtonError (T-09a-059)
  source/
    mod.rs
    model.rs               // Source, AnimeSource, SourceKey, MangaSourceWithCount, AnimeSourceWithIds, DeletableAnime   (T-09a-002, T-09a-007)
    pin.rs                 // Pins(u8) newtype                                                                       (T-09a-003)
    stub.rs                // StubSource, SourceHandle::Stub, SourceNotInstalled                                     (T-09a-005)
    browse.rs              // BrowseQuery enum + route()                                                             (T-09a-006)
    registry.rs            // SourceRegistry<K>, watch channels                                                       (T-09a-004)
    repo.rs                // MangaSourceRepo / AnimeSourceRepo traits, PageSource trait                             (T-09a-004, T-09a-060)
    non_library.rs         // non-library listings                                                                   (T-09a-007)
    upcoming.rs            // status sets                                                                             (T-09a-009)
  track/
    mod.rs
    model.rs               // MangaTrack, AnimeTrack                                                                 (T-09a-010)
    repo.rs                // TrackRepo trait (per kind)                                                             (T-09a-013)
    service.rs             // Get/Insert/Delete with log-and-default policy                                          (T-09a-011, T-09a-059)
  download/
    mod.rs
    prefs.rs               // DownloadPrefs (typed keys, defaults)                                                   (T-09a-014)
    filter.rs              // new-item auto-download filter, generic over EntryKind                                  (T-09a-015, T-09a-016, T-09a-017, T-09a-018)
    policy.rs              // remove-after-read slot math, speed limit conversion                                    (T-09a-019, T-09a-021)
  extension_repo/
    mod.rs
    model.rs               // ExtensionRepo                                                                          (T-09a-024)
    dto.rs                 // ExtensionRepoMetaDto                                                                   (T-09a-027)
    service.rs             // fetch repo.json                                                                        (T-09a-027)
    repo.rs                // MangaExtensionRepoRepo trait                                                           (T-09a-025)
    create.rs              // add flow + Result classification                                                       (T-09a-026)
    refresh.rs             // refresh with fingerprint guard                                                         (T-09a-028)
    write.rs               // replace / delete / get / count                                                         (T-09a-029)
  extension_store/
    mod.rs
    model.rs               // AnimeExtensionStore, Contact                                                           (T-09a-030)
    repo.rs                // AnimeExtensionStoreRepo trait                                                          (T-09a-031..034)
    wire.rs                // store wire enum + format sniffing                                                      (T-09a-031)
    add.rs / refresh.rs / fetch.rs / read.rs                                                                        (T-09a-031..034)
  custom_buttons/
    mod.rs
    model.rs               // CustomButton, CustomButtonUpdate, placeholder substitution                             (T-09a-036)
    repo.rs                // CustomButtonRepo trait                                                                 (T-09a-037)
    ops.rs                 // create/update/delete/reorder/favorite with the mutex                                   (T-09a-038..040)
  release/
    mod.rs
    model.rs               // Release                                                                                 (T-09a-045)
    check.rs               // throttle + compare                                                                      (T-09a-042, T-09a-043, T-09a-044)
    github.rs              // GitHub latest-release client + asset choice                                             (T-09a-042, T-09a-045)
  storage/
    mod.rs
    layout.rs              // folder names and derived paths                                                          (T-09a-045, T-09a-046)
    manager.rs             // StorageManager (watch channel, lazy creation)                                           (T-09a-044, T-09a-048, T-09a-049)
  backup/
    mod.rs
    prefs.rs               // interval, last timestamp, flags                                                         (T-09a-052)
    flags.rs               // BackupFlag tags                                                                         (T-09a-053)
  upcoming/mod.rs
```
Rule: no module in `kuro-domain` imports `gtk`, `glib`, `gio` or `adw`. Platform paths are computed by `storage::layout` from `directories::ProjectDirs` (crate `directories`), which is the only platform-specific call in this area.

## 2. Shared types

```rust
// kuro-domain/src/lib.rs
pub type Id = i64;                       // all Kotlin `Long` ids map to i64 (SQLite INTEGER)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind { Manga, Anime }      // shared with area 08; per-kind data as associated consts
```

```rust
// kuro-domain/src/source/pin.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pins(u8);
impl Pins {
    pub const UNPINNED: Pins = Pins(0b00);
    pub const PINNED: Pins = Pins(0b11);            // Pinned | Actual
    pub fn bits(self) -> u8 { self.0 }
    pub fn from_bits(b: u8) -> Self { Pins(b & 0b11) }
    pub fn contains(self, flag: Pin) -> bool { (flag as u8) & self.0 == flag as u8 }   // quirk Q-PIN-1 preserved
    pub fn with(self, flag: Pin) -> Pins { Pins(self.0 | flag as u8) }
    pub fn without(self, flag: Pin) -> Pins { Pins(self.0 & !(flag as u8)) }   // Proposal, Kotlin uses xor (Q-PIN-2)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Pin { Unpinned = 0b00, Pinned = 0b01, Actual = 0b10 }
```
Note `Pins::contains(Pin::Unpinned)` is always `true` (as in Kotlin). Callers must not use it as "is unpinned"; the port adds `Pins::is_pinned()` = `self == PINNED`.

```rust
// kuro-domain/src/source/model.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: Id,
    pub lang: String,
    pub name: String,
    pub supports_latest: bool,
    pub is_stub: bool,
    pub pin: Pins,              // default Pins::UNPINNED
    pub is_used_last: bool,     // default false
}
impl Source {
    pub fn visual_name(&self) -> String {
        if self.lang.is_empty() { self.name.clone() } else { format!("{} ({})", self.name, self.lang.to_uppercase()) }
    }
    pub fn key(&self) -> String {
        if self.is_used_last { format!("{}-lastused", self.id) } else { self.id.to_string() }
    }
}
// AnimeSource: identical shape (no data-saver field). Use one generic struct: pub struct SourceInfo<K> { … }  (Proposal, see section 4.1)
```

```rust
pub struct SourceWithCount { pub source: Source, pub count: i64 }             // MangaSourceWithCount
pub struct SourceWithIds { pub source: Source, pub ids: Vec<Id>, pub orphaned: Vec<Id> }   // AnimeSourceWithIds
impl SourceWithIds { pub fn count(&self) -> i64 { self.ids.len() as i64 } }
pub struct DeletableEntry { pub entry_id: Id, pub source_id: Id, pub fetch_type: FetchType }  // DeletableAnime
pub enum FetchType { Seasons, Episodes }   // upstream enum, values as in source-api
```

## 3. Errors

```rust
// kuro-domain/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("network error: {0}")]
    Net(#[from] reqwest::Error),
    #[error("invalid input: {0}")]
    Invalid(&'static str),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("source not installed: {0}")]
    SourceNotInstalled(String),
    #[error("storage base directory unavailable")]
    StorageUnavailable,
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum RepoStoreError {
    #[error("error saving repository to database")]
    Save(#[source] rusqlite::Error),   // mirrors SaveExtensionRepoException (SQLiteException wrapper)
    #[error(transparent)]
    Other(#[from] DomainError),
}
```
- `RepoStoreError::Save` is produced only by `insert_repo` / `upsert_repo` (matches `MangaExtensionRepoRepositoryImpl.kt:43-47,57-61`). Constraint detection is by inspecting `rusqlite::ErrorCode::ConstraintViolation` rather than string matching.
- Track interactors never return `Err` to callers (log-and-default, see section 6.3).

## 4. Traits (domain ports)

### 4.1 Source registry and listing
```rust
// kuro-domain/src/source/repo.rs
use std::pin::Pin as StdPin;
use futures_core::Stream;

#[async_trait::async_trait]   // Proposal: use native async fn in traits where object-safety is not needed; the registry needs dyn, so async_trait is used here
pub trait PageSource<T>: Send + Sync {
    /// Keys are page numbers starting at 1 (Kotlin PagingSource<Long, _>). Returns (items, next_page).
    async fn load(&self, page: i64) -> Result<(Vec<T>, Option<i64>), DomainError>;
}

pub trait MangaSourceRepo: Send + Sync {
    fn online_sources(&self) -> tokio::sync::watch::Receiver<Vec<Source>>;
    fn sources_with_favorite_count(&self) -> tokio::sync::watch::Receiver<Vec<(Source, i64)>>;
    fn sources_with_non_library(&self) -> tokio::sync::watch::Receiver<Vec<SourceWithCount>>;
    fn popular(&self, source_id: Id) -> Box<dyn PageSource<MangaEntry>>;
    fn latest(&self, source_id: Id) -> Box<dyn PageSource<MangaEntry>>;
    fn search(&self, source_id: Id, query: &str, filters: FilterList) -> Box<dyn PageSource<MangaEntry>>;
}
// AnimeSourceRepo mirrors this with AnimeEntry and AnimeFilterList.
```
`MangaEntry` / `AnimeEntry` are the remote DTOs of the source API (area 15/16). They are referenced, not defined, here.

```rust
// kuro-domain/src/source/registry.rs
pub enum SourceHandle<S> { Installed(std::sync::Arc<S>), Stub(StubSource) }

pub struct SourceRegistry<S: ?Sized> {
    initialized: tokio::sync::watch::Sender<bool>,          // is_initialized (StateFlow<Boolean>)
    installed: tokio::sync::watch::Sender<Vec<std::sync::Arc<S>>>,
    stubs: MangaStubRepo,                                     // persisted stub rows
}
impl<S: ?Sized> SourceRegistry<S> {
    pub fn get(&self, key: Id) -> Option<std::sync::Arc<S>>;
    pub fn get_or_stub(&self, key: Id) -> SourceHandle<S>;   // falls back to stub row, else Stub with "unknown" name
    pub fn all(&self) -> Vec<std::sync::Arc<S>>;
    pub fn online(&self) -> Vec<std::sync::Arc<S>>;           // only HTTP-backed sources (filter in impl)
    pub fn stubs(&self) -> Vec<StubSource>;
    pub fn subscribe_initialized(&self) -> tokio::sync::watch::Receiver<bool>;
}
```
Proposal: `SourceRegistry<dyn Source>` for manga and `SourceRegistry<dyn AnimeSourceApi>` for anime; the trait objects come from the extension loader (area 18/19).

### 4.2 Stubs
```rust
// kuro-domain/src/source/stub.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubSource { pub id: Id, pub lang: String, pub name: String }
impl StubSource {
    pub fn is_invalid(&self) -> bool { self.name.trim().is_empty() || self.lang.trim().is_empty() }   // Kotlin isBlank() == trim().is_empty() for ASCII/Unicode whitespace
    pub fn display_name(&self) -> String {
        if !self.is_invalid() { format!("{} ({})", self.name, self.lang.to_uppercase()) } else { self.id.to_string() }
    }
    pub fn fail<T>(&self) -> Result<T, DomainError> { Err(DomainError::SourceNotInstalled(self.display_name())) }
}
pub trait StubSourceRepo: Send + Sync {
    fn subscribe_all(&self) -> tokio::sync::watch::Receiver<Vec<StubSource>>;
    fn get(&self, id: Id) -> Result<Option<StubSource>, DomainError>;
    fn upsert(&self, id: Id, lang: &str, name: &str) -> Result<(), DomainError>;
}
```

### 4.3 Browse routing
```rust
// kuro-domain/src/source/browse.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowseQuery { Popular, Latest, Search(String) }
impl BrowseQuery {
    /// Kotlin compatibility: the sentinel strings are accepted as input from the old backup/intent path only.
    pub fn from_legacy(query: &str) -> BrowseQuery {
        match query {
            "eu.kanade.domain.source.manga.interactor.POPULAR" | "eu.kanade.domain.source.anime.interactor.POPULAR" => BrowseQuery::Popular,
            "eu.kanade.domain.source.manga.interactor.LATEST" | "eu.kanade.domain.source.anime.interactor.LATEST" => BrowseQuery::Latest,
            other => BrowseQuery::Search(other.to_string()),
        }
    }
}
```
Fixes quirk Q-BR-1 in the UI path: the UI builds `BrowseQuery` directly and never passes sentinel text through the search field. `from_legacy` exists only for parity tests (T-09a-006).

### 4.4 Track repository (per kind)
```rust
// kuro-domain/src/track/repo.rs
pub trait TrackRepo<T>: Send + Sync {   // T = MangaTrack | AnimeTrack
    fn by_row_id(&self, row_id: Id) -> Result<Option<T>, DomainError>;          // Q-TRK-1: named by row id
    fn by_entry(&self, entry_id: Id) -> Result<Vec<T>, DomainError>;
    fn all_watch(&self) -> tokio::sync::watch::Receiver<Vec<T>>;
    fn by_entry_watch(&self, entry_id: Id) -> tokio::sync::watch::Receiver<Vec<T>>;
    fn delete(&self, entry_id: Id, tracker_id: Id) -> Result<(), DomainError>;  // DELETE … WHERE entry_id = ? AND sync_id = ?
    fn insert(&self, track: &T) -> Result<(), DomainError>;                      // Proposal: ON CONFLICT DO UPDATE (Q-TRK-2)
    fn insert_all(&self, tracks: &[T]) -> Result<(), DomainError>;               // one transaction
}
```
`watch` receivers are used because the Kotlin `Flow` is conflated (latest value only) and the UI only needs the latest list. Change detection comes from `kuro-db` (area 03/04), which emits after each committed write touching the table.

### 4.5 Download preferences
```rust
// kuro-domain/src/download/prefs.rs
pub struct DownloadPrefs { store: kuro_prefs::Store }
impl DownloadPrefs {
    pub fn download_only_over_wifi(&self) -> kuro_prefs::Pref<bool>;       // default true
    pub fn number_of_downloads(&self) -> kuro_prefs::Pref<i32>;            // default 1
    pub fn download_speed_limit(&self) -> kuro_prefs::Pref<i32>;           // default 0 (KiB/s)
    pub fn save_chapters_as_cbz(&self) -> kuro_prefs::Pref<bool>;           // default true
    pub fn split_tall_images(&self) -> kuro_prefs::Pref<bool>;              // default true
    pub fn auto_download_while_reading(&self) -> kuro_prefs::Pref<i32>;    // default 0
    pub fn auto_download_while_watching(&self) -> kuro_prefs::Pref<i32>;   // default 0
    pub fn remove_after_read_slots(&self) -> kuro_prefs::Pref<i32>;        // default -1
    pub fn remove_after_marked_as_read(&self) -> kuro_prefs::Pref<bool>;   // default false
    pub fn remove_bookmarked(&self) -> kuro_prefs::Pref<bool>;             // default false
    pub fn download_fillermarked(&self) -> kuro_prefs::Pref<bool>;         // default false
    pub fn remove_exclude_manga_categories(&self) -> kuro_prefs::Pref<BTreeSet<i64>>;   // default {}
    pub fn remove_exclude_anime_categories(&self) -> kuro_prefs::Pref<BTreeSet<i64>>;   // default {}
    pub fn download_new_chapters(&self) -> kuro_prefs::Pref<bool>;         // default false (key download_new)
    pub fn download_new_episodes(&self) -> kuro_prefs::Pref<bool>;         // default false
    pub fn download_new_chapter_categories(&self) -> kuro_prefs::Pref<BTreeSet<i64>>;   // default {}
    pub fn download_new_episode_categories(&self) -> kuro_prefs::Pref<BTreeSet<i64>>;   // default {}
    pub fn download_new_chapter_categories_exclude(&self) -> kuro_prefs::Pref<BTreeSet<i64>>;
    pub fn download_new_episode_categories_exclude(&self) -> kuro_prefs::Pref<BTreeSet<i64>>;
    pub fn download_new_unread_chapters_only(&self) -> kuro_prefs::Pref<bool>;  // default false
    pub fn download_new_unseen_episodes_only(&self) -> kuro_prefs::Pref<bool>;  // default false
    pub fn use_external_downloader(&self) -> kuro_prefs::Pref<bool>;       // dropped on Linux (T-09a-022), kept for backup parity only
    pub fn external_downloader_selection(&self) -> kuro_prefs::Pref<String>;
}
```
Keys: exact strings in `p1-data-and-config.md` section 4.1. Sets of category ids are stored as decimal strings; `BTreeSet<i64>` decoding skips non-numeric entries (Proposal, Q-DL-2).

Note: `kuro_prefs::Pref<T>` is the typed handle from area 02 with `get()`, `set(v)`, `watch() -> watch::Receiver<T>`.

### 4.6 Auto-download filter (generic)
```rust
// kuro-domain/src/download/filter.rs
pub trait NewItem {                     // implemented by Chapter (area 07) and Episode (area 07)
    fn number(&self) -> f32;            // chapterNumber / episodeNumber
    fn is_recognized(&self) -> bool { self.number() >= 0.0 }
}
pub trait ReadState { fn is_read(&self) -> bool; }   // read for chapters, seen for episodes

pub struct CategoryDecision { pub included: BTreeSet<i64>, pub excluded: BTreeSet<i64> }

/// Returns true if new items of an entry in `categories` should be queued (behavior 5.2).
pub fn should_download_new(favorite: bool, categories: &[i64], rule: &CategoryDecision) -> bool {
    const DEFAULT_CATEGORY_ID: i64 = 0;   // UNCATEGORIZED_ID
    if !favorite { return false; }
    let cats: Vec<i64> = if categories.is_empty() { vec![DEFAULT_CATEGORY_ID] } else { categories.to_vec() };
    match (rule.included.is_empty(), rule.excluded.is_empty()) {
        (true, true) => true,
        _ if cats.iter().any(|c| rule.excluded.contains(c)) => false,
        _ if rule.included.is_empty() => true,
        _ => cats.iter().any(|c| rule.included.contains(c)),
    }
}

/// Behavior 5.1
pub fn filter_new<T: NewItem + Clone, R: ReadState + NewItem>(
    enabled: bool,              // download_new (or _episode)
    entry_favorite_and_cats_ok: bool,   // result of should_download_new
    unread_only: bool,
    new_items: &[T],
    existing: &[R],
) -> Vec<T> {
    if new_items.is_empty() || !enabled || !entry_favorite_and_cats_ok { return vec![]; }
    if !unread_only { return new_items.to_vec(); }
    let read: std::collections::HashSet<u32> = existing.iter()          // number key: see note
        .filter(|e| e.is_read() && e.is_recognized())
        .map(|e| e.number().to_bits())
        .collect();
    new_items.iter().filter(|n| !read.contains(&n.number().to_bits())).cloned().collect()
}
```
Notes: keyed by `f32::to_bits` to make the set hashable (Kotlin uses `Float` equality; `-0.0` vs `0.0` is excluded by the `>= 0` filter, and NaN never occurs). The Kotlin code evaluates `shouldDownloadNewChapters()` only after `newChapters.isEmpty()` and `downloadNewChapters()` (short-circuit) — the Rust version keeps that order. Category lookup is async in Kotlin (`getCategories.await`); the caller resolves categories first, so `filter_new` stays synchronous and pure.

### 4.7 Extension repos (manga)
```rust
// kuro-domain/src/extension_repo/model.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionRepo {
    pub base_url: String,
    pub name: String,
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String,
}

// kuro-domain/src/extension_repo/repo.rs
pub trait MangaExtensionRepoRepo: Send + Sync {
    fn subscribe_all(&self) -> tokio::sync::watch::Receiver<Vec<ExtensionRepo>>;
    fn get_all(&self) -> Result<Vec<ExtensionRepo>, DomainError>;
    fn get(&self, base_url: &str) -> Result<Option<ExtensionRepo>, DomainError>;
    fn get_by_fingerprint(&self, fp: &str) -> Result<Option<ExtensionRepo>, DomainError>;
    fn count(&self) -> tokio::sync::watch::Receiver<usize>;
    fn insert(&self, r: &ExtensionRepo) -> Result<(), RepoStoreError>;
    fn upsert(&self, r: &ExtensionRepo) -> Result<(), RepoStoreError>;
    fn replace(&self, r: &ExtensionRepo) -> Result<(), DomainError>;
    fn delete(&self, base_url: &str) -> Result<(), DomainError>;
}

// kuro-domain/src/extension_repo/create.rs
pub enum AddRepoOutcome {
    Success,
    InvalidUrl,
    RepoAlreadyExists,
    DuplicateFingerprint { old: ExtensionRepo, new: ExtensionRepo },
    Error,
}
pub async fn add_manga_repo(
    repo: &dyn MangaExtensionRepoRepo,
    fetcher: &dyn RepoDetailsFetcher,
    index_url: &str,
) -> Result<AddRepoOutcome, DomainError>;

#[async_trait::async_trait]
pub trait RepoDetailsFetcher: Send + Sync {
    /// GET `<base>/repo.json`; None on any failure (behavior 6.1 step 4).
    async fn fetch_repo_details(&self, base_url: &str) -> Option<ExtensionRepo>;
}
```
The index-URL validator is:
```rust
pub fn validate_index_url(input: &str) -> Option<(String /*formatted*/, String /*base*/)> {
    let url = url::Url::parse(input).ok()?;                 // OkHttp toHttpUrlOrNull equivalent
    let formatted = url.to_string();
    let re = regex::Regex::new(r"^https://.*/index\.min\.json$").unwrap();   // compiled once via OnceLock
    if !re.is_match(&formatted) { return None; }
    let base = formatted.strip_suffix("/index.min.json")?.to_string();
    Some((formatted, base))
}
```
Caveat: `url::Url::parse` lowercases the scheme and host and may add a trailing slash to an empty path; Kotlin's `HttpUrl.toString()` does the same for these cases.

### 4.8 Extension stores (anime)
```rust
// kuro-domain/src/extension_store/model.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeExtensionStore {
    pub index_url: String,
    pub name: String,
    pub badge_label: String,
    pub signing_key: String,
    pub contact: Contact,
    pub is_legacy: bool,
    pub extension_list_url: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contact { pub website: String, pub discord: Option<String> }

// kuro-domain/src/extension_store/repo.rs
#[async_trait::async_trait]
pub trait AnimeExtensionStoreRepo: Send + Sync {
    async fn insert(&self, index_url: &str) -> Result<(), DomainError>;          // fetch + upsert
    async fn insert_from_preference(&self, index_url: &str, name: &str) -> Result<(), DomainError>;
    async fn refresh_all(&self) -> Result<(), DomainError>;                      // sequential, per-store errors logged
    async fn fetch_extensions(&self) -> Vec<AvailableAnimeExtension>;           // parallel, failures -> empty
    fn get_all(&self) -> Result<Vec<AnimeExtensionStore>, DomainError>;
    fn subscribe_all(&self) -> tokio::sync::watch::Receiver<Vec<AnimeExtensionStore>>;
    fn count(&self) -> tokio::sync::watch::Receiver<i64>;
    fn remove(&self, index_url: &str) -> Result<(), DomainError>;
}
```
`AvailableAnimeExtension` is defined in area 19.

Wire decoding (behavior 7.1) is a pure function, unit-testable without network:
```rust
// kuro-domain/src/extension_store/wire.rs
pub enum StoreWire { LegacyList, LegacyRepo(LegacyRepoDto), CurrentJson(CurrentStoreDto), Proto(Vec<u8>) }
pub fn sniff(body: &[u8]) -> Result<StoreWire, DomainError>;   // after gunzip; first byte 0x5B / 0x7B / other
pub fn gunzip_if_needed(body: Vec<u8>) -> Result<Vec<u8>, DomainError>;   // magic 0x1f 0x8b, flate2::read::GzDecoder
```
`LegacyRepoDto.index_v2: Option<String>` drives the single recursion step (behavior 7.1 step 3). The recursion depth is limited to 1 in Rust (Kotlin recurses unbounded; a cycle would stack-overflow there; Proposal E-STO-07).

### 4.9 Custom buttons
```rust
// kuro-domain/src/custom_buttons/model.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomButton {
    pub id: Id, pub name: String, pub is_favorite: bool, pub sort_index: i64,
    pub content: String, pub long_press_content: String, pub on_startup: String,
}
impl CustomButton {
    pub fn render(&self, template: &str, primary_id: Id) -> String {   // getButtonContent / LongPress / OnStartup
        let is_primary = if primary_id == self.id { "true" } else { "false" };
        template.replace("$id", &self.id.to_string()).replace("$isPrimary", is_primary)
    }
    pub fn content_for(&self, primary: Id) -> String { self.render(&self.content, primary) }
    pub fn long_press_for(&self, primary: Id) -> String { self.render(&self.long_press_content, primary) }
    pub fn on_startup_for(&self, primary: Id) -> String { self.render(&self.on_startup, primary) }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomButtonUpdate {
    pub id: Id, pub name: Option<String>, pub is_favorite: Option<bool>, pub sort_index: Option<i64>,
    pub content: Option<String>, pub long_press_content: Option<String>, pub on_startup: Option<String>,
}

// kuro-domain/src/custom_buttons/repo.rs
pub trait CustomButtonRepo: Send + Sync {
    fn subscribe_all(&self) -> tokio::sync::watch::Receiver<Vec<CustomButton>>;
    fn get_all(&self) -> Result<Vec<CustomButton>, DomainError>;               // ORDER BY sortIndex
    fn insert(&self, name: &str, sort_index: i64, content: &str, long_press: &str, on_startup: &str) -> Result<(), DomainError>;   // is_favorite = false
    fn update_partial(&self, u: &CustomButtonUpdate) -> Result<(), DomainError>;      // coalesce semantics
    fn update_partial_batch(&self, us: &[CustomButtonUpdate]) -> Result<(), DomainError>;   // one transaction
    fn delete(&self, id: Id) -> Result<(), DomainError>;
}
```
`ops.rs` holds the mutex-guarded operations (`create`, `update`, `delete`, `reorder`, `toggle_favorite`) with the exact sequencing in behavior 8.x. Mutex: `tokio::sync::Mutex<()>` per `CustomButtonOps` instance (one per process).

### 4.10 Release check
```rust
// kuro-domain/src/release/check.rs
pub struct CheckArgs { pub is_preview: bool, pub commit_count: i64, pub version_name: String, pub repository: String, pub force: bool }
pub enum CheckResult { NewUpdate(Release), NoNewUpdate }   // OsTooOld dropped (T-09a-046)

#[async_trait::async_trait]
pub trait ReleaseSource: Send + Sync {
    async fn latest(&self, args: &CheckArgs) -> Result<Option<Release>, DomainError>;
}
pub struct ReleaseChecker<S: ReleaseSource> { source: S, last_checked: kuro_prefs::Pref<i64>, clock: fn() -> std::time::SystemTime }
impl<S: ReleaseSource> ReleaseChecker<S> {
    pub async fn check(&self, args: &CheckArgs) -> Result<CheckResult, DomainError>;
}
pub fn is_new_version(args: &CheckArgs, release_version: &str) -> Result<bool, DomainError>;
```
Throttle constant: `const CHECK_INTERVAL: std::time::Duration = Duration::from_secs(3 * 24 * 60 * 60);` (behavior 11.1). `last_checked` key: `__APP_STATE_last_app_check`.

Version compare (Proposal, fixes Q-REL-1..4):
```rust
pub fn is_new_version(args: &CheckArgs, tag: &str) -> Result<bool, DomainError> {
    let digits: String = tag.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    if args.is_preview {
        let n: i64 = digits.parse().map_err(|_| DomainError::Invalid("preview tag"))?;
        return Ok(n > args.commit_count);
    }
    let new = semver::Version::parse(&digits).or_else(|_| semver::Version::parse(&format!("{digits}.0.0")))?;
    let old = semver::Version::parse(&args.version_name.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect::<String>())?;
    Ok(new > old)
}
```
(Parsing both sides as `semver::Version` gives lexicographic comparison; the `.0.0` fallback covers two-part tags. Preview parse failure is an error the caller logs and treats as no update.)

### 4.11 GitHub release source
```rust
// kuro-domain/src/release/github.rs (no GTK; uses kuro-net)
#[derive(serde::Deserialize)]
pub struct GithubRelease {
    #[serde(rename = "tag_name")] pub version: String,
    #[serde(rename = "body")] pub info: String,
    #[serde(rename = "html_url")] pub release_link: String,
    pub assets: Vec<GithubAsset>,
}
#[derive(serde::Deserialize)]
pub struct GithubAsset { pub name: String, #[serde(rename = "browser_download_url")] pub download_link: String }

pub fn linkify_mentions(info: &str) -> String;     // regex from behavior 11.3
pub fn choose_asset(assets: &[GithubAsset], arch: &str) -> Option<String>;   // Linux version of BUILD_TYPES logic
```
`choose_asset` for Linux: match `arch` to the asset-name token (`x86_64`, `aarch64`); asset names must contain `-linux-` (format decided in T-09a-042). Returns `None` when nothing matches (→ `NoNewUpdate`, lastChecked not updated, behavior 11.3).

### 4.12 Storage
```rust
// kuro-domain/src/storage/layout.rs
pub const AUTOMATIC_BACKUPS: &str = "autobackup";
pub const DOWNLOADS: &str = "downloads";
pub const LOCAL_MANGA: &str = "local";
pub const LOCAL_ANIME: &str = "localanime";
pub const MPV_CONFIG: &str = "mpv-config";
pub const MPV_FONTS: &str = "fonts";
pub const MPV_SCRIPTS: &str = "scripts";
pub const MPV_SCRIPT_OPTS: &str = "script-opts";
pub const MPV_SHADERS: &str = "shaders";

pub fn default_base_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("", "", "kurobayomi")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("kurobayomi"))
}

// kuro-domain/src/storage/manager.rs
pub struct StorageManager {
    base: tokio::sync::watch::Sender<Option<std::path::PathBuf>>,   // None when unavailable
    changes: tokio::sync::watch::Sender<u64>,                       // bumps on every base change
}
impl StorageManager {
    pub fn open(configured: &str) -> Self;                                // no tree creation here (behavior 9.2 step 3)
    pub fn set_base(&self, new_base: std::path::PathBuf) -> Result<(), DomainError>;   // creates tree, then notifies
    pub fn base(&self) -> Option<std::path::PathBuf>;
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<u64>;
    pub fn downloads_dir(&self) -> Result<std::path::PathBuf, DomainError>;   // creates on call (behavior 9.3)
    pub fn automatic_backups_dir(&self) -> Result<std::path::PathBuf, DomainError>;
    pub fn local_manga_dir(&self) -> Result<std::path::PathBuf, DomainError>;
    pub fn local_anime_dir(&self) -> Result<std::path::PathBuf, DomainError>;
    pub fn mpv_config_dir(&self) -> Result<std::path::PathBuf, DomainError>;
    pub fn mpv_fonts_dir(&self) -> Result<std::path::PathBuf, DomainError>;
    pub fn mpv_scripts_dir(&self) -> Result<std::path::PathBuf, DomainError>;
    pub fn mpv_script_opts_dir(&self) -> Result<std::path::PathBuf, DomainError>;
    pub fn mpv_shaders_dir(&self) -> Result<std::path::PathBuf, DomainError>;
}
```
`downloads_dir()` etc. return `Err(DomainError::StorageUnavailable)` when `base` is `None` (Kotlin returns null). Each accessor calls `create_dir_all`, mirroring `createDirectory`.

Base directory selection UI: `gtk::FileDialog::select_folder` (GTK 4.10+, available with gtk4 feature `v4_16`) returns a `gio::File`; `gio::File::path()` gives the `PathBuf`. Non-local (`gvfs`, `sftp`) locations are rejected with `StorageUnavailable` because `std::fs` cannot write to them (Proposal, T-09a-051).

### 4.13 Backup preferences
```rust
// kuro-domain/src/backup/prefs.rs
pub struct BackupPrefs { store: kuro_prefs::Store }
impl BackupPrefs {
    pub fn interval_hours(&self) -> kuro_prefs::Pref<i32>;             // key backup_interval, default 12
    pub fn last_auto_backup_ms(&self) -> kuro_prefs::Pref<i64>;        // key __APP_STATE_last_auto_backup_timestamp, default 0
    pub fn flags(&self) -> kuro_prefs::Pref<BTreeSet<BackupFlag>>;     // key backup_flags, default {Categories, Chapters, History, Track}
}
// kuro-domain/src/backup/flags.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BackupFlag { Categories, Chapters, History, Track, Settings, ExtSettings, Extensions }
impl BackupFlag {
    pub const fn tag(self) -> &'static str {
        match self {
            BackupFlag::Categories => "1", BackupFlag::Chapters => "2", BackupFlag::History => "4",
            BackupFlag::Track => "8", BackupFlag::Settings => "10", BackupFlag::ExtSettings => "20", BackupFlag::Extensions => "40",
        }
    }
    pub fn from_tag(s: &str) -> Option<Self> { /* inverse of tag, None for unknown */ }
}
```
`interval_hours == 0` means disabled (EnableAutoBackupMigration sets 12 when 0; Proposal: store as `Option<NonZeroU32>`).

### 4.14 Upcoming
```rust
// kuro-domain/src/upcoming/mod.rs
pub const UPCOMING_MANGA_STATUSES: &[i64] = &[1, 4];        // ONGOING, PUBLISHING_FINISHED
pub const UPCOMING_ANIME_STATUSES: &[i64] = &[1, 4, 7];     // + UPCOMING
```
The feed query itself is `kuro-db` (area 05 SQL) called with these sets.

## 5. Async and threading model

| Concern | Model | Rationale |
|---|---|---|
| SQLite (rusqlite) | Single writer thread owned by `kuro-db` (area 03). Domain calls `db.call(move |conn| …).await` which runs on that thread | rusqlite is blocking; Kotlin `handler.await` maps to this |
| Network (reqwest) | `tokio` runtime on a background thread pool, started by area 01 | Kotlin `withIOContext` |
| Reactive lists (`Flow`) | `tokio::sync::watch` per query; `kuro-db` re-runs the query and sends after relevant commits | `StateFlow`-like; latest-value semantics match Kotlin's conflated collectors |
| Mutex-guarded interactors | `tokio::sync::Mutex<()>` per instance (custom buttons create/reorder/toggle; repo update via JoinSet) | Kotlin `Mutex.withLock` |
| "non-cancellable" mutations | `tokio::spawn` detached task whose `JoinHandle` is awaited by the caller; if the caller is dropped the task still completes | Kotlin `withNonCancellableContext` |
| Parallel repo refresh | `tokio::task::JoinSet`; each task returns `Result`; errors collected, not propagated (behavior 6.3 proposal) | Kotlin `coroutineScope` cancels siblings — Q-REPO-2 fixed |
| Parallel store fetch | `JoinSet` with per-task `Result`; collect `Ok` values | Kotlin `supervisorScope` semantics |
| Sequential store refresh | plain `for` loop with `.await` per store | Kotlin behavior 7.3 |
| GTK bridge | results delivered to the GTK thread through `glib::MainContext::channel` or `glib::spawn_future_local` awaiting a `tokio` join handle | GTK objects must not be touched off the main thread |
| Storage change events | `watch::Sender<u64>` bumped by `set_base` | `shareIn(replay=1)` equivalent |

Rule: domain functions are `async` and never block the GTK thread; rusqlite calls go through `kuro-db`, which is the only place that holds a connection.

## 6. Error handling and logging

### 6.1 Classes
- `DomainError` for ordinary failures (returned as `Result`).
- `RepoStoreError::Save` for SQLite constraint failures on repo inserts; `add_manga_repo` maps it to `AddRepoOutcome` (behavior 6.1).
- Track interactors: `log_and_default` helper:
```rust
pub fn log_and_default<T: Default>(r: Result<T, DomainError>, what: &'static str) -> T {
    match r { Ok(v) => v, Err(e) => { tracing::error!(error = %e, "{what}"); T::default() } }
}
```
`T::default()` gives `None` / `Vec::new()` / `()` as required.

### 6.2 Logging
- `tracing::error!` for failures; `tracing::warn!` for expected conflicts (`RepoAlreadyExists`, `DuplicateFingerprint`); `tracing::debug!` for skipped throttled checks.
- Never log secrets; URLs are logged as-is (public index URLs).

### 6.3 User-visible errors
- The domain returns typed results; the UI (areas 36/37/38) maps them to toasts. Mapping rules belong to the UI areas; this area only guarantees that the variants exist.

## 7. Android → Linux mapping (this scope)

| Android / Kotlin | Kurobayomi / Linux | Notes |
|---|---|---|
| `UniFile.fromUri(context, uri)` (SAF) | `std::path::PathBuf` from `gio::File::path()` | SAF has no Linux equivalent; local paths only (T-09a-051) |
| `UniFile.createDirectory` | `std::fs::create_dir_all` | |
| `DiskUtil.createNoMediaFile` | dropped | Android media scanner only (T-09a-050) |
| `Context` for storage | `directories::ProjectDirs` | XDG base dirs |
| `FolderProvider.path()` | `storage::layout::default_base_dir()` | |
| `Preference.appStateKey` | `kuro_prefs` key prefix `__APP_STATE_` kept for backup parity | |
| `kotlinx.coroutines.flow.Flow` / `StateFlow` | `tokio::sync::watch` | conflated latest value |
| `Channel(UNLIMITED)` + `shareIn` | `watch::Sender<u64>` | |
| `PagingSource<Long, T>` | `PageSource<T>` trait with `load(page)` | |
| `Mutex` (coroutines) | `tokio::sync::Mutex<()>` | |
| `withNonCancellableContext` | spawned task | |
| `coroutineScope` + `async` | `JoinSet` | |
| `supervisorScope` | `JoinSet` with per-task `Result` | |
| `OkHttp` `GET` + `awaitSuccess` | `reqwest::get` + `error_for_status()` | |
| `HttpUrl.toHttpUrlOrNull()` | `url::Url::parse` | |
| kotlinx `Json` / `@SerialName` | `serde` / `#[serde(rename)]` | |
| kotlinx `ProtoBuf` | deferred (area 19 schema) | |
| `Okio gzip()` | `flate2::read::GzDecoder` | |
| `java.io.Serializable` | dropped; `serde` for persistence | T-09a-054 |
| `Build.SUPPORTED_ABIS[0]` | `std::env::consts::ARCH` | |
| `logcat(LogPriority, e)` | `tracing` | |
| `IOException` subclasses | `thiserror` enums | |
| `Pins` (Kotlin data class) | `Pins(u8)` newtype | |
| `sealed interface Result` | Rust `enum` with same variant names | |
| `NonCancellable` | spawned task | |
| `@Suppress("OverridingDeprecatedMember")` | nothing (no deprecated API) | |

## 8. Crates

| Crate | Use | Why |
|---|---|---|
| `rusqlite` (bundled) | DB via `kuro-db` | required by the stack |
| `reqwest` (`rustls-tls`, `json`, `gzip`) | HTTP | required by the stack; `gzip` feature also handles `Content-Encoding`, but index bodies can be gzipped without the header, so the manual magic check stays |
| `flate2` | gzip sniffing of bodies | |
| `serde`, `serde_json` | wire DTOs | |
| `semver` | release compare | Q-REL-1..4 fixes |
| `regex` | URL and mention patterns | |
| `url` | index URL parsing | |
| `thiserror` | error enums | |
| `tokio` (`sync`, `rt-multi-thread`, `macros`, `time`, `task`) | async, watch, mutex, JoinSet | |
| `async-trait` | object-safe async traits (`PageSource`, `RepoDetailsFetcher`, `ReleaseSource`, `AnimeExtensionStoreRepo`) | `dyn` dispatch needed by the registry |
| `directories` | XDG dirs | replaces Android context |
| `tracing` | logging | |
| `kuro-prefs`, `kuro-db`, `kuro-net` | sibling crates | |
| (not used) `protobuf`/`prost` | deferred | area 19 |

No new crate is added for "set" or "watch" behaviour — `std::collections::BTreeSet` and `tokio::sync::watch` are enough.

## 9. Integration with the existing mockup

Current mockup: single package `kurobayomi` (Cargo.toml, `gtk` = `gtk4` 0.11.5 with feature `v4_16`, serde, serde_json). Source files: `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/spec.rs`, `src/entry.rs`, `src/settings.rs`, `src/player.rs`, `src/clamp.rs`; spec files `spec/app.json`, `spec/entries.json`, `spec/reader_player.json`, `spec/settings.json`.

Relevant mockup code (all fake data, no domain calls):
- `src/entry.rs:283-300` `sources(anime)`: sections "Last used", "Pinned", "English", "Other" with placeholder names "Source A" … and the local source label. Maps to `Source.is_used_last`, `Pins::PINNED`, `lang` grouping and the local source row (area 17). Wire to `SourceRegistry` in T-09a-004 follow-up.
- `src/entry.rs:360-375` `migrate_sources(anime)`: counts per source (12, 4, 1) → `SourceWithCount.count`.
- `src/entry.rs:380-460` `browse_source(id, migrating)`: Popular / Latest / Search via `BrowseQuery` (T-09a-006).
- `src/entry.rs:496-530` `tracking()`: tracker rows; `remove_tracking` dialog → `TrackRepo::delete`.
- `src/entry.rs:171-222` episode/chapter download menu: "Download", "Delete"; "Next N"/"Unwatched"/"Unread" entries = `download::filter` inputs and the download-ahead values (0,2,3,5,10).
- `spec/settings.json`: `settings_downloads`, `settings_data_storage`, `settings_backup_schema`, `settings_create_backup`, `auto_download_manga_categories`, `auto_download_anime_categories` → `DownloadPrefs` and `BackupPrefs` (T-09a-014, T-09a-052).
- `spec/app.json`: `storage`, `storage_delete_manga`, `storage_delete_anime`, `Downloaded`, `download_queue` → `StorageManager` + downloader (area 23/24).
- `spec/entries.json`: `source_options`, `source_preferences`, `sources_filter`, `tracker_search`, `tracking_sheet`, `remove_tracking`, `browse_source`, `migrate_source_search`.

Mockup gaps for this area (what it lacks): no persistence, no reactive lists, no real custom-button editor, no release check, no extension repo / store screens (the spec has none; those screens belong to area 18/19/20 UI), no storage picker. Details in `p1-tasks.md` T-09a-004/T-09a-006/T-09a-021 acceptance text.

Proposal: convert the single package into a workspace (`Cargo.toml` `[workspace] members = ["crates/*"]`) with the mockup binary left at the root until area 01 finishes; `kuro-domain` has no dependency on the root package.

## 10. Pitfalls

1. **Row-id vs entry-id** (Q-TRK-1): `awaitOne` takes a row id. Name the Rust method `by_row_id`; never pass an entry id.
2. **Track insert replaces the row** (Q-TRK-2): the UI must re-read after insert; with the Proposal `ON CONFLICT DO UPDATE` the row id is stable.
3. **Pins `contains(Unpinned)` is true** (Q-PIN-1): don't use it for "is unpinned".
4. **Sentinel strings** (Q-BR-1): never pass browse modes as text.
5. **Repo update cancels siblings** (Q-REPO-2): use `JoinSet` with per-task results.
6. **Silent fingerprint change ignored** (behavior 6.3): keep; surface a warning to the user in the UI (Proposal).
7. **Legacy store URL rewrite creates a second row** (Q-STORE-1): key by the user-entered URL.
8. **Release compare** (Q-REL-1..4): use `semver`; never index arrays by a foreign length.
9. **Storage tree not created at start** (behavior 9.2): call `StorageManager::set_base` or the accessors; the app's first-run path must call `ensure_tree()` explicitly (Proposal, T-09a-048).
10. **Custom button sort gaps**: sortIndex may have gaps after a failed compaction; the reader must sort, not index.
11. **Category id parse** (Q-DL-2): skip non-numeric strings.
12. **`toLowerCase` vs `to_lowercase`**: `visualName` uses `uppercase()` (Kotlin, locale-insensitive for ASCII); Rust `to_uppercase()` is locale-independent too; keep both ASCII-only if the lang is always ISO-639 (Proposal).
13. **Mention regex** must use case-insensitive matching (`(?i)`), as in the Kotlin `RegexOption.IGNORE_CASE`.
14. **Gzip detection**: check the magic bytes, not the header (`Content-Encoding` may be absent).
