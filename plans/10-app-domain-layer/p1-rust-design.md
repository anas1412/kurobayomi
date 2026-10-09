# Area 10 - Rust design (part 1 of 1)

Target: stable Rust 2024 edition, tokio, rusqlite (area:05), gtk4-rs 0.11 only in the GUI crate. This area is GTK-free and OS-free; it lives in the crate `kuro-domain` (name reserved by area:01 section 1: "New app crates for other areas (`kuro-data`, `kuro-domain`, ...) depend on `kuro-core`; none depends on the GUI crate"). Repo-relative Kotlin paths are relative to `/home/blackbox/aniyomi-src`.

## 1. Crate and module layout

```
crates/kuro-domain/
  Cargo.toml                    deps: kuro-core, kuro-platform (traits only), tokio, futures, serde, serde_json, time, thiserror, tracing, tempfile, regex, fancy-regex, async-trait
  src/lib.rs                    pub mod app; (areas 07/08/09 add their modules beside it)
  src/app/mod.rs
  src/app/domain.rs             Domain, DomainDeps, Domain::new, Domain::start            (DomainModule.kt, SYDomainModule.kt)
  src/app/error.rs              DomainError, NoChaptersError / NoEpisodesError / NoSeasonsError
  src/app/rx.rs                 combine_latest!, watch_pref, Latest<T>                      (kotlinx combine/changes())
  src/app/flags.rs              set_flag, ViewerFlags helpers, AnimeViewerFlags            (Set*ViewerFlags.kt, Anime.kt masks)
  src/app/entries/mod.rs
  src/app/entries/manga.rs      UpdateManga, SetMangaViewerFlags, GetExcludedScanlators, SetExcludedScanlators
  src/app/entries/manga_model.rs  toSManga / copyFrom / toDomainManga, downloaded_filter, chapters_filtered
  src/app/entries/comic_info.rs get_comic_info                                          (Manga.kt getComicInfo)
  src/app/entries/anime.rs      UpdateAnime, SetAnimeViewerFlags
  src/app/entries/anime_model.rs  toSAnime / copyFrom / toDomainAnime, filter helpers
  src/app/entries/anime_seasons.rs  SyncSeasonsWithSource
  src/app/entries/anime_related.rs  SyncRelatedAnimeWithSource
  src/app/items/mod.rs
  src/app/items/chapter.rs      SetReadStatus, GetAvailableScanlators
  src/app/items/chapter_sync.rs plan_chapter_sync (pure) + SyncChaptersWithSource (applier)
  src/app/items/chapter_model.rs  to_s_chapter, copy_from_s_chapter
  src/app/items/chapter_filter.rs apply_filters
  src/app/items/episode.rs, episode_sync.rs, episode_model.rs, episode_filter.rs   (episode twins)
  src/app/download/{manga,anime}.rs  DeleteChapterDownload, DeleteEpisodeDownload
  src/app/source/mod.rs         ToggleLanguage, SetMigrateSorting, ordering helpers (cmp_ignore_case_java, lang_cmp)
  src/app/source/prefs.rs       SourcePreferences section for area:02
  src/app/source/manga.rs, anime.rs  enabled/languages/favorite-count/toggles/incognito
  src/app/source/icon.rs        SourceIcons trait
  src/app/extension/mod.rs
  src/app/extension/manga.rs, anime.rs  ExtensionsByType, ExtensionLanguages, ExtensionSources
  src/app/extension/trust.rs    TrustExtension (one implementation for both kinds)
  src/app/track/mod.rs
  src/app/track/model.rs        Track (merged), copy_personal_from
  src/app/track/prefs.rs        TrackPreferences, AutoTrackState
  src/app/track/manga.rs, anime.rs  TrackChapter/TrackEpisode, AddTracks, RefreshTracks, SyncProgress
  src/app/track/delayed.rs      DelayedTrackingStore, DelayedTrackingWorker
```
Manga and anime variants are two near-identical Kotlin trees (about 40 percent duplicated code). Proposal: implement each interactor ONCE, generic over an `EntryKind` marker (`Manga`, `Anime`) with associated types for entity, item, update and repository, and expose the two public names as type aliases (`pub type SetReadStatus = SetItemStatus<Manga>`). The differences listed in p1-behavior.md (scanlator exclusion, fillermark, summary/preview, queue-on-failure rule, empty-sources fallback) are expressed as `Kind::` associated consts/functions, so no behaviour is lost. If the generic form fights the borrow checker during implementation, fall back to two thin modules sharing the pure planner functions; the acceptance tests in p1-tasks.md hold either way.

## 2. The `Domain` container (replaces Injekt + DomainModule)

```rust
pub struct DomainDeps {
    pub prefs: Arc<Prefs>,                         // area:02 (SourcePrefs, LibraryPrefs, DownloadPrefs, TrackPrefs, BasePrefs)
    pub manga_db: Arc<Db>, pub anime_db: Arc<Db>, // area:05
    pub manga_repo: Arc<dyn MangaRepo>,    pub anime_repo: Arc<dyn AnimeRepo>,
    pub chapter_repo: Arc<dyn ChapterRepo>, pub episode_repo: Arc<dyn EpisodeRepo>,
    pub history: Arc<dyn HistoryRepo>, pub tracks: Arc<dyn TrackRepo>,
    pub sources: Arc<dyn SourceCatalog>,           // area:17: get(id), get_or_stub(id), all(), online()
    pub extensions: Arc<dyn ExtensionCatalog>,     // area:20
    pub trackers: Arc<dyn TrackerRegistry>,        // area:27
    pub downloads: Arc<dyn DownloadFacade>,        // area:23/24/25
    pub covers: Arc<dyn CoverCache>,               // area:34
    pub connectivity: Arc<dyn Connectivity>,       // kuro-platform
    pub data_dir: PathBuf,
}
pub struct Domain { /* singletons: the Arc fields above + DelayedTrackingStore + worker handle */ }
impl Domain {
    pub fn new(deps: DomainDeps) -> Domain;
    pub async fn start(&self);                    // replays the delayed tracking queue if non-empty
    // factories become cheap constructors returning Clone structs holding Arc handles:
    pub fn set_read_status(&self) -> SetReadStatus;            pub fn set_seen_status(&self) -> SetSeenStatus;
    pub fn sync_chapters(&self) -> SyncChaptersWithSource;     pub fn sync_episodes(&self) -> SyncEpisodesWithSource;
    pub fn sync_seasons(&self) -> SyncSeasonsWithSource;       pub fn sync_related_anime(&self) -> SyncRelatedAnimeWithSource;
    pub fn update_manga(&self) -> UpdateManga;                 pub fn update_anime(&self) -> UpdateAnime;
    pub fn track_chapter(&self) -> TrackChapter;               pub fn track_episode(&self) -> TrackEpisode;
    pub fn add_manga_tracks(&self) -> AddMangaTracks;          pub fn add_anime_tracks(&self) -> AddAnimeTracks;
    pub fn enabled_sources(&self, k: EntryKind) -> EnabledSources;   // one accessor per registry row
}
```
Every accessor corresponds to a row of p1-topic-domainmodule-registry.md; T-10a-037 adds a test that fails if a registry row has neither an accessor nor a documented drop. `UpdateManga` and `UpdateAnime` hold an `Arc<FetchInterval>` from area:07.

Trait notes (all `#[async_trait]` because they are used as `dyn`):
```rust
pub trait SourceCatalog: Send + Sync {
    fn get(&self, id: i64) -> Option<Arc<dyn Source>>;                  // sourceManager.get
    fn get_or_stub(&self, id: i64) -> Arc<dyn Source>;
    fn watch_all(&self) -> watch::Receiver<Arc<Vec<SourceInfo>>>;       // repository.getMangaSources()
    fn watch_online(&self) -> watch::Receiver<Arc<Vec<SourceInfo>>>;    // getOnlineMangaSources()
    fn watch_with_favorite_count(&self) -> watch::Receiver<Arc<Vec<(SourceInfo, i64)>>>;
}
pub trait ExtensionCatalog: Send + Sync {
    fn watch_installed(&self) -> watch::Receiver<Arc<Vec<InstalledExt>>>;
    fn watch_untrusted(&self) -> watch::Receiver<Arc<Vec<UntrustedExt>>>;
    fn watch_available(&self) -> watch::Receiver<Arc<Vec<AvailableExt>>>;
    fn extension_package(&self, source_id: i64) -> Option<String>;      // getExtensionPackage
    fn watch_extension_package(&self, source_id: i64) -> watch::Receiver<Option<String>>;
}
pub trait DownloadFacade: Send + Sync {
    fn is_chapter_downloaded(&self, name: &str, scanlator: Option<&str>, manga_title: &str, source_id: i64) -> bool;
    fn is_episode_downloaded(&self, name: &str, scanlator: Option<&str>, anime_title: &str, source_id: i64) -> bool;
    fn is_chapter_dir_name_changed(&self, old: &Chapter, new: &Chapter) -> bool;
    fn is_episode_dir_name_changed(&self, old: &Episode, new: &Episode) -> bool;
    async fn rename_chapter(&self, src: &dyn Source, m: &Manga, old: &Chapter, new: &Chapter);
    async fn rename_episode(&self, src: &dyn Source, a: &Anime, old: &Episode, new: &Episode);
    async fn delete_chapters(&self, c: &[Chapter], m: &Manga, src: &dyn Source);
    async fn delete_episodes(&self, e: &[Episode], a: &Anime, src: &dyn Source);
}
pub trait CoverCache: Send + Sync { fn has_custom_cover(&self, id: i64) -> bool; fn delete_from_cache(&self, m: &Manga, delete_custom: bool); fn has_custom_background(&self, id: i64) -> bool; fn delete_background_from_cache(&self, a: &Anime, delete_custom: bool); }
pub trait Connectivity: Send + Sync { fn is_online(&self) -> bool; async fn wait_online(&self); }
```

## 3. Key types and signatures

### 3.1 Flags (`flags.rs`)
```rust
#[inline] pub fn set_flag(value: i64, flag: i64, mask: i64) -> i64 { (value & !mask) | (flag & mask) }
pub mod manga_viewer { pub const READING_MODE_MASK: i64 = 0x07; pub const ORIENTATION_MASK: i64 = 0x38; }
pub mod anime_viewer {
    pub const INTRO_MASK: i64 = 0xFF;
    pub const AIRING_EPISODE_MASK: i64 = 0xFFFF00;
    pub const AIRING_TIME_MASK: i64 = 0x0FF_FFFF_FF00_0000;   // == 0xFFFF_FFFF << 24 == 0x0FFFFFFFF000000
    pub const INTRO_DISABLE_MASK: i64 = 0x100_0000_0000_0000;
}
pub fn skip_intro_length(v: i64) -> i32 { (v & anime_viewer::INTRO_MASK) as i32 }
pub fn skip_intro_disabled(v: i64) -> bool { v & anime_viewer::INTRO_DISABLE_MASK == anime_viewer::INTRO_DISABLE_MASK }
pub fn next_episode_to_air(v: i64) -> i32 { ((v & AIRING_EPISODE_MASK) >> 8) as i32 }
pub fn next_episode_airing_at(v: i64) -> i64 { (v & AIRING_TIME_MASK) >> 24 }
pub fn with_skip_intro(v: i64, length: i64) -> i64 { set_flag(set_flag(v, length, INTRO_MASK), if length == 0 { INTRO_DISABLE_MASK } else { 0 }, INTRO_DISABLE_MASK) }
pub fn with_airing(v: i64, ep: i32, at: i64) -> i64 { set_flag(set_flag(v, (ep as i64) << 8, AIRING_EPISODE_MASK), at << 24, AIRING_TIME_MASK) }
```
Shifts equal the Kotlin `* 16^n` (`16^2 = 1<<8`, `16^6 = 1<<24`, `16^14 = 1<<56`). The literal `0x0FF_FFFF_FF00_0000` is `0x0FFFFFFFF000000` (15 hex digits) grouped by four; the test asserts `AIRING_TIME_MASK == 0xFFFF_FFFF << 24` and `INTRO_DISABLE_MASK == 1 << 56`.

### 3.2 Reactive helper (`rx.rs`)
```rust
/// kotlinx `combine(a, b, c) { .. }`: waits for the first value of every input, then emits on every change.
pub fn combine3<A, B, C, R>(a: impl Stream<Item = A>, b: impl Stream<Item = B>, c: impl Stream<Item = C>,
                            f: impl Fn(&A, &B, &C) -> R + Send + 'static) -> impl Stream<Item = R>;
pub fn pref_stream<T: PrefType>(p: &Pref<T>) -> impl Stream<Item = T>;       // current value first, then every change (area:02 Changes<T>)
pub fn watch_stream<T: Clone + Send + Sync + 'static>(rx: watch::Receiver<T>) -> impl Stream<Item = T>;  // current first
pub trait StreamExt2: Stream { fn distinct_until_changed(self) -> impl Stream<Item = Self::Item> where Self::Item: PartialEq + Clone; }
```
Implemented on `tokio_stream::wrappers::WatchStream` + `futures::stream::select_all`-style latest-value tuples; arities 2 to 5 are macro-generated. The interactors that return `Flow` in Kotlin expose `impl Stream<Item = Arc<T>>` and ALSO a pure function computing one output from plain inputs (`fn compute_enabled_sources(&Inputs, &[Source]) -> Vec<Source>`), which is what unit tests exercise.

### 3.3 Entry/item operations
```rust
pub struct UpdateManga { repo: Arc<dyn MangaRepo>, fetch: Arc<MangaFetchInterval>, covers: Arc<dyn CoverCache> }
impl UpdateManga {
    pub async fn update(&self, u: MangaUpdate) -> Result<bool>;
    pub async fn update_all(&self, u: Vec<MangaUpdate>) -> Result<bool>;
    pub async fn update_from_source(&self, local: &Manga, remote: &SManga, manual: bool) -> Result<bool>;
    pub async fn update_fetch_interval(&self, m: &Manga, now: OffsetDateTime, window: Option<(i64, i64)>) -> Result<bool>;
    pub async fn update_last_update(&self, id: i64) -> Result<bool>;
    pub async fn update_cover_last_modified(&self, id: i64) -> Result<bool>;
    pub async fn update_favorite(&self, id: i64, favorite: bool) -> Result<bool>;   // date_added = now_ms or 0
}
pub struct SyncChaptersWithSource { /* handles */ }
impl SyncChaptersWithSource {
    pub async fn run(&self, raw: Vec<SChapter>, manga: &Manga, source: &dyn MangaSource, manual: bool, window: (i64, i64)) -> Result<Vec<Chapter>, DomainError>;
}
pub enum SetStatusResult { Success, NoItems, InternalError(Arc<anyhow::Error>) }
pub struct SetReadStatus { /* download prefs, DeleteChapterDownload, repos */ }
impl SetReadStatus {
    pub async fn run(&self, read: bool, chapters: Vec<Chapter>) -> SetStatusResult;       // non-cancellable
    pub async fn run_manga(&self, manga_id: i64, read: bool) -> SetStatusResult;
}
```
`window: (i64, i64)` with `(0, 0)` meaning "compute now" (same sentinel as Kotlin so area:26 callers port unchanged). `DomainError` variants: `NoChapters`, `NoEpisodes`, `NoSeasons`, `Data(DataError)`, `Source(SourceError)`, `Tracker(TrackError)`, `Io(io::Error)`; `Display` of `NoChapters`, `NoEpisodes` and `NoSeasons` is the message key lookup result (`no_chapters_error`), implemented in the GUI via `fn user_message(&DomainError) -> Message` using area:51 `tr!("no_chapters_error")`.

### 3.4 Source and extension listing
```rust
pub struct Pins(pub u8);                       // Unpinned=0, Pinned=1, Actual=2
impl Pins { pub const UNPINNED: Pins = Pins(0); pub const PINNED: Pins = Pins(0b11);
    pub fn contains(self, p: u8) -> bool { self.0 & p == p }            // Unpinned (0) is always contained, as in Kotlin
    pub fn plus(self, p: u8) -> Pins { Pins(self.0 | p) }
    pub fn minus(self, p: u8) -> Pins { Pins(self.0 ^ p) }              // XOR, as in Kotlin
}
pub struct SourceInfo { pub id: i64, pub lang: String, pub name: String, pub supports_latest: bool, pub is_stub: bool,
                        pub pin: Pins, pub is_used_last: bool, pub is_excluded_from_data_saver: bool }
pub fn compute_enabled_sources(inp: &EnabledInputs, all: &[SourceInfo]) -> Vec<SourceInfo>;
pub fn compute_languages(inp: &LangInputs, online: &[SourceInfo]) -> Vec<(String, Vec<SourceInfo>)>;   // ordered like SortedMap
pub fn compute_extensions_by_type<K: EntryKind>(inp: &ExtInputs, installed: &[InstalledExt<K>], untrusted: &[UntrustedExt<K>], available: &[AvailableExt<K>]) -> Extensions<K>;
pub fn cmp_ignore_case_java(a: &str, b: &str) -> Ordering;
pub fn lang_cmp(a: &str, b: &str) -> Ordering;                          // LocaleHelper.comparator
```
`Extensions<K> { updates, installed, available, untrusted }`.

### 3.5 Tracking types
```rust
pub struct Track { pub id: Option<i64>, pub entry_id: i64, pub tracker_id: i64, pub remote_id: i64, pub library_id: Option<i64>,
                   pub title: String, pub last_progress: f64, pub total: i64, pub status: i64, pub score: f64,
                   pub remote_url: String, pub start_date: i64, pub finish_date: i64, pub private: bool }
// last_progress == lastChapterRead / lastEpisodeSeen; EntryKind decides labels. id None only for fresh remote tracks before insert.
pub struct TrackChapter { /* trackers, tracks repo, queue, worker, connectivity */ }
impl TrackChapter { pub async fn run(&self, manga_id: i64, chapter_number: f64, setup_job_on_failure: bool); }
pub struct AddMangaTracks;
impl AddMangaTracks {
    pub async fn bind(&self, tracker: &dyn MangaTracker, item: Track, manga_id: i64) -> Result<(), TrackError>;
    pub async fn bind_enhanced_trackers(&self, manga: &Manga, source: &dyn MangaSource);
}
pub struct RefreshMangaTracks;
impl RefreshMangaTracks { pub async fn run(&self, manga_id: i64) -> Vec<(TrackerId, TrackError)>; }
pub enum AutoTrackState { Always, Ask, Never }       // name strings "ALWAYS","ASK","NEVER"
```

## 4. Crates and why

| Crate | Replaces | Why |
|---|---|---|
| `tokio` (`rt-multi-thread`, `sync`, `time`, `macros`), `tokio-stream` (`sync`) | coroutines, `Flow`, `WorkManager` | `watch` for latest-value state, `WatchStream` for `Flow`, `sleep` for backoff |
| `futures` (`join_all`, `select`) | `async { } .awaitAll()`, `supervisorScope` | independent per-tracker futures; failures collected |
| `async-trait` | Kotlin interfaces with `suspend` | collaborators are `dyn` |
| `fancy-regex` | `kotlin.text.Regex` with look-behind (`SeasonRecognition.basic`) | `regex` has no look-around; only `SeasonRecognition` needs it. All other patterns in this scope use plain `regex` |
| `regex` | the remaining regexes | linear time |
| `time` (`local-offset`, `formatting`) | `ZonedDateTime`, `ZoneId.systemDefault()` | local-midnight windows and the UTC-shifted tracker dates (p1-topic-tracking-orchestration.md section 5). `OffsetDateTime::now_local()` can fail in multithreaded programs on Linux, so the local UTC offset is obtained through the single helper in `kuro-core::time` (area:05 `time.rs`), which resolves it once on the main thread at startup and re-resolves per instant with `libc::localtime_r` wrapped in `kuro-platform` for DST-correct historical dates |
| `icu_collator` (ICU4X) | `java.text.Collator.getInstance()` | locale-aware tertiary collation without system ICU; only needed for `compareToWithCollator` uses (source migration order, chapter alphabetical sort) |
| `sys-locale` | `Locale.getDefault()` | default enabled languages |
| `serde` / `serde_json` (`preserve_order`) | kotlinx `JsonObject` (memo), queue file | memo equality in `ShouldUpdateDbEpisode` compares JSON objects structurally: use `serde_json::Value` equality (object key order is irrelevant for `==`) |
| `tempfile` | `SharedPreferences.edit` atomicity | atomic queue file replace |
| `thiserror`, `tracing` | exceptions, logcat | |
| `indexmap` | `LinkedHashMap` | stable group order for `SortedMap` outputs |

No ORM, no DI crate, no `rx`-style crate beyond tokio-stream.

## 5. Async and threading model

- All interactors are `async fn` on the tokio runtime owned by `kuro-core` (area:01 `bootstrap.rs`). They never touch GTK. The GUI crate bridges with `glib::MainContext::default().spawn_local(async move { let r = domain.set_read_status().run(..).await; update_widgets(r) })` for one-shot calls and an `async_channel::Receiver` drained in `glib::spawn_future_local` for streams (area:02 section 4 shows the pattern for preferences; the same helper `ui_stream(stream, |v| ...)` is reused here).
- `non_cancellable<F: Future + Send + 'static>(f: F) -> impl Future<Output = F::Output>`: `tokio::spawn(f)`, return the `JoinHandle` mapped; dropping the returned future does not cancel `f`. Used by `SetReadStatus`, `SetSeenStatus`, `DeleteChapterDownload`, `DeleteEpisodeDownload`, `AddMangaTracks::bind`, `AddAnimeTracks::bind`, `TrackChapter`, `TrackEpisode`.
- DB access uses area:05 `Db::write` closures (transactions run on the writer thread). The sync appliers should run their writes as ONE `Db::write` transaction (delete ids, insert, update, fetch interval, last_update): this deviates from Kotlin's sequence of independent calls, is invisible to users, and removes the window in which readers see a half-synced list. The planner stays pure and runs before opening the transaction.
- Preferences: `Pref::get()` is a blocking in-memory read (area:02), safe on any thread.
- Concurrency inside interactors: `TrackChapter`, `RefreshTracks` run per-track futures concurrently with `join_all`; the sync and set-status interactors are sequential.
- Reentrancy: `SetReadStatus` can be called while the reader writes `last_page_read` for the same chapter; both are single-row coalesce updates so the later write wins; no lock is needed.

## 6. Error handling

- `DomainError` (`thiserror`) at the library boundary; `anyhow` is not used inside this crate except inside `SetStatusResult::InternalError(Arc<anyhow::Error>)` which mirrors `Result.InternalError(Throwable)`.
- Kotlin's silent `catch` sites map to `tracing::warn!`/`info!` plus a returned value: `bindEnhancedTrackers` swallows per-tracker errors, `SyncProgressWithTrack` swallows, `TrackChapter` returns `()` and logs at info.
- Panics: a panic inside a spawned `non_cancellable` task becomes `DomainError::Join` and is logged at error; the crash handler of area:01 sees only panics on the main thread.
- User-visible messages are produced by the GUI from `DomainError` using area:51 keys: `no_chapters_error` (manga), `no_episodes_error`, `no_seasons_error`, `confirm_tracker_update`, `confirm_tracker_update_anime`, `trackers_updated_summary_manga`, `trackers_updated_summary_anime`, `lock_always`, `default_category_summary`, `lock_never`, `action_ok`.

## 7. Android API to Linux mapping (symbols touched by this scope)

| Android / Kotlin | Where used | Linux/Rust replacement |
|---|---|---|
| Injekt (`Injekt.get`, `InjektModule`, `addFactory`, `addSingletonFactory`) | DomainModule, AddTracks (inline `get`) | `Domain` struct with explicit fields |
| kotlinx coroutines `Flow`, `combine`, `distinctUntilChanged`, `supervisorScope`, `async`/`awaitAll`, `withContext(NonCancellable/IO)` | all interactors | tokio `watch` + `tokio_stream` + `futures::join_all` + `tokio::spawn` |
| `android.content.pm.PackageInfo`, `PackageInfoCompat.getLongVersionCode` | TrustExtension | `PackageIdentity { name, version_code, fingerprints }` (area:20) |
| `androidx.work` (`CoroutineWorker`, `OneTimeWorkRequestBuilder`, `Constraints`, `BackoffPolicy`, `ExistingWorkPolicy`, `NetworkType`, `workManager`) | Delayed*TrackingUpdateJob | `DelayedTrackingWorker` tokio task + `Connectivity::wait_online` (area:53 decides OS scheduling; this area needs none) |
| `SharedPreferences` (`getSharedPreferences`, `getFloat`, `putFloat`, `edit {}`, `all`) | Delayed*TrackingStore | JSON file with atomic replace |
| `Context.isOnline()` (`ConnectivityManager`, `NetworkCapabilities`) | TrackEpisode | `gio::NetworkMonitor` via `kuro-platform::Connectivity` |
| `Drawable`, `toBitmap`, `ImageBitmap` (Compose) | `AnimeSource.icon`, `Source.icon` | `gdk::Texture` from a cached PNG (`SourceIcons` trait, GUI crate) |
| `ZonedDateTime`, `ZoneId`, `ZoneOffset`, `ChronoUnit`, `Instant` | sync, UpdateManga, AddTracks | `time::OffsetDateTime`, `time::UtcOffset`, `time::Duration` |
| `java.text.Collator` (via `compareToWithCollator`) | favorite-count sorting | `icu_collator::Collator` |
| `java.util.TreeSet<Double>` (`deletedChapterNumbers`) | sync | `BTreeSet<OrderedF64>` is unnecessary: only membership is used; use `HashSet<u64>` of `f64::to_bits()` (all numbers are produced by the same widening, so equal values have equal bits; normalise `-0.0` to `0.0`) |
| `logcat` | all | `tracing` |
| `uy.kohesive.injekt.api.get` | in-function lookups | removed |
| Compose `ImageBitmap`, `androidx.compose.runtime.Immutable` | models | removed |
| moko-resources `StringResource` (`AutoTrackState.titleRes`) | AutoTrackState | `fn label(self) -> &'static str` key (`"lock_always"`, `"default_category_summary"`, `"lock_never"`) resolved by area:51 |

## 8. How this plugs into the existing Kurobayomi mockup

Existing files (`src/ui.rs`, `src/screens.rs`, `src/entry.rs`, `src/settings.rs`, `src/player.rs`, `src/spec.rs`, `spec/*.json`) render static data from the JSON spec. Hook-up points for this area (the GUI adds `kuro-domain` as a dependency and a `Domain` instance in an `Rc<AppState>` created in `src/main.rs` after area:01 startup):
- `src/entry.rs::sources(anime)` (line 283): replace the hard-coded rows with the stream from `Domain::enabled_sources(kind)`; the section headers use the grouping rule of p1-topic-source-and-extension-listing.md section 2 (`last_used`, `pinned`, then language, `""` last).
- `src/entry.rs::extensions(anime)` (line 311): bind `Extensions { updates, installed, available, untrusted }` to the four sections (updates, installed, available, untrusted) and the language filter dialog to `ExtensionLanguages`.
- `src/entry.rs::migrate_sources(anime)` (line 360): `SourcesWithFavoriteCount` plus the sort chips from `SetMigrateSorting`.
- `src/entry.rs::entry(title, anime)` (line 81) chapter/episode list: apply `apply_filters` + sort; the filter sheet toggles flags through area:07 `SetMangaChapterFlags`; the "mark read" actions call `SetReadStatus`; the exclude-scanlator dialog uses `GetAvailableScanlators`/`SetExcludedScanlators`.
- `src/entry.rs::tracking()` / `tracker_search()` (lines 498, 539): `RefreshTracks`, `AddMangaTracks::bind`, and `AutoTrackState` list in settings (`spec/settings.json` page for tracking).
- `src/settings.rs` pages "Browse" and "Tracking": the `list`/`switch` items map to `SourcePreferences` and `TrackPreferences` keys of p1-data-and-config.md section 4 through area:02's `prefs_bind::bind`.
- `src/player.rs`: the skip-intro length control uses `with_skip_intro` / `skip_intro_length` through `SetAnimeViewerFlags` (the mockup's "intro length" slider range: the stored value is 0-255).
No change to `spec.rs` types is required; this area adds no spec data.

## 9. Pitfalls

1. f32 vs f64 chapter numbers (p1-topic-chapter-episode-sync.md section 4): keep `SChapter.chapter_number: f32`, widen with `as f64`.
2. `coalesce` updates cannot clear nullable columns: do not "fix" this in the planner, or `ShouldUpdateDbChapter` will stop converging with Kotlin-written databases; if you do need clearing, add an explicit `clear_scanlator` flag in area:05.
3. `Pins::contains(Unpinned)` is always true and `minus` is XOR; port with tests, do not "clean up".
4. `CASE_INSENSITIVE_ORDER` is not Unicode case folding; `str::to_lowercase().cmp()` differs for `ß`, `İ` and non-BMP characters.
5. `getAndSet` preference toggles are not atomic across the GTK thread and tokio threads; use `Pref::update` inside the store lock.
6. A refresh must not write `title` for favourites (`UpdateManga`), but `NetworkToLocal` DOES rewrite the title of non-favourites: both rules together keep library titles stable.
7. Queue key collision between manga and anime tracks (fixed by namespacing).
8. `SyncSeasonsWithSource` inserts rows via `NetworkToLocalAnime` before diffing, so an aborted sync leaves orphan non-favourite rows; they are harmless (cleaned by the "clear database" action, area:47).
9. `time::OffsetDateTime::now_local()` returns `Err` on some multi-threaded setups; always go through the `kuro-core::time` helpers.
10. Do not hold a `std::sync::Mutex` across `.await` in `DelayedTrackingStore`; use `tokio::sync::Mutex` or keep the critical section synchronous and short (it is: map update plus file write).
