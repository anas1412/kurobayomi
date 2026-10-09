# 39-ui-entry-details-anime (part 2): Rust design

This document turns the behavior in `p2-behavior.md` and the topic files into a crate layout, type and trait signatures, an async model that works with GTK4 (gtk4-rs 0.11, GTK 4.16+, no libadwaita), and the mapping from Android/Kotlin APIs. It also says how the current mockup in `/home/blackbox/kurobayomi/src/` changes.

Existing mockup (read in full for the parts that touch this screen): `src/entry.rs` (585 lines; `entry(title, anime)` at 81-240 renders manga and anime with the same static code, `tracking()` at 498-537, `tracker_search()` at 539-570), `src/screens.rs` (route `"anime_entry_details"` at line 39, `open_entry` at 62), `src/ui.rs` (`screen`, `page`, `open`, `show_sheet` 391-404, `show_dialog` 406-430, `resolve` 437-467, `menu_button` 202-219, `row` 147-164, `boxed_list` 133-139), `src/spec.rs` (spec loader; `Item`, `Group`, `Dialog`, `Sheet`, `Screen`), `spec/entries.json` (screens `anime_entry_details`, `tracking_sheet`, `tracker_search`), `Cargo.toml` (`gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde`, `serde_json`; no tokio, no rusqlite yet).

---

## 1. Crate and module layout

Workspace (`/home/blackbox/kurobayomi/Cargo.toml` becomes `[workspace]` with these members):

```
kuro-core/                       # no GTK; pure logic + services (testable without a display)
  src/
    anime/
      mod.rs
      flags.rs                   # episode/season/viewer bit codec, TriState, SeasonDisplayMode
      episodes.rs                # Episode, EpisodeUpdate, sort, filter, separators, gap, next_unseen, filter_downloaded
      selection.rs               # Selection { anchors, ids } and toggle/all/invert algorithm
      seasons.rs                 # SeasonAnime, SeasonItem, season filter/sort/overlay pipeline
      entry_model.rs             # AnimeEntryModel: State, Dialog, all actions (dispatches to services)
      library.rs                 # favorite toggle, duplicate check, category choice, enhanced bind
      refresh.rs                 # source refresh (episodes + seasons), error mapping, cover rules
      defaults.rs                # set-as-default and apply-to-all (SetAnimeDefault*)
      related.rs                 # SyncRelatedAnimeWithSource port (TTL 7 d, max 10 per group)
      airing.rs                  # loadAiringTime consumer contract + AniList/Simkl client (owner area 52)
      viewer.rs                  # skip-intro and airing writes (SetAnimeViewerFlags port)
    tracking/
      mod.rs
      row.rs                     # TrackRow (domain AnimeTrack), TrackItem, DB mapping
      service.rs                 # trait AnimeTrackerService (setters with exact Kotlin semantics)
      sheet.rs                   # TrackPage stack, status/episode/score rules, search state
      dates.rs                   # date validity rules and wall-clock conversion
    downloads/
      mod.rs                     # trait DownloadService (the subset used here)
    settings/
      mod.rs                     # typed accessors for the keys in p2-data-and-config.md section 4.5
  tests/                         # integration tests with fixtures (see p2-edge-cases-and-tests.md)

kuro-ui/   (the existing src/ mockup moves here and gains modules)
  src/
    anime_details/
      mod.rs                     # page constructor, owns EntryPageController
      page.rs                    # header, action row, description, related, list header, FAB, bottom bar
      episode_list.rs            # gio::ListStore<EpisodeRowObject> + ListView, separators, selection bar
      season_grid.rs             # gio::ListStore<SeasonRowObject> + GridView or ListView
      dialogs.rs                 # ChangeCategory, DeleteEpisodes, DuplicateAnime, Migrate, FetchInterval, SkipIntro, Settings sheets, Quality
      track_sheet.rs             # bound/unbound rows, child pages (status, episodes, score, dates, search, remove)
      snackbar.rs                # overlay snackbar/toast with action button and dismiss (see 5)
    main.rs
    ui.rs                        # existing generic helpers (keep)
    spec.rs                      # existing (keep for settings screens)
```

Rules for the split:
- Everything that decides state (filters, sort, selection, flags, validity, which dialog to show next) is in `kuro-core`. The GTK layer only renders a `State` snapshot and forwards user intents.
- `kuro-core` never imports `gtk`. Tests run with `cargo test -p kuro-core` on a headless machine.
- `kuro-core` exposes async functions; the executor is chosen in `kuro-ui` (section 3).

---

## 2. Key types and trait signatures

### 2.1 Flags (kuro-core/src/anime/flags.rs)

```rust
pub mod episode {
    pub const SORT_DIR_MASK: u64 = 0x1;
    pub const SORT_DESC: u64 = 0x0;
    pub const SORT_ASC: u64 = 0x1;
    pub const UNSEEN_MASK: u64 = 0x6;
    pub const SHOW_UNSEEN: u64 = 0x2;
    pub const SHOW_SEEN: u64 = 0x4;
    pub const DOWNLOADED_MASK: u64 = 0x18;
    pub const SHOW_DOWNLOADED: u64 = 0x8;
    pub const SHOW_NOT_DOWNLOADED: u64 = 0x10;
    pub const BOOKMARKED_MASK: u64 = 0x60;
    pub const SHOW_BOOKMARKED: u64 = 0x20;
    pub const SHOW_NOT_BOOKMARKED: u64 = 0x40;
    pub const FILLERMARKED_MASK: u64 = 0x180;
    pub const SHOW_FILLERMARKED: u64 = 0x80;
    pub const SHOW_NOT_FILLERMARKED: u64 = 0x100;
    pub const SORTING_MASK: u64 = 0x600;
    pub const SORTING_SOURCE: u64 = 0x0;
    pub const SORTING_NUMBER: u64 = 0x200;
    pub const SORTING_UPLOAD_DATE: u64 = 0x400;
    pub const SORTING_ALPHABET: u64 = 0x600;
    pub const PREVIEWS_MASK: u64 = 0x800;
    pub const SHOW_PREVIEWS: u64 = 0x0;
    pub const SHOW_NOT_PREVIEWS: u64 = 0x800;
    pub const SUMMARIES_MASK: u64 = 0x1000;
    pub const SHOW_SUMMARIES: u64 = 0x0;
    pub const SHOW_NOT_SUMMARIES: u64 = 0x1000;
    pub const DISPLAY_MASK: u64 = 0x100000;
    pub const DISPLAY_NAME: u64 = 0x0;
    pub const DISPLAY_NUMBER: u64 = 0x100000;
}

/// Same semantics as Kotlin `Long.setFlag`: keep bits outside `mask`, take `flag & mask` inside.
#[inline]
pub const fn set_flag(word: u64, flag: u64, mask: u64) -> u64 {
    (word & !mask) | (flag & mask)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TriState { Disabled, EnabledIs, EnabledNot }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonDisplayMode { CompactGrid = 0, ComfortableGrid = 1, CoverOnlyGrid = 2, List = 3 }

impl TryFrom<u64> for SeasonDisplayMode {
    type Error = FlagError;
    fn try_from(v: u64) -> Result<Self, FlagError> {
        match v {
            0 => Ok(Self::CompactGrid),
            1 => Ok(Self::ComfortableGrid),
            2 => Ok(Self::CoverOnlyGrid),
            3 => Ok(Self::List),
            other => Err(FlagError::InvalidSeasonDisplayMode(other)),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FlagError {
    #[error("invalid season display mode value {0}")]
    InvalidSeasonDisplayMode(u64),
}

/// Typed view over `animes.episode_flags` (Anime.kt:219-255).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EpisodeFlags(pub u64);

impl EpisodeFlags {
    pub fn sort_descending(self) -> bool { self.0 & episode::SORT_DIR_MASK == episode::SORT_DESC }
    pub fn sorting(self) -> u64 { self.0 & episode::SORTING_MASK }
    pub fn unseen_filter(self) -> TriState { tri(self.0, episode::UNSEEN_MASK, episode::SHOW_UNSEEN, episode::SHOW_SEEN) }
    pub fn bookmarked_filter(self) -> TriState { tri(self.0, episode::BOOKMARKED_MASK, episode::SHOW_BOOKMARKED, episode::SHOW_NOT_BOOKMARKED) }
    pub fn fillermarked_filter(self) -> TriState { tri(self.0, episode::FILLERMARKED_MASK, episode::SHOW_FILLERMARKED, episode::SHOW_NOT_FILLERMARKED) }
    /// Raw downloaded filter; the global downloaded-only override is applied by `EffectiveFilters` (see 2.3).
    pub fn downloaded_filter_raw(self) -> TriState { tri(self.0, episode::DOWNLOADED_MASK, episode::SHOW_DOWNLOADED, episode::SHOW_NOT_DOWNLOADED) }
    pub fn show_previews(self) -> bool { self.0 & episode::PREVIEWS_MASK == episode::SHOW_PREVIEWS }
    pub fn show_summaries(self) -> bool { self.0 & episode::SUMMARIES_MASK == episode::SHOW_SUMMARIES }
    pub fn display_number(self) -> bool { self.0 & episode::DISPLAY_MASK == episode::DISPLAY_NUMBER }
}

fn tri(word: u64, mask: u64, is_bit: u64, not_bit: u64) -> TriState {
    match word & mask {
        m if m == is_bit => TriState::EnabledIs,
        m if m == not_bit => TriState::EnabledNot,
        _ => TriState::Disabled,
    }
}
```

Season flags (`SeasonFlags(pub u64)`) and viewer flags (`ViewerFlags(pub u64)`) follow the same pattern. Their masks are listed in `p2-topic-seasons.md` section 2 and `p2-topic-airing-and-related.md` section 1. Viewer getters must use `u64` shifts only:

```rust
pub struct ViewerFlags(pub u64);
impl ViewerFlags {
    pub const INTRO_MASK: u64 = 0x0000_0000_0000_00FF;
    pub const AIRING_EPISODE_MASK: u64 = 0x00FF_FF00;        // bits 8-23 (Anime.kt ANIME_AIRING_EPISODE_MASK = 0xFFFF00)
    pub const AIRING_TIME_MASK: u64 = 0x0FFF_FFFF_F000_000;  // bits 24-55 (Anime.kt ANIME_AIRING_TIME_MASK)
    pub const INTRO_DISABLE_BIT: u64 = 1 << 56;               // Anime.kt ANIME_INTRO_DISABLE_MASK
    pub fn skip_intro_seconds(self) -> u8 { (self.0 & Self::INTRO_MASK) as u8 }
    pub fn intro_disabled(self) -> bool { self.0 & Self::INTRO_DISABLE_BIT == Self::INTRO_DISABLE_BIT }
    pub fn next_airing_episode(self) -> u16 { ((self.0 & Self::AIRING_EPISODE_MASK) >> 8) as u16 }
    pub fn next_airing_at_seconds(self) -> i64 { ((self.0 & Self::AIRING_TIME_MASK) >> 24) as i64 }
}
```

Implementation note: the masks are the Kotlin constants of `Anime.kt:310-313` written as Rust literals (`AIRING_TIME_MASK` = `0x0FFF_FFFF_F000_000`, 32 bits at bits 24..55; `AIRING_EPISODE_MASK` = `0x00FF_FF00`, 16 bits at bits 8..23). Test V-1 in `p2-edge-cases-and-tests.md` pins them against the Kotlin values.

### 2.2 Episodes (kuro-core/src/anime/episodes.rs)

```rust
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
    pub date_upload: i64,       // -1 = unknown
    pub episode_number: f64,    // -1.0 = unknown
    pub scanlator: Option<String>,
    pub summary: Option<String>,
    pub preview_url: Option<String>,
    pub last_modified_at: i64,
    pub version: i64,
}

impl Episode {
    pub fn is_recognized_number(&self) -> bool { self.episode_number >= 0.0 }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState { NotDownloaded = 0, Queue = 1, Downloading = 2, Downloaded = 3, Error = 4 }

#[derive(Clone, Debug, PartialEq)]
pub struct EpisodeItem {
    pub episode: Episode,
    pub download_state: DownloadState,
    pub download_progress: i32,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ListRow {
    Item(EpisodeItem),
    MissingCount { id: String, count: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey { Source, Number, UploadDate, Alphabet }

/// Pure function over the view; mirrors EpisodeSorter.kt:7-30 exactly (see table in topic episode-list section 2.2).
pub fn episode_comparator(key: SortKey, desc: bool) -> impl Fn(&Episode, &Episode) -> std::cmp::Ordering + Copy;

pub struct EffectiveFilters {
    pub unseen: TriState,
    pub bookmarked: TriState,
    pub fillermarked: TriState,
    pub downloaded: TriState,   // already forced to EnabledIs when the global downloaded-only flag is on
    pub is_local: bool,         // all episodes count as downloaded
}

/// Filter then sort (AnimeScreenModel.kt:1682-1699). Stable sort.
pub fn process(items: &[EpisodeItem], filters: &EffectiveFilters, key: SortKey, desc: bool) -> Vec<EpisodeItem>;

/// Separator rule (AnimeScreenModel.kt:1636-1659), generator over (before, after).
pub fn with_separators(view: &[EpisodeItem], desc: bool) -> Vec<ListRow>;

pub fn episode_gap(higher: &Episode, lower: &Episode) -> i64;             // MissingEpisodes.kt:40-44
pub fn gap_numbers(higher: f64, lower: f64) -> i64;                        // MissingEpisodes.kt:46-49

pub fn next_unseen<'a>(view: &'a [EpisodeItem], desc: bool) -> Option<&'a Episode>;  // findLast / find
pub fn filter_downloaded<'a>(eps: &'a [Episode], is_local: bool, cache: &dyn DownloadPresence) -> Vec<&'a Episode>;

pub trait DownloadPresence {
    /// Equivalent of AnimeDownloadCache.isEpisodeDownloaded(name, scanlator, title, source, skipCache)
    fn is_episode_downloaded(&self, name: &str, scanlator: Option<&str>, anime_title: &str, source_id: i64) -> bool;
}
```

`with_separators` in Rust (must match section 4.2 of the episode-list topic):

```rust
pub fn with_separators(view: &[EpisodeItem], desc: bool) -> Vec<ListRow> {
    if view.is_empty() { return vec![]; }
    let mut out = Vec::with_capacity(view.len() + 2);
    // Kotlin loop: for i in -1..lastIndex { before = get(i); push(before); sep(before, get(i+1)) }
    for i in -1isize..(view.len() as isize) {
        let before = if i >= 0 { Some(&view[i as usize]) } else { None };
        if let Some(b) = before { out.push(ListRow::Item(b.clone())); }
        let after = view.get((i + 1) as usize).filter(|_| i + 1 >= 0);
        if let Some(sep) = separator(before, after, desc) { out.push(sep); }
    }
    out
}

fn separator(before: Option<&EpisodeItem>, after: Option<&EpisodeItem>, desc: bool) -> Option<ListRow> {
    let (lower, higher) = if desc { (after, before) } else { (before, after) };
    let higher = higher?;                                   // no higher -> no separator
    let count: i64 = match lower {
        None => ((higher.episode.episode_number.floor() as i64) - 1).max(0),
        Some(l) => episode_gap_or_zero(&higher.episode, &l.episode),
    };
    if count <= 0 { return None; }
    let lower_id = lower.map(|l| l.episode.id.to_string()).unwrap_or_else(|| "null".into());
    Some(ListRow::MissingCount { id: format!("{lower_id}-{}", higher.episode.id), count: count as u32 })
}
```

Note: `floor(x).toInt()` in Kotlin truncates toward zero for values beyond Int range; `as i64` on f64 saturates in Rust. Values are small in practice; the test suite pins the behavior for -1.0, 0.0, 2.5.

```rust
// Selection (kuro-core/src/anime/selection.rs). Exact transcription of AnimeScreenModel.kt:1369-1455.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub anchors: [i64; 2],          // indexes into the processed view; -1 = none
    pub ids: std::collections::HashSet<i64>,
}

impl Selection {
    pub fn new() -> Self { Self { anchors: [-1, -1], ids: Default::default() } }

    /// `view` is the processed (filtered, sorted) list; returns the new view with flags applied.
    /// Returns `false` when nothing changed (Kotlin early returns).
    pub fn toggle_one(&mut self, view: &mut [EpisodeItem], episode_id: i64, selected: bool,
                      user_selected: bool, from_long_press: bool) -> bool;

    /// Applies to the full (unfiltered) list (AnimeScreenModel.kt:1433-1443).
    pub fn set_all(&mut self, full: &mut [EpisodeItem], selected: bool);

    /// Applies to the full (unfiltered) list (AnimeScreenModel.kt:1445-1455).
    pub fn invert(&mut self, full: &mut [EpisodeItem]);
}
```

`toggle_one` must follow the algorithm in `p2-topic-episode-list.md` section 5.2 line by line (the branch structure, the exclusive ranges `(a+1)..b`, `firstSelection` computed before the write, anchor updates only in the two branches named there).

### 2.3 Seasons (kuro-core/src/anime/seasons.rs)

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonAnime {
    pub anime: AnimeRef,           // the season row (id, title, status, is_local, fetch_type, ...)
    pub total: i64,
    pub seen: i64,
    pub bookmarks: i64,
    pub fillermarks: i64,
    pub latest_upload: i64,
    pub fetched_at: i64,
    pub last_seen: i64,
    pub season_number: f64,
    pub season_source_order: i64,
}
impl SeasonAnime {
    pub fn is_seen(&self) -> bool { self.total == self.seen }       // SeasonAnime.kt:18-19
    pub fn unseen_count(&self) -> i64 { self.total - self.seen }    // :21-22
    pub fn has_started(&self) -> bool { self.seen > 0 }             // :24
    pub fn has_bookmarks(&self) -> bool { self.bookmarks > 0 }
    pub fn has_fillermarks(&self) -> bool { self.fillermarks > 0 }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SeasonItem {
    pub season: SeasonAnime,
    pub download_count: i64,       // -1 = overlay hidden
    pub unseen_count: i64,         // -1 = overlay hidden
    pub is_local: bool,
    pub source_language: String,   // "" = hidden
    pub show_continue: bool,
}

pub struct SeasonFilters { pub unseen: TriState, pub downloaded: TriState, pub started: TriState,
                           pub completed: TriState, pub bookmarked: TriState, pub fillermarked: TriState }

pub struct SeasonOverlays { pub downloaded: bool, pub unseen: bool, pub local: bool, pub lang: bool, pub cont: bool }

/// Mirrors AnimeScreenModel.kt:1701-1737 exactly, including the unclamped counts for filters.
pub fn process_seasons(seasons: &[(SeasonAnime, i64 /*download count*/, String /*lang*/)],
                       filters: &SeasonFilters, key: SeasonSortKey, desc: bool,
                       overlays: &SeasonOverlays, parent_is_episodes_child: impl Fn(&SeasonAnime) -> bool) -> Vec<SeasonItem>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonSortKey { Source, Season, Upload, Alphabet, UnseenCount, LastSeen, Fetched }
```

Season download counts are computed by the caller (the download service), then passed in (keeps `kuro-core` free of download I/O).

### 2.4 Entry model (kuro-core/src/anime/entry_model.rs)

```rust
pub struct AnimeEntryModel<S: Services> {
    services: S,
    anime_id: i64,
    from_source: bool,
    state: EntryState,               // Loading | Success(Box<SuccessState>)
    selection: Selection,
    has_prompted_to_add: bool,
    is_from_change_category: bool,
    settings: EntrySettings,         // snapshot taken at construction (behavior 2.2)
}

pub enum EntryState { Loading, Success(Box<SuccessState>) }

pub struct SuccessState {
    pub anime: AnimeRef,             // full entry row
    pub source: SourceRef,           // resolved or stub
    pub is_from_source: bool,
    pub episodes: Vec<EpisodeItem>,  // FULL list (never replaced by a view; defect 23.2 fixed)
    pub seasons: Vec<SeasonItem>,    // unprocessed season items (filters are applied on read)
    pub tracking_count: usize,
    pub has_logged_in_trackers: bool,
    pub related: Vec<RelationGroup>,
    pub is_loading_related: bool,
    pub is_refreshing: bool,
    pub dialog: Option<Dialog>,
    pub track_items: Vec<TrackItem>, // populated by the tracker observer (defect 23.3 fixed)
    pub next_airing: (u16, i64),
}

/// Derived views; recomputed on demand, never stored (fixes the 23.2 issue by construction).
impl SuccessState {
    pub fn processed_episodes(&self, settings: &EntrySettings) -> Vec<EpisodeItem>;
    pub fn episode_rows(&self, settings: &EntrySettings) -> Vec<ListRow>;   // with separators
    pub fn processed_seasons(&self, settings: &EntrySettings) -> Vec<SeasonItem>;
    pub fn airing_time_ms(&self, now_ms: i64) -> i64 { self.next_airing.1 * 1000 - now_ms }   // not clamped (behavior 3.1)
}

#[derive(Clone, Debug)]
pub enum Dialog {
    ChangeCategory { anime: AnimeRef, initial: Vec<(CategoryRef, bool)> },
    DeleteEpisodes { episodes: Vec<i64> },
    DuplicateAnime { anime: AnimeRef, duplicate: AnimeRef },
    Migrate { new_anime: AnimeRef, old_anime: AnimeRef },
    SetAnimeFetchInterval { anime: AnimeRef },
    ShowQualities { episode: Episode, anime: AnimeRef, source: SourceRef },
    ChangeAnimeSkipIntro,
    EpisodeSettingsSheet,
    SeasonSettingsSheet,
    TrackSheet,
    FullImages,
}

pub enum Event {                       // what the UI receives; mirrors "snackbar/toast/navigate" effects
    Snack { message: UserMessage, action: Option<SnackAction>, duration: SnackDuration, dismissible: bool },
    Toast(UserMessage),
    Navigate(NavRequest),
    StateChanged,
}

#[derive(Clone, Debug)]
pub enum UserMessage {
    NoEpisodesFound,                   // AYMR no_episodes_error
    NoSeasonsFound,                    // AYMR no_seasons_error
    Generic(String),                   // formatted exception message
    DeleteDownloadedEpisodes,          // AYMR delete_downloads_for_anime
    AddToLibrary,                      // AYMR snack_add_to_anime_library
    TrackersUpdated(i32),              // AYMR trackers_updated_summary_anime (%d)
    ConfirmTrackerUpdate(i32),         // AYMR confirm_tracker_update_anime (%d)
    EpisodeSettingsUpdated,            // AYMR episode_settings_updated
    SeasonSettingsUpdated,             // AYMR season_settings_updated
    TrackError { service: String, detail: String }, // MR track_error
    NoMatchFound,                      // MR error_no_match
}

pub enum SnackAction { Delete, Add, Ok }
pub enum SnackDuration { Short, Default }

impl<S: Services> AnimeEntryModel<S> {
    pub async fn load(&mut self) -> Result<(), EntryError>;             // behavior 2.3
    pub async fn toggle_favorite(&mut self, on_remove: impl FnOnce(), check_duplicate: bool) -> Result<(), EntryError>;
    pub async fn fetch_all(&mut self, manual: bool) -> Result<(), EntryError>;   // behavior 6.1
    pub async fn mark_seen(&mut self, episode_ids: &[i64], seen: bool) -> Result<(), EntryError>;   // behavior 12
    pub async fn mark_previous_seen(&mut self, pointer_id: i64) -> Result<(), EntryError>;
    pub async fn bookmark(&mut self, episode_ids: &[i64], value: bool) -> Result<(), EntryError>;
    pub async fn fillermark(&mut self, episode_ids: &[i64], value: bool) -> Result<(), EntryError>;
    pub async fn set_episode_flag(&mut self, change: EpisodeFlagChange) -> Result<(), EntryError>;  // filters, sort, display, previews, summaries
    pub async fn set_season_flag(&mut self, change: SeasonFlagChange) -> Result<(), EntryError>;
    pub async fn set_fetch_interval(&mut self, value: u32) -> Result<(), EntryError>;   // 0 = default (smart), 1..=28 custom
    pub async fn set_skip_intro_seconds(&mut self, seconds: u8) -> Result<(), EntryError>;
    pub fn dispatch(&mut self, action: EntryAction) -> Vec<Event>;     // dialogs, selection, navigation requests
}

pub enum EpisodeFlagChange {
    Unseen(TriState), Downloaded(TriState), Bookmarked(TriState), Fillermarked(TriState),
    Sort(u64), Display(u64), Previews(u64), Summaries(u64), SetAsDefault { apply_to_existing: bool },
}
pub enum SeasonFlagChange {
    Downloaded(TriState), Unseen(TriState), Started(TriState), Completed(TriState),
    Bookmarked(TriState), Fillermarked(TriState), Sort(u64), GridMode(SeasonDisplayMode),
    GridSize(u8), OverlayDownloaded(bool), OverlayUnseen(bool), OverlayLocal(bool),
    OverlayLang(bool), OverlayContinue(bool), DisplayMode(u64), SetAsDefault { apply_to_existing: bool },
}
```

`Services` is the trait boundary the model depends on. Each method maps to a Kotlin call in `p2-data-and-config.md` section 7 and `p2-behavior.md` section 1.3.

```rust
#[async_trait::async_trait]     // or native async fn in trait (Rust 1.75+); pick native async fn if no dyn use
pub trait Services: Send + Sync + 'static {
    // entries and episodes
    async fn load_anime(&self, id: i64) -> Result<AnimeRef, EntryError>;
    async fn subscribe_entry(&self, id: i64) -> Result<tokio::sync::watch::Receiver<EntrySnapshot>, EntryError>;
    async fn set_favorite(&self, id: i64, value: bool) -> Result<bool, EntryError>;   // AnimeRepository.updateAnime(favorite)
    async fn set_categories(&self, id: i64, category_ids: &[i64]) -> Result<(), EntryError>;
    async fn categories_for(&self, id: i64) -> Result<Vec<CategoryRef>, EntryError>;
    async fn user_categories(&self) -> Result<Vec<CategoryRef>, EntryError>;             // system category excluded
    async fn duplicate_library_anime(&self, anime: &AnimeRef) -> Result<Vec<AnimeRef>, EntryError>;
    async fn update_episodes(&self, updates: Vec<EpisodeUpdate>) -> Result<(), EntryError>;
    async fn set_seen(&self, seen: bool, episodes: &[Episode], remove_downloads_after_read: bool) -> Result<SeenResult, EntryError>;
    async fn write_episode_flags(&self, id: i64, word: u64) -> Result<bool, EntryError>;
    async fn write_season_flags(&self, id: i64, word: u64) -> Result<bool, EntryError>;
    async fn write_viewer_flags(&self, id: i64, word: u64) -> Result<bool, EntryError>;
    async fn write_fetch_interval(&self, id: i64, interval: i32) -> Result<(), EntryError>;

    // source refresh
    async fn refresh_episodes(&self, anime: &AnimeRef, fetch_details: bool, fetch_episodes: bool, manual: bool) -> Result<EpisodeUpdate, EntryError>;
    async fn refresh_seasons(&self, anime: &AnimeRef, fetch_details: bool, fetch_seasons: bool, manual: bool) -> Result<SeasonUpdate, EntryError>;

    // trackers, airing, related
    async fn track_rows(&self, anime_id: i64) -> Result<Vec<TrackRow>, EntryError>;
    async fn refresh_tracks(&self, anime_id: i64) -> Vec<(TrackerRef, EntryError)>;   // supervisor semantics
    async fn track_episode(&self, anime_id: i64, episode_number: f64) -> Result<(), EntryError>;
    async fn airing_for(&self, anime: &AnimeRef, tracks: &[TrackItem], manual: bool) -> Result<(u16, i64), EntryError>;
    async fn sync_related(&self, anime: &AnimeRef, force: bool) -> Result<(), EntryError>;

    // downloads
    fn download_presence(&self) -> &dyn DownloadPresence;
    async fn queue_downloads(&self, anime: &AnimeRef, episodes: &[Episode], video: Option<VideoRef>) -> Result<(), EntryError>;
    async fn start_download_now(&self, episode_id: i64) -> Result<(), EntryError>;
    async fn cancel_downloads(&self, episode_ids: &[i64]) -> Result<(), EntryError>;
    async fn delete_episode_downloads(&self, anime: &AnimeRef, source: &SourceRef, episodes: &[Episode]) -> Result<(), EntryError>;
    async fn delete_anime_downloads(&self, anime: &AnimeRef, source: &SourceRef) -> Result<(), EntryError>;
    fn download_count(&self, anime: &AnimeRef) -> i64;
    async fn filter_new_for_download(&self, anime: &AnimeRef, new_eps: &[Episode]) -> Result<Vec<Episode>, EntryError>;

    // player
    async fn open_episode(&self, anime_id: i64, episode_id: i64, external: bool) -> Result<(), EntryError>;
    async fn ensure_torrent_server(&self, source: &SourceRef) -> Result<(), EntryError>;

    // defaults
    async fn library_defaults(&self) -> Result<EntrySettings, EntryError>;
    async fn save_library_defaults(&self, s: &EntrySettings) -> Result<(), EntryError>;
    async fn apply_defaults_to_favorites(&self, kind: DefaultsKind) -> Result<(), EntryError>;
}
```

### 2.5 Tracking (kuro-core/src/tracking)

```rust
pub struct TrackRow {                  // domain AnimeTrack (AnimeTrack.kt:5-20)
    pub id: i64, pub anime_id: i64, pub tracker_id: i64, pub remote_id: i64,
    pub library_id: Option<i64>, pub title: String,
    pub last_episode_seen: f64, pub total_episodes: i64, pub status: i64, pub score: f64,
    pub remote_url: String, pub start_date: i64, pub finish_date: i64, pub private: bool,
}

pub struct TrackItem { pub track: Option<TrackRow>, pub tracker: TrackerRef }

/// Remote operations with the exact semantics of AnimeTracker.kt:67-108.
#[allow(async_fn_in_trait)]
pub trait AnimeTrackerService: Send + Sync {
    fn id(&self) -> i64;
    fn name(&self) -> &str;
    fn supports_private(&self) -> bool;
    fn completion_status(&self) -> i64;
    fn watching_status(&self) -> i64;
    fn rewatching_status(&self) -> i64;
    fn status_list(&self) -> Vec<i64>;
    fn status_label(&self, status: i64) -> Option<String>;
    fn score_list(&self) -> Vec<String>;
    fn index_to_score(&self, index: i32) -> f64 { index as f64 }
    fn display_score(&self, track: &TrackRow) -> String;
    async fn update(&self, track: &mut TrackRow, did_watch_episode: bool) -> Result<(), TrackError>;
    async fn bind(&self, track: &mut TrackRow, has_seen_episodes: bool) -> Result<(), TrackError>;
    async fn search(&self, query: &str) -> Result<Vec<TrackSearch>, TrackError>;
    async fn refresh(&self, track: &TrackRow) -> Result<TrackRow, TrackError>;
    async fn register(&self, item: TrackSearch, anime_id: i64) -> Result<(), TrackError>;

    // default implementations that mirror the Kotlin methods
    async fn set_status(&self, track: &mut TrackRow, status: i64) -> Result<(), TrackError> {
        track.status = status;
        if track.status == self.completion_status() && track.total_episodes != 0 {
            track.last_episode_seen = track.total_episodes as f64;
        }
        self.update_remote(track).await
    }
    async fn set_last_episode_seen(&self, track: &mut TrackRow, episode_number: i32) -> Result<(), TrackError> {
        if track.last_episode_seen == 0.0 && track.last_episode_seen < episode_number as f64
            && track.status != self.rewatching_status() {
            track.status = self.watching_status();
        }
        track.last_episode_seen = episode_number as f64;
        if track.total_episodes != 0 && track.last_episode_seen as i64 == track.total_episodes {
            track.status = self.completion_status();
            track.finish_date = now_millis();          // System.currentTimeMillis()
        }
        self.update_remote(track).await
    }
    async fn set_score(&self, track: &mut TrackRow, score_string: &str) -> Result<(), TrackError> {
        let idx = self.score_list().iter().position(|s| s == score_string).map(|i| i as i32).unwrap_or(-1);
        if idx < 0 { return Err(TrackError::ScoreNotInList(score_string.into())); }   // defect D-4: reject, do not store -1
        track.score = self.index_to_score(idx);
        self.update_remote(track).await
    }
    async fn set_start_date(&self, track: &mut TrackRow, millis: i64) -> Result<(), TrackError> {
        track.start_date = millis; self.update_remote(track).await
    }
    async fn set_finish_date(&self, track: &mut TrackRow, millis: i64) -> Result<(), TrackError> {
        track.finish_date = millis; self.update_remote(track).await
    }
    async fn set_private(&self, track: &mut TrackRow, private: bool) -> Result<(), TrackError> {
        track.private = private; self.update_remote(track).await
    }
    async fn update_remote(&self, track: &mut TrackRow) -> Result<(), TrackError>;   // update() then local insert; errors -> UI toast
}

pub trait EnhancedTracker: AnimeTrackerService {
    fn accepts_source(&self, source: &SourceRef) -> bool;   // source class name in accepted list
    async fn match_anime(&self, anime: &AnimeRef) -> Result<Option<TrackSearch>, TrackError>;
}

pub trait DeletableTracker: AnimeTrackerService {
    async fn delete_remote(&self, track: &TrackRow) -> Result<(), TrackError>;
}
```

Date rules (kuro-core/src/tracking/dates.rs), using `chrono::NaiveDate` and the system zone from `chrono::Local`:

```rust
pub struct DateBounds { pub start_date: i64, pub finish_date: i64 }   // 0 = unset

/// AnimeTrackInfoDialog.kt:465-520, with the zone defect (D-5) fixed: `picked` is a calendar date, not millis.
pub fn is_selectable(picked: NaiveDate, today: NaiveDate, bounds: DateBounds, start: bool, zone: &impl chrono::TimeZone) -> bool {
    if picked > today { return false; }
    let local = |ms: i64| chrono::DateTime::from_timestamp_millis(ms).unwrap().with_timezone(zone).date_naive();
    if start && bounds.finish_date > 0 { return picked <= local(bounds.finish_date); }
    if !start && bounds.start_date > 0 { return picked >= local(bounds.start_date); }
    true
}
pub fn is_selectable_year(year: i32, today: NaiveDate, bounds: DateBounds, start: bool, zone: &impl chrono::TimeZone) -> bool {
    if year > today.year() { return false; }
    let local_year = |ms: i64| chrono::DateTime::from_timestamp_millis(ms).unwrap().with_timezone(zone).year();
    if start && bounds.finish_date > 0 { return year <= local_year(bounds.finish_date); }
    if !start && bounds.start_date > 0 { return year >= local_year(bounds.start_date); }
    true
}
/// Stored value for a picked date: local midnight in the system zone (see topic track 6.3).
pub fn picked_to_stored_millis(picked: NaiveDate, zone: &impl chrono::TimeZone) -> i64;
/// Picker initial value: stored millis -> calendar date in the system zone (wall-clock rule of convertEpochMillisZone).
pub fn stored_to_picked(stored_millis: i64, zone: &impl chrono::TimeZone) -> NaiveDate;
```

Note on `stored_to_picked`: Kotlin keeps the wall-clock time and re-interprets it in UTC for the picker. Rust stores and reads calendar dates directly, so the picker never shifts a day. The stored millis keep the same value Kotlin would write for a local-midnight pick, so existing Aniyomi backups round-trip.

Tracking sheet pages:

```rust
pub enum TrackPage {
    Home,
    Status { service_id: i64, track: TrackRow, selection: i64 },
    Episode { service_id: i64, track: TrackRow, selection: i32 },   // initial = last_episode_seen as i32 (truncate)
    Score { service_id: i64, track: TrackRow, selection: String },  // initial = display_score
    Date { service_id: i64, track: TrackRow, start: bool },
    DateRemove { service_id: i64, track: TrackRow, start: bool },
    Search { anime_id: i64, service_id: i64, query: String, current_url: Option<String>,
             result: SearchState },
    Remove { anime_id: i64, service_id: i64, track: TrackRow, remove_remote: bool },
}
pub enum SearchState { Idle, Loading, Done(Result<Vec<TrackSearch>, String>), }
pub struct TrackSheet { pub stack: Vec<TrackPage> }   // root is always Home; pop_until(Home) for DateRemove confirm
```

Episode range rule: `fn episode_range(total: i64) -> std::ops::RangeInclusive<i32> { 0..=(if total > 0 { total as i32 } else { 10000 }) }`.

### 2.6 Related and airing (kuro-core)

```rust
pub struct RelationGroup { pub name: String, pub animes: Vec<AnimeRef> }
pub const RELATED_TTL: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 3600);
pub const RELATED_MAX_PER_GROUP: usize = 10;

pub async fn sync_related<S: RelatedSource>(anime: &AnimeRef, force: bool, src: &S, now_ms: i64) -> Result<SyncOutcome, EntryError>;
pub enum SyncOutcome { Skipped, Unsupported, SourceMissing, Replaced { groups: usize } }

pub struct AiringClient { http: reqwest::Client }          // one shared client (defect A-4 fixed)
impl AiringClient {
    pub async fn load_airing(&self, anime: &AnimeRef, tracks: &[TrackItem], manual: bool) -> (u16, i64);
    pub async fn anilist_next(&self, anilist_id: i64) -> (u16, i64);   // (1, 0) when none
    pub async fn anilist_id_from_mal(&self, mal_id: i64) -> i64;       // 0 when none
    pub async fn simkl_next(&self, simkl_id: i64, now_ms: i64) -> (u16, i64);
}
```

### 2.7 Dialog and Snackbar types in the UI (kuro-ui)

The UI maps `Dialog` to a window. There is one `DialogHost` widget per page. `dispatch` returns `Vec<Event>`; the UI applies them in order.

---

## 3. Async model

### 3.1 Executors and threads

| Thread | Owns | Runs |
|---|---|---|
| GTK main thread (`glib::MainContext::default()`) | all widgets, `EntryState` (the model struct lives here), `gio::ListStore` objects | model `dispatch`, state application, dialog creation, snackbar overlay |
| tokio runtime `rt-multi-thread`, 2 worker threads, created in `main` | all network (`reqwest`), `tokio::time`, long refresh pipelines, tracker calls | `Services` implementations that do I/O |
| DB actor thread (dedicated `std::thread`, `rusqlite::Connection` owned, WAL on) | every SQL statement | requests arrive on a `std::sync::mpsc::Sender<DbRequest>` with a reply `tokio::sync::oneshot`; change events are published on a `tokio::sync::broadcast` |

Rules:
1. Widgets and `EntryState` are touched only on the GTK thread. Background work returns results through `async_channel::Sender<Msg>` consumed by `glib::spawn_future_local` on the main context (the `async-channel` crate is runtime-agnostic and avoids the deprecated `glib::MainContext::channel`).
2. The model's `async fn` methods run on the GTK thread but `await` only `Services` futures. Those futures are `Send` and execute on the tokio runtime; `spawn_future_local` polls the handle via `tokio::runtime::Handle::spawn` + `async_channel`. No `RefCell` borrow is held across an `.await` (lint: clippy `await_holding_refcell_ref`).
3. The DB actor never calls back into GTK.

### 3.2 Replacement for Kotlin dispatchers

| Kotlin | Rust |
|---|---|
| `screenModelScope.launch` (main) | `glib::spawn_future_local(async move { ... })` on the GTK thread |
| `launchIO` | `tokio::spawn` on the runtime; result returns via `async_channel` |
| `launchNonCancellable` | `tokio::spawn` detached (no abort handle kept). The task survives page destruction. Results that target a closed page are dropped by checking a `Weak` page handle. |
| `withUIContext` | send to `async_channel` and let the main-context future apply it |
| `withIOContext` | `tokio::task::spawn_blocking` for CPU-only work (sorting 10k items is fast; not needed) |
| `async(Dispatchers.IO)` + `awaitAll` (UnmeteredSource) | `futures::future::join_all` over tokio tasks, bounded by a `Semaphore` of 8 |
| `supervisorScope` + `async` (RefreshAnimeTracks) | `futures::future::join_all` with per-future error values (no `?` short-circuit) |
| `flowWithLifecycle(lifecycle)` | subscription `broadcast::Receiver` is dropped on `page.unmap` and recreated on `map` |
| `collectLatest` | a single consumer loop per subscription that cancels the previous derived computation (`tokio_util::sync::CancellationToken`) |
| `ensureActive()` in season loop | check `CancellationToken::is_cancelled()` between items |
| `CancellationException` ignored in refresh | a `EntryError::Cancelled` variant that the refresh path returns without a message |

### 3.3 State publication

- `EntryState` is a plain struct. After each `dispatch` or completed `Services` call the model returns `Event::StateChanged`. The GTK layer calls `apply(&state)`, which diffs the derived lists against the previous `gio::ListStore` contents and uses `splice` (one call per list, so no flicker).
- DB change events: a `broadcast` event for `EntryChanged(anime_id)`, `EpisodesChanged(anime_id)`, `TracksChanged(anime_id)`, `DownloadsChanged`. The model reloads the affected part and publishes `StateChanged`. This matches the Kotlin combine of subscribe + download cache + queue.
- Download status and progress arrive from the downloader on `broadcast<DownloadEvent>`; they update only the `EpisodeItem` at the matching id (behavior 13.4), keeping the same index.

### 3.4 Timers

- Torrent server wait: `tokio::time::interval(Duration::from_secs(1))`, 11 ticks max (behavior 22).
- Snackbar auto-dismiss: `glib::timeout_add_local_once(duration)`.
- Related TTL and refresh guard: pure time comparisons using `std::time::SystemTime::now()` (not monotonic, because the TTL is persisted).

---

## 4. Crates (with reasons)

| Crate | Version pin (at writing) | Used for | Why this and not another |
|---|---|---|---|
| `gtk4` | 0.11.5 with `v4_16` (already in Cargo.toml) | UI | Required target |
| `glib` | matching gtk4 0.11 | main context, `spawn_future_local`, `timeout_add_local_once` | Bundled with gtk4 |
| `gio` | matching | `ListStore`, `ListModel`, `Settings` if needed | Bundled |
| `async-channel` | 2.x | results from tokio to GTK main | Runtime-agnostic, no `glib::MainContext::channel` (deprecated path) |
| `tokio` | 1.x, features `rt-multi-thread`, `sync`, `time`, `macros` | async runtime for I/O | Standard |
| `tokio-util` | 0.7 | `CancellationToken` | Standard |
| `futures` | 0.3 | `join_all`, `StreamExt` | Standard |
| `rusqlite` | 0.32+ with `bundled` | SQLite | Matches the sqldelight schema shape; bundled avoids system sqlite version drift |
| `reqwest` | 0.12 with `rustls-tls`, `json` (default-features = false) | AniList GraphQL, Simkl calendar, tracker HTTP | Standard, no OpenSSL dependency |
| `serde`, `serde_json` | 1.x | settings, GraphQL, Simkl | Already in Cargo.toml |
| `chrono` | 0.4 | dates, zone conversions, `NaiveDate` | Standard |
| `chrono-tz` | 0.10 (optional, only if an explicit zone is needed) | not required (system zone via `chrono::Local`) | Keep off by default |
| `icu_collator` | 1.x | PRIMARY collation (behavior 2.2 of episode-list topic) | Unicode-correct, replaces `java.text.Collator` |
| `thiserror` | 2.x | `EntryError`, `TrackError`, `FlagError` | Standard |
| `tracing` | 0.1 | logs replacing `logcat` | Standard |
| `async-trait` | not used if `async fn` in traits is enough (`Services` is only used generically) | — | Avoid the extra macro |

Not used: `libadwaita` (forbidden by the brief), `adw::Toast` (libadwaita), `notify-rust` (no notifications in this screen), `ashpd` (no portal call needed; `gtk::UriLauncher` covers "open in browser").

---

## 5. Android to Linux mapping (this scope)

| Android / Kotlin | Linux / Rust replacement | Notes and user impact |
|---|---|---|
| Voyager `Screen`, `Navigator`, `push`, `pop`, `replace`, `popUntil` | GTK page stack in `kuro-ui::ui` (`push`, `pop`, `pop_to_root` exist) plus a `Vec<TrackPage>` for the sheet | Same user flow; back navigation via header back button and Escape |
| Voyager `StateScreenModel`, `screenModelScope` | `AnimeEntryModel` + the executors in section 3 | No behavior change |
| Compose `LazyColumn` | `gtk::ListView` with `gio::ListStore<RowObject>` | Virtualized; row widgets re-used |
| Compose `LazyVerticalGrid` (season grid) | `gtk::GridView` (GTK 4.16 has it) with `GridView::set_min_columns` set from grid size | Items per row slider maps to `set_min_columns` |
| `SnackbarHostState`, `showSnackbar` | `snackbar.rs`: a `gtk::Revealer` at the bottom of a `gtk::Overlay`, label + optional `gtk::Button`, dismiss button when `dismissible`, auto-hide with `glib::timeout_add_local_once` (Short = 4000 ms, Default = 4000 ms, matching Compose's Short; no Long used here) | `adw::Toast` is libadwaita and is not allowed; the custom revealer is ~120 lines. Only one snack at a time; a new message replaces the old one (Compose queues them, see 23-edge E-UI-3) |
| `context.toast(message)` (Android Toast) | same revealer, no action, 2000 ms | Android Toast is non-blocking; same behavior |
| `NavigatorAdaptiveSheet` | `gtk::Window` modal and transient for the main window (`ui::modal` already exists) | Swipe-to-dismiss is dropped (task 068); Escape and the close button dismiss |
| `AlertDialogContent` (delete, remove date, remove tracking) | `gtk::AlertDialog` (GTK 4.10+) for plain confirmations; a custom `gtk::Window` when a checkbox is needed (tracker remove) | `gtk::AlertDialog` has no checkbox, so the remote-delete checkbox uses the custom window |
| `ChangeCategoryDialog` (checkbox list) | custom `gtk::Window` with `gtk::CheckButton` rows | |
| `SetIntervalDialog` (wheel picker) | `gtk::DropDown` with 29 items ("Default", "1".."28") | The release gating (behavior 23.8) is not carried over: the dropdown is always enabled |
| `SkipIntroLengthDialog` (wheel 0..255) | `gtk::SpinButton` 0..255 with step 1 and unit "s" | |
| Material3 `DatePicker` (`TrackDateSelector`) | `gtk::Calendar` in a popover with a "Remove" button when allowed | Year navigation is a `gtk::SpinButton` above the calendar, because `gtk::Calendar` has month navigation only |
| Haptic feedback (`HapticFeedbackType.LongPress`) | none | Dropped (task 067). Desktop users get the existing row selection feedback |
| `Intent.createChooser` + `toShareIntent` (share URL) | copy to clipboard (`gdk::Display::default().clipboard().set_text(url)`) and a toast "URL copied" | Dropped: no system share sheet on Linux. Users lose the app picker; they keep copying |
| `openInBrowser(url)` | `gtk::UriLauncher::new(url).launch(Some(&window), gio::Cancellable::NONE, |r| ...)` | Goes through `xdg-desktop-portal` when sandboxed |
| `copyToClipboard(url, label)` | `gdk::Display` clipboard (above) | Same |
| `WebViewScreen` | owned by area 36 (`webkitgtk-6.0`, mockup comment in `src/entry.rs:637`) | Named here only |
| `MainActivity.startPlayerActivity(context, animeId, episodeId, external)` | `kuro-core::player::open(anime_id, episode_id, external)` implemented by the player area (libmpv for internal, `std::process::Command` for external) | Same two modes |
| `TorrentServerService` (Android service) | a child process started with `std::process::Command` and stopped on app exit; the echo is an HTTP `GET` on `localhost` | Requires the user to install the torrent server binary; if absent, the feature reports "not available" (Drop of the Android service model, task 052) |
| `AnimeImageScreenModel` / `AnimeImagesDialog` (cover editor) | part 1 owns the screen; this scope only routes `Dialog::FullImages` | |
| `ifAnimeSourcesLoaded()` + `LoadingScreen` | `gtk::Spinner` centered until `EntryState::Success` | |
| `AssistContentScreen.onProvideAssistUrl` | none | Dropped (task 066). Android assistant feature |
| `formattedMessage` (exception text) | `thiserror` `Display` of `EntryError` | |
| `logcat` | `tracing::error!` | |
| `Injekt.get<T>()` constructor defaults | constructor injection through `Services` | Test doubles are trivial |
| `DataStore`/`PreferenceStore` | `kuro-core::settings` over the `settings` table | Keys in `p2-data-and-config.md` section 4.5 |
| Android `Calendar` / `Instant` | `chrono` | |
| `java.text.Collator` | `icu_collator` PRIMARY | |
| `OkHttpClient` | `reqwest::Client` shared | |
| `kotlinx.serialization` (`JsonObject`) | `serde_json::Value` | memo fields are not touched here |

---

## 6. Mockup integration (what changes in `/home/blackbox/kurobayomi/src/`)

Current behavior of the mockup for this screen:
- `entry::entry(title, anime = true)` renders a fixed 24-episode list ("Episode 24" to "Episode 1"), a fixed info box, and a fixed action row. Nothing is data-driven.
- `screens.rs:39` routes `anime_entry_details` to `entry::entry("Title 1", true)`.
- `ui::show_dialog` and `ui::show_sheet` render the spec JSON for every dialog and sheet, including the tracking sheet, which is also spec-driven (`spec/entries.json` `tracking_sheet`, `tracker_search`).
- Buttons in the dialogs are not wired to any state (they only close the window).

Migration plan:
1. Add `kuro-core` and the module tree of section 1. Add the tests of `p2-edge-cases-and-tests.md` before wiring.
2. Replace `entry::entry(title, true)` with `anime_details::build(anime_id, Services)`. Keep `entry::entry(title, false)` for manga (area 38 owns it).
3. Keep `ui::show_dialog`/`show_sheet` for settings screens (they are still spec-driven). For the anime dialogs in section 2.4 `Dialog`, use `dialogs.rs` typed windows instead of spec JSON. The spec entries for `anime_entry_details.dialogs` and `sheets` are then unused; keep them until area 38 finishes its migration, then delete (task 065).
4. `tracking_sheet` and `tracker_search` in spec (anime and manga share them): the anime sheet moves to `track_sheet.rs`; the manga sheet keeps the spec until area 38 migrates.
5. `spec_is_consistent` test in `src/spec.rs` keeps checking the remaining screens; add a check that `anime_entry_details` has no `Link` items pointing at missing screens after the move.

---

## 7. Pitfalls and invariants

1. RefCell re-entrancy. A GTK signal (for example a `gtk::Switch::connect_state_set`) can fire while the model is borrowed to apply state. Rule: model mutation returns `Vec<Event>`, and widgets are updated after the borrow is released. Signal handlers set a `Cell<bool>` suppressor when they apply a programmatic change.
2. Never `await` while a `RefCell` borrow is alive (clippy `await_holding_refcell_ref` must be denied in `kuro-core`).
3. `gio::ListStore::splice` must be used for bulk updates; repeated `append` causes O(n²) redraws on 1000-episode anime.
4. Weak references: every callback that captures the page uses `glib::clone!(@weak page => ...)` so that a closed page does not receive updates.
5. The season and episode lists in `State` are the full lists. The views are computed on read. Never write a view back into the state (the source of defect 23.2).
6. Time: `now_ms` is always passed in from the caller (tests pass fixed values). Do not call `SystemTime::now()` inside pure functions.
7. Date rule: picker output is a `NaiveDate`. Never convert a picker value through UTC millis (defect D-5).
8. Preference key collisions: all Kurobayomi keys are namespaced (`entry.*`, `track.*`, `player.*`, `downloads.*`, `library.*`), section 4.5.
9. Every `Services` call that writes must be followed by the DB change event; the model must not optimistically update `episodes` for flags that the DB owns (single source of truth).
10. `TrackRow` writes to the remote must re-read the local row first (defect D-1 fix). The `set_*` default methods operate on the value passed in, so the caller (`TrackSheet`) must fetch the fresh row before calling them.
11. The torrent-server precheck must not panic when the source's extension is not installed (defect 23.4). `ensure_torrent_server` returns `Ok(())` when the source is unknown.
12. Collation is per-process: `icu_collator` is created once with the user's locale (`sys-locale` crate or `LANG`), falling back to `en`.
