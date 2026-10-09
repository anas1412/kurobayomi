# 45-player-core: area overview

Area id: `45-player-core`. Repo root for every path: `/home/blackbox/aniyomi-src` (repo-relative paths, e.g. `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`). Target: Kurobayomi (`/home/blackbox/kurobayomi`, crate `kurobayomi`, edition 2024, gtk4-rs 0.11 with `v4_16`, no libadwaita).

Labels: `OBSERVED` = what the Kotlin does. `Proposal:` = what the Rust port should do.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here

The anime player is the screen that plays one episode of a source's video. It is the Kotlin code under `app/src/main/java/eu/kanade/tachiyomi/ui/player/`, plus two local HTTP servers under `app/src/main/java/eu/kanade/tachiyomi/data/`. It covers:

- Opening an episode from an entry, restoring the saved position, and loading the stream into libmpv (`mpv` is the only playback engine).
- Choosing among several hosters (servers) and videos (qualities) per episode, including lazy URL resolution.
- Audio and subtitle track selection, chapters, and the AniSkip skip-intro button.
- Progress and history writes, completion marking, next/previous episode and autoplay, and download-ahead.
- Keyboard, gesture and media-key input, sleep timer, screenshots, share, and set-as-art.
- External player hand-off (Android intents in the original), the source-side local HTTP server, and the torrent engine (TorrServer).
- The `user-data/aniyomi` Lua bridge that lets mpv scripts and custom buttons drive the app.

### 1.2 Why it matters for a usable Kurobayomi

Watching anime is one of the two daily-use flows (the other is reading manga). Without this area there is no way to play a stream. The P0 subset (73 tasks, section 6) is the minimum for "watch one episode from one working source, keep progress, go to the next episode". Everything else here (torrents, mini-player, Lua custom buttons, stats overlay, PiP) is parity or long tail.

### 1.3 Scope split

Two documents own the Kotlin scope, split by file:

| Part | Kotlin files (scope) | Task ids | Doc prefix |
|---|---|---|---|
| Part 1 (shell, mpv, bridge, external, servers) | 10 files: `ui/player/PlayerActivity.kt` (1413 lines), `ui/player/AniyomiMPVView.kt` (290), `ui/player/ExternalIntents.kt` (610), `ui/player/PlayerEnums.kt`, `ui/player/PlayerObserver.kt`, `ui/player/PlayerUtils.kt`, `ui/player/PipActions.kt`, `data/player/service/HttpServerService.kt`, `data/torrent/service/TorrentServerService.kt`, `app/src/main/assets/aniyomi.lua` | `T-45a-001` to `T-45a-084` (84 tasks) | `p1-*` |
| Part 2 (view model, loaders, settings, skip, chapters, tracks) | 12 files: `ui/player/PlayerViewModel.kt` (2177 lines), `ui/player/loader/EpisodeLoader.kt`, `ui/player/loader/HosterLoader.kt`, six `ui/player/settings/*Preferences.kt`, `ui/player/utils/AniSkipApi.kt`, `ui/player/utils/ChapterUtils.kt`, `ui/player/utils/TrackSelect.kt` | `T-45b-001` to `T-45b-083` (80 tasks) | `p2-*` |

Out of scope but used here: the Compose controls and sheets under `ui/player/controls/` (about 50 files). They are the UI side and are covered by area:46-player-controls-ui. Settings screens are area:48-settings-player-and-scripts.

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Package or file | Responsibility | Scope |
|---|---|---|
| `ui/player/PlayerActivity.kt` | Window lifecycle (`onNewIntent`, `onCreate`, `onPause`, `onStop`, `onDestroy`), mpv setup orchestration (`setupPlayerMPV`), episode switch (`changeEpisode`, `setVideo`), `fileLoaded`, media session, audio focus, PiP, orientation, screenshots, torrent routing | part 1 |
| `ui/player/AniyomiMPVView.kt` | libmpv view subclass: `initOptions`, `observeProperties` (`observedProps`, 34 entries), `onKey`, track delegates | part 1 |
| `ui/player/PlayerObserver.kt` | mpv event and log callbacks, forwarded to the activity on the UI thread | part 1 |
| `ui/player/PlayerEnums.kt` | Shared enums and sealed UI state (`Decoder`, `Debanding`, `Sheets`, `Panels`, `Dialogs`, `PlayerUpdates`, `VideoFilters`, `PlayerOrientation`, `VideoAspect`, `SingleActionGesture`, `CustomKeyCodes`, `ArtType`, `SetAsArt`) | part 1 |
| `ui/player/ExternalIntents.kt` | Launch intents for external players, and writing progress back from their results | part 1 |
| `ui/player/PipActions.kt` | Android picture-in-picture remote actions and broadcast codes | part 1 |
| `ui/player/PlayerUtils.kt` | Converts `file`, `content`, `data` and protocol URIs into mpv-loadable strings | part 1 |
| `ui/player/PlayerViewModel.kt` | Central player state: playlist, hoster and video selection, tracks, chapters, skip-intro machine, progress, completion, gestures, volume, brightness, aspect, decoder, sleep timer, screenshots, Lua dispatch, custom buttons | part 2 |
| `ui/player/loader/EpisodeLoader.kt` | Hoster list per episode kind (downloaded, HTTP source, local) | part 2 |
| `ui/player/loader/HosterLoader.kt` | `selectBestVideo`, `getBestVideo` race, `getResolvedVideo`, hoster state machine (`HosterState`) | part 2 |
| `ui/player/settings/` (6 files: `PlayerPreferences`, `AudioPreferences`, `DecoderPreferences`, `GesturePreferences`, `SubtitlePreferences`, `AdvancedPlayerPreferences`) | Typed preference accessors and their defaults | part 2 |
| `ui/player/utils/AniSkipApi.kt` | AniSkip HTTP client (skip-intro timestamps), MAL id mapping | part 2 |
| `ui/player/utils/ChapterUtils.kt` | Chapter merge with AniSkip timestamps, chapter type labels | part 2 |
| `ui/player/utils/TrackSelect.kt` | Language, whitelist and blacklist track choice | part 2 |
| `ui/player/controls/**` | Compose UI for controls, panels, dialogs, sheets, gesture handler, seek bar, thumbnails | UI, area:46-player-controls-ui |
| `data/player/service/HttpServerService.kt` | Foreground service that runs a source's `HttpServer` (NanoHTTPD) on localhost | part 1 |
| `data/torrent/service/TorrentServerService.kt` | Foreground service that hosts the TorrServer engine (`xyz.secozzi.torrserver`, Go library) | part 1 |
| `app/src/main/assets/aniyomi.lua` | Lua bridge: 28 functions under the global `aniyomi`, each writing a `user-data/aniyomi/*` property | part 1 |

Cross-package helpers referenced by this area but owned elsewhere: `Video`, `Hoster`, `TimeStamp`, `ChapterType` (source-api), `TorrentServerApi`, `TorrentServerUtils`, `TorrentPreferences`, `CustomButton` (domain), `EpisodeSorter` (`domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt`).

### 2.2 Proposed Rust module map (unified)

The two part documents each propose a layout. They overlap and disagree on several names (see section 4, decision D-1). This is the unified layout this overview proposes. Module ownership of each Kotlin file is in `p1-rust-design.md` section 2.1 and `p2-rust-design.md` section 1.

```
src/player.rs                 GTK facade (existing mockup entry: pub fn player(...) -> GBox)
src/player/
  mod.rs                      re-exports; PlayerRequest; PlayerError
  enums.rs                    all PlayerEnums.kt types (part 1)
  types.rs                    Video, Hoster, HosterState, ChapterType, TimeStamp, ThumbnailInfo, TileInfo, VideoTrack (part 2)
  state.rs                    PlayerState: every observable field (replaces the mockup's fake state)
  controller.rs               window-level state machine (p1-behavior 1.12), trait PlayerModel (boundary), file_loaded
  session.rs                  init(), loadEpisode, playlist build, needs_init
  playlist.rs                 filter_episode_list, adjacent episode ids, sort via area:07
  actions.rs                  pause, seek, volume, aspect, double-tap dispatch, controls visibility
  loader/mod.rs, loader/episode.rs, loader/hoster.rs
  tracks.rs                   mpv track-list load, select_sub, add_audio/sub
  track_select.rs             TrackSelect (language, whitelist, blacklist, ICU display names)
  chapters.rs                 chapter list, merge_chapters, chapter type labels
  skip/mod.rs, skip/aniskip.rs  SkipIntroMachine; AniSkipClient (reqwest)
  progress.rs                 onSecondReached, save progress, history, completion, incognito gate, download-ahead, delete-after-read
  thumbs.rs                   seek preview tile cache (LRU 3), throttle 2 s, crop
  screenshot.rs               screenshot, save, share, set-as-art messages
  mpv.rs                      MpvHandle: safe wrapper over libmpv2 (create, options, props, commands, render)
  options.rs                  PURE: PlayerPrefs -> Vec<MpvSetting> (startup options)
  keys.rs                     PURE: gdk keyval + modifiers -> mpv key string
  observer.rs                 MpvEvent enum, 34-entry property table, dispatch
  bridge.rs                   user-data/aniyomi protocol parsing and dispatch (replaces Lua protocol and lua.rs)
  source_url.rs               PURE: file / data / protocol strings; basename helper
  external.rs                 ExternalPlayer: presets, argv builder (PURE), spawn, IPC progress poll
  window.rs                   GTK ApplicationWindow + GLArea + overlay host
  mini.rs                     mini-player window and its actions (P2)
  mpris.rs                    MPRIS server (P1)
  prefs.rs                    typed accessors over the settings store
  session_file.rs             process-restart snapshot (episode_id, position_ms, quality_index)
  error.rs                    PlayerError { kind, message_key }
src/services/mod.rs
src/services/local_server.rs  LocalServer (127.0.0.1, ephemeral port)
src/services/torrent.rs       TorrentServer lifecycle, TorrentApi client, play-link builder (PURE)
src/settings/player_*.rs      typed settings structs (area:02 owns the store; key strings and defaults from p2-data-and-config.md section 4)
assets/aniyomi.lua            kept only for custom-button scripts (P2)
```

Pure modules (no GTK, no libmpv, no I/O) carry the unit tests: `options.rs`, `keys.rs`, `bridge.rs` (parsing), `source_url.rs`, `external.rs` (argv), `torrent.rs` (link and magnet index), and `playlist.rs` / `skip` merge logic.

Engine and tooling choices (from `p2-rust-design.md` section 2): `libmpv2` (render API into `gtk::GLArea`), `reqwest` with rustls (shared client from area:12-network-stack), `serde` and `serde_json`, `tokio` with `tokio-util` `CancellationToken`, `async-channel` and `glib::spawn_future_local` for the main-thread bridge, `rusqlite` through the domain layer (area:03 to area:05), `icu_displaynames` and `icu_locale` (fallback `unic-langid`), `regex`, `chrono`, `tracing`. Versions must be checked on crates.io before adding each one; the design docs do not pin them.

---

## 3. Reading guide

All files are in `/home/blackbox/kurobayomi/plans/45-player-core/`.

| Doc | What it holds |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Part 1 file inventory: each of the 10 files, its purpose, port decision, target module, priority, task ids. |
| [p1-behavior.md](p1-behavior.md) | Part 1 behaviour spec: activity lifecycle, mpv init, loading, file-loaded orchestration, Lua protocol, observers, keys, audio. |
| [p1-data-and-config.md](p1-data-and-config.md) | Part 1 data: files and folders, models, preferences, golden mpv option list, default trackers, network endpoints, DB tables, message keys. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Part 1 edge cases, race conditions, failure modes, new Rust tests, fixtures, manual GL and mpv checks, known gaps. |
| [p1-rust-design.md](p1-rust-design.md) | Part 1 Rust design: the `PlayerModel` boundary trait, module layout, key types, threading, controller state machine, HW decoding, Android-to-Linux mapping, keyboard map, error and logging rules. |
| [p1-tasks.md](p1-tasks.md) | Part 1 tasks `T-45a-001` to `T-45a-084`, with priority, effort, dependencies, sources and acceptance criteria. |
| [p1-ui-parity.md](p1-ui-parity.md) | Part 1 UI parity: window, toasts, loading, PiP actions, keyboard and mouse, MPRIS, keep-awake, share dialogs, enum-driven states, stats overlay, gap list against the mockup. |
| [p1-topic-mpv-bridge.md](p1-topic-mpv-bridge.md) | How libmpv embedding, the `user-data/aniyomi` Lua protocol, key input and observers map to Rust. |
| [p1-topic-external-players.md](p1-topic-external-players.md) | External player hand-off on Linux: preset table (`mpv`, `vlc`, `celluloid`, `custom`, desktop default), progress feedback, failure modes. |
| [p1-topic-local-servers.md](p1-topic-local-servers.md) | Local source HTTP server and torrent engine: Android behaviour, Rust design, engine decision (TorrServer process vs librqbit), security notes. |
| [p2-inventory.md](p2-inventory.md) | Part 2 file inventory: the 12 files, every member of `PlayerViewModel.kt`, external symbols used and where they are owned. |
| [p2-behavior.md](p2-behavior.md) | Part 2 behaviour spec: view-model init, episode identity and playlist, hoster and video resolution, track handling, chapters and AniSkip, progress and completion, seeking and gestures, volume, aspect, decoder, sleep timer, Lua dispatch, screenshots, concurrency, timings and message keys. |
| [p2-data-and-config.md](p2-data-and-config.md) | Part 2 data: data models, enums quoted from part 1, anime and episode flag layouts, preference keys with defaults, endpoints, file formats, mpv properties and commands, DB columns, process-local state. |
| [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md) | Part 2 edge cases, failure table, race conditions, tests to write, fixtures, manual desktop test script. |
| [p2-rust-design.md](p2-rust-design.md) | Part 2 Rust design: module layout under `src/player/`, dependency choices, core types, source boundary, state store, repositories, concurrency model, mpv wrapper, algorithms as functions, Android-to-Linux mapping, session file, pitfalls. |
| [p2-tasks.md](p2-tasks.md) | Part 2 tasks `T-45b-001` to `T-45b-083`. |
| [p2-ui-parity.md](p2-ui-parity.md) | Part 2 UI surfaces driven by the view model, desktop input mapping, desktop-only surfaces, mockup gap list, accessibility, rendering edge rules. |
| [p2-topic-hoster-resolution.md](p2-topic-hoster-resolution.md) | `HosterState` per-slot state machine, selection rules table, concurrency, edge cases, open decisions. |
| [p2-topic-progress-and-completion.md](p2-topic-progress-and-completion.md) | The one-second tick, field-by-field writes, incognito matrix, completion side effects, download-ahead, shutdown, open decisions. |
| [p2-topic-skip-intro-and-chapters.md](p2-topic-skip-intro-and-chapters.md) | Where chapters come from, AniSkip end to end, the merge algorithm, the skip-intro state machine, labels, open decisions. |
| [00-overview.md](00-overview.md) | This document. |

Start with `p1-rust-design.md` section 1 and `p2-rust-design.md` section 1 for the contract between the two halves, then `p1-behavior.md` section 1 and `p2-behavior.md` section 0 for the global facts.

---

## 4. Key design decisions and open questions

Each decision lists the recommendation this overview makes. Where the part documents already state a default, it is marked "documented default".

### 4.1 Decisions that change the design

| ID | Question | Recommendation |
|---|---|---|
| D-1 | The two part documents propose overlapping module names with different meaning: `src/player/state.rs` (part 1) vs `model.rs` (part 2); `PlayerModel` is a trait in part 1 (`p1-rust-design.md` section 1) and a state struct in part 2 (`p2-rust-design.md` section 1); `progress.rs`, `mpv.rs`, `screenshot.rs` exist in both; `bridge.rs` (part 1) vs `lua.rs` (part 2); `window.rs` (part 1) vs `src/player.rs` facade (part 2). | Adopt the unified map in section 2.2. Name the state struct `PlayerState` (`state.rs`), and keep `trait PlayerModel` in `controller.rs` as the boundary part 2 implements. Single `bridge.rs`. Single `progress.rs` that holds the part-2 tick logic and the part-1 external writes. Blocks every P0 task that touches the state store, so decide before `T-45b-002` starts. |
| D-2 | Torrent engine: spawn a TorrServer (Go) process, or use `librqbit` in-process (`p1-topic-local-servers.md` section 3.2). | Documented recommendation: `librqbit` for the default path, TorrServer as a fallback for users who already run it. Both must produce the same play-link shape (`p1-data-and-config.md` section 6.1). Needed before `T-45a-073`. All torrent tasks (`T-45a-072` to `T-45a-079`) are P2, so this does not block daily use. |
| D-3 | libmpv handle lifetime: one per window (Kotlin creates one per activity, `p1-topic-mpv-bridge.md` section 3) or one per process. | One per window, as in Kotlin. Simpler teardown. Re-check if episode switching is slow; the Kotlin keeps one handle for each episode switch inside a window. |
| D-4 | Renderer preference `gpu` vs `gpu-next` (Kotlin sets `setVo(if (gpuNext) "gpu-next" else "gpu")`). | Keep the preference. Apply it to the libmpv render output, verify at runtime against the installed libmpv version, and fall back to the default renderer with a logged warning if rejected. Check in `p1-edge-cases-and-tests.md` section 7 item 4. |
| D-5 | Clear `user-data/aniyomi/*` after dispatch. Kotlin never clears it, so an identical repeated command does not re-fire. | Clear to an empty string after dispatch, and make the observer ignore empty values (`p1-topic-mpv-bridge.md` section 5). Needed so repeated custom-button presses work. |
| D-6 | Hoster in `ERROR` state: retry allowed or not (`p2-topic-hoster-resolution.md` section 7, E-08). | Documented default: no retry, Kotlin behaviour. Add a `hoster_retry` flag, off by default. |
| D-7 | Subtitle "Off" entry (E-31, `p2-topic-hoster-resolution.md` section 7). | Recommendation: add an explicit Off entry that maps to `sid=no`, so users can disable subtitles from the Subtitle Tracks sheet. Decide in the settings pass with area:48-settings-player-and-scripts. |
| D-8 | `pref_preserve_watching_position` (default false): whether to resume at the last position. Kotlin keeps `episode_position` in saved state for process death only (`p2-topic-progress-and-completion.md` section 10). | Documented default: keep the preference, implement resume only when it is true, and confirm with the user. The P0 resume rule is `T-45a-012`. |
| D-9 | History writes: write every second, or coalesce (E-47). | Coalesce to every 5 s, with a flush on pause and on close (`p2-topic-progress-and-completion.md` section 10). |
| D-10 | AniSkip on by default, or on when a MAL/AniList tracker is linked (`p2-topic-skip-intro-and-chapters.md` section 8). | Off by default, unchanged from Kotlin. |
| D-11 | `pip_replace_with_previous` (Android PiP) and whether it follows the chapter model. | Not in scope for Linux. Mini-player (`T-45a-048`, `T-45b-050`) gets transport controls only. Revisit after P1 is done. |
| D-12 | Local source server hardening: per-session token in the URL path so other local processes cannot read the stream. | Defer (P2). The server is loopback-only and ephemeral (`p1-topic-local-servers.md` section 4). Add later if the source handler serves sensitive data. |
| D-13 | Subtitle display names: `icu_displaynames` vs `unic-langid` fallback. | `icu_displaynames` plus `icu_locale`. Kotlin `Locale.getDisplayName` and `isO3Language` are replaced by the ISO 639-2 table from `icu_locale`. |
| D-14 | External player presets: which ship on Linux (`p1-topic-external-players.md` section 3). | Ship `default` (empty value, desktop handler via `gio`), `mpv`, `vlc`, `celluloid`, and `custom` with the placeholders `{url}`, `{title}`, `{position}`, `{subtitle}`, `{headers}`. MX Player, Just Player, Next Player, X Player, the mpv remote and Web Video Caster are dropped (no Linux build). |

### 4.2 Open questions that need the user

These are answered by the user, not by the documents.

1. D-2 (torrent engine). Is torrent playback wanted at all for daily use? If not, the whole `T-45a-072` to `T-45a-079` group can be cut.
2. D-6 and D-8 (hoster retry, resume position). Confirm defaults.
3. D-7 (subtitle Off). Confirm the entry should exist.
4. D-10 (AniSkip default). Confirm off.
5. Whether the Lua custom-button scripts (`T-45a-021`, P2) must keep working for existing user scripts, or whether a native custom-button editor is enough.

### 4.3 Items marked as conflicts or gaps in the part documents

- `p1-inventory.md` lists PipActions as P2 for tasks 047 and 048, but `T-45a-047` is P3 in `p1-tasks.md`. The task file is authoritative; treat the inventory row as stale.
- `p1-topic-mpv-bridge.md` section 8 lists Kotlin defects that are not carried over (for example `vf` overwrite between debanding and yuv420p, `trackId` `toInt()` exception, `FLAG`-stuck `isExiting`). The Rust port must not copy them. Fixes are in the referenced behaviour sections.
- `BaseMPVView` (external `com.github.aniyomiorg:aniyomi-mpv-lib:1.18.n`, `gradle/aniyomi.versions.toml:2`) is not in this repository. Its behaviour is inferred from call sites only (`p1-edge-cases-and-tests.md` section 8). Mpv behaviour checks are in section 7 of the same document.
- `Utils.PROTOCOLS` is external. Rust uses the runtime `protocol-list` property instead (`p1-edge-cases-and-tests.md` section 8).

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What this area needs | Blocks |
|---|---|---|
| area:01-foundation-app-bootstrap | Crate skeleton, app state, GTK init, tokio runtime | `T-45a-001`, `T-45b-074` |
| area:02-preferences-system | Typed settings store and key strings; message keys and localisation | `T-45b-048`, `T-45b-061` |
| area:05-data-repos-entries-items | `UpdateEpisode`, `GetEpisodesByAnimeId`, episode row writes (progress, seen) | `T-45b-026`, `T-45b-003` |
| area:06-data-repos-history-track-source-misc | `UpsertAnimeHistory` and incognito state for history writes | `T-45a-063`, `T-45b-028` |
| area:07-domain-entries-items | `GetNextEpisodes`, episode sort (`EpisodeSorter`), episode flags, `SetAnimeViewerFlags` | `T-45b-004`, `T-45b-005`, `T-45b-006` |
| area:08-domain-library-category-history-updates | `markDuplicateSeenEpisodeAsSeen` and `MARK_DUPLICATE_EPISODE_SEEN_EXISTING` | `T-45b-027` |
| area:12-network-stack | Shared `reqwest::Client` for AniSkip and the torrent API | `T-45b-022` (AniSkip request path), `T-45a-072` to `T-45a-073` |
| area:13-core-util-storage-archive-torrent | `DiskUtil`, cache image directory, torrent helpers | `T-45a-068`, torrent tasks |
| area:16-source-api-anime and area:17-source-app-layer | `getHosterList`, `getVideoList`, `getVideoUrl`, `resolveVideo`, `getVideoThumbnails`, `getImageTile`, `sortHosters`, `sortVideos`, `isInitialized`, `createHttpServer` | `T-45b-001`, `T-45b-007`, `T-45b-008`, `T-45b-012`, `T-45a-010` |
| area:24-downloads-anime and area:25-downloads-shared | `isEpisodeDownloaded`, `buildVideo`, `enqueueEpisodesToDelete`, `deletePendingEpisodes`, download-ahead de-duplication | `T-45b-007`, `T-45b-064`, `T-45b-027` |
| area:27-tracking-core, area:28-tracker-anilist, area:29-tracker-myanimelist | `GetAnimeTracks`, `TrackEpisode`, `TrackerManager`, MAL id mapping for AniSkip | `T-45b-027`, `T-45a-040` |
| area:36-ui-shell-navigation-security-webview | Window routing, navigation into the player | `T-45b-002`, `T-45b-075` |
| area:43-reader-core | `SaveImageNotifier` (save and share notification pattern) | `T-45b-050`, screenshot tasks |
| area:34-images-covers-cache | Set-as-art targets (cover, background, thumbnail) | `T-45a-051` |
| area:45-player-core (part 1 and part 2) | Each half needs the other (`PlayerModel` trait, `p1-rust-design.md` section 1). | All of `T-45a-011` and `T-45b-073` |

### 5.2 What others need from this area

| Area | What it needs from area:45-player-core |
|---|---|
| area:46-player-controls-ui | `PlayerState` store and change notification (`T-45b-002`), overlay bindings (`T-45b-075`), sheets and panels binding (`T-45b-078`), enum types (`T-45a-001`) |
| area:48-settings-player-and-scripts | Preference keys and defaults (`p2-data-and-config.md` section 4), gesture and decoder catalogue (`T-45b-047`, `T-45b-048`), Lua script import (`T-45a-027`) |
| area:38 to area:42 (entries, browse, updates, history screens) | `newIntent(animeId, episodeId, hostList, hostIndex, vidIndex)` equivalent: a single entry point to open the player for an episode (`T-45a-038`) |
| area:53-platform-manifest-intents-workers | External launch and intent replacement: the external player preset path (`T-45a-055` to `T-45a-062`) and the Linux equivalents of the Android service model (`T-45a-080`, `T-45a-081`) |
| area:24-downloads-anime | Local file playback path for downloaded episodes (`T-45a-068`, `T-45b-064`) |
| area:27 to area:29 (trackers) | Episode progress and completion events (`T-45b-027`) |

Note on tokens: `area:<id>` references in prose in the behaviour documents (for example `area:07` and `area:13`) are shorthand; the dependency fields in every task file use the full area ids and were checked against the directory names in `/home/blackbox/kurobayomi/plans`.

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export` and rows of `/home/blackbox/kurobayomi/plans/TASKS.csv` whose `file` starts with `45-player-core/`. Both task files pass `tasks.py lint` (`ok`).

### 6.1 Counts

| Scope | Total | P0 | P1 | P2 | P3 | S | M | L | XL |
|---|---|---|---|---|---|---|---|---|---|
| Part 1 (`p1-tasks.md`, T-45a) | 84 | 34 | 25 | 17 | 8 | 54 | 29 | 1 | 0 |
| Part 2 (`p2-tasks.md`, T-45b) | 80 | 39 | 35 | 3 | 3 | 41 | 30 | 9 | 0 |
| Area total | 164 | 73 | 60 | 20 | 11 | 95 | 59 | 10 | 0 |

The area has no XL tasks. The 10 L tasks are: `T-45a-002` (libmpv handle and GL render context, P0), `T-45b-002` (state store, P0), `T-45b-007` (hoster list per episode kind, P0), `T-45b-013` (`load_hosters` concurrency, P0), `T-45b-074` (render context on GtkGLArea, P0), `T-45b-075` (surface and overlay, P0), `T-45b-078` (sheets and quality binding, P0), `T-45b-022` (skip-intro state machine, P1), `T-45b-035` (Lua user-data dispatcher, P1), and `T-45b-050` (mini-player, P2).

### 6.2 P0 critical path (in dependency order)

Each step lists the tasks that must be done before the next. Tasks in the same step can run in parallel.

1. Foundation: `T-45a-001` (module skeleton and UI state model, S) and `T-45b-001` (player domain types, M). Depends on area:01 and on the source-api types (area:16, area:17).
2. libmpv and state: `T-45a-002` (libmpv handle and GL render context, L) and `T-45b-002` (state store, L). `T-45b-002` also needs `T-45b-001` and area:36.
3. Options and observation: `T-45a-003` (startup option table, M), then `T-45a-004` (decoder and video output), `T-45a-005` (cache, TLS, log level), `T-45a-026` (`mpv.conf` and `input.conf` generation), `T-45a-028` (bundled `subfont.ttf` and `cacert.pem`), `T-45a-030` (subtitle options), `T-45a-031` (audio options), `T-45a-033` (speed presets). In parallel: `T-45a-006` (property observation table, M) and `T-45a-007` (event marshalling to GTK main loop, M), then `T-45a-009` (playback error reporting, S).
4. Source to mpv: `T-45a-010` (URI resolution, S) and `T-45b-004` (playlist build and sort, S), `T-45b-005` (playlist filter, M), `T-45b-006` (adjacent episodes, S).
5. Session init: `T-45b-003` (session init and episode resolution, M). Needs `T-45b-002`, `T-45b-004`, `T-45b-005`, `T-45b-007`, `T-45b-030`, area:05 and area:07.
6. Hosters and videos: `T-45b-007` (hoster list per episode kind, L), `T-45b-008` (has_hoster_list capability, S), `T-45b-009` (video list and sort, M), `T-45b-010` (`select_best_video`, S), `T-45b-012` (resolve video, S), `T-45b-013` (`load_hosters` concurrency and automatic selection, L), `T-45b-014` (`load_video` state machine, M), `T-45b-015` (video and hoster click, M).
7. Load into mpv: `T-45a-011` (loadfile with start position, M), `T-45a-012` (resume start rule, S), `T-45a-014` (HTTP header passthrough, S), `T-45a-018` (media title, S), `T-45a-052` (initial load error path, S), then `T-45a-084` (`fileLoaded` orchestration order, M), which needs `T-45a-011`, `T-45a-013`, `T-45a-015`, `T-45a-016`, `T-45a-017`, `T-45a-018`.
8. Tracks: `T-45b-016` (track list and default selection, M) and `T-45b-017` (TrackSelect, M), and `T-45a-015` (track selection, M).
9. Lifecycle and progress: `T-45a-039` (lifecycle, `isExiting` gate and shutdown order, M), `T-45a-040` (progress persistence on pause and exit, M), `T-45b-026` (progress save and resume, M), `T-45b-027` (completion side effects, M), `T-45a-063` (history with incognito gate, M, needs area:06), `T-45a-064` (completion threshold, S), `T-45b-028` (history upsert, S), `T-45b-030` (process-restart snapshot, S).
10. Transport and episode switch: `T-45a-019` (transport and seek events, M), `T-45a-036` (episode switching and autoplay, M), `T-45a-037` (next-episode and empty-hoster messages, S).
11. Input and gestures: `T-45a-024` (keyboard to mpv key names, M), `T-45a-025` (window key bindings and media-key codes, S), `T-45b-023` (seek primitives and double-tap accumulator, M), `T-45b-024` (gesture dispatch, S), `T-45b-051` (controls visibility, M).
12. Surface: `T-45b-073` (mpv observer set and whole-second channel, M), then `T-45b-074` (render context on GtkGLArea, L), then `T-45b-075` (player surface and overlay, L), then `T-45b-078` (sheets, panels and quality binding, L).
13. Verification: `T-45b-062` (player core test suite and fixtures, M), needs `T-45a-010`, `T-45b-017`, `T-45a-019`, `T-45b-022`, `T-45b-005`, `T-45b-023`, `T-45b-026`, `T-45b-027`.

The longest chain is roughly: `T-45a-002` to `T-45a-007` to `T-45a-011` to `T-45a-084`, joined with `T-45b-013`, `T-45b-074`, `T-45b-075`, `T-45b-078`. The two L GL tasks (`T-45a-002` and `T-45b-074`) are the single biggest risk on that path.

### 6.3 Work outside this area that P0 depends on

P0 is not reachable without area:05, area:06, area:07, area:16, area:17, area:24, area:25, area:27 and area:36 being at least partly done. Plan these first or stub the interfaces.

---

## 7. Risks

1. GL embedding. libmpv render API inside a `gtk::GLArea` on GTK 4.16 under both Wayland and X11 (`T-45a-002`, `T-45b-074`) is the single highest-risk item. Mitigation: a spike that plays one local file before any UI work.
2. libmpv binding maturity. The `libmpv2` crate version and its render feature are not pinned in the docs (`p2-rust-design.md` section 2). Verify on crates.io, and keep `MpvHandle` as the only place that calls the crate, so it can be swapped.
3. Unknown external library. `BaseMPVView` (aniyomi-mpv-lib 1.18.n) is not in the repository, so mpv option behaviour is inferred from call sites (`p1-edge-cases-and-tests.md` section 8). Compare with a real mpv for the golden option list (`p1-edge-cases-and-tests.md` section 5.1).
4. Two-author drift. The part documents disagree on module names (decision D-1). Left unresolved, the first two P0 tasks will collide on `state.rs`, `model.rs` and `progress.rs`.
5. Cross-area blocking. Almost every P0 task depends on another area's data layer or source API. The area cannot be shipped alone.
6. Torrent scope. TorrServer is Go, so the in-process option (`librqbit`) is a new dependency with its own stream and range behaviour, which the docs do not test. Keep it P2 until D-2 is answered.
7. Copied defects. The Kotlin has known defects (`p1-topic-mpv-bridge.md` section 8, `p2-behavior.md` section 6 and 16). Each must be fixed in the port, not copied, and each fix needs a test.
8. Behavioural dependence on the system. Subtitle display names (`icu`), desktop default app selection (`gio`), and the `protocol-list` property differ by distribution. Test on the CachyOS KDE target.

---

## 8. Definition of done for this area in Kurobayomi

Observable behaviour, checkable by hand on the target machine (CachyOS, KDE Plasma, Wayland and X11). Each item cites the task or document that carries the detail.

1. From an entry's episode list, choosing an episode opens a player window. The episode plays in libmpv inside the window, with video and audio, from the start position computed by the resume rule (`T-45a-012`).
2. Play and pause (Space, or the on-screen button) toggles playback. Keyboard seek keys and the rest of the keymap follow `p1-rust-design.md` section 8 (Proposal). Gesture seek amounts come from the preferences catalogue (`T-45b-047`).
3. The seek bar shows the current position and duration, updated once per whole second (`T-45b-073`). Dragging the seek bar seeks.
4. With several hosters or videos, the Quality sheet lists them, selecting one loads the new stream (`T-45b-014`, `T-45b-015`). An unresolvable hoster shows its error message key (`T-45b-061`).
5. Audio and subtitle sheets list the tracks mpv reports. Choosing a track changes it. Default track selection follows `TrackSelect` (`T-45b-016`, `T-45b-017`).
6. Chapters are listed and the current chapter is shown. Chapter seek works (`T-45b-018`).
7. When AniSkip is enabled in settings (off by default, D-10), a skip-intro button appears during the intro and skips it (`p2-topic-skip-intro-and-chapters.md`).
8. Progress: after playing for a while and closing the window, reopening the episode resumes at the saved position. History lists the episode with the last position. Writes coalesce to every 5 s with a flush on pause and close (D-9). Incognito follows the matrix in `p2-topic-progress-and-completion.md` section 4: with per-source incognito on and no tracker, neither progress nor history is written; with a tracker linked, progress is written but history is not (`T-45a-063`, `T-45b-028`).
9. Completion: an episode past the completion threshold is marked seen, and the duplicate-marking side effect runs when its preference is set (`T-45a-064`, `T-45b-027`).
10. Next and previous episode work from the controls, and autoplay on end of file opens the next episode. With no next episode, the message `T-45a-037` is shown.
11. Killing the app mid-episode and relaunching restores the episode and position from the process-restart snapshot (`T-45b-030`).
12. Playing a downloaded episode works offline from the local file, with no network call (`T-45a-068`, `T-45b-064`).
13. Playback speed presets, aspect modes (crop, fit, stretch), volume with mpv boost, and brightness via the mpv filter work from the controls (`T-45b-031` to `T-45b-033`, `T-45b-038`).
14. Screenshot saves to the configured folder and shows a notification. Share copies or shares the image (`T-45a-050`, `T-45b` screenshot tasks).
15. External player: with the `mpv` preset chosen in settings, an episode opens in a separate mpv process at the saved position, with headers and subtitle passed as arguments. A missing binary shows `Player not found: <program>` and does not spawn (`p1-topic-external-players.md` sections 2 and 3).
16. Source HTTP server: a source that needs `createHttpServer` plays through `http://127.0.0.1:<ephemeral-port>` (`T-45a-069` to `T-45a-071`).
17. Lifecycle: closing the window stops mpv, saves progress, and releases the handle. Quitting the app stops any local server (`T-45a-039`, `T-45a-080`).
18. Mouse and keyboard focus work on KDE. Fullscreen toggles with the key bound in `p1-rust-design.md` section 8 (`T-45a-044`).
19. The Linux player has no user-visible Android-only features: no PiP, no orientation lock, no audio focus policy UI (`T-45a-035`, `T-45a-042`, `T-45a-047` are P3 drops, and `T-45b-083` documents each loss).
20. P2 and P3 items are optional for this definition. Torrent playback, Lua custom buttons, mini-player and stats overlay are not required for "usable daily".

Acceptance of the area is: items 1 to 12 and 15 pass on the target machine, with `cargo test` passing for the pure modules listed in section 2.2.
