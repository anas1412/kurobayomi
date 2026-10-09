# 45-player-core (part 2 of 2) — Rust design

Target: Kurobayomi (`/home/blackbox/kurobayomi`, crate `kurobayomi`, edition 2024, `gtk` = gtk4-rs 0.11.5 with `v4_16`, `serde`, `serde_json`). This part describes the player-core modules that sit under `src/player/`. The existing mockup file `src/player.rs` (337 lines, `pub fn player(title, episode) -> GBox`) is a UI shell with fake state; this design replaces its body and keeps its helper calls (`sheet`, `dialog`, `pill`, `icon_button`, `on_long_press`, `two_line`, `margins`, `tag`, `pop`, `osd`) from `src/ui.rs`.

Every Kotlin reference below points to `/home/blackbox/aniyomi-src` (repo-relative). Behavior references point to `p2-behavior.md` sections (§n).

## 1. Crate and module layout

```
src/
  player.rs                  -> becomes `mod player;` facade: pub fn player(...) -> GBox (UI, see p2-ui-parity.md)
  player/
    mod.rs                   re-exports
    types.rs                 Video, VideoState, Hoster, HosterState, ChapterType, TimeStamp, ThumbnailInfo, TileInfo, VideoTrack
    model.rs                 PlayerModel (state store, all observable fields), Sheet/Panel/Dialog enums
    actions.rs               user actions (pause, seek, volume, aspect, double-tap dispatch, controls visibility)
    session.rs               init(), episode switch (loadEpisode), needs_init, playlist build
    playlist.rs              filter_episode_list, adjacent-episode ids, sort (uses area:07 sort)
    loader/
      mod.rs
      episode.rs             EpisodeLoader (hosters per episode kind, lazy loading, video resolution)
      hoster.rs              HosterLoader (select_best_video, get_best_video race, get_resolved_video)
    tracks.rs                mpv track-list load, on_finish_loading_tracks, select_sub, add_audio/sub
    track_select.rs          TrackSelect (language/whitelist/blacklist) with ICU-based display names
    chapters.rs              chapter list, merge_chapters (ChapterUtils), chapter type labels
    skip/
      mod.rs                 SkipIntroMachine (setChapter / onSkipIntro / countdown)
      aniskip.rs             AniSkipClient (reqwest), skip_times(), mal_id_from_anilist()
    progress.rs              onSecondReached, completion, save_progress, save_history, incognito gating, download-ahead, delete-after-read
    thumbs.rs                seek preview: tile cache (LRU 3), throttle 2 s, crop
    screenshot.rs            screenshot, save, share, set-as-art
    lua.rs                   user-data/aniyomi command dispatcher
    mpv.rs                   libmpv2 wrapper: property get/set/observe, commands, event thread, render context
    prefs.rs                 typed accessors over the settings store (all keys in p2-data-and-config.md §4)
    session_file.rs          process-restart snapshot (episode_id, position_ms, quality_index)
    error.rs                 PlayerError { kind, message_key }  (ExceptionWithStringResource)
```

Settings types live under `src/settings/` (the mockup already has `settings.rs`): `player_audio.rs`, `player_decoder.rs`, `player_gesture.rs`, `player_subtitle.rs`, `player_advanced.rs`, `player.rs`. Each is a plain struct with `fn load(&Store) -> Self` plus setters, so the Kotlin "getter returns a Preference" pattern becomes `store.get::<T>(KEY)` / `store.set(KEY, v)`. The store is area:02's responsibility; this part only fixes key strings and defaults (data file §4).

## 2. Dependency choices

| Need | Crate | Why / version note |
|---|---|---|
| mpv playback | `libmpv2` (safe bindings over libmpv client API, with the `render` module for an embedded GL surface) | Replaces `is.xyz.mpv.MPVLib`. Verify the current crate version and feature names at implementation time; the design only needs: create handle, set option, set/get property typed, observe property with format, `command`, event loop, render context on a `GtkGLArea`. |
| HTTP | `reqwest` with `rustls-tls`, `json` features | Replaces OkHttp (AniSkip, AniList). One shared `reqwest::Client` lives in area:12-network-stack; this part borrows it (Kotlin used a private `OkHttpClient()`, see E-05). |
| JSON | `serde`, `serde_json` (already in Cargo.toml) | Replaces kotlinx.serialization. `#[serde(rename = "mixed-op")]` etc. |
| Async | `tokio` (multi-thread runtime, owned by main) + `tokio-util` (`CancellationToken`) | Replaces Kotlin coroutines `Dispatchers.IO`, `async`, `awaitAll`, `coroutineScope`, cancellation. |
| Main-thread bridge | `async-channel` + `glib::spawn_future_local` | Results from tokio and mpv events are delivered to the GTK main loop. |
| Database | `rusqlite` (area:03-05) | Via the domain layer; this part calls only `Repo`-level async functions (see §6). |
| Locale display names | `icu_displaynames` + `icu_locale` (or `unic-langid` as fallback) | Replaces `Locale.getDisplayName(locale)` and `isO3Language` (ISO 639-2 table from `icu_locale`/`isolang`). |
| Regex | `regex` | Replaces `Regex(...)`; use `(?i)` for IGNORE_CASE. |
| Dates | `chrono` | Replaces `java.util.Date`; stored as epoch seconds in DB. |
| Logging | `tracing` | Replaces `logcat`. |
| Collation for alphabet sort | area:07 chooses (`icu_collator`) | Used by `playlist.rs` only through the shared sort function. |

Explicitly not used: libadwaita (forbidden), GStreamer (mpv is the single player backend), any Android shim.

## 3. Core types (`src/player/types.rs`)

```rust
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoState { Queue, LoadVideo, Ready, Error }        // Video.State

#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    pub video_url: String,                 // "null" = lazy, resolve via source.video_url()
    pub video_title: String,
    pub resolution: Option<i32>,
    pub bitrate: Option<i32>,
    pub headers: Vec<(String, String)>,    // okhttp Headers -> Vec, order preserved
    pub preferred: bool,
    pub subtitle_tracks: Vec<Track>,
    pub audio_tracks: Vec<Track>,
    pub timestamps: Vec<TimeStamp>,
    pub mpv_args: Vec<(String, String)>,
    pub ffmpeg_stream_args: Vec<(String, String)>,
    pub ffmpeg_video_args: Vec<(String, String)>,
    pub internal_data: String,
    pub initialized: bool,
    pub memo: serde_json::Map<String, serde_json::Value>,
}
impl Video {
    pub const LAZY_URL: &'static str = "null";
    pub fn is_lazy(&self) -> bool { self.video_url == Self::LAZY_URL }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track { pub url: String, pub lang: String }

#[derive(Debug, Clone, PartialEq)]
pub struct Hoster {
    pub hoster_url: String,
    pub hoster_name: String,
    pub video_list: Option<Vec<Video>>,   // None = not loaded
    pub internal_data: String,
    pub lazy: bool,
    pub memo: serde_json::Map<String, serde_json::Value>,
}
pub const NO_HOSTER_LIST: &str = "no_hoster_list";

#[derive(Debug, Clone, PartialEq)]
pub enum HosterState {
    Idle { name: String },
    Loading { name: String },
    Error { name: String },
    Ready { name: String, videos: Vec<Video>, states: Vec<VideoState> },
}
impl HosterState {
    /// Kotlin HosterState.Ready.getChangedAt
    pub fn with_video(&self, index: usize, video: Video, state: VideoState) -> Option<HosterState> {
        if let HosterState::Ready { name, videos, states } = self {
            let mut v = videos.clone(); v[index] = video;
            let mut s = states.clone(); s[index] = state;
            Some(HosterState::Ready { name: name.clone(), videos: v, states: s })
        } else { None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterType { Opening, Ending, Recap, MixedOp, Other }

#[derive(Debug, Clone, PartialEq)]
pub struct TimeStamp { pub start: f64, pub end: f64, pub name: String, pub kind: ChapterType }

#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {                       // IndexedSegment
    pub name: String,
    pub start: f32,
    pub color: Option<u32>,                // ARGB; None = Unspecified
    pub index: i32,                        // -2 external start, -1 end/synthetic, >=0 mpv chapter
    pub kind: ChapterType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThumbnailInfo { pub tiles: Vec<TileInfo>, pub image_tile_urls: Vec<String> }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileInfo { pub image_index: usize, pub time_ms: i64, pub x: u32, pub y: u32, pub width: u32, pub height: u32 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoTrack { pub id: i64, pub name: String, pub language: Option<String> }  // id -1 = Off
```

Notes:
- `Video` is `Clone` and `Eq`-free because of `f64`-free fields; the Kotlin `copy(initialized = true)` maps to struct update syntax.
- `TileInfo.image_index` is `usize` because it indexes `image_tile_urls`; Kotlin uses `Int` (see E-30 for negative-index guard).
- `ThumbnailInfo` tile list is sorted by `time_ms` on construction (behavior §4.7).

## 4. Source boundary (consumed trait, implemented by area:16/17)

```rust
#[async_trait::async_trait]   // or native async-fn-in-trait with Send bounds
pub trait AnimeSourceApi: Send + Sync {
    fn id(&self) -> i64;
    fn is_http(&self) -> bool;                                  // source is AnimeHttpSource
    fn is_local(&self) -> bool;                                 // source is LocalAnimeSource
    fn has_hoster_list(&self) -> bool;                          // replaces Kotlin reflection checkHasHosters (§3.3)
    async fn hoster_list(&self, ep: &SEpisode) -> anyhow::Result<Vec<Hoster>>;   // getHosterList + sortHosters
    async fn video_list(&self, ep: &SEpisode) -> anyhow::Result<Vec<Video>>;     // legacy getVideoList + sortVideos
    async fn video_list_for_hoster(&self, h: &Hoster) -> anyhow::Result<Vec<Video>>;
    async fn video_url(&self, v: &Video) -> anyhow::Result<String>;             // lazy URL
    async fn resolve_video(&self, v: &Video) -> anyhow::Result<Option<Video>>;  // resolveVideo
    async fn video_thumbnails(&self, v: &Video) -> anyhow::Result<Option<ThumbnailInfo>>;
    async fn image_tile(&self, url: &str) -> anyhow::Result<Option<gdk_pixbuf::Pixbuf>>;
}
```
`has_hoster_list` replaces the reflective method probe in `EpisodeLoader.checkHasHosters` (EpisodeLoader.kt:60-78). Every compiled-in source overrides it; the legacy path (`video_list` → `to_hoster_list`) is kept for extension-library compatibility below 1.6 and is marked `// compat: ext-lib<1.6`.

`sort_hosters` / `sort_videos` are source-side and are called inside the trait implementation (area:16).

## 5. Player state store (`src/player/model.rs`)

Kotlin `MutableStateFlow` fields become one struct owned by the GTK main thread. Subscribers receive a change notification keyed by field, so widgets update without polling.

```rust
pub struct PlayerModel {
    // playlist / episode
    pub playlist: Vec<EpisodeRow>,           // Episode rows after filter (domain Episode + legacy fields)
    pub current_episode: Option<EpisodeRow>,
    pub current_anime: Option<AnimeRow>,
    pub current_source: Option<Arc<dyn AnimeSourceApi>>,
    pub has_prev: bool,
    pub has_next: bool,
    pub is_episode_online: bool,
    pub is_loading_episode: bool,
    pub decoder: Decoder,
    pub media_title: String,
    pub anime_title: String,
    pub is_loading: bool,
    pub playback_speed: f32,
    // tracks
    pub sub_tracks: Vec<VideoTrack>,
    pub selected_sub: (i64, i64),            // (primary, secondary); -1 = none
    pub audio_tracks: Vec<VideoTrack>,
    pub selected_audio: i64,
    pub is_loading_tracks: bool,
    // hosters / videos
    pub hoster_list: Vec<Hoster>,
    pub is_loading_hosters: bool,
    pub hoster_state: Vec<HosterState>,
    pub hoster_expanded: Vec<bool>,
    pub selected_hoster_video: (i32, i32),   // (-1,-1) = none
    pub current_video: Option<Video>,
    // chapters / skip
    pub chapters: Vec<Chapter>,
    pub current_chapter: Option<Chapter>,
    pub skip_intro_text: Option<String>,
    // position
    pub pos: f32,
    pub duration: f32,
    pub read_ahead: f32,
    pub seek_position: f32,
    pub is_seeking: bool,
    pub seek_thumbnail: Option<gdk_pixbuf::Pixbuf>,
    pub paused: bool,
    pub paused_state: Option<bool>,          // pending pause/unpause applied after load (Kotlin _pausedState)
    // controls
    pub controls_shown: bool,
    pub seek_bar_shown: bool,
    pub controls_locked: bool,
    pub sheet: Sheet,
    pub panel: Panel,
    pub dialog: Dialog,
    pub dismiss_sheet: bool,
    pub seek_text: Option<String>,
    pub double_tap_seek_amount: i32,
    pub is_seeking_forwards: bool,
    pub remaining_sleep: i32,
    pub primary_button: Option<CustomButton>,
    pub primary_button_title: String,
    pub custom_buttons: CustomButtonsState,  // Loading | Success(Vec) | Error(String)
    // volume / brightness
    pub volume_system: u8,                   // 0..=100 (PipeWire/Pulse sink)
    pub volume_mpv: i64,                     // mpv volume, 0..=100+cap
    pub volume_boost_cap: i64,               // mpv volume-max - 100
    pub brightness: f32,                     // -0.75..=1.0 (state), see behavior §9.2
    pub is_brightness_slider_shown: bool,
    pub is_volume_slider_shown: bool,
    pub playlist_flags: u64,                 // anime.episode_flags snapshot
    // one-shots
    pub player_update: PlayerUpdate,         // None | DoubleSpeed | AspectRatio | ShowText(String) | ShowTextResource(MsgKey)
}

pub enum Sheet { None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot }
pub enum Panel { None, SubtitleSettings, SubtitleDelay, AudioDelay, VideoFilters }
pub enum Dialog { None, EpisodeList, IntegerPicker(IntegerPicker) }
pub struct IntegerPicker { pub default: i64, pub min: i64, pub max: i64, pub step: i64, pub name_format: String, pub title: String, pub property: String }
```

Change notification:

```rust
pub enum Change { Pos, Paused, Controls, Sheet, Panel, Dialog, Tracks, Hosters, Chapters, SkipText, Playlist, Episode, Video, Volume, Brightness, SeekText, SeekThumb, Loading, PlayerUpdate, /* ... one per field group */ }

pub struct Store { pub model: PlayerModel, subs: Vec<Box<dyn Fn(Change, &PlayerModel)>> }
impl Store {
    pub fn update(&mut self, f: impl FnOnce(&mut PlayerModel) -> Vec<Change>) { let ch = f(&mut self.model); for c in ch { for s in &self.subs { s(c, &self.model) } } }
}
```
Rule: every mutation goes through `Store::update`, mirroring Kotlin `update { }`. Single-threaded, so no atomics.

`Sheet`/`Panel`/`Dialog` exclusivity (behavior §10) is enforced by methods on `PlayerModel`, not by widgets: `show_sheet`, `show_panel`, `show_dialog` clear the other two.

## 6. Repositories and domain calls

The player calls, by name, domain interactors (area:05-07, 24-27). Declared as async functions on a `PlayerDeps` struct so tests can stub them:

```rust
pub struct PlayerDeps {
    pub get_anime: Arc<dyn Fn(i64) -> BoxFut<Option<AnimeRow>> + Send + Sync>,
    pub get_episodes_by_anime: Arc<dyn Fn(i64) -> BoxFut<Vec<EpisodeRow>> + Send + Sync>,
    pub get_next_episodes: Arc<dyn Fn(i64, i64) -> BoxFut<Vec<EpisodeRow>> + Send + Sync>,
    pub update_episodes: Arc<dyn Fn(Vec<EpisodeUpdate>) -> BoxFut<()> + Send + Sync>,   // awaitAll: swallows errors (UpdateEpisode.kt:20-27)
    pub upsert_history: Arc<dyn Fn(i64, chrono::DateTime<chrono::Utc>) -> BoxFut<()> + Send + Sync>,
    pub set_skip_intro_length: Arc<dyn Fn(i64, u8) -> BoxFut<()> + Send + Sync>,        // masks to 0xFF (E-27)
    pub anime_categories: Arc<dyn Fn(i64) -> BoxFut<Vec<String>> + Send + Sync>,
    pub anime_tracks: Arc<dyn Fn(i64) -> BoxFut<Vec<TrackRow>> + Send + Sync>,
    pub incognito_for_source: Arc<dyn Fn(Option<i64>) -> BoxFut<bool> + Send + Sync>,
    pub tracker_episode_seen: Arc<dyn Fn(i64, f64) -> BoxFut<()> + Send + Sync>,
    pub downloads: Arc<dyn DownloadApi + Send + Sync>,   // see below
    pub prefs: Arc<PlayerPrefs>,
    pub custom_buttons: Arc<dyn Fn() -> BoxFut<Vec<CustomButton>> + Send + Sync>,
    pub source_for: Arc<dyn Fn(i64) -> Arc<dyn AnimeSourceApi> + Send + Sync>,            // getOrStub
}
pub type BoxFut<T> = std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<T>> + Send>>;

#[async_trait::async_trait]
pub trait DownloadApi: Send + Sync {
    fn is_episode_downloaded(&self, name: &str, scanlator: Option<&str>, anime_title: &str, source_id: i64) -> bool; // skipCache=true at EpisodeLoader only
    async fn build_video(&self, source: &dyn AnimeSourceApi, anime: &AnimeRow, ep: &EpisodeRow) -> anyhow::Result<Video>;
    async fn download_episodes(&self, anime: &AnimeRow, eps: Vec<EpisodeRow>);
    async fn enqueue_delete(&self, eps: Vec<EpisodeRow>, anime: &AnimeRow);
    async fn delete_pending(&self);
    async fn add_to_start_of_queue(&self, eps: Vec<AnimeDownload>);
}
```
`is_episode_downloaded` is synchronous in Kotlin, implemented against the download manager cache; in Rust it is a filesystem check with no cache when called from `EpisodeLoader::is_download` (behavior §3.2), and cached elsewhere (area:24 decision).

## 7. Concurrency model (the core rule)

```
GTK main thread (glib MainContext)
  owns Store (PlayerModel), all widgets, libmpv *handle* calls that must be on main thread (render, set_property for vo/window)
  receives: MpvEvent (from mpv thread), LoadResult (from tokio), IoResult (thumbnail tiles, screenshot save)
tokio runtime (multi-thread, 2 workers suffices; created in main.rs, Handle stored in an Rc/OnceCell)
  runs: hoster list, video list, resolve_video, video_url, thumbnails, AniSkip, AniList, DB via repo, download manager calls
libmpv event thread (owned by libmpv2)
  property-change and end-file events -> async_channel::Sender<MpvEvent> (non-blocking send)
```

Bridges:
```rust
// main.rs (or player::bridge)
let (mpv_tx, mpv_rx) = async_channel::unbounded::<MpvEvent>();
glib::spawn_future_local(clone!(@strong store => async move {
    while let Ok(ev) = mpv_rx.recv().await { on_mpv_event(&store, ev); }
}));
```
Same for `io_rx` carrying `IoEvent` results from tokio; tokio tasks clone the `async_channel::Sender` (which is `Send`).

Rules:
1. Store is touched only on the main thread. Tokio tasks never receive `&mut Store`; they return data through `IoEvent`.
2. Cancellation: each `load_hosters` call creates a `CancellationToken`. Kotlin `getHosterVideoLinksJob?.cancel()` becomes `token.cancel()`. Child tasks use `tokio::select! { _ = token.cancelled() => return, r = fut => ... }`. On cancel the hoster states are reset to `Idle` (behavior §4.1 step 4).
3. Fan-out: `loadHosters` uses `futures::future::join_all` over per-hoster tasks spawned with `tokio::spawn`; the "first preferred wins" compare-and-set is an `AtomicBool` (matches `AtomicBoolean` in Kotlin). `getBestVideo` uses `tokio::task::JoinSet` and aborts the rest on the first success (`EarlyReturnException` → a `ControlFlow::Break(video)` channel).
4. Non-cancellable work (progress saves, deletes, screenshot save) is spawned with `tokio::spawn` and is NOT tied to the token, matching `launchNonCancellable`.
5. `onCleared` (ViewModel teardown) → `Store::on_close()`: spawns the final save, then the process exits; the runtime is shut down with `Runtime::shutdown_timeout(Duration::from_secs(3))` so pending saves finish (see E-29).

## 8. mpv wrapper (`src/player/mpv.rs`)

```rust
pub struct Mpv { handle: libmpv2::Mpv /* or Arc */ , events: async_channel::Sender<MpvEvent> }
pub enum MpvEvent {
    Property { name: String, value: MpvValue },   // format per observed list (see below)
    EndFile { reason: EndReason },
    FileLoaded,
    Log { level: String, text: String },
}
pub enum MpvValue { I64(i64), F64(f64), Bool(bool), Str(String), None }

impl Mpv {
    pub fn set_str(&self, k: &str, v: &str) -> Result<()>;
    pub fn set_i64(&self, k: &str, v: i64) -> Result<()>;
    pub fn set_f64(&self, k: &str, v: f64) -> Result<()>;
    pub fn get_i64(&self, k: &str) -> Option<i64>;         // Kotlin getPropertyInt (nullable)
    pub fn get_f64(&self, k: &str) -> Option<f64>;
    pub fn get_str(&self, k: &str) -> Option<String>;
    pub fn command(&self, args: &[&str]) -> Result<()>;     // MPVLib.command
    pub fn observe(&self, name: &str, fmt: ObserveFmt);      // mirrors observedProps table
}
pub enum ObserveFmt { I64, F64, Str, Flag, None }
```

Observed property set (must match `AniyomiMPVView.kt:210-230` exactly, because whole-second behavior depends on INT64):
- `I64`: `chapter`, `time-pos`, `demuxer-cache-time`, `duration`, `volume`, `volume-max`.
- `None`: `chapter-list`, `track-list`.
- `Str`: `sid`, `secondary-sid`, `aid`, `hwdec-current`, `hwdec`.
- `F64`: `speed`, `video-params/aspect`.
- `Flag`: `pause`, `paused-for-cache`.
- plus `user-data/aniyomi` (`Str`) for the Lua bridge (PlayerActivity.kt:740).

Event delivery: libmpv2's event loop runs on its own thread; the closure forwards with `try_send` (channel is unbounded, so never blocks libmpv). Kotlin's `eventChannel` rendezvous semantics are not reproduced (E-23).

Render: the GL render context is created on the `GtkGLArea` `realize` signal and freed on `unrealize`; `render` is called from the `render` signal handler on the main thread. This replaces the `AniyomiMPVView` `SurfaceView` pipeline (part 1 owns the widget, this part only needs the `Mpv` handle).

## 9. Algorithms as Rust functions (signatures and mapping)

| Kotlin | Rust | Notes |
|---|---|---|
| `init(animeId, initialEpisodeId, hostList, hostIndex, vidIndex)` | `session::init(&mut Store, &PlayerDeps, InitArgs) -> Result<InitResult, PlayerError>` | async; returns `Result<_, PlayerError>` (replaces `Pair<InitResult, Result<Boolean>>`). `Ok(None)` for the "anime null" branch. |
| `needsInit` | `fn needs_init(m: &PlayerModel, anime: i64, ep: i64) -> bool` | pure |
| `initEpisodeList` | `playlist::build(anime, episodes, downloaded_only) -> Vec<EpisodeRow>` | sort is area:07 `episode_sort(anime, false)` |
| `filterEpisodeList` | `playlist::filter(anime_flags, episodes, selected_id, is_downloaded: impl Fn(&EpisodeRow)->bool) -> Result<Vec<EpisodeRow>, PlayerError>` | `error("Requested episode…")` → `PlayerError::EpisodeNotFound` |
| `getHosters` | `loader::episode::get_hosters(&dyn Deps, &EpisodeRow, &AnimeRow, &dyn AnimeSourceApi) -> Result<Vec<Hoster>>` | dispatch order from behavior §3.1 |
| `getBestVideo` | `loader::hoster::get_best_video(source, &[Hoster], cancel) -> Option<Video>` | JoinSet race |
| `selectBestVideo` | `loader::hoster::select_best_video(&[HosterState]) -> Option<(usize, usize)>` | pure; `None` = (-1,-1) |
| `getResolvedVideo` | `loader::hoster::resolve(source, video) -> Option<Video>` | `.copy(initialized=true)` |
| `loadHosters` | `session::load_hosters(&mut Store, deps, source, hosters, hi, vi)` | spawns tasks; results via `IoEvent::HosterDone{idx, state}` |
| `loadVideo` (recursive) | `session::load_video(&mut Store, deps, source, video, hi, vi) -> BoxFut<bool>` implemented as a loop (not recursion) with the same termination (each failure marks one slot ERROR) | avoids deep async recursion |
| `setChapter` / `onSkipIntro` | `skip::SkipIntro::on_tick(&mut self, store, pos)` / `on_skip(&mut self, store)` | state in `SkipIntro { waiting: i32, default_wait: i32, netflix, auto_skip, enabled }` |
| `onSecondReached` | `progress::on_second(&mut Store, deps, position_s: i32, duration_s: i32)` | returns follow-up `Vec<ProgressJob>` spawned on tokio |
| `seekTo` / `seekBy` | `actions::seek_to(&Mpv, pos_i64, precise: bool)`, `actions::seek_by(&Mpv, delta_i64, precise)` | exact flag strings from behavior §8.2 |
| `changeVolumeBy` | `actions::change_volume_by(store, mpv, sys: &dyn SystemVolume, delta)` | `SystemVolume` trait: PipeWire/Pulse impl (§12) |
| `changeVideoAspect` | `actions::set_aspect(mpv, aspect, monitor_ratio: f64)` | table in behavior §9.3 |
| `handleLuaInvocation` | `lua::dispatch(&mut Store, deps, name, value)` | table in behavior §12; clears the property at end |
| `aniSkipResponse` | `skip::aniskip::resolve_timestamps(deps, anime, ep_num, duration) -> Option<Vec<TimeStamp>>` | first-track fix (behavior §6.5 proposal) |
| `mergeChapters` | `chapters::merge(current, stamps, duration) -> Vec<Chapter>` | pure; unit-testable (see p2-edge-cases) |
| `getPreferredTrackIndex` | `track_select::preferred(tracks, cfg, subtitle: bool) -> Option<&VideoTrack>` | pure |

`PlayerError`:
```rust
#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("{0}")] Keyed { message: String, key: MsgKey },    // ExceptionWithStringResource
    #[error("requested episode {0} not found in episode list")] EpisodeNotFound(i64),
    #[error(transparent)] Other(#[from] anyhow::Error),
}
pub enum MsgKey { NoEpisodeLoaded, SelectHosterFromEmptyList, NoHosters, NoAvailableVideos, NoPrevEpisode, NoNextEpisode, /* … full list in p2-behavior §18 */ }
```
Each `MsgKey` is resolved through the app's localisation table (area:02 `i18n` module). The English base text in §18 is the fallback.

## 10. Pure logic that must be unit-testable (keep out of GTK)

- `select_best_video`, `filter`, `build` (sort + downloaded-only), `merge_chapters`, `preferred` (TrackSelect), `seek_by` accumulator, `skip::SkipIntro` transitions, `aniskip` JSON decode, `skip_type` mapping, `setFlag`/mask helpers (`viewer_flags` read/write), `thumb_crop`.
Test strategy and fixtures: `p2-edge-cases-and-tests.md`.

## 11. Integration with the existing mockup

Current `src/player.rs` builds: an `Overlay` with a placeholder `osd` surface, a top toolbar (back, title button → `dialog("player","episode_list")`, auto-play switch, four icon buttons with long-press sheets), a middle row (prev, play/pause, next), a bottom OSD with lock, rotate, speed label (`1.00x`, cycles 0.25 steps, long press → `playback_speed` sheet), chapter button (`00:00 • Opening`), skip button, PiP, aspect button (cycles three labels), seek row (`Scale` 0..1420, marks at 85 and 1330), and a double-tap/long-press gesture on the surface.

Replacement plan (details in `p2-ui-parity.md`):
1. The surface `GBox` becomes a `gtk::GLArea` with the mpv render context (`mpv.rs`). Keep the same overlay stack.
2. All local widget state (`Cell` index for aspect, label text for speed, play icon swap, lock visibility) is removed and replaced by bindings to `Store` change notifications. The mock's toggles become calls into `actions::*`.
3. `duration = 1420.0` constant becomes `store.model.duration`.
4. `pill(&o, text)` calls become `store.model.seek_text` / `player_update` rendering through the same `pill` helper.
5. `sheet("player", id)` / `dialog("player", id)` remain; they are fed by `Sheet`/`Dialog` changes (exclusivity in `PlayerModel`).
6. `spec/reader_player.json` supplies the static labels/icons used by `ui.rs` (unchanged); no new spec keys are needed for this part beyond the message keys in `p2-behavior.md` §18, which go into the i18n table.

## 12. Android API → Linux mapping (this part)

| Android/Kotlin | Linux / Rust replacement | Decision |
|---|---|---|
| `MPVLib` (`is.xyz.mpv`) | `libmpv2` crate (`mpv.rs`) | Replace |
| `PlayerActivity`/`ViewModel`/`SavedStateHandle` | `Store` + session file (`session_file.rs`) | Rewrite |
| `Settings.System.SCREEN_BRIGHTNESS`, `WindowManager.LayoutParams.screenBrightness` | mpv `brightness` filter (state −0.75..1 mapped to −100..100); optional sysfs backlight writer behind a pref (P2) | Replace (T-45b-032) |
| `AudioManager.getStreamVolume/setStreamVolume(STREAM_MUSIC)` | `SystemVolume` trait: `wpctl get-volume @DEFAULT_AUDIO_SINK@` / `set-volume` (PipeWire) with `pactl` (PulseAudio) fallback; no GTK API exists for sink volume | Replace (T-45b-031) |
| `DisplayMetrics.getRealMetrics` (stretch ratio) | `gdk::Monitor::geometry()` of the window's monitor | Replace |
| `WindowInsetsCompat` status bars | Not applicable (desktop has no system status bar); `pref_show_system_status_bar` is kept as a no-op with a settings note (T-45b-054) | Drop |
| `ActivityInfo.SCREEN_ORIENTATION_*` cycling | Window fullscreen toggle (`gtk::Window::fullscreen`) | Drop orientation; replace with fullscreen (T-45b-058) |
| `InputMethodManager` software keyboard | Not applicable on desktop; custom text fields use GTK entry | Drop (T-45b-065) |
| `Uri.openContentFd`, `getFileName` (content://) | `gtk::FileDialog` returns a `gio::File` with a local path; pass the path to `sub-add`/`audio-add` | Replace (T-45b-072) |
| `Bitmap.createBitmap` (crop) | `gdk_pixbuf::Pixbuf::new_subpixbuf` (copy with `copy()` if the source is reused) | Replace |
| `ImageBitmap` | `gdk::Texture` from `Pixbuf` | Replace |
| `Injekt` singletons | explicit `PlayerDeps` struct passed at construction | Rewrite |
| `Channel<Event>` | `async_channel` to main loop | Replace |
| `PiP` (`setPictureInPictureParams`) | Separate always-on-top mini window (see p2-ui-parity §PiP) | Rewrite (T-45b-050) |
| `ExternalIntents` (part 1) | `xdg-open` for a configured external player, or `mpv` itself | Replace (part 1) |
| `CustomButton.execute` (`script-message`) | `Mpv::command(&["script-message", "call_button_<id>"])` | Keep |
| `java.util.Locale` | `icu_locale` + `icu_displaynames` | Replace |
| `kotlinx.serialization Json` | `serde_json` with `#[serde(default)]` and `deny_unknown_fields` off | Replace |
| `okhttp3.Headers` | `Vec<(String,String)>` → `reqwest::header::HeaderMap` | Replace |

## 13. Persistence of process-death state (`session_file.rs`)

The Kotlin `SavedStateHandle` keeps three values across process death: `episode_id`, `episode_position` (ms), `quality_index` (pair). The Rust port writes `$XDG_STATE_HOME/kurobayomi/player-session.json` (fallback `~/.local/state/kurobayomi/`):
```json
{"episode_id": 123, "episode_position_ms": 456000, "quality_index": [0, 1]}
```
Write policy: on each `on_second` tick, debounced to at most one write per 5 seconds, plus on close. Read policy: at `session::init`, if the file's `episode_id` equals the requested id, use `episode_position_ms` and `quality_index`; otherwise ignore. Write errors are logged, never fatal (matches the Kotlin "best effort" savedState).

## 14. Pitfalls specific to this part

1. Whole-second `time-pos` cadence: if the Rust side requests `F64` for `time-pos`, the completion and skip-intro timing change (more calls, different countdown). Keep `I64` (E-10).
2. `Store::update` re-entrancy: a `Change` subscriber must not call `update` synchronously; queue via `glib::idle_add_local_once`. Kotlin `StateFlow` never re-enters because collectors run on their own dispatcher.
3. GLArea render context must be created on the GL thread (main thread); creating it in a tokio task deadlocks.
4. libmpv property reads from a non-main thread are allowed but `set_property` for `vo`/`hwdec` before the GL context exists fails; order: create context → set options → load file.
5. `reqwest` blocking calls must not run on the GTK thread. `runBlocking` calls in Kotlin (`initEpisodeList`, `checkTrackers`) become `await` points in `session::init`.
6. Track id `-1` means "Off"; never pass `-1` to `aid`/`sid` expecting a real track (mpv interprets `no`; the Kotlin passes `-1` through `player.sid = -1`, which mpv treats as disabled).
7. Percent vs ratio: mpv `volume` is percent (0..100+cap); `brightness` filter is -100..100; `panscan` is 0..1; `video-aspect-override` is a ratio or -1.
8. `Chapter.color` uses ARGB `0xFFD8BBDF` for external chapters; GTK draws with RGBA — convert when drawing the seek bar (part 1 UI).
9. Sorting: the Kotlin SOURCE inversion (behavior §2.6) must be kept for the playlist; it is easy to "fix" accidentally when porting the sorter.
10. Filesystem names: `DiskUtil.buildValidFilename` must keep the 250-byte cap in bytes, not chars (multi-byte titles).

## 15. Test hooks

`PlayerDeps` and `AnimeSourceApi` are traits / boxed closures, so `session::init`, `loader::*`, `progress::*` can run against in-memory fakes in `#[cfg(test)]` modules. Fixtures are listed in `p2-edge-cases-and-tests.md`.
