# Unified entry/item model and domain (step 4)

Source: `docs/design/unify-map.md` (sections 1, 1b, 1c, 3, the step 4 row, section 5 Data and Backup rows) and
`docs/design/unified-db.md` (section 5.2), plus a read of `crates/kuro-{platform,db,data,domain,prefs,backup,track,
testkit}` and every caller in `src/` on 2026-10-12. Read only; nothing was built or run. Steps 1 to 3 are done: one
`Kind`, one `kurobayomi.db` (schema v1), per-kind statement modules (`statements_manga`, `statements_anime`) that
alias the unified columns back to the old names, per-kind row structs and repos still in place.

Counts below are `rg` line counts of the word, split `src` (lines/files) and other crates (lines/files, kuro-data and
kuro-db excluded unless stated). They include comments and tests, so read them as size, not as exact edit counts.

## 0. Decisions this design makes

1. **No schema change.** `user_version` stays 1. Step 4 is Rust and SQL text only, so it needs no migration and a
   rollback loses no data (section 6).
2. **New SQL module `kuro_db::tables`** (one function per statement, `kind` is a parameter). The two old modules
   stay, untouched and still translating, until their last caller moves (backup restore, domain sync). Reason: 35
   outside files use them (23 + 25 crate files, 9 src files); converting all of them in the same commit as the type
   change is the failure mode step 3 avoided.
3. **Entry and Item carry the anime-only columns as `Option`**, exactly the nullable columns of `entries`/`items`
   (the two `CHECK`s on `entries` already enforce "manga: all NULL, anime: all NOT NULL"). `Track` has no anime-only
   columns (the `tracks` table has none): it gets `kind` and the neutral names `last_item` / `total_items`.
4. **`Item.kind` is not a column.** `items` has no `kind`; the item statements join `entries` and select `e.kind`.
5. **One item flag layout: the anime superset** (`entries.item_flags`). Manga `chapter_flags` is translated only in
   `kuro-backup` (`flags.rs`, new). The wire value is first masked to the six manga groups (`0x1|0x6|0x18|0x60|0x300|
   0x100000`) so a stray bit cannot collide with the moved sorting bits.
6. **One sort codec, in `kuro-platform::sort`** (lowest crate). `kuro-db`, `kuro-prefs`, `kuro-data`, `kuro-domain`
   can all see it; `kuro-prefs` cannot see `kuro-db` or `kuro-domain`, which is why the map's "kuro-domain::entries"
   cannot host it.
7. **Categories:** one read repo in `kuro-data` (`CategoryRepo`, no write methods), `CategoryService` stays the only
   write path and stays one object per kind (`App::categories(kind)` unchanged). Backup restore keeps its own
   writes on its raw connection (edge, step 12).
8. **History:** SQL and rows move to one `kuro_data::history::HistoryRepo`; `kuro_domain::history::repo::HistoryRepo`
   keeps its API (18 src lines) and delegates. One recorder, `SessionTimer`, with the 12 h clamp.
9. **Expand then contract.** Phases A to C add the shared types and turn the old per-kind types into thin wrappers
   (`From` conversions, bodies delegate). Phase D moves callers off the wrappers in disjoint lanes. Phase E deletes
   the wrappers. Every commit compiles because the old names keep working until nobody uses them.
10. **Decision 4 (library defaults as a session copy)** is delivered here as `SessionFlags` in the domain and wired
    on the manga page; the anime page already works that way and switches to the shared type.
11. **Defaults I chose where the docs are silent (say if you want otherwise):**
    - (a) A flag write bumps `entries.version` for both kinds. The manga title page already does (E-DB-1,
      `src/entry_core/store.rs:34`); `kuro_data::flags_write` and so the anime path do not.
    - (b) The cover/background refresh rule is Aniyomi's: the cached file is always deleted before a new timestamp
      is set, also when the stored URL is empty (today anime skips the delete, test vector E-R7 changes).
    - (c) A custom cover needs a library entry for both kinds (`kuro-local::art_route` already says so for local
      rows; `kuro-images/src/invalidate.rs:168` lets manga skip it today). Needs your call: it removes a manga
      ability.

---

## 1. The shared types and where they live

### 1.1 `kuro-platform` (no internal dependencies)

```rust
// crates/kuro-platform/src/kind.rs  (add to the existing impl)
impl Kind {
    /// Manga [1, 4], anime [1, 4, 7] (replaces kuro_data::{manga,anime}::UPCOMING_STATUSES, 8 refs).
    pub const fn upcoming_statuses(self) -> &'static [i64];
}

// crates/kuro-platform/src/sort.rs  (new; kuro_domain::library::sort moved verbatim + the flag helpers of
// kuro_db::flags::library_sort + the token lists of kuro_prefs::library)
pub const SORT_TYPE_MASK: i64 = 0x3C;   pub const SORT_DIR_MASK: i64 = 0x40;   pub const SORT_MASK: i64 = 0x7C;
pub const DEFAULT_FLAGS: i64 = 0x40;    // Alphabetical, ascending: the seed of `categories.flags`
pub enum Direction { Ascending, Descending }
pub enum SortType { Alphabetical, LastActivity, LastUpdate, UnreadCount, TotalItems, LatestItem, ItemFetchDate,
                    DateAdded, TrackerMean, AiringTime, Random }
pub struct LibrarySort { pub ty: SortType, pub dir: Direction }
impl SortType   { pub const fn types(k: Kind) -> &'static [SortType]; pub const fn flag(self) -> i64;
                  pub const fn valid_for(self, k: Kind) -> bool; pub const fn token(self, k: Kind) -> Option<&'static str>;
                  pub fn from_token(k: Kind, t: &str) -> Option<Self>; pub fn from_flags(k: Kind, flags: i64) -> Self; }
impl LibrarySort{ pub const DEFAULT: Self; pub const fn flag(self) -> i64; pub fn from_flags(k: Kind, f: Option<i64>) -> Self;
                  pub fn is_ascending(self) -> bool; pub const fn apply_to(self, flags: i64) -> i64;
                  pub fn serialize(self, k: Kind) -> String; pub fn deserialize(k: Kind, s: &str) -> Self;       // lenient
                  pub fn parse(k: Kind, s: &str) -> Option<Self>; }   // strict: None for "" or no comma (the pref decoder)
pub fn tokens(k: Kind) -> Vec<&'static str>;                  // replaces MANGA_SORT_TOKENS / ANIME_SORT_TOKENS
pub fn flags_from_text(k: Kind, s: &str) -> i64;              // = library_sort::from_serialized
pub fn text_from_flags(k: Kind, flags: i64) -> String;        // = library_sort::to_serialized
```

`kuro_domain::library::sort` becomes `pub use kuro_platform::sort::*;` (shim, deleted in phase E).

### 1.2 `kuro-db`

```rust
// crates/kuro-db/src/flags.rs: one item layout, neutral names (values = kuro_domain::entries::anime::episode_flags)
pub mod item {                                  // entries.item_flags, the anime superset
    pub const SORT_DESC: i64 = 0x0;  SORT_ASC = 0x1;  SORT_DIR_MASK = 0x1;
    pub const SHOW_UNDONE: i64 = 0x2; SHOW_DONE = 0x4; UNDONE_MASK = 0x6;          // unread/unseen
    pub const SHOW_DOWNLOADED = 0x8; SHOW_NOT_DOWNLOADED = 0x10; DOWNLOADED_MASK = 0x18;
    pub const SHOW_BOOKMARKED = 0x20; SHOW_NOT_BOOKMARKED = 0x40; BOOKMARKED_MASK = 0x60;
    pub const SHOW_FILLERMARKED = 0x80; SHOW_NOT_FILLERMARKED = 0x100; FILLERMARKED_MASK = 0x180;   // anime
    pub const SORTING_SOURCE = 0x0; SORTING_NUMBER = 0x200; SORTING_UPLOAD_DATE = 0x400;
    pub const SORTING_ALPHABET = 0x600; SORTING_MASK = 0x600;
    pub const SHOW_NOT_PREVIEWS = 0x800; PREVIEWS_MASK = 0x800; SHOW_NOT_SUMMARIES = 0x1000; SUMMARIES_MASK = 0x1000; // anime
    pub const DISPLAY_NAME = 0x0; DISPLAY_NUMBER = 0x100000; DISPLAY_MASK = 0x100000;
    pub const MANGA_MASK: i64 = 0x10_067F;      // the groups a manga row may carry; masked writes on manga use it
}
pub mod season { /* SEASON_* of schema_anime::flags, prefix dropped */ }
pub mod viewer { pub mod manga { /* READING_MODE_*, ORIENTATION_* */ } pub mod anime { /* INTRO_*, AIRING_* */ } }
pub const fn set_flag(value: i64, flag: i64, mask: i64) -> i64;          // unchanged
```
`flags::chapter`, `library_sort`, `manga_item_flags_{to,from}_unified` and `schema_anime::flags` stay as shims until
phase E (the two translation fns then move to `kuro-backup`).

```rust
// crates/kuro-db/src/codec.rs: FetchType, Memo move here (schema_anime re-exports them); Millis is deleted with the
// anime history statements.

// crates/kuro-db/src/tables/{mod,rows,entries,items,history,tracks,categories,stub_sources,library,updates}.rs  (new)
// Rows hold the stored layout: no translation, no aliases. Same fields as the columns.
pub struct EntryRow { /* every `entries` column; kind: Kind; anime-only columns are Option */ }
pub struct EntryPatch { pub id: i64, /* every other field Option<_>, coalesce update */ }
pub struct ItemRow { /* every `items` column + kind: Kind (joined) */ }   pub struct ItemPatch { .. }
pub struct HistoryRow { pub id: i64, pub item_id: i64, pub last_at: i64, pub duration_ms: i64 }
pub struct HistoryViewRow { /* historyView columns incl. kind, entry_id, item_id, item_number, last_at, duration_ms */ }
pub struct TrackRow { /* tracks columns + kind (joined) */ }   pub struct CategoryRow { kind, id, name, order, flags, hidden }
pub struct LibraryRow { pub entry: EntryRow, pub category: i64, pub total: i64, pub done: i64, pub bookmarks: i64,
                        pub fillermarks: i64, pub latest_upload: i64, pub item_fetched_at: i64, pub last_activity: i64 }
pub struct UpdateRow { /* updatesView columns */ }

pub enum FlagColumn { ItemFlags, SeasonFlags }
pub mod entries {
    pub fn by_id(c: &Connection, id: i64) -> DbResult<EntryRow>;
    pub fn by_url_and_source(c, kind: Kind, url: &str, source: i64) -> Result<Option<EntryRow>>;
    pub fn favorites(c, kind) -> Result<Vec<EntryRow>>;     pub fn favorites_by_source(c, kind, source: i64) -> ..;
    pub fn watched_not_in_library(c, kind) -> ..;            pub fn all_source_and_url(c, kind) -> Result<Vec<(i64, String)>>;
    pub fn insert(c, e: &EntryRow) -> Result<i64>;           pub fn update(c, p: &EntryPatch) -> Result<usize>;
    pub fn set_viewer_bits(c, id: i64, value: i64, mask: i64) -> Result<usize>;     // bumps version
    pub fn reset_viewer(c, kind) -> Result<usize>;
    pub fn read_flags(c, col: FlagColumn, id: i64) -> Result<Option<i64>>;
    pub fn set_flags_masked(c, col: FlagColumn, id: i64, flag: i64, mask: i64) -> Result<usize>;  // bumps version; for
        // kind = 'manga' and ItemFlags the mask is ANDed with item::MANGA_MASK in SQL (CASE kind)
    pub fn write_flags(c, col: FlagColumn, id: i64, word: i64) -> Result<usize>;
    pub fn duplicates(c, kind, id: i64, title_lower: &str) -> Result<Vec<EntryRow>>;   // kuro_lower(title) = ?, SQL for both kinds
    pub fn upcoming(c, kind, start_of_day_ms: i64, statuses: &[i64]) -> ..;
    pub fn favorite_counts(c, kind) -> Result<Vec<(i64, i64)>>;   pub fn non_library_counts(c, kind) -> ..;
    pub fn clear_non_library(tx: &Transaction, kind, source_ids: &[i64]) -> Result<usize>;  // delete non-favourites of the
        // sources, set parent_id NULL on favourites whose parent went, delete reset history; returns history rows removed
    pub fn clear_ids(tx, ids: &[i64], orphaned: &[i64]) -> Result<()>;
    pub fn seasons_of(c, parent_id: i64) -> Result<Vec<SeasonRow>>;    pub fn children_of(c, parent_id) -> ..;
    pub fn deletable_parents(c) -> ..;  pub fn clearable_tree(c, root: i64) -> ..;  pub fn remove_parent_ids(c, ids) -> ..;
    pub fn set_categories(tx, entry_id: i64, ids: &[i64]) -> Result<()>;   // INSERT OR IGNORE, kind from the entry row
}
pub mod items {   // by_id, by_entry (apply_scanlator_filter), bookmarked, scanlators, by_url(kind), by_url_and_entry (ORDER BY _id, lowest
                  // first, both kinds), insert_all(tx, &[ItemRow]) -> Vec<i64> (bulk, both kinds), update(c, &ItemPatch), remove_with_ids }
pub mod history { // by_entry, by_item_url(kind), reset_by_id, reset_by_entry, remove_all(kind), remove_resetted(kind),
                  // upsert(item_id, at_ms, delta_ms)  -- sets last_at, ADDS delta (anime passes 0 until step 9),
                  // total_duration(kind), list(c, kind, query) / latest(c, kind)  -- kuro_lower, `%` `_` literal
}
pub mod tracks     { by_id, all(kind), by_entry, insert (upsert, keeps row id), update_coalesced, delete(entry_id, tracker_id) }
pub mod categories { get(kind,id), all(kind), visible(kind), by_entry(kind,entry_id), insert(kind,..) -> RETURNING _id, update, delete,
                     update_all_flags(kind), renumber_all(kind) }
pub mod stub_sources { find_all(kind), find_one(kind,id), upsert(kind, id, lang, name) }
pub mod library    { pub fn library(c, kind) -> Result<Vec<LibraryRow>>; }     // libraryView, kind-filtered
pub mod updates    { pub fn recent(c, kind, after: i64, limit: i64); pub fn by_done(c, kind, done: bool, after, limit); }
                   // ORDER BY datefetch DESC, itemId DESC in SQL (the Rust re-sort in kuro-domain is deleted)
```

### 1.3 `kuro-data` (`crates/kuro-data/src/`)

```rust
// entry.rs  (new; replaces manga.rs + anime.rs data types)
pub struct Entry {
    pub id: i64,                         // -1 until stored
    pub kind: Kind,
    pub source: i64, pub url: String, pub title: String,
    pub artist: Option<String>, pub author: Option<String>, pub description: Option<String>,
    pub genre: Option<Vec<String>>, pub status: i64, pub thumbnail_url: Option<String>,
    pub favorite: bool, pub last_update: i64, pub next_update: i64, pub fetch_interval: i32, pub date_added: i64,
    pub viewer_flags: i64,               // kind-native bits (reader mode / intro+airing), NOT translated
    pub item_flags: i64,                 // the unified layout (kuro_db::flags::item)
    pub cover_last_modified: i64, pub update_strategy: UpdateStrategy, pub initialized: bool,
    pub last_modified_at: i64, pub favorite_modified_at: Option<i64>, pub version: i64,
    // anime only: None for manga
    pub fetch_type: Option<FetchType>, pub parent_id: Option<i64>, pub season_flags: Option<i64>,
    pub season_number: Option<f64>, pub season_source_order: Option<i64>,
    pub background_url: Option<String>, pub background_last_modified: Option<i64>, pub memo: Option<Memo>,
}
impl Entry {
    pub fn create(kind: Kind) -> Self;           // Manga::create / Anime::create defaults (anime: Some(Episodes), Some(0), Some(-1.0), ..)
    pub fn fetch_type(&self) -> FetchType;       // Episodes for manga
    pub fn season_flags(&self) -> i64;           // 0 for manga
    pub fn season_number(&self) -> f64;          // -1.0
    pub fn season_source_order(&self) -> i64;    // 0
    pub fn background_last_modified(&self) -> i64;   pub fn memo(&self) -> &Map<String, Value>;   // empty for manga
    pub fn is_local(&self) -> bool;              // source == LOCAL_SOURCE_ID
}
pub struct EntryUpdate { pub id: i64, /* every field above except kind/last_modified_at/favorite_modified_at as Option<_> */ }
impl EntryUpdate { pub fn new(id: i64) -> Self; }     // an anime-only field set on a manga entry fails the CHECK (DataError::Constraint)

pub struct LibraryEntry { pub entry: Entry, pub category: i64, pub total: i64, pub done: i64, pub bookmarks: i64,
                          pub fillermarks: i64, pub latest_upload: i64, pub item_fetched_at: i64, pub last_activity: i64 }
impl LibraryEntry { pub fn id(&self) -> i64; pub fn pending(&self) -> i64; pub fn has_bookmarks(&self) -> bool; pub fn has_started(&self) -> bool; }
pub struct SeasonEntry { pub entry: Entry, pub total: i64, pub done: i64, pub bookmarks: i64, pub fillermarks: i64,
                         pub latest_upload: i64, pub fetched_at: i64, pub last_activity: i64 }
impl SeasonEntry { pub fn is_done(&self) -> bool; pub fn pending(&self) -> i64; pub fn to_library_entry(&self) -> LibraryEntry; /* category -1 */ }
pub struct DeletableEntry { pub entry_id: i64, pub source_id: i64, pub fetch_type: FetchType }

#[derive(Clone)] pub struct EntryRepo { db: Arc<Db> }
impl EntryRepo {
    pub fn new(db: Arc<Db>) -> Self;
    // reads (kind-scoped where the key is not an id)
    pub fn by_id(&self, id: i64) -> DataResult<Entry>;                                       // NotFound
    pub fn by_url_and_source(&self, kind: Kind, url: &str, source: i64) -> DataResult<Option<Entry>>;
    pub fn favorites(&self, kind: Kind) -> DataResult<Vec<Entry>>;
    pub fn favorites_by_source(&self, kind: Kind, source: i64) -> DataResult<Vec<Entry>>;
    pub fn watched_not_in_library(&self, kind: Kind) -> DataResult<Vec<Entry>>;
    pub fn library(&self, kind: Kind) -> DataResult<Vec<LibraryEntry>>;
    pub fn duplicates(&self, kind: Kind, id: i64, title: &str) -> DataResult<Vec<Entry>>;   // SQL, Unicode fold, both kinds
    pub fn upcoming(&self, kind: Kind, now_ms: i64) -> DataResult<Vec<Entry>>;               // statuses = kind.upcoming_statuses()
    pub fn all_source_and_url(&self, kind: Kind) -> DataResult<Vec<(i64, String)>>;
    pub fn favorite_counts_by_source(&self, kind: Kind) -> DataResult<Vec<(i64, i64)>>;
    pub fn non_library_counts_by_source(&self, kind: Kind) -> DataResult<Vec<(i64, i64)>>;
    pub fn seasons_by_parent(&self, parent_id: i64) -> DataResult<Vec<SeasonEntry>>;
    pub fn children_by_parent(&self, parent_id: i64) -> DataResult<Vec<Entry>>;
    pub fn deletable_parents(&self) -> DataResult<Vec<DeletableEntry>>;
    pub fn excluded_scanlators(&self, id: i64) -> DataResult<BTreeSet<String>>;
    pub fn available_scanlators(&self, id: i64) -> DataResult<BTreeSet<String>>;
    // live queries (all take the kind they need; debounce constants unchanged: entry 50 ms, library 100 ms)
    pub fn subscribe_by_id(&self, id: i64) -> Subscription<Option<Entry>>;
    pub fn subscribe_by_url_and_source(&self, kind: Kind, url: &str, source: i64) -> Subscription<Option<Entry>>;
    pub fn subscribe_favorites_by_source(&self, kind: Kind, source: i64) -> Subscription<Vec<Entry>>;
    pub fn subscribe_library(&self, kind: Kind) -> Subscription<LibraryState<LibraryEntry>>;
    pub fn subscribe_seasons_by_parent(&self, parent_id: i64) -> Subscription<Vec<SeasonEntry>>;
    pub fn subscribe_upcoming(&self, kind: Kind) -> Subscription<Vec<Entry>>;
    pub fn subscribe_upcoming_with_clock(&self, kind: Kind, clock: Arc<dyn Clock>) -> Subscription<Vec<Entry>>;
    pub fn subscribe_favorite_counts(&self, kind: Kind) -> Subscription<Vec<(i64, i64)>>;
    pub fn subscribe_non_library_counts(&self, kind: Kind) -> Subscription<Vec<(i64, i64)>>;
    pub fn subscribe_deletable_parents(&self) -> Subscription<Vec<DeletableEntry>>;
    pub fn subscribe_non_library_tree(&self) -> Subscription<(Vec<(i64, i64)>, Vec<(i64, i64)>)>;
    pub fn subscribe_excluded_scanlators(&self, id: i64) -> Subscription<BTreeSet<String>>;
    pub fn subscribe_available_scanlators(&self, id: i64) -> Subscription<BTreeSet<String>>;
    // writes
    pub fn insert(&self, e: &Entry) -> DataResult<i64>;
    pub fn lookup_or_insert(&self, e: &Entry) -> DataResult<LookupOrInsert<Entry>>;          // one writer tx, race-safe
    pub fn update(&self, u: &EntryUpdate) -> DataResult<()>;
    pub fn update_all(&self, updates: &[EntryUpdate]) -> DataResult<()>;                     // one tx
    pub fn set_categories(&self, id: i64, category_ids: &[i64]) -> DataResult<()>;
    pub fn set_excluded_scanlators(&self, id: i64, wanted: &BTreeSet<String>) -> DataResult<()>;
    pub fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool>;       // ONE masked write
    pub fn reset_viewer_flags(&self, kind: Kind) -> DataResult<()>;
    pub fn set_item_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool>;
    pub fn update_item_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool>;   // read-modify-write, one tx
    pub fn set_season_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool>;
    pub fn update_season_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool>;
    pub fn remove_parent_ids(&self, ids: &[i64]) -> DataResult<()>;
    pub fn clear_non_library(&self, kind: Kind, source_ids: &[i64]) -> DataResult<usize>;    // one rule for both kinds
    pub fn clear_non_library_ids(&self, ids: &[i64], orphaned: &[i64]) -> DataResult<()>;    // the clear screen's tree path
}

// item.rs  (new; replaces chapter.rs + episode.rs data types)
pub struct Item {
    pub id: i64, pub entry_id: i64, pub kind: Kind,
    pub url: String, pub name: String, pub scanlator: Option<String>,
    pub done: bool, pub bookmark: bool, pub fillermark: bool,          // fillermark false for manga
    pub progress: i64,                                                  // last_page_read | last_second_seen
    pub number: f64,                                                    // -1.0 unrecognised, -2.0 numberless
    pub source_order: i64, pub date_fetch: i64, pub date_upload: i64,
    pub last_modified_at: i64, pub version: i64,
    pub total_seconds: Option<i64>, pub summary: Option<String>,        // anime only
    pub preview_url: Option<String>, pub memo: Option<Memo>,
}
impl Item { pub fn create(kind: Kind) -> Self; pub fn total_seconds(&self) -> i64; pub fn memo(&self) -> &Map<String, Value>; }
pub struct ItemUpdate { pub id: i64, /* every field Option<_>; None keeps */ }   impl ItemUpdate { pub fn new(id: i64) -> Self; }
pub struct ItemKey { pub id: i64, pub url: String }                      impl From<&Item> for ItemKey
#[derive(Clone)] pub struct ItemRepo { db: Arc<Db> }
impl ItemRepo {
    pub fn new(db: Arc<Db>) -> Self;
    pub fn by_id(&self, id: i64) -> DataResult<Option<Item>>;
    pub fn by_url(&self, kind: Kind, url: &str) -> DataResult<Vec<Item>>;                    // any entry of the kind
    pub fn by_url_and_entry_id(&self, url: &str, entry_id: i64) -> DataResult<Option<Item>>; // lowest id, warns on duplicates (both kinds)
    pub fn by_entry_id(&self, entry_id: i64, apply_scanlator_filter: bool) -> DataResult<Vec<Item>>;
    pub fn bookmarked(&self, entry_id: i64) -> DataResult<Vec<Item>>;
    pub fn scanlators(&self, entry_id: i64) -> DataResult<Vec<String>>;
    pub fn subscribe_by_entry_id(&self, entry_id: i64, apply_scanlator_filter: bool) -> Subscription<Vec<Item>>;
    pub fn add_all(&self, new: &[Item]) -> DataResult<Vec<Item>>;      // one bulk statement; item.kind must equal the entry's kind
    pub fn remove_with_ids(&self, ids: &[i64]) -> DataResult<()>;      // chunks of ID_CHUNK
    pub fn update(&self, u: &ItemUpdate) -> DataResult<()>;
    pub fn update_all(&self, updates: &[ItemUpdate]) -> DataResult<()>;
}

// history.rs  (rewritten; replaces MangaHistoryRepo, AnimeHistoryRepo, HistoryRecorder, HistoryWrites)
pub struct History { pub id: i64, pub item_id: i64, pub last_at: i64, pub duration_ms: i64 }      // last_at 0 = reset
pub struct HistoryItem { pub id: i64, pub kind: Kind, pub entry_id: i64, pub item_id: i64, pub title: String,
    pub thumbnail_url: Option<String>, pub source: i64, pub favorite: bool, pub cover_last_modified: i64,
    pub item_number: f64, pub last_at: i64, pub duration_ms: i64 }
#[derive(Clone)] pub struct HistoryRepo { db: Arc<Db> }
impl HistoryRepo {
    pub fn new(db: Arc<Db>) -> Self;
    pub fn list(&self, kind: Kind, query: &str) -> DataResult<Vec<HistoryItem>>;    // newest first, ties higher id; Unicode fold; % _ literal
    pub fn watch(&self, kind: Kind, query: &str) -> Subscription<Vec<HistoryItem>>;
    pub fn latest(&self, kind: Kind) -> DataResult<Option<HistoryItem>>;
    pub fn total_duration(&self, kind: Kind) -> DataResult<i64>;
    pub fn by_entry_id(&self, entry_id: i64) -> DataResult<Vec<History>>;           // reset rows included
    pub fn by_item_url(c: &Connection, kind: Kind, url: &str) -> DataResult<Option<History>>;   // restore, lowest id
    pub fn upsert(&self, item_id: i64, at_ms: i64, delta_ms: i64) -> DataResult<()>;            // adds delta
    pub fn reset_by_id(&self, history_id: i64) -> DataResult<()>;
    pub fn reset_by_entry_id(&self, entry_id: i64) -> DataResult<()>;
    pub fn delete_all(&self, kind: Kind) -> DataResult<()>;
    pub fn remove_resetted(c: &Connection, kind: Kind) -> DataResult<usize>;
}

// track.rs  (rewritten; replaces MangaTrack, AnimeTrack, *TrackRepo, *TrackPatch)
#[derive(Serialize, Deserialize)] pub struct Track {
    pub id: i64, pub entry_id: i64, pub kind: Kind, pub tracker_id: i64, pub remote_id: i64, pub library_id: Option<i64>,
    pub title: String, pub last_item: f64, pub total_items: i64, pub status: i64, pub score: f64, pub remote_url: String,
    pub start_date: i64, pub finish_date: i64, pub private: bool,
}
impl Track { pub fn new(kind: Kind, tracker_id: i64) -> Self; pub fn copy_personal_from(&mut self, o: &Track);
             pub fn copy_personal_keeping_private(&mut self, o: &Track); }
pub struct TrackPatch { pub id: i64, /* entry_id, tracker_id, remote_id, library_id, title, last_item, total_items, status,
                          score, remote_url, start_date, finish_date, private: Option<_> */ }
pub fn update_coalesced(c: &Connection, p: &TrackPatch) -> DataResult<()>;
#[derive(Clone)] pub struct TrackRepo { db: Arc<Db> }
impl TrackRepo { pub fn new(db: Arc<Db>) -> Self; pub fn by_id(&self, id: i64) -> DataResult<Option<Track>>;
    pub fn all(&self, kind: Kind) -> DataResult<Vec<Track>>; pub fn by_entry(&self, entry_id: i64) -> DataResult<Vec<Track>>;
    pub fn insert(&self, t: &Track) -> DataResult<i64>; pub fn insert_all(&self, ts: &[Track]) -> DataResult<()>;
    pub fn delete(&self, entry_id: i64, tracker_id: i64) -> DataResult<()>;
    pub fn watch_all(&self, kind: Kind) -> Subscription<Vec<Track>>; pub fn watch_by_entry(&self, entry_id: i64) -> Subscription<Vec<Track>>; }

// source.rs: StubSource {id, lang, name} unchanged
pub struct StubSourceRepo { db: Arc<Db>, kind: Kind }
impl StubSourceRepo { pub fn new(db: Arc<Db>, kind: Kind) -> Self; pub fn upsert(&self, id: i64, lang: &str, name: &str) -> DataResult<()>;
    pub fn get(&self, id: i64) -> DataResult<Option<StubSource>>; pub fn all(&self) -> DataResult<Vec<StubSource>>;
    pub fn watch_all(&self) -> Subscription<Vec<StubSource>>; }

// category.rs: Category / CategoryUpdate unchanged. CategoryRepo is READ ONLY (kind-scoped), debounce 20 ms (the service's value):
impl CategoryRepo { pub fn new(db: Arc<Db>, kind: Kind) -> Self; pub fn kind(&self) -> Kind;
    pub fn get(&self, id: i64) -> DataResult<Option<Category>>; pub fn all(&self) -> DataResult<Vec<Category>>;
    pub fn visible(&self) -> DataResult<Vec<Category>>; pub fn by_entry(&self, entry_id: i64, visible_only: bool) -> DataResult<Vec<Category>>;
    pub fn watch_all(&self) -> Subscription<Vec<Category>>; pub fn watch_visible(&self) -> Subscription<Vec<Category>>;
    pub fn watch_by_entry(&self, entry_id: i64, visible_only: bool) -> Subscription<Vec<Category>>; }   // ordered (order, id)
// DELETED: insert, update_partial(s), update_all_flags, delete, create_manga_category, create_anime_category.
// anime_relation.rs: AnimeRelationRepo unchanged (ids only). flags_write.rs: deleted (EntryRepo owns the masked writes).
// compat.rs: update_entry / update_all_entries / reset_viewer_flags / remove_parent_ids log-and-bool wrappers (one set).
// Legacy wrappers (phases A to C, deleted in E): manga.rs, anime.rs, chapter.rs, episode.rs, and the old track/history/source
// repo names, each a few lines: `From<Entry> for Manga` etc. translate item_flags <-> chapter_flags; repos delegate.
```

### 1.4 `kuro-prefs`

```rust
// category.rs: ONE list of the category-id keys (today: category.rs 5 per kind, import.rs 6 per kind, download.rs 6 keys,
// library.rs 6 keys; backup/domain read them through four different names)
pub struct CategoryKeys { pub default: &'static str, pub update: &'static str, pub update_exclude: &'static str,
                          pub remove_exclude: &'static str, pub download_new: &'static str, pub download_new_exclude: &'static str }
pub const fn category_keys(kind: Kind) -> CategoryKeys;
impl CategoryKeys { pub fn all(&self) -> [&'static str; 6]; pub fn sets(&self) -> [&'static str; 5]; }
pub fn category_id_set_keys() -> Vec<&'static str>;          // both kinds' sets, derived (kept, 2 callers in kuro-domain)
// library.rs: accessors take the kind (the per-kind fields stay pub for the registry; callers stop matching on kind)
impl LibraryPrefs {
    pub fn default_category(&self, kind: Kind) -> &Pref<i32>;     pub fn last_used_category(&self, kind: Kind) -> &Pref<i32>;
    pub fn new_updates_count(&self, kind: Kind) -> &Pref<i32>;
    pub fn filter_downloaded(&self, kind: Kind) -> &Pref<TriState>;
    pub fn filter_unread(&self, kind: Kind) -> &Pref<TriState>;   // manga filter_unread, anime filter_unseen
    pub fn filter_completed(&self, kind: Kind) -> &Pref<TriState>;
    pub fn filter_tracked(&self, kind: Kind, tracker_id: i32) -> Pref<TriState>;
    pub fn sorting_mode(&self, kind: Kind) -> String;             // canonical text through kuro_platform::sort::LibrarySort::parse
    pub fn set_sorting_mode(&self, kind: Kind, text: &str) -> Result<(), PrefError>;
}
// SortSetting<const ANIME: bool> stays as the typed wrapper (Pref<T> needs a static decoder); its decode calls the one codec.
// DELETED: sort.rs (LibrarySort<K>, MangaSortKind, AnimeSortKind, SortDirection, SortKind), MANGA_SORT_TOKENS, ANIME_SORT_TOKENS,
//          category_preference_keys and the six *_PREF_KEY consts, download.rs key consts + CATEGORY_SET_KEYS,
//          import.rs MANGA_CATEGORY_KEYS / ANIME_CATEGORY_KEYS (-> category_keys(kind).all()), flags.rs CHAPTER_* duplicates
//          (the layouts live in kuro_db::flags::item; kuro-prefs gets no kuro-db dependency: flags.rs keeps ONE copy of the
//          default-template words it needs and a test pins it to kuro_db::flags::item in kuro-domain).
```

### 1.5 `kuro-domain` (`crates/kuro-domain/src/`)

```rust
// entries/flags.rs (new; replaces MangaExt/AnimeExt accessor halves, ChapterExt/EpisodeExt, viewer_flags.rs, flag_setters.rs getters)
pub trait EntryFlags {           // implemented for kuro_data::Entry
    fn sorting(&self) -> i64;  fn display_mode(&self) -> i64;  fn sort_descending(&self) -> bool;
    fn undone_filter(&self) -> TriState;  fn bookmarked_filter(&self) -> TriState;  fn fillermarked_filter(&self) -> TriState; // Disabled on manga
    fn downloaded_filter_raw(&self) -> i64;  fn show_previews(&self) -> bool;  fn show_summaries(&self) -> bool;
    fn expected_next_update(&self) -> Option<i64>;                      // STATUS_COMPLETED rule, both kinds
    fn as_cover(&self) -> Cover;                                         // one Cover { entry_id, source_id, favorite, url, last_modified }
    fn to_update(&self) -> EntryUpdate;  fn apply_update(&mut self, u: &EntryUpdate);
    // anime-only helpers stay on the same trait, answered from the Options (season_* filters, skip_intro_*, airing_*,
    // set_skip_intro_length, set_next_episode_airing); they are no-ops / zero for manga.
}
pub use kuro_db::flags::item;                   // the one constant set (replaces entries::anime::episode_flags, flags::chapter)
// entries/flag_setters.rs
pub enum ItemFlagGroup { Undone, Downloaded, Bookmarked, Fillermarked, Display, Previews, Summaries }   // last three anime only
pub fn set_item_flag(repo: &EntryRepo, id: i64, g: ItemFlagGroup, flag: i64) -> DataResult<bool>;        // Ok(false) for a group the kind lacks
pub fn set_item_sort_or_flip(repo: &EntryRepo, id: i64, flag: i64) -> DataResult<bool>;
pub fn set_item_all(repo: &EntryRepo, id: i64, word: i64) -> DataResult<bool>;
pub fn item_all(unread: i64, downloaded: i64, bookmarked: i64, fillermarked: i64, sorting: i64, direction: i64, display: i64,
                previews: i64, summaries: i64) -> i64;
// season_* setters, SeasonAll, SeasonOverlay unchanged but over EntryRepo.
// entries/viewer_flags.rs: reset_viewer_flags(repo, kind) -> bool; set_reading_mode, set_skip_intro_length, set_next_episode_airing over EntryRepo.
// entries/defaults.rs
pub fn item_default_word(store: &PrefStore, kind: Kind) -> i64;      // manga: the six manga groups only (anime-only groups masked out)
pub fn season_default_word(store: &PrefStore) -> i64;
pub fn apply_item_defaults(repo: &EntryRepo, store: &PrefStore, e: &Entry) -> DataResult<bool>;     // on add to library, "apply to all"
pub fn apply_season_defaults(repo: &EntryRepo, store: &PrefStore, e: &Entry) -> DataResult<bool>;   // anime parents
pub async fn defaults_to_library_detached(repo: EntryRepo, store: PrefStore, kind: Kind) -> Result<usize, DomainError>;
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub struct SessionFlags { pub item_flags: i64, pub season_flags: Option<i64> }
impl SessionFlags {              // decision 4: the page shows these for an entry NOT in the library; the row is written only by a user flag change
    pub fn for_entry(store: &PrefStore, e: &Entry) -> Option<SessionFlags>;     // None for a favourite (it follows its row)
    pub fn overlay(&self, e: &mut Entry);
    pub fn note_item_write(&mut self, flag: i64, mask: i64);   pub fn note_season_write(&mut self, flag: i64, mask: i64);
}
// DELETED: on_open_manga, on_open_anime, on_add_to_library_*, chapter_default_word, episode_default_word.
// entries/update.rs  (new home of app/update.rs + source/remote_update.rs rules; replaces UpdateManga, UpdateAnime, CoverCache)
pub enum CacheAction { Keep, DeleteCacheThenKeep, Set(i64), DeleteCacheThenSet(i64) }
pub fn image_action(stored_url: Option<&str>, is_local: bool, remote_url: Option<&str>, manual: bool,
                    has_custom: impl FnOnce() -> bool, now_ms: i64) -> CacheAction;                 // the ONE rule, public
pub trait CacheFiles: Send + Sync { fn has_custom_cover(&self, e: &Entry) -> bool; fn has_custom_background(&self, e: &Entry) -> bool;
                                    fn delete_cover(&self, e: &Entry); fn delete_background(&self, e: &Entry); }
pub struct RemoteEntry { pub title: Option<String>, pub author: Option<String>, pub artist: Option<String>, pub description: Option<String>,
    pub genre: Option<Option<Vec<String>>>, pub thumbnail_url: Option<String>, pub background_url: Option<String>, pub status: i64,
    pub update_strategy: UpdateStrategy, pub initialized: bool }                    // SourceMetadata + title + background (step 10 replaces by SEntry)
pub fn remote_from_manga(m: &SManga) -> RemoteEntry;   pub fn remote_from_anime(a: &SAnime) -> RemoteEntry;
#[derive(Clone)] pub struct UpdateEntry { repo: EntryRepo, items: ItemRepo }
impl UpdateEntry { pub fn new(repo: EntryRepo, items: ItemRepo) -> Self;
    pub fn update(&self, u: &EntryUpdate) -> DataResult<()>;  pub fn update_all(&self, u: &[EntryUpdate]) -> DataResult<()>;
    pub fn update_from_source<Tz: TimeZone>(&self, local: &Entry, remote: &RemoteEntry, manual: bool, files: &dyn CacheFiles, now: &DateTime<Tz>) -> DataResult<()>;
    pub fn update_fetch_interval<Tz: TimeZone>(&self, e: &Entry, now: &DateTime<Tz>, window: Option<(i64, i64)>) -> DataResult<()>;
    pub fn update_last_update / update_cover_last_modified / update_background_last_modified / update_favorite(id, favorite, now) ...; }
// entries/network_to_local.rs: pub fn network_to_local(repo: &EntryRepo, e: &Entry) -> DataResult<Entry>;
// entries/fetch_interval.rs: to_manga_update + to_anime_update -> to_entry_update(items: &ItemRepo, e: &Entry, now, window, now_ms) -> EntryUpdate;
//                            set_manga/anime_fetch_interval -> set_fetch_interval(repo, e, wheel ..)
// entries/queries.rs: get_entry_or_none, get_entry_by_url_and_source(kind, ..), get_favorites(kind), get_duplicate_library(kind, ..),
//   await_entry, await_items, subscribe_entry_with_items(..), get_library(kind); the anime seasons joins stay (3 fns).

// items/sorter.rs
pub trait SortItem { fn source_order(&self) -> i64; fn number(&self) -> f64; fn date_upload(&self) -> i64; fn name(&self) -> &str; }   // impl for Item
pub fn item_key(sorting: i64) -> SortKey;                       // superset masks (SORTING_NUMBER = 0x200 ...); one warn on unknown
pub fn item_cmp(sorting: i64, descending: bool) -> impl Fn(&Item, &Item) -> Ordering;
pub fn sort_seasons(list: &mut [SeasonEntry], e: &Entry);
// items/sync.rs  (replaces chapter_sync.rs + episode_sync.rs; sync_plan.rs, season_sync.rs, recognition.rs kept)
pub struct SourceItem { pub url: String, pub name: String, pub date_upload: i64, pub number: f32, pub scanlator: Option<String>,
                        pub fillermark: bool, pub summary: Option<String>, pub preview_url: Option<String>, pub memo: Option<Map<String, Value>> }
pub fn source_item_from_chapter(c: &SChapter) -> SourceItem;     // scanlator trimmed (normalize_scanlator), as today
pub fn source_item_from_episode(e: &SEpisode) -> SourceItem;     // scanlator only blank -> None, as today
pub trait ItemRenames: Send + Sync { fn rename(&self, e: &Entry, stored: &Item, prepared: &Item); }   pub struct NoRenames;
pub struct EntrySyncLocks;                                        // = MangaSyncLocks renamed (key: entry id, unique across kinds now)
pub struct SyncOptions { pub now_ms: i64, pub manual: bool, pub window: (i64, i64), pub mark_duplicate_done: bool, pub is_local: bool }
pub enum SyncItemsError { NoItems, Db(DbError) }
#[derive(Clone)] pub struct SyncItems { .. }
impl SyncItems { pub fn new(db: Arc<Db>, locks: Arc<EntrySyncLocks>, renames: Arc<dyn ItemRenames>) -> Self;
    pub async fn run(&self, raw: Vec<SourceItem>, e: &Entry, opts: SyncOptions, prepare: Option<&(dyn Fn(&mut SourceItem) + Sync)>)
        -> Result<Vec<Item>, SyncItemsError>; }                  // returns the NEW items that notify / auto-download
pub fn mark_duplicate_done_from_pref(kind: Kind, rule: &BTreeSet<String>) -> bool;   // "new" / "new_episode"
// items/status.rs (replaces app/status.rs SetReadStatus + SetSeenStatus)
pub enum SetDoneResult { Success, NoItems, InternalError(String) }
pub trait ItemDownloads: Send + Sync { fn delete(&self, e: &Entry, items: &[Item]); }      // a missing source deletes nothing (EC-27)
pub fn items_to_change(done: bool, items: &[Item]) -> Vec<Item>;        // mark done: undone rows; mark undone: done rows or progress > 0
pub fn done_update(done: bool, item: &Item) -> ItemUpdate;              // undone resets progress to 0, total_seconds untouched
#[derive(Clone)] pub struct SetDoneStatus { .. }
impl SetDoneStatus { pub fn new(items: ItemRepo, entries: EntryRepo, downloads: Arc<dyn ItemDownloads>) -> Self;
    pub async fn run(&self, done: bool, items: Vec<Item>, remove_downloads: bool) -> SetDoneResult;      // non_cancellable
    pub async fn run_entry(&self, entry_id: i64, done: bool, remove_downloads: bool) -> SetDoneResult; }
// items/filters (app/items.rs): FilterRow for Item; item_filters(e: &Entry, downloaded_only) -> Result<ListFilters, DomainError>;
//   items_filtered(&ListFilters) (one rule: fillermarked counts only when the entry is anime, Disabled otherwise); apply_filters unchanged.
// history/recorder.rs
pub const MAX_SESSION_MS: i64 = 12 * 60 * 60 * 1000;
pub struct SessionTimer { .. }                                  // = ReadTimer, kind-agnostic
impl SessionTimer { pub fn start(&mut self, now_ms: i64);
    pub fn flush(&mut self, now_ms: i64, kind: Kind, item_id: i64, incognito: bool) -> Option<HistoryUpdate>; }  // clamp 0..=12 h, start cleared always, None in incognito
pub struct WatchTicker { .. }  impl WatchTicker { pub fn tick(&mut self, now_ms, item_id, incognito) -> Option<HistoryUpdate>; pub fn force(..) }
    // 5 s throttle unchanged; HistoryUpdate.session_ms is now the playback time since the previous write (anime stores duration)
// history/repo.rs: HistoryRepo API unchanged (list, watch_list, latest, by_entry, upsert, record, record_detached, reset_one, reset_entry,
//   clear_all, remove_reset, total_duration(kind) <- gains the kind, join_pending_flushes); bodies delegate to kuro_data::history::HistoryRepo.
// updates/repo.rs: UpdatesRepo API unchanged (recent, with_read, watch_recent); ONE statement pair (kuro_db::tables::updates), no Rust re-sort.
// library/repo.rs: LibraryRepo::all(kind) over EntryRepo::library(kind); library/model.rs: LibraryRow::from_entry(&LibraryEntry)
//   (viewer_flags = entry.viewer_flags for anime, 0 for manga: identical to today); library/sort.rs = the kuro-platform re-export.
// category/: repo.rs deleted (re-export kuro_data::category::CategoryRepo); model.rs: Category re-exported + `trait CategoryExt { is_system,
//   display_name, sort(kind) }`; service.rs: CategoryService::{new(db, kind, prefs), repo, global_sort, initial_flags, create, rename,
//   toggle_hidden, update_partial(_many), move_before, delete, set_entry_categories, set_sort_mode, reset_category_flags} signatures UNCHANGED,
//   bodies call kuro_db::tables::categories(kind); the only write path (the kuro-data write methods are gone).
// track/: Track trait and TrackRepo<T> port replaced by `pub trait TrackPort { by_row_id, by_entry, all(kind), insert, insert_all, delete }`
//   over kuro_data::Track (fakes keep working); service.rs helpers lose the generic parameter.
// source/: StubSourceRepo port renamed StubSourcePort (one impl, for kuro_data::source::StubSourceRepo); MangaSourceDb/AnimeSourceDb
//   keep their two listing impls (step 10) but read EntryRepo with a kind; non_library.rs get_deletable_children over EntryRepo.
// app/delete_download.rs: DeleteChapterDownload + DeleteEpisodeDownload (no production caller) -> DeleteItemDownload implementing ItemDownloads.
```

### 1.6 `kuro-backup` (the edge; phase D1)

```rust
// crates/kuro-backup/src/flags.rs (new)
pub const MANGA_WIRE_MASK: i64 = 0x1 | 0x6 | 0x18 | 0x60 | 0x300 | 0x100000;
pub const fn chapter_flags_from_wire(x: i64) -> i64;   // tr(x & MANGA_WIRE_MASK) = (m & !0x300) | ((m & 0x300) << 1)
pub const fn chapter_flags_to_wire(item_flags: i64) -> i64;   // un(x & item::MANGA_MASK)
```
Restore writes `item_flags = chapter_flags_from_wire(b.chapter_flags)`; the creator writes `chapter_flags_to_wire(e.item_flags)`.
Anime `episodeFlags` is the stored word as is. Restore and creator use `kuro_db::tables` and the shared `Track`/`History`
types; anime history keeps `readDuration` both ways. `AnimeOutcome.dropped_read_durations` is deleted (only its test reads it).

---

## 2. Mapping: current per-kind item to its shared replacement

Callers = word count, `src` lines/files | other crates lines/files. "W" = a wrapper keeps the old name until phase E.

### 2.1 Types and repos (kuro-data)

| Current | Shared replacement | Callers | W |
|---|---|---|---|
| `Manga`, `Anime` (kuro_data::{manga,anime}) | `Entry` (+ `Entry::create(kind)`) | src 430/121, crates 717/193 (counts include `Kind::` variants; real struct use is about half) | W |
| `MangaUpdate`, `AnimeUpdate` | `EntryUpdate` | src 63/15, crates 91/16 | W |
| `LibraryManga`, `LibraryAnime` | `LibraryEntry` | src 1/1, crates 8/4 | W |
| `SeasonAnime`, `DeletableEntry` | `SeasonEntry`, `DeletableEntry` | src 18/5, crates 28/7 | W |
| `Chapter`, `Episode` | `Item` | src 430/89, crates 639/117 | W |
| `ChapterUpdate`, `EpisodeUpdate` | `ItemUpdate` | src 68/15, crates 45/10 | W |
| `ChapterKey`, `EpisodeKey` | `ItemKey` | src 23/7, crates 0/0 | |
| `MangaRepo`, `AnimeRepo` | `EntryRepo` (kind in the call) | src 195/67, crates 206/44 | W |
| `ChapterRepo`, `EpisodeRepo` | `ItemRepo` | src 117/41, crates 81/17 | W |
| `MangaTrack`, `AnimeTrack` (kuro_data) | `Track` (`last_chapter_read`/`last_episode_seen` -> `last_item`, `total_chapters`/`total_episodes` -> `total_items`) | src 27/8, crates 46/12 outside kuro-track's own types | W |
| `MangaTrackRepo`, `AnimeTrackRepo` (+ `*TrackPatch`) | `TrackRepo`, `TrackPatch`, `update_coalesced` | src 11/5, crates 38/14 | W |
| `MangaHistoryRepo`, `AnimeHistoryRepo` | `HistoryRepo` | src 9/3, crates 16/8 | W |
| `HistoryItem`, `AnimeHistoryItem`, `MangaHistory`, `AnimeHistory` | `HistoryItem`, `History` | crates 10/5 | W |
| `HistoryRecorder`, `HistoryWrites` (kuro-data) | domain `SessionTimer`; `HistoryWrites` deleted (0 users) | src 3/2, crates 5/2 | |
| `MangaStubSourceRepo`, `AnimeStubSourceRepo` | `StubSourceRepo::new(db, kind)` | src 17/5, crates 5/2 | W |
| `kuro_data::category::create_{manga,anime}_category`, `CategoryRepo::{insert,update_partial(s),update_all_flags,delete}` | deleted; `CategoryService` only | 13 test sites, 0 production | |
| `flags_write.rs` (six consts, two helpers) | `EntryRepo::{set_item_flags_masked, update_item_flags, set_season_flags_masked, update_season_flags}` | crate-private | |
| `compat::{update_manga, update_all_manga, reset_manga_viewer_flags, update_anime, update_all_anime, reset_anime_viewer_flags, remove_anime_parent_ids}` | one set over `EntryRepo` | crates 1 file | |
| `UPCOMING_STATUSES` (manga and anime) | `Kind::upcoming_statuses()` | crates 8/5 | |

### 2.2 kuro-db

| Current | Shared replacement | Callers | W |
|---|---|---|---|
| `statements_manga::{mangas, chapters, categories, mangas_categories, history, views, manga_sync, excluded_scanlators, sources}` | `kuro_db::tables::*` | src 5 files, crates 23 files | until D1 and C done (E deletes) |
| `statements_anime::*` (50 fns) | `kuro_db::tables::*` | src 4 files, crates 25 files | same |
| `models::{MangaRow, ChapterRow, ...}`, `AnimeRow`, `EpisodeRow`, `AnimeTrack`, `LibraryAnimeRow`, `SeasonAnimeRow`, `AnimeUpdateRow`, `HistoryEntry` | `tables::{EntryRow, ItemRow, TrackRow, LibraryRow, SeasonRow, UpdateRow, HistoryRow}` | src 2 files, crates 2 files + restore | same |
| `flags::chapter` + `entries::anime::episode_flags` + `schema_anime::flags::EPISODE_*` (three copies of the item layout) | `kuro_db::flags::item` | `chapter_flags::` src 10/2, crates 47/3; `episode_flags::` src 12/2, crates 56/5; `flags::chapter` src 14/10 | shims |
| `manga_item_flags_{to,from}_unified` + two SQL `tr/un` expressions in `statements_manga`, `flags_write.rs`, `src/entry_core/store.rs:34` (4 copies) | `kuro_backup::flags::{chapter_flags_from_wire, chapter_flags_to_wire}` (edge only) | crates 14/3 | E |
| `schema_anime::{FetchType, Memo, Millis}` | `codec::{FetchType, Memo}`; `Millis` deleted | src 10 files, crates 15 files | re-export |

### 2.3 Sort codec and prefs

| Current (4 codecs) | Shared replacement | Callers |
|---|---|---|
| `kuro_db::flags::library_sort` (manga flags, tokens) | `kuro_platform::sort` | crates 32/3 |
| `kuro_db::schema_anime::flags::CATEGORY_SORT_*`, `category_sort_type` | `kuro_platform::sort` | 0 outside kuro-db |
| `kuro_prefs::sort::{LibrarySort<K>, MangaSortKind, AnimeSortKind, SortDirection, SortKind}` + `MANGA/ANIME_SORT_TOKENS` | `kuro_platform::sort::{LibrarySort::parse, tokens}` | crates 15/4, 8/3 |
| `kuro_domain::library::sort::{SortType, Direction, LibrarySort}` | `kuro_platform::sort` (shim re-export) | src 75 lines in 6 files, crates 95/8 |
| `manga_sorting_mode` / `anime_sorting_mode` fields | `LibraryPrefs::{sorting_mode, set_sorting_mode}(kind)` | crates 11/4 (src reads it through `CategoryService`) |
| `default_manga_category` / `default_anime_category`, `last_used_*`, `new_*_updates_count`, `filter_downloaded_{manga,anime}`, `filter_unread` / `filter_unseen`, `filter_completed_*`, `filter_tracked_{manga,anime}` | `LibraryPrefs::{default_category, last_used_category, new_updates_count, filter_downloaded, filter_unread, filter_completed, filter_tracked}(kind)` | about 60 src lines in 12 files (`library/view/{mod,options}.rs`, `library/load.rs`, `library_update/counters.rs`, `updates/{prefs,mount}.rs`, `browse/prefs.rs`, `misc/badge.rs`, `settings_ui/library.rs`, `entry_core/{route,list_options}.rs`, `entry_core/anime/{route,store}.rs`), crates 26/6 |
| category-id keys in `kuro-prefs/{category,import,download,library}.rs` (4 lists) | `category_keys(kind)` | `kuro-backup/restore/prefs.rs` 3, `kuro-domain/category/backup.rs` 2 |

### 2.4 kuro-domain

| Current | Shared replacement | Callers | W |
|---|---|---|---|
| `MangaExt`, `AnimeExt`, `ChapterExt`, `EpisodeExt` | `EntryFlags`, `ItemExt` (number/`widen_source_number`) | src 10/10, crates 19/10 | W |
| `chapter_flags`, `episode_flags` modules; `chapter_all`, `episode_all` | `item` consts; `item_all` | see 2.2 | W |
| `set_chapter_{downloaded,unread,bookmarked,display,sort_or_flip,all}`, `set_episode_{unseen,downloaded,bookmarked,fillermarked,display,previews,summaries,sort_or_flip,all}` (15 fns) | `set_item_flag(group)`, `set_item_sort_or_flip`, `set_item_all` | src 0, crates 7 (tests) | |
| `reset_viewer_flags_manga/_anime`, `set_manga_reading_mode`, `set_anime_skip_intro_length`, `set_anime_next_episode_airing` | `reset_viewer_flags(repo, kind)`, `set_reading_mode`, `set_skip_intro_length`, `set_next_episode_airing` over `EntryRepo` | src 2/2, crates 12/4 | W |
| `chapter_default_word`, `episode_default_word`, `apply_default_{chapter,episode}_flags`, `on_open_*`, `on_add_to_library_*`, `apply_{chapter,anime}_defaults_to_library`, `*_to_library_detached` | `item_default_word(kind)`, `apply_item_defaults`, `SessionFlags`, `defaults_to_library_detached` | src 11/5 (`browse/repo_store.rs`, `entry_core/anime/store.rs`, `entry_core/route.rs`), crates 12/2 | W |
| `network_to_local_manga/_anime` | `network_to_local` | src 3/2, crates 15/5 | W |
| `UpdateManga`, `UpdateAnime`, `CoverCache`, `CacheFiles`, `manga_cover_last_modified`, `cover_last_modified`, `background_last_modified`, `image_action` (private) | `UpdateEntry`, `CacheFiles`, `image_action` (public) | src 2/2, crates 32/7; no production caller of `UpdateManga` | W |
| `to_manga_update`, `to_anime_update`, `set_manga_fetch_interval`, `set_anime_fetch_interval` | `to_entry_update`, `set_fetch_interval` | src 4/1, crates 24/6 | W |
| `SyncChapters`, `SyncEpisodes`, `ChapterSyncOptions`, `EpisodeSyncOptions`, `ChapterRenames`, `EpisodeRenames`, `NoRenames`, `NoEpisodeRenames`, `MangaSyncLocks`, `mark_duplicate_as_{read,seen}_from_pref` | `SyncItems`, `SyncOptions`, `ItemRenames`, `NoRenames`, `EntrySyncLocks`, `mark_duplicate_done_from_pref(kind)` | src 7/2 + 7/2 (`library_update/{sync,renames}.rs`, `entry_core/{backend,anime/remote,anime/refresh}.rs`), `MangaSyncLocks` src 13/4 | W |
| `SetReadStatus`, `SetSeenStatus`, `SetStatusResult`, `SetSeenResult`, `ChapterDownloads`, `EpisodeDownloads`, `DeleteChapterDownload`, `DeleteEpisodeDownload`, `chapters_to_change`/`episodes_to_change`/`chapter_status_update`/`episode_status_update` | `SetDoneStatus`, `SetDoneResult`, `ItemDownloads`, `DeleteItemDownload`, `items_to_change`, `done_update` | production uses only the four pure fns (`library/actions.rs`, 3 lines); the two structs have no src caller | W (pure fns) |
| `chapter_cmp`, `episode_cmp`, `chapter_key`, `episode_key`, `chapter_sort_key`, `episode_sort_key`, `SortItem for Chapter/Episode` | `item_cmp`, `item_key`, `SortItem for Item` | src 10/5, crates 22/4 | W |
| `chapter_filters`, `episode_filters`, `chapters_filtered`, `episodes_filtered`, `FilterRow for Chapter/Episode` | `item_filters`, `items_filtered`, `FilterRow for Item` | src 5/1, crates 26/2 | W |
| `MangaCover`, `AnimeCover` | `Cover` | src 3/3, crates 64/12 | W |
| `queries.rs` facades (`get_manga_or_none`, `get_anime_or_none`, `await_chapters`, `await_episodes`, `get_library_manga` ...) | the `*_entry_*` set (about half are deleted: no src caller) | src 0, crates 18/1 | |
| `facades.rs` (`get_chapter(s)`, `update_chapter`, `get_episode(s)`, `update_episode` ...) | `get_item`, `get_items_by_entry`, `update_item`, `update_all_items` | src 0, crates 1 | |
| `kuro_domain::category::repo::CategoryRepo` | `kuro_data::category::CategoryRepo` | src 36/10 (reads), crates 30/7 | re-export |
| `kuro_domain::track::{MangaTrack, AnimeTrack, Track, TrackRepo<T>}` | `Track`, `TrackPort` | crates 36 lines in 3 files | W |
| `source::repo::StubSourceRepo` (+ 2 impls) | `StubSourcePort` + 1 impl | crates 20/3 | W |
| `ReadTimer`, `WatchTicker`; `kuro_data::HistoryRecorder`; `src/reader/history_rules.rs::history_duration` (3 copies of the session timer) | `SessionTimer`, `WatchTicker` | src 3/2 (`reader/session.rs:239`, `reader/rules.rs` test), crates 10/2 | |
| `updates::repo` two statement pairs + Rust re-sort | one `updates` statement, one `UpdatesRepo` | src 10/4, crates 13/4 | none (API unchanged) |

### 2.5 Backup, track, images, src

| Current | Shared replacement | Callers |
|---|---|---|
| `kuro-backup` restore: `MangaRow`/`ChapterRow`/`TrackRow` + `AnimeRow`/`EpisodeRow`/`AnimeTrack` statements, `chapter_flags` as manga layout | `kuro_db::tables` + `flags::chapter_flags_from_wire`; anime history `duration_ms` kept | `restore/{manga,anime,driver,categories,prefs}.rs`, `creator/{db,entries,meta,mod}.rs`, 6 test files |
| `kuro-track/src/repo.rs` impls for 6 kuro-data repos (34 refs) and `model.rs::{to_db, from_db}` (6) | impls for `Track` / `ItemRepo` / `HistoryRepo` through small adapter structs with a `Kind`; its own seams (`MangaTrackRepo` ...) stay until step 8 | 2 files |
| `kuro-images/src/invalidate.rs:168` `art.owner == Kind::Manga \|\| art.favorite` | `art_route(is_local, favorite)` for both kinds (decision 11c) | 1 line |
| `kuro-testkit/src/domain.rs` builders (`manga()`, `anime()`, `chapter(n)`, `episode(n)`, `season(..)`), `db.rs` | `entry(kind)`, `item(kind, number)`, `season(..)`; `db.rs` unchanged | crates 9/4 |
| `src/entry_core/store.rs:34` `MASKED_WRITE` (4th copy of the translation, bumps version) | `EntryRepo::set_item_flags_masked` (bumps version for both kinds, decision 11a) | 1 |
| src per-kind repos: `MangaRepo` 122/36 files, `AnimeRepo` 73/31, `ChapterRepo` 67/22, `EpisodeRepo` 50/19 | `EntryRepo`, `ItemRepo` | lanes D2 to D4 (section 4) |

---

## 3. Behaviour that must stay identical, and what changes on purpose

### 3.1 Identical (a test pins each)

1. `libraryView` numbers per kind: totals, done, bookmarks, fillermarks, season rollup, excluded scanlators, favourites
   prefilter, category 0 for an entry in none.
2. The visible order of a chapter list for each stored sorting value and direction, including "source order is the
   inverse" and the collation compare. Stored values differ (superset), the rendered list must not.
3. The `.tachibk` wire: manga `chapterFlags` and anime `episodeFlags` have the same values in and out; a backup made
   by this build restores into a fresh profile to the same library, categories, tracks, history.
4. Triggers: `entries.version` bumps on url/description/favorite, `items.version` on done/bookmark/progress and the
   entry with it; restore's version rules (E-25) and `update_is_noop` are unchanged.
5. `lookup_or_insert` is race-safe (one row, one id); `network_to_local` returns the stored row.
6. `plan_sync` output (added, updated, removed, renames, duplicate-done marking), the fetch-interval maths, the
   "read made during a sync is not lost" columns, the excluded-scanlator filter on the new-item list. Scanlator
   normalisation stays per kind (chapters trimmed, episodes blank-to-None) in the two `SourceItem` adapters.
7. History list rules: one row per entry (the max `last_at`), `last_at > 0`, newest first, ties by higher id, Unicode
   fold, `%` `_` `\` literal, a reset keeps the row and its duration, upsert sets the time and adds the delta.
8. Category rules: system id 0 not deletable, name trimmed and unique (system excluded, hidden included), new order =
   max + 1, per-kind ids, membership drops id 0 and unknown ids, empty means Default, reorder lock per kind.
9. Preference keys and stored text (`library_sorting_mode`, `animelib_sorting_mode`, the filter keys, the category-id
   sets); an unknown sort text reads as the default and the stored text is kept.
10. Season parent/child rules: favourite children are orphaned (never deleted), non-favourite children are cleared.
11. A flag write is one masked statement: two setters on different groups never overwrite each other (EC-23).
12. Existing tests: `kuro-data` `tests.rs` (1,512 lines) and the per-repo tests; `kuro-domain` `domain_golden`,
    `quirks`, `flag_word_properties`, `library_*`, `section5_properties`, `updates_window`; the backup suites.

### 3.2 Drift assigned to step 4 (section 5) and the decided behaviour

| Drift (section 5) | Decided behaviour | Where |
|---|---|---|
| Live HistoryRecorder: no 12 h cap, start kept in incognito (`kuro-data/history.rs:332,349`); the app uses a third copy `history_duration` (`src/reader/history_rules.rs`, no cap) | `SessionTimer::flush`: clamp 0..=12 h, clear the start always, nothing written in incognito; `HistoryRepo::upsert(item, at, delta)` for both kinds; `WatchTicker` hands the anime playback delta (duration stored; step 9 wires the player) | domain recorder; `src/reader/session.rs:239` calls it |
| Duplicate episode url picks an arbitrary row | `ItemRepo::by_url_and_entry_id`: lowest id, warn on duplicates, both kinds | `kuro_db::tables::items` |
| Anime clear-non-library keeps reset history, does not orphan favourite seasons | `EntryRepo::clear_non_library(kind, sources)`: delete non-favourites, orphan favourite children, delete reset history, return the count; `src/settings.rs:251-253` calls it once for both kinds | `tables::entries::clear_non_library` |
| Anime duplicate check scans favourites in Rust | SQL `kuro_lower(title) = ?` for both kinds | `tables::entries::duplicates` |
| `create_*_category` skips the duplicate/system checks; debounce 50 ms vs 20 ms | the write methods are deleted; `CategoryService` is the only write path (also closes the Browse "new category" path, `src/live.rs:182`); one read repo at 20 ms | kuro-data category, kuro-domain category |
| Updates feed ordered only by `datefetch`, re-sorted in Rust, tie-break only in a dead copy | SQL `ORDER BY datefetch DESC, itemId DESC`, Rust re-sort deleted, limit in SQL | `tables::updates` |
| Cover refresh rule written twice | one `image_action` (public), one `CacheFiles` port; callers: `entry_core/anime/remote.rs`, `library_update/metadata.rs` (its "manual = true, no custom check" copy goes), the manga refresh path in `entry_core`; decision 11b | `entries/update.rs` |
| Episode flag constants twice, sort codes four times | `kuro_db::flags::item`; `kuro_platform::sort` | 1.1, 1.2 |
| Category-id pref keys listed three times (5 vs 6 per kind) | `category_keys(kind)` | 1.4 |
| Custom cover refused for non-favourite anime, allowed for manga | favourite required for both (decision 11c, needs your call) | `kuro-images/src/invalidate.rs:168` |
| Anime history duration dropped on restore, written 0 on create (steps 4 + 12) | restore keeps `max(db, backup)` like manga does; the creator writes `duration_ms`; `dropped_read_durations` deleted | `kuro-backup` D1 |
| Library defaults: manga writes on every open, anime uses a session copy (1b decision 4) | session copy for both: `SessionFlags::for_entry` (non-favourite only), the row is written only when the user changes a filter; on add to library `apply_item_defaults` (+ `apply_season_defaults` on the entry page); the manga runner stops calling `write_defaults` at `src/entry_core/runner.rs:136` | domain + `src/entry_core` (D2) |
| Mark read follows "delete after marked read" the same way from Library, Updates and the title page (1b decision 7) | one `SetDoneStatus::run(.., remove_downloads)`; the flag comes from one place (today the diet constant `DOWNLOAD_REMOVE_AFTER_MARKED_READ = false`, `src/misc/diet_defaults.rs:89`, read by the page and Updates; Library ignores it). Step 4 builds the service and re-points Library `mark_manga/mark_anime` and Updates `bulk::apply` to it; downloads are only actually removed once step 5 installs the download host (`download_host::install` has no caller today) | `items/status.rs`, `src/library/actions.rs`, `src/updates/bulk.rs` |

### 3.3 Found while reading (not in section 5; decided here)

- `SetReadStatus`, `SetSeenStatus`, `UpdateManga`, `UpdateAnime`, `DeleteChapterDownload`, `DeleteEpisodeDownload`,
  `CoverCache` have no production caller (tests only). They are rebuilt as the shared services above, not wrapped.
- The flag write version bump differs by kind (decision 11a).
- Manga sync inserts in one bulk statement, anime one row at a time: bulk for both.
- `HistoryRepo::total_duration` counts manga only (`get_read_duration` has no kind filter beyond the manga ids): it takes
  the kind.
- `kuro_data::source::StubSource::display_name` / `fallback` stay as they are (used by the restore screen).

---

## 4. Transition plan (compiles at every commit)

### 4.1 Rules

- Expand then contract (decision 9). Old names stay as wrappers that delegate (`From<Entry> for Manga`, `Manga::from`
  translates `item_flags <-> chapter_flags`, repos call the new repos). A wrapper is deleted in phase E only when
  `rg` shows no user.
- **No raw flag word crosses a lane seam.** A flag word travels inside an `Entry` or a legacy `Manga`; crossing is
  `Entry::from(&manga)` / `Manga::from(&entry)`, which translate. Gate after every lane: `rg '\.chapter_flags|chapter_flags::'`
  in that lane's files returns nothing.
- No schema change, so any phase can be reverted alone.
- Cargo: only the designated lane of a phase builds, and only its own crate (section 4.3). The integrator runs
  `cargo check --workspace --all-targets` after each merge and the full suite at the phase gates.

### 4.2 Phases and lanes (file ownership is disjoint)

| Phase | Lane | Owns (only these files) | Builds / tests | Seam it exports |
|---|---|---|---|---|
| A | **A** (one agent) | `crates/kuro-platform/src/{sort.rs,kind.rs,lib.rs}`; `crates/kuro-db/**` (new `tables/`, `flags.rs`, `codec.rs`, `schema_anime.rs` re-exports, `notify.rs` only if a table list changes); `crates/kuro-prefs/src/{sort,library,category,import,download,flags}.rs`; `crates/kuro-testkit/src/db.rs` | `-p kuro-platform -p kuro-db -p kuro-prefs` | `kuro_platform::sort`, `Kind::upcoming_statuses`, `kuro_db::{flags::item, tables::*, codec::*}`, `LibraryPrefs::*(kind)`, `category_keys`. Old items kept as shims. |
| B | **B** | `crates/kuro-data/src/**` | `-p kuro-data` | 1.3: `Entry/Item/Track/History/StubSourceRepo/CategoryRepo` + the legacy wrappers (all old names work) |
| C | **C** | `crates/kuro-domain/**`, `crates/kuro-testkit/src/domain.rs` | `-p kuro-domain` | 1.5, old per-kind domain names kept as wrappers for names with a src/backup/track caller (list in 2.4) |
| D | **D1** crates | `crates/kuro-backup/**`, `crates/kuro-track/src/{repo,model,fakes}.rs`, `crates/kuro-images/src/invalidate.rs`, `crates/kuro-local/src/images.rs` | `-p kuro-backup -p kuro-track` (the phase's designated builder) | 1.6; `kuro_backup::flags` |
| D | **D2** entry flow | `src/entry_core/**`, `src/reader/**`, `src/player_view/**`, `src/local_flow/**` | none | uses 1.3, 1.5 only |
| D | **D3** lists A | `src/library/**`, `src/library_update/**`, `src/misc/**`, `src/shell/**`, `src/settings_ui/**`, `src/settings.rs`, `src/data.rs`, `src/live.rs`, `src/cli.rs`, `src/backup_*.rs`, `src/watch.rs`, `src/startup.rs`, `src/feed/**`, `src/screens.rs`, `src/selection.rs`, `src/recipe_sources.rs`, `src/ext_host.rs`, `src/app_update.rs`, `src/main.rs` | none | uses 1.3, 1.5 only |
| D | **D4** lists B | `src/updates/**`, `src/browse/**`, `src/downloads/**`, `src/widgets/**`, `src/theme/**`, `src/pref_model/**` | none | uses 1.3, 1.5 only |
| E | **E** (integrator, one agent) | wrappers and shims in every crate; `crates/kuro-db/src/{statements_manga,statements_anime,models}.rs` and their tests; `kuro-prefs` per-kind leftovers | whole workspace | final gates |

Sizes (word counts of the per-kind types, see 2.1): D2 about 710 src lines, D3 about 380, D4 about 270, D1 about 750
(the track crate's own types are step 8 and untouched); B and C are the large crate rewrites. The lanes of phase D do
not share a file; the only cross-lane calls are through ids and the shared repos (the 38 `pub fn` signatures that carry
a per-kind type sit inside `src/entry_core`, plus 7 in `reader`, 2 each in `misc`, `library_update`, `downloads`; the
cross-directory ones are bridged with `From` until E).

### 4.3 Order and the commit train

1. **A1** `kuro_platform::sort` + `Kind::upcoming_statuses` (new files only). `kuro_domain::library::sort` untouched.
2. **A2** `kuro_db::flags::item`, `flags::season`, `flags::viewer`, `codec::{FetchType, Memo}` (old paths re-export).
3. **A3** `kuro_db::tables::*` with its own tests against `schema_unified::open_in_memory()`. Nothing calls it yet.
4. **A4** `kuro-prefs`: `category_keys`, the `*(kind)` accessors, `SortSetting` decoding through `kuro_platform::sort`;
   old consts and fields kept. (A1 to A4 are independent after A1; one agent does them in order.)
5. **B1** `Entry`, `EntryUpdate`, `LibraryEntry`, `SeasonEntry`, `EntryRepo`, `Item`, `ItemUpdate`, `ItemRepo`
   (new files; old repos untouched). **B2** `Track`, `TrackRepo`, `History`, `HistoryRepo`, `StubSourceRepo`, read-only
   `CategoryRepo` (new names beside the old). **B3** the old repos become wrappers over B1/B2 and translate flags in
   Rust; the SQL inside `kuro-data` is gone (`flags_write.rs` deleted). Gate: `cargo test -p kuro-data` green with the
   OLD tests unchanged plus the new ones.
6. **C1** pure domain moves: `EntryFlags`, `item` consts use, sorter, filters, `SessionTimer`, `SessionFlags`, defaults,
   `image_action`/`UpdateEntry`, fetch interval, `SyncItems`, `SetDoneStatus`. **C2** `UpdatesRepo`, `HistoryRepo`,
   `LibraryRepo`, `CategoryService` internals onto `tables`/`EntryRepo`/`HistoryRepo`; read `CategoryRepo` re-exported.
   **C3** the per-kind wrappers (list 2.4). Gate: `cargo test -p kuro-domain` green (old tests through wrappers, new
   tests direct).
7. **D1 to D4 in parallel**, each lane in its own worktree, small commits per directory, rebased on main before each
   merge. Merge order: D1, D3, D4, D2 (D2 is the largest and touches the pages the UI workflow also edits). After
   each merge the integrator runs `cargo check --workspace --all-targets`; a break goes back to its lane.
8. **E1** delete wrappers: `rg` per old name must be empty (counts in section 2 go to 0). **E2** delete
   `statements_manga`, `statements_anime`, `models`, `kuro_db::flags::{chapter, library_sort}`, `schema_anime::flags`,
   `kuro_prefs::sort`, the kuro-domain shims. **E3** `rg` gate (section 5.3), full suite, real flows.

Thin wrappers are the only compatibility code; none carries logic. Estimated wrapper size: kuro-data about 350 lines,
kuro-domain about 250, deleted in E1 (this is part of the "~5,500 lines out" of the map, which is net).

### 4.4 Contention with the UI workflow

Phase D edits roughly 190 src files, mostly import lines and field names. The other workflow edits UI files in the
same directories. Keep each D commit mechanical (types, imports, field names), rebase before merging, and ask the UI
workflow to avoid `use kuro_data::` lines and `Manga`/`Anime`/`Chapter`/`Episode` identifiers during phase D. If a
file conflicts, the D lane takes the UI side and redoes its mechanical edit.

---

## 5. Test plan

### 5.1 Existing tests that must keep passing (assertions unchanged, setup may change)

- `kuro-db`: statement tests in `statements_manga.rs` / `statements_anime.rs` (until E), `tests/unified.rs`, `runtime`,
  `subscribe`, `notify`, `open`, `codec`, `flags`.
- `kuro-data`: `tests.rs` and per-repo tests, run through the wrappers at B3 (the proof that the merge changed nothing);
  after E they are rewritten to the shared names with the same assertions.
- `kuro-domain`: `domain_golden`, `quirks`, `flag_word_properties` (its `episode_flags::SORTING_MASK` vector becomes
  `item::SORTING_MASK`), `library_flags`, `library_sort_golden`, `section5_properties`, `updates_window`, `release`,
  `recognition_vectors`; `category/tests.rs`, `history/*`, `items/*`, `entries/*`, `app/*` unit tests.
- `kuro-prefs`: registry/round-trip tests and `tests/no_other_preference_readers.rs`; `import.rs` and `category.rs` unit tests.
- `kuro-backup`: `restore_integration`, `restore_fixtures`, `create_restore_round_trip`, `end_to_end`, `prefs_wire`,
  `restore_shikimori_bangumi`, every unit test in `restore/{manga,anime,categories,driver}.rs`.
- `kuro-track` queue/retry tests; `src/` library, updates, library_update, local_flow, reader writer, player store tests.

Changed on purpose: the E-R7 cover vector (decision 11b); the anime masked-write version test (11a);
`restore/anime.rs::history_uses_the_later_timestamp_and_counts_dropped_durations` (durations are kept);
`kuro-data` tests that insert through `create_*_category` (use the service).

### 5.2 New tests that prove the merge

1. `kuro-platform`: one codec test over both kinds: every type x direction round-trips through `serialize/deserialize/parse/
   flags_from_text/text_from_flags`; the four old codecs' golden strings (taken from the old tests before deletion) all
   decode identically; `AiringTime` is anime only.
2. `kuro-db/tests/tables.rs`: every `tables` statement prepares against the baseline; one full-row round trip per row
   struct for each kind; `set_flags_masked` on a manga row ignores anime-only bits (`MANGA_MASK`) and bumps `version`;
   `clear_non_library` orphans a favourite season and removes reset history for anime (the section 5 drift row); the
   Updates query returns the same order as `ORDER BY datefetch DESC, itemId DESC` on ties.
3. `kuro-data`: `entry_repo_serves_both_kinds_on_one_handle` (same `(source, url)` in both kinds, ids differ, `favorites(kind)`
   and `library(kind)` each see their own); `item_repo_round_trips_anime_only_fields` (and `None` for manga);
   `duplicate_item_url_uses_the_lowest_id_for_both_kinds`; `history_upsert_adds_delta_for_both_kinds`;
   `history_search_folds_non_ascii_for_both_kinds`; `track_repo_by_entry_is_kind_blind_but_all_is_kind_scoped`;
   `category_repo_has_no_write_path` (compile-fail doc test or a `no_pub_write` source lint like `no_unconditional_delete_in_this_crate`);
   `legacy_manga_wrapper_round_trips_chapter_flags` (every manga-legal value, property test) until E.
4. `kuro-domain`: `session_timer_clamps_and_clears_in_incognito` (both kinds); `watch_ticker_reports_playback_delta`;
   `sync_items_matches_sync_chapters_and_sync_episodes` (the two old golden scenarios from `chapter_sync.rs` and `episode_sync.rs`
   run through `SyncItems` with identical results, including excluded-scanlator filtering and the fillermark/preview merge rules);
   `set_done_status_deletes_downloads_only_when_asked` (both kinds, one test); `session_flags_overlay_never_writes_the_row`
   (non-favourite shows defaults, a user write updates row and copy, becoming a favourite drops the copy);
   `image_action_is_one_rule` (the five rows x {manga entry, anime entry, local}); `category_service_refuses_duplicates_for_both_kinds`;
   `updates_repo_orders_by_datefetch_then_id`; `item_cmp_orders_like_chapter_cmp_and_episode_cmp` (golden `sort_orders.json`).
5. `kuro-prefs`: `category_keys_cover_every_old_list` (the union of the four old lists equals `category_keys(Manga).all() +
   category_keys(Anime).all()`); `sorting_mode_accessor_matches_the_registry_keys`.
6. `kuro-backup/tests/item_flags_edge.rs`: a backup with manga `chapterFlags` for each sorting value restores to the
   unified word and the chapter list order equals the one from `chapter_cmp`'s old golden; the creator writes the original
   wire value back; stray bits in the wire value are dropped, not moved; anime history `readDuration` survives restore and create.
7. `kuro-track`: adapters over `Track`/`ItemRepo`/`HistoryRepo` pass the existing fake-based tests; `TrackChanged` still carries an entry id.

### 5.3 Gates

- After E: `rg -w 'Manga|Anime|Chapter|Episode|MangaRepo|AnimeRepo|ChapterRepo|EpisodeRepo|MangaTrack|AnimeTrack|MangaHistoryRepo|
  AnimeHistoryRepo|MangaStubSourceRepo|AnimeStubSourceRepo' -g '*.rs'` finds only the wire structs in `kuro-backup` and the
  source-layer types (`SManga`, `SAnime`, `SChapter`, `SEpisode`, step 10); `rg 'chapter_flags|statements_manga|statements_anime|schema_anime|library_sort'`
  finds only `kuro-backup` (wire edge). `cargo build`, `cargo test --workspace` green, app launches.

### 5.4 Real flows on the test display (end of phase D and again after E)

1. **Library filters and sort per kind survive a restart.** Manga library: sort Last read, filter Unread; Anime library:
   sort Airing time, filter Downloaded. Quit, relaunch: both libraries keep their own sort and filter, the other kind is untouched.
2. **Restore an Aniyomi backup, check manga chapter order.** Restore a backup with both sections. Open a manga whose
   chapters were sorted by number (and one by upload date) in Aniyomi: the list order matches. Create a backup, restore it
   into a fresh profile, open the same manga: same order. Aniyomi's anime episode sort is unchanged.
3. **Duplicate category refused from Browse.** Browse a manga, Add to library, New category "Reading" twice: the second is
   refused; same for an anime. (`src/live.rs:182` path.)
4. **Library update on both kinds.** Run Update library with one manga and one anime that have new items: new items appear
   in Updates for both, the manga's excluded scanlator is respected, covers refresh.
5. **Read two chapters, history shows time.** Read two chapters of a manga for a few minutes: History shows the manga at the
   latest chapter with a read time; Watch an episode: History shows it.
6. Title-page defaults (decision 4): open a non-library manga and a non-library anime, change a filter, leave and reopen:
   defaults again; add to library: the entry keeps the word it had when added; reopen it from the library: its own row.
7. Mark read from Library and from Updates for one manga and one anime gives the same rows (decision 7).
8. Clear database screen: clearing a source of each kind leaves favourites and orphans a favourite season (anime).

---

## 6. Risks and rollback

| Risk | Mitigation |
|---|---|
| About 2,500 mechanical edits across 190 src files while the UI workflow edits the same files | wrappers keep every lane green; small per-directory commits; rebase before merge; ask the UI workflow to avoid imports (4.4) |
| A raw flag word crosses a seam in the wrong layout (manga 0x100 vs unified 0x200): a wrong sort order, no crash | rule in 4.1, `From` conversions are the only crossing, `rg` gate per lane, property test on the wrapper, flow 2 |
| `SessionFlags` changes manga behaviour (the row is no longer rewritten on open) | test in 5.2.4, flow 6; the old behaviour is recoverable (one call) |
| One `CategoryService` write path breaks a screen that wrote through the removed data methods | the removed methods have 0 production callers (2.1); compile errors show any miss |
| `Item.kind` by join costs one join on every item read | `items(entry_id)` and `entries(_id)` are indexed; benchmark `ItemRepo::by_entry_id` on a 5,000-item entry against the old chapter read; fall back to passing the kind into the statement (we always know it) |
| A subscription now wakes on the other kind's writes (one file) | results are compared before publishing (step 3 decision); per-kind table keys can be added to `subscribe` if profiling shows churn |
| `UNIQUE(kind, source, url)` surprises in `lookup_or_insert` callers | unchanged from step 3; `network_to_local` is the only production path |
| Wrapper deletion misses a user in a test-only module | `rg` gates in E1, `cargo check --workspace --all-targets` |
| Decisions 11a to 11c are my defaults | listed in section 0; each is a few lines to flip |
| Anime history durations start being stored at restore and in `HistoryRepo`, but the player still writes 0 until step 9 | accepted; the repo and recorder are ready, step 9 passes the delta |

**Rollback point.** The last commit of step 3 (tag it `unify-step3` before starting). Step 4 changes no schema and no
stored layout (the unified `item_flags` layout is already what step 3 stores), so `kurobayomi.db` written by a step 4 build
opens in a step 3 build unchanged and a revert loses no data. Inside the step, each phase is a mergeable unit: A and B
add code beside the old (revert = delete new files), C keeps the old domain names, D lanes are independent (a failed
lane is reverted alone), E is the only irreversible-in-practice phase and runs last, after the full suite and the
flows of 5.4 pass on the wrapper-backed tree.


## 7. Added by the lead (2026-10-12, found in the user's library)

**First sync is not an update.** When SyncItems inserts items into an entry that had none before, it stamps their
date_fetch with the entry's date_added (not "now"), so the back catalogue never shows in Updates (updatesView keeps
`date_fetch > date_added`). Today the result depends on timing: adding a title before its chapter list has loaded put
all 1,195 One Piece chapters into Updates; adding after did not. Phase C implements it in the one SyncItems, with a test
that adding before and after the first fetch both leave Updates empty, and that a later sync's new items do show.
