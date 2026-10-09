# Topic: external players

Scope files: `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt` (610 lines), plus the external launch path in `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:600-660` (outside scope, cited for the entry point). Behaviour: `p1-behavior.md` section 16. Data: `p1-data-and-config.md` sections 2.7, 3.1, 7.

## 1. What the Android side does

Entry: `MainActivity.startPlayerActivity(..., extPlayer = true, ...)` (`MainActivity.kt:612-630`):

1. Start the local HTTP server (`startHttpServerService`, 5 s timeout) unconditionally for the external path.
2. Build the intent with `ExternalIntents.newIntent(context, animeId, episodeId, video)`.
3. Launch it through `externalPlayerResult` (an `ActivityResultLauncher`).

Intent building (`ExternalIntents.getExternalIntent`, behaviour 16.1):

- Empty package: a generic `ACTION_VIEW` with a URI and MIME type from the extension, plus extras `title`, `position`, `return_result`, `secure_uri` (added only for supported players), `headers` and (non-supported players only) `http-header-fields`.
- Named package: `getIntentForPackage` → Web Video Caster special case or the standard path with a component name from the table in behaviour 16.6.

Result (`onActivityResult`, behaviour 16.9): reads `end_by`, `position`, `duration`, `extra_position`, `extra_duration`, saves progress and history.

## 2. What Linux does instead

Android intents do not exist. The Linux player hand-off is a command line, and the external player does not return a result. The replacement (T-45a-055 to T-45a-062):

1. Resolve the chosen preset from `external_player_preference` (string key). Empty value means the desktop default app.
2. Build argv (pure function, unit tested).
3. Spawn detached with `std::process::Command`, stdio to null, new process group (`CommandExt::process_group(0)`).
4. For mpv only, add `--input-ipc-server=<socket>` and poll the progress every 5 s (T-45a-061).

## 3. Preset table (Linux, proposal)

| Preference value | Program | Argv pattern (proposal) | Position argument | Subtitle | Headers |
|---|---|---|---|---|---|
| `""` (empty) | desktop default for the URI | `gio open <file-or-url>` via `gio::AppInfo::launch_default_for_uri` | none | none | none |
| `mpv` | `mpv` | `mpv --title=<title> --start=<seconds> --http-header-fields=<h> --sub-file=<url> --input-ipc-server=<sock> <url>` | `--start=<s>` (from ms / 1000) | `--sub-file=<url>` (locale match) | `--http-header-fields=` joined with `,` |
| `vlc` | `vlc` | `vlc --meta-title=<title> --start-time=<s> --http-referrer=<referer> --sub-file=<url> <url>` | `--start-time=<s>` | `--sub-file=<url>` | header `User-Agent` / `Referer` only (VLC has per-option headers) |
| `celluloid` | `celluloid` | `celluloid --mpv-options=start=<s>,... <url>` | `start=<s>` in mpv options | `--mpv-options=sub-file=<url>` | `--mpv-options=http-header-fields=...` |
| `custom` | user template | user-defined, placeholders `{url}`, `{title}`, `{position}`, `{subtitle}`, `{headers}` | `{position}` seconds | `{subtitle}` | `{headers}` as `Key: value,...` |

Rules:

- Positions are seconds (`f64` with three decimals) in the argv for all presets; the internal value is milliseconds.
- Titles are one argv element each. No shell.
- If the binary is not on `PATH` (`which`-style lookup over `PATH`), return the error `Player not found: <program>` and do not spawn (EC-85).
- Subtitle selection by system locale (behaviour 16.5): the track whose language contains the locale's simple display name (case-insensitive) is preferred; else the first track.

Android package constants are kept only as documentation of the upstream list (not used for launch):

| Android package | Name | Linux status |
|---|---|---|
| `is.xyz.mpv` | mpv (Android) | replaced by `mpv` preset |
| `com.mxtech.videoplayer`, `.ad`, `.pro` | MX Player | not available on Linux; dropped |
| `org.videolan.vlc` | VLC (Android) | replaced by `vlc` preset |
| `live.mehiz.mpvkt`, `.preview` | mpvKt | replaced by `mpv` preset |
| `com.husudosu.mpvremote` | mpv remote | dropped |
| `com.brouken.player` | Just Player | dropped |
| `dev.anilbeesetti.nextplayer` | Next Player | dropped |
| `video.player.videoplayer` | X Player | dropped |
| `com.instantbits.cast.webvideo` | Web Video Caster | dropped (T-45a-060) |

## 4. Progress feedback (mpv only)

- The Rust process starts mpv with `--input-ipc-server=$XDG_RUNTIME_DIR/kurobayomi-mpv-<pid>-<n>.sock`.
- Every 5 s it sends `{"command":["get_property","time-pos"]}` and `duration` over the socket (JSON IPC protocol of mpv).
- On process exit, the last `time-pos` and `duration` are saved with the same rules as `onActivityResult`:
  - If the position is within 1 s of the duration, cause `playback_completion`: save total as position (behaviour 16.9 step 5).
  - Else save position and duration.
  - History is written (behaviour 16.10).
- Other players: no progress is recorded (documented loss).

Known gap: a player that is killed before the first poll (EC-91) leaves progress unchanged.

## 5. Database writes after an external session

Same code path as the internal player (`progress.rs`, behaviour 16.10):

- `UpdateEpisode` with `seen`, `bookmark`, `fillermark`, `lastSecondSeen`, `totalSeconds`.
- `UpsertAnimeHistory` with `Date()`.
- Incognito gate.
- Completion threshold `pref_progress_preference` (default 0.85).
- Delete-after-watch (`remove_after_read_slots`).
- Tracker update (`pref_auto_update_manga_sync_key`, default true) and offline queue.

## 6. Local files and downloads

- Local source (`LocalAnimeSource.ID`) or downloaded episode: path passed as is (no FileProvider), T-45a-068.
- Android `FileProvider` authority `${applicationId}.provider` and `provider_paths.xml` roots are dropped.

## 7. Online episodes through a local server

When the resolved video `usesHttpServer()`, the URL is rewritten to `http://127.0.0.1:<port>` (T-45a-070). The external player talks to the local server; the server exists only for the duration of the external session (stopped on player exit or app exit). See `p1-topic-local-servers.md`.

## 8. Failure modes

| Case | Result |
|---|---|
| No video URL | error pill `Video URL is empty.` (Kotlin text) |
| No video list | error pill with `Video list is empty` |
| Preset missing | error pill `Player not found: <program>` (proposal text) |
| Spawn fails | error pill with the OS error |
| Server start fails | error pill `Failed to start http server` |
| Progress DB fails | logged; toast proposal `Could not save progress` |

## 9. Tests

- Argv builders: section 5.5 of `p1-edge-cases-and-tests.md`.
- Start position rule: 5.5 table.
- Subtitle locale match: 5.5.
- Progress writes: 5.6.
- Manual check: section 7 items 9.
