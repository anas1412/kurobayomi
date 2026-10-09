# Area 07 — Rust design: entries and items domain

Target: stable Rust, `rusqlite` via area:05-data-repos-entries-items's repositories, `tokio`, `gtk4-rs 0.11` (no libadwaita). Everything here is GTK-free and lives in the `kuro-core` crate (workspace layout owned by area:01-foundation-app-bootstrap; no `cfg(target_os)` here, OS code only in `kuro-platform`). Observed behaviour: p1-behavior.md. Layouts: p1-data-and-config.md and p1-topic-flags-bit-layouts.md.

## 1. Module layout

```
kuro-core/src/
  data/models.rs               // structs Manga, Anime, Chapter, Episode, MangaUpdate, AnimeUpdate, ChapterUpdate, EpisodeUpdate, SeasonAnime, LibraryManga, LibraryAnime (OWNED BY area:05-data-repos-entries-items, T-05a-004/011)
  domain/
    mod.rs
    flags.rs                   // set_flag, TriState, apply_filter, tri()
    entries/
      mod.rs
      manga.rs                 // chapter_flags consts + impl Manga { accessors }
      anime.rs                 // episode_flags, season_flags, viewer_flags consts + impl Anime { accessors }
      season_display.rs        // SeasonDisplayMode
      update.rs                // impl Manga/Anime { to_update() }, apply_update() for tests
      cover.rs                 // MangaCover / AnimeCover (cache keys)
      season_anime.rs          // impl SeasonAnime { seen(), unseen_count(), to_library_anime() }
      fetch_interval.rs        // calculate_interval, get_window, calculate_next_update
      queries.rs               // get_manga, get_anime, favourites, duplicates, by url, library subscription
      network_to_local.rs
      flag_setters.rs          // chapter/episode/season set_* (SetMangaChapterFlags & co)
      default_flags.rs         // SetDefault*Flags, reset viewer flags
    items/
      mod.rs
      model.rs                 // impl Chapter/Episode { create(), copy_from(), is_recognized_number(), to_update() }
      recognition.rs           // Recogniser, parse_chapter_number / episode / season
      sorter.rs                // chapter_cmp, episode_cmp, season_cmp, collator
      missing.rs               // missing_items_count, chapter_gap, episode_gap
      should_update.rs         // should_update_db_chapter/episode/season
      queries.rs               // get_chapter, get_chapter_by_url_and_manga_id, get_chapters_by_manga_id, get_episode, get_episode_by_url_and_anime_id, get_episodes_by_anime_id, get_seasons_by_parent_id, update_chapter, update_all_chapters, update_episode, update_all_episodes
    errors.rs                  // NoChapters / NoEpisodes / NoSeasons
    release.rs                 // GetApplicationRelease logic (P2; owned jointly with area:09-domain-source-track-download-release-misc)
```

The structs are defined once in `data/models.rs` (area:05-data-repos-entries-items) with exactly the fields of p1-data-and-config.md sections 1-2; `domain/*` adds `impl` blocks and constants. This keeps the dependency direction `domain -> data` (domain functions take repository handles), never `data -> domain`.

Tests: unit tests next to the code (`#[cfg(test)] mod tests`), integration tests in `kuro-core/tests/domain_*.rs` using `kuro-testkit` (area:01-foundation-app-bootstrap T-01a-045) and the in-memory repository harness (area:05-data-repos-entries-items T-05a-023).

## 2. Crates

| Crate | Use | Why |
|---|---|---|
| `regex` | recognisers | linear time, no look-around needed after the rewrite (p1-topic-item-number-recognition.md 4) |
| `chrono` (`clock`) | fetch interval local-day arithmetic | OS zone rules, DST correct; generic `TimeZone` for tests (Decision D2) |
| `icu_collator` + `sys-locale` | alphabetical comparisons | Android `Collator` is ICU; PRIMARY strength (D3) |
| `serde_json` | `memo` (`Map<String, Value>`), equality in `should_update_db_episode` | key-order-insensitive equality like Kotlin `JsonObject` |
| `tracing` | replaces `logcat` | area:01-foundation-app-bootstrap T-01a-005 |
| `thiserror` | error enums | small |
| `tokio` | async repository calls | area:01-foundation-app-bootstrap T-01a-009 |

No `async-trait`, no `bitflags` (D7), no repository traits (D5).

## 3. Core types and signatures

### 3.1 Flags and TriState (`domain/flags.rs`)

```rust
pub const fn set_flag(v: i64, flag: i64, mask: i64) -> i64 { (v & !mask) | (flag & mask) }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TriState { #[default] Disabled, EnabledIs, EnabledNot }
impl TriState {
    pub fn next(self) -> Self { match self { Self::Disabled => Self::EnabledIs, Self::EnabledIs => Self::EnabledNot, Self::EnabledNot => Self::Disabled } }
}
/// `applyFilter`: lazy predicate.
pub fn apply_filter(f: TriState, p: impl FnOnce() -> bool) -> bool {
    match f { TriState::Disabled => true, TriState::EnabledIs => p(), TriState::EnabledNot => !p() }
}
```

`TriState` type ownership: area:02-preferences-system (`kuro-core/src/prefs/tri_state.rs`, area:01-foundation-app-bootstrap rust-design shows it); this area re-exports it from `domain::flags`. If area:02-preferences-system lands later, T-07a-001 defines it and area:02-preferences-system imports it.

### 3.2 Entry accessors (`domain/entries/manga.rs`, `anime.rs`)

```rust
impl Manga {
    pub fn sorting(&self) -> i64 { self.chapter_flags & chapter_flags::SORTING_MASK }
    pub fn display_mode(&self) -> i64 { self.chapter_flags & chapter_flags::DISPLAY_MASK }
    pub fn unread_filter_raw(&self) -> i64 { self.chapter_flags & chapter_flags::UNREAD_MASK }
    pub fn downloaded_filter_raw(&self) -> i64 { /* & DOWNLOADED_MASK */ }
    pub fn bookmarked_filter_raw(&self) -> i64 { /* & BOOKMARKED_MASK */ }
    pub fn unread_filter(&self) -> TriState { tri(self.unread_filter_raw(), chapter_flags::SHOW_UNREAD, chapter_flags::SHOW_READ) }
    pub fn bookmarked_filter(&self) -> TriState { /* SHOW_BOOKMARKED / SHOW_NOT_BOOKMARKED */ }
    pub fn sort_descending(&self) -> bool { self.chapter_flags & chapter_flags::SORT_DIR_MASK == chapter_flags::SORT_DESC }
    pub fn expected_next_update(&self) -> Option<i64> { (self.status != STATUS_COMPLETED).then_some(self.next_update) }
    pub fn as_cover(&self) -> MangaCover { /* id, source, favorite, thumbnail_url.clone(), cover_last_modified */ }
    pub fn create() -> Self { /* defaults of p1-data-and-config.md 1.1 */ }
    pub fn to_update(&self) -> MangaUpdate { /* all 20 optional fields */ }
}
pub const STATUS_COMPLETED: i64 = 2;   // SManga.COMPLETED; the full status table lives with the source API (the source API (no plans folder yet))

impl Anime {
    // episode_flags
    pub fn sorting(&self) -> i64; pub fn display_mode(&self) -> i64;
    pub fn unseen_filter(&self) -> TriState; pub fn bookmarked_filter(&self) -> TriState; pub fn fillermarked_filter(&self) -> TriState;
    pub fn sort_descending(&self) -> bool;
    pub fn show_previews(&self) -> bool; pub fn show_summaries(&self) -> bool;
    // season_flags
    pub fn season_unseen_filter(&self) -> TriState;
    pub fn season_downloaded_filter_raw(&self) -> i64;
    pub fn season_started_filter(&self) -> TriState;
    pub fn season_completed_filter(&self) -> TriState;
    pub fn season_bookmarked_filter(&self) -> TriState;
    pub fn season_fillermarked_filter(&self) -> TriState;
    pub fn season_sorting(&self) -> i64; pub fn season_sort_descending(&self) -> bool;
    pub fn season_display_grid_mode(&self) -> SeasonDisplayMode;      // 2-bit field cannot fail
    pub fn season_display_grid_size(&self) -> i32;
    pub fn season_downloaded_overlay(&self) -> bool;
    pub fn season_unseen_overlay(&self) -> bool;
    pub fn season_local_overlay(&self) -> bool;
    pub fn season_lang_overlay(&self) -> bool;
    pub fn season_continue_overlay(&self) -> bool;
    pub fn season_display_mode(&self) -> i64;
    // viewer_flags
    pub fn skip_intro_length(&self) -> i32;           // (flags & 0xFF)
    pub fn skip_intro_disable(&self) -> bool;
    pub fn next_episode_to_air(&self) -> i32;         // (flags & 0xFFFF00) >> 8
    pub fn next_episode_airing_at(&self) -> i64;      // (flags & 0x0FFFFFFFF000000) >> 24
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonDisplayMode { CompactGrid = 0, ComfortableGrid = 1, CoverOnlyGrid = 2, List = 3 }
impl SeasonDisplayMode { pub fn from_i64(v: i64) -> Option<Self>; pub fn to_i64(self) -> i64 { self as i64 } }
```

The airing getters use shifts; the Kotlin `Double` division is exact for these masked values (p1-data-and-config.md 3.4) so the results are identical.

### 3.3 Partial updates

`MangaUpdate` / `AnimeUpdate` are `#[derive(Default, Clone, Debug)]` structs of `Option<T>` fields (area:05-data-repos-entries-items owns them as `models.rs` rows). Helper for tests and optimistic UI: `fn apply_update(&mut self, u: &MangaUpdate)` mirrors the repository's COALESCE semantics (p1-behavior.md 2.4).

### 3.4 Items (`domain/items/`)

```rust
impl Chapter {
    pub fn create() -> Self;                              // dateUpload -1, chapterNumber -1.0, version 1
    pub fn is_recognized_number(&self) -> bool { self.chapter_number >= 0.0 }
    pub fn copy_from(&self, other: &Chapter) -> Chapter;  // name, url, date_upload, chapter_number, scanlator (blank -> None)
    pub fn to_update(&self) -> ChapterUpdate;
}
impl Episode {
    pub fn create() -> Self;                              // date_upload -1, episode_number -1.0, version 1, memo {}
    pub fn is_recognized_number(&self) -> bool { self.episode_number >= 0.0 }
    pub fn copy_from(&self, other: &Episode) -> Episode;  // name, url, date_upload, episode_number, fillermark, scanlator/summary/preview_url (blank -> None)
    pub fn to_update(&self) -> EpisodeUpdate;
}

pub struct Recogniser { basic: Regex, unwanted: Regex, strip_tags: bool }
pub fn parse_chapter_number(title: &str, name: &str, known: Option<f64>) -> f64;
pub fn parse_episode_number(title: &str, name: &str, known: Option<f64>) -> f64;
pub fn parse_season_number(title: &str, name: &str, known: Option<f64>) -> f64;

pub fn chapter_cmp(sorting: i64, descending: bool) -> fn(&Chapter, &Chapter) -> Ordering   // returns a closure in practice; shown as fn for brevity
pub fn episode_cmp(sorting: i64, descending: bool) -> impl Fn(&Episode, &Episode) -> Ordering;
pub fn season_cmp(anime: &Anime) -> impl Fn(&SeasonAnime, &SeasonAnime) -> Ordering;      // ascending
pub fn sort_seasons(list: &mut [SeasonAnime], anime: &Anime);                              // reverse + alphabetical tie-break

pub fn missing_items_count(numbers: &[f64]) -> i32;
pub fn chapter_gap(higher: Option<&Chapter>, lower: Option<&Chapter>) -> i32;
pub fn gap(higher: f64, lower: f64) -> i32;                                                // shared by chapter and episode
pub fn should_update_db_chapter(db: &Chapter, src: &Chapter) -> bool;
pub fn should_update_db_episode(db: &Episode, src: &Episode) -> bool;
pub fn should_update_db_season(db: &Anime, src: &Anime) -> bool;
```

`Recogniser` is built once per kind in `LazyLock`s: the shared core `fn parse(rec: &Recogniser, title: &str, name: &str, known: Option<f64>) -> f64` implements the 7-step algorithm of p1-topic-item-number-recognition.md section 2; the three public functions are one-liners selecting the recogniser.

### 3.5 Fetch interval (`domain/entries/fetch_interval.rs`)

```rust
pub const MAX_INTERVAL: i32 = 28;
pub struct ItemDates { pub date_upload: i64, pub date_fetch: i64 }

pub fn get_window<Tz: TimeZone>(now: &DateTime<Tz>) -> (i64, i64);
pub fn calculate_interval<Tz: TimeZone>(items: &[ItemDates], tz: &Tz) -> i32;
fn increase_interval(delta: i32, since: i32, over: i32) -> i32;

pub struct IntervalUpdate { pub next_update: i64, pub fetch_interval: i32 }
pub fn to_update<Tz: TimeZone>(
    entry_fetch_interval: i32, entry_next_update: i64, entry_last_update: i64,
    items: &[ItemDates], now: &DateTime<Tz>, window: (i64, i64), clock_now_ms: i64,
) -> IntervalUpdate;
```

Callers map `Vec<Chapter>`/`Vec<Episode>` to `ItemDates` (manga: scanlator-filtered chapter list; anime: all episodes). `window == (0, 0)` is the "compute from now" sentinel exactly as in Kotlin; keep it (area:10-app-domain-layer sync calls pass `(0, 0)` by default).

### 3.6 Facades (`queries.rs`, `network_to_local.rs`, `flag_setters.rs`, `default_flags.rs`)

```rust
// entries/queries.rs  (D8: error contracts preserved)
pub async fn get_manga_or_none(repo: &MangaRepo, id: i64) -> Option<Manga>;                 // logs, swallows
pub fn subscribe_manga(repo: &MangaRepo, id: i64) -> Subscription<Manga>;
pub fn subscribe_manga_by_url(repo: &MangaRepo, url: &str, source: i64) -> Subscription<Option<Manga>>;
pub async fn get_manga_by_url_and_source(repo: &MangaRepo, url: &str, source: i64) -> Result<Option<Manga>>;
pub async fn get_favorites(repo: &MangaRepo) -> Result<Vec<Manga>>;
pub fn subscribe_favorites_by_source(repo: &MangaRepo, source: i64) -> Subscription<Vec<Manga>>;
pub async fn get_duplicates(repo: &MangaRepo, m: &Manga) -> Result<Vec<Manga>>;            // passes m.title.to_lowercase()
pub fn subscribe_library(repo: &MangaRepo) -> Subscription<Vec<LibraryManga>>;               // bounded retry, p1-behavior.md 4.1
pub fn subscribe_manga_with_chapters(m: &MangaRepo, c: &ChapterRepo, id: i64, scanlator_filter: bool)
    -> Subscription<(Manga, Vec<Chapter>)>;
// anime counterparts incl. subscribe_anime_with_episodes_and_seasons -> Subscription<(Anime, Vec<Episode>, Vec<SeasonAnime>)>

// network_to_local.rs
pub async fn network_to_local_manga(repo: &MangaRepo, m: Manga) -> Result<Manga>;          // one writer transaction
pub async fn network_to_local_anime(repo: &AnimeRepo, a: Anime) -> Result<Anime>;

// flag_setters.rs  (return Result<bool> like the repository)
pub async fn set_chapter_sorting_or_flip(repo: &MangaRepo, m: &Manga, sorting: i64) -> Result<bool>;
pub async fn set_chapter_unread_filter(repo: &MangaRepo, m: &Manga, flag: i64) -> Result<bool>;
pub fn chapter_flags_after_sort(m: &Manga, sorting: i64) -> i64;                           // pure part of the sort-or-flip state machine

// default_flags.rs
pub async fn apply_default_chapter_flags(repo: &MangaRepo, prefs: &LibraryPrefs, m: &Manga) -> Result<bool>;
pub async fn apply_default_chapter_flags_to_library(repo: &MangaRepo, prefs: &LibraryPrefs) -> Result<()>;
pub async fn reset_viewer_flags_manga(repo: &MangaRepo) -> Result<bool>;
```

Complete setter list (names of the Rust functions; each takes the entry id and performs a masked UPDATE, plus a pure `*_flags_after_*` twin):

| Kind | Functions |
|---|---|
| Chapter (6 + all) | `set_chapter_downloaded_filter`, `set_chapter_unread_filter`, `set_chapter_bookmark_filter`, `set_chapter_display_mode`, `set_chapter_sorting_or_flip`, `set_all_chapter_flags` |
| Episode (8 + all) | `set_episode_downloaded_filter`, `set_episode_unseen_filter`, `set_episode_bookmark_filter`, `set_episode_fillermark_filter`, `set_episode_display_mode`, `set_episode_sorting_or_flip`, `set_episode_previews`, `set_episode_summaries`, `set_all_episode_flags` |
| Season (15 + all) | `set_season_downloaded_filter`, `set_season_unseen_filter`, `set_season_started_filter`, `set_season_completed_filter`, `set_season_bookmarked_filter`, `set_season_fillermarked_filter`, `set_season_sorting_or_flip`, `set_season_grid_mode`, `set_season_grid_size`, `set_season_downloaded_overlay`, `set_season_unseen_overlay`, `set_season_local_overlay`, `set_season_lang_overlay`, `set_season_continue_overlay`, `set_season_display_mode`, `set_all_season_flags` |
| Viewer (anime) | `set_skip_intro_length`, `set_next_episode_airing` |
| Defaults | `apply_default_chapter_flags`, `apply_default_episode_flags`, `apply_default_season_flags`, and the matching `*_to_library` functions |
| Reset | `reset_viewer_flags_manga`, `reset_viewer_flags_anime` |

The pure core of each setter (`fn chapter_flags_after_sort(&Manga, i64) -> i64`) is separate from the repository write so the state machine is unit-testable without a DB. Setter `awaitSetAllFlags` becomes `fn chapter_flags_from_all(unread, downloaded, bookmarked, sorting, direction, display) -> i64` (pure) plus a write wrapper.

`LibraryPrefs` is the typed view of the `default_*` keys (area:02-preferences-system registry): one accessor per key of the p1-data-and-config.md section 4 table (for example `fn filter_chapter_by_read(&self) -> i64`, `fn season_unseen_overlay(&self) -> bool`), defaults as listed there.

## 4. Concurrency and threading

- All domain functions that touch the DB are `async` and run on the tokio runtime; repository handles are `Arc`-cloneable (`MangaRepo(Arc<Db>)`, area:05-data-repos-entries-items).
- The pure functions (flags, recognition, sorting, missing, fetch interval, should_update) are synchronous and cheap; call them on the GTK main thread only for tiny inputs. Sorting and filtering a 5 000-chapter list with the collator (ICU) takes milliseconds; the details-screen model sorts in a `tokio::task::spawn_blocking` job and posts the sorted `Arc<Vec<_>>` to the main loop.
- `Subscription<T>` is area:05-data-repos-entries-items's `tokio::sync::watch`-based type (T-05a-002, T-05a-022). `subscribe_manga_with_chapters` spawns one task that selects over the two receivers, rebuilds the pair on any change, and publishes through its own `watch` sender (this is `combine`: first value only after both have a value). The GTK side consumes through the area:05-data-repos-entries-items bridge `Subscription::attach(main_context, |value| render(value))`.
- `default_flags::*_to_library` must not be cancelled half way: spawn it with `tokio::spawn` and do not abort the `JoinHandle` when the dialog closes (replaces `withNonCancellableContext`).
- Time: all "now" reads go through a `Clock` (`fn now_ms(&self) -> i64`, `fn local_zone(&self) -> chrono::FixedOffset/Local`) owned by `AppContext`; tests inject a fixed clock (D9).

## 5. Error handling

- Pure functions return values, never `Result`; invalid input (unknown sort code) falls back and logs `tracing::warn!`.
- Repository errors are `data::DbError`; facades named in D8 convert them to `Option`/empty/unit and log with `tracing::error!` (message prefix `domain:`), all others return `Result`.
- `NoChapters`, `NoEpisodes`, `NoSeasons` are unit structs in `errors.rs` implementing `std::error::Error` + `Display` with the user-facing text: `"No chapters found"`, `"No episodes found"`, `"No seasons found"` (details-screen snackbars; keys `MR.strings.no_chapters_error`, `AYMR.strings.no_episodes_error`, `AYMR.strings.no_seasons_error`) and `fn library_update_text(&self) -> &'static str` returning `"No results found"` for chapters/episodes (`MR.strings.no_results_found`, used by library-update logs and generic error dialogs; seasons have no special case upstream and use their own `Display`). Strings live in the i18n catalog (the i18n catalog (no plans folder yet), `kuro-i18n`); the Rust code references keys, not literals.

## 6. How this plugs into the existing mockup

The mockup (`src/entry.rs`, `src/ui.rs`, `src/spec.rs`, `spec/entries.json`) renders static spec data: the details screen builds its chapter-settings sheet from `spec/entries.json` screens `manga_entry_details` (sheet `chapter_settings`) and `anime_entry_details` (sheets `episode_settings`, `season_settings`). The binding (T-07a-040) maps each spec item to a setter without changing the spec:

| Spec sheet / tab / item | Domain call |
|---|---|
| `chapter_settings` / Filter / `tristate Downloaded` | `set_chapter_downloaded_filter(manga, 0 / 0x8 / 0x10)` for DISABLED / ENABLED_IS / ENABLED_NOT (UI `TriState` to flag: IS = SHOW_DOWNLOADED, NOT = SHOW_NOT_DOWNLOADED) |
| `chapter_settings` / Filter / `tristate Unread` | `set_chapter_unread_filter`: IS = `SHOW_UNREAD (0x2)`, NOT = `SHOW_READ (0x4)` |
| `chapter_settings` / Filter / `tristate Bookmarked` | `set_chapter_bookmark_filter`: IS = `0x20`, NOT = `0x40` |
| `chapter_settings` / Filter / `action Scanlator` | opens `scanlator_filter` dialog (area:05-data-repos-entries-items excluded scanlators; not a flag) |
| `chapter_settings` / Sort / `sort Sort` options `By source`, `By chapter number`, `By upload date`, `Alphabetically` | `set_chapter_sorting_or_flip(manga, 0x0 / 0x100 / 0x200 / 0x300)`; the tapped option shows the arrow from `sort_descending()` |
| `chapter_settings` / Display / `list Display` (`Source title`, `Chapter number`) | `set_chapter_display_mode(manga, 0x0 / 0x100000)` |
| `episode_settings` Filter | downloaded, unseen (IS `0x2`, NOT `0x4`), bookmarked, fillermarked (IS `0x80`, NOT `0x100`) |
| `episode_settings` Sort | `0x0 / 0x200 / 0x400 / 0x600` (source, number, upload date, alphabetical) |
| `episode_settings` Display | `list Display` -> display mode; `switch Episode thumbnails` -> `awaitShowEpisodePreviews` with ON = `EPISODE_SHOW_PREVIEWS (0x0)`, OFF = `EPISODE_SHOW_NOT_PREVIEWS (0x800)`; `switch Episode summaries` -> ON = `0x0`, OFF = `0x1000` |
| `season_settings` Filter | downloaded `0x2/0x4`, unseen `0x8/0x10`, started `0x20/0x40`, completed `0x80/0x100`, bookmarked `0x200/0x400`, fillermarked `0x800/0x1000` |
| `season_settings` Sort (7 options) | `0x0, 0x2000, 0x4000, 0x6000, 0x8000, 0xA000, 0xC000` |
| `season_settings` Display `chips Display mode` | grid mode `0..3` via `set_season_grid_mode` |
| `season_settings` `slider Items per row` | `set_season_grid_size(0..=15)` |
| `season_settings` Overlay switches | five boolean overlay setters (downloaded `0x400000`, unseen `0x800000`, local `0x1000000`, language `0x2000000`, continue `0x4000000`) |
| `season_settings` `list Grid display mode` (`Source title`, `Season number`) | misnamed in the mockup: this is `SEASON_DISPLAY_MODE` (bit 27, title display), not the grid mode; correct the label to "Display" when wiring (spec text comes from the Aniyomi strings; do not edit `spec/*.json`, override the title in the binding) |
| dialogs `set_as_default`, `set_as_default_season` | `libraryPreferences.set*SettingsDefault` then optional `apply_default_*_to_library`; snackbars "Updated default chapter settings" / "Updated default episode settings" / "Updated default season settings" |
| dialog `set_fetch_interval` | `fetch_interval::to_update` with `-(wheel value)`; text "New chapters predicted to be released in around %1$s, checking around every %2$s." (anime form "New episodes predicted to be released in around %1$s, checking around every %2$s.", null form "This manga is either completed, or there is no predicted release date.") |
| dialog `skip_intro_length` | `set_skip_intro_length(anime, seconds)` (T-07a-005) |

Mockup code changes belong to T-07a-040: `entry.rs::entry` receives an `EntryViewModel` (area:10-app-domain-layer owns the full model) holding `Arc<Manga>`/`Arc<Anime>` and `Vec<ChapterRow>` built with `domain::items::sorter` + filters; the `chapter_settings` sheet's `tristate` widgets call the setter and the screen re-renders from the subscription, not from local widget state.

## 7. Android API / library mapping

| Android / Kotlin | Linux / Rust |
|---|---|
| `kotlinx.coroutines.flow.Flow`, `combine`, `retry`, `catch` | `tokio::sync::watch` + task (area:05-data-repos-entries-items `Subscription`) |
| `suspend` + `Dispatchers.IO` | `tokio` + `spawn_blocking` for rusqlite (area:05-data-repos-entries-items) |
| `withNonCancellableContext` | detached `tokio::spawn` |
| `logcat` | `tracing` |
| `java.time.ZonedDateTime/ChronoUnit` | `chrono` (`DateTime<Tz>`, `NaiveDate`) |
| `java.text.Collator(PRIMARY)` | `icu_collator` strength Primary |
| `kotlin.text.Regex` with look-around | `regex` with consuming rewrite |
| `kotlinx.serialization.json.JsonObject` | `serde_json::Map<String, Value>` |
| `androidx.compose.runtime.Immutable`, `java.io.Serializable` | dropped (T-07a-038) |
| `Injekt` factories | plain functions / `AppContext` (T-07a-038) |

## 8. Pitfalls

- `Long` vs `i32`: `fetch_interval` is `i32` (Kotlin `Int`), flags and timestamps `i64`. Narrowing from the DB uses `i32::try_from(...).unwrap_or(0)` (area:05-data-repos-entries-items).
- Kotlin `Double.compareTo` is a total order; Rust `partial_cmp` returns `None` for NaN: use `total_cmp`.
- Kotlin `String.lowercase()` is locale-independent; Rust `to_lowercase()` also; do NOT use `to_lowercase` with locale-aware crates.
- Kotlin `String.replace(old, "")` replaces ALL occurrences; Rust `str::replace` also.
- `split(", ")` for genres keeps empty segments; Rust `split(", ")` also, but `""` must map to the empty list (area:05-data-repos-entries-items).
- Do not iterate `HashMap`s when producing ordered output (airing/season lists): use `Vec` + explicit sort.
- `i64` shifts: `1_i64 << 56` is fine; `0x0FFFFFFFF000000` fits in `i64`; never use `i32` literals for masks.
- `calculate_interval` must stay lazy like the Kotlin `Sequence` (sort first, then `map -> distinct -> take(window)` as iterator adaptors that stop after `window` distinct days): converting every timestamp of a 3 000-episode list through `chrono::Local` is wasteful and, on some libc builds, takes a lock per conversion.
