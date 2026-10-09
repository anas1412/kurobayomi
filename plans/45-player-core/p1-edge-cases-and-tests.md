# Area 45 (player core, part 1): edge cases, failure modes, races, and tests

References: behaviour sections are in `p1-behavior.md`; config in `p1-data-and-config.md`; tasks in `p1-tasks.md` (IDs `T-45a-NNN`).

---

## 1. Upstream tests

Searched for `*Test*.kt` that reference any scope class (`PlayerActivity`, `ExternalIntents`, `HttpServer`, `copyHttpServer`, `usesHttpServer`, `TorrentServer*`, `PlayerObserver`, `AniyomiMPVView`, `PipActions`, `PlayerUtils`, `MPV_ARGS_TAG`, `getLastSecondSeen`, `ANIYOMI_MPV`): **none**. Test directories under `app/src/test` contain no player tests.

Adjacent upstream test (not in scope, owned by area 13, relevant to torrent play links):

- `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt`: parses two fixtures and checks `Torrent(title, hash, size, trackers, fileStats)`.
  - `test-torrent-file.torrent`: single file, `FileStats(null, "test-torrent-file", 18)`, hash `104701de4b710ee9720ee249bf220384659e8c38`.
  - `test-torrent-directory.torrent`: four files with ids 1-4: `FileStats(1, "3/3.1", 4)`, `(2, "2", 2)`, `(3, "1", 2)`, `(4, "3/3.2", 4)`, hash `863eb76f1cd0643d9fd2eb4a7da866c3752991b5`.
  - Use both fixtures in the play-link tests (section 5, `play_link_*`), since the file id and basename logic of `getTorrentPlayLink` depends on them.

Port decision: the fixtures and expected values are copied into `tests/fixtures/torrent/` (Rust), the test body is re-written in Rust for `services::torrent::play_link`.

---

## 2. Edge cases (catalogue)

Format: ID, situation, expected behaviour, Kotlin reference, task.

### 2.1 Lifecycle and intent

- **EC-01** `animeId` or `episodeId` missing (-1): window closes. Ref behaviour 1.2 step 2. T-45a-038.
- **EC-02** Second episode opened while the window is open: reuse the window; save progress of the previous episode first (`saveCurrentEpisodeWatchingProgress`). Ref 1.2 step 4. T-45a-038, T-45a-040.
- **EC-03** `onNewIntent` during an in-flight `init` for the previous episode: the new init wins; the stale result must not overwrite state. Kotlin has no cancellation (`launchNonCancellable`); Rust uses a generation counter and drops stale results. T-45a-038.
- **EC-04** `init` returns `Result.failure`: window closes with the exception text; `loadHosters` is not called (Kotlin calls it with `!!` on `currentSource`, possible NPE). Ref 1.2 steps 5c-5e. T-45a-052.
- **EC-05** `hostList` string extra is empty: `initResult.first.hosterList` falls back to empty list. Ref 1.2 step 1. T-45a-011.
- **EC-06** `onPause` during PiP: no `pause`, no `stop`, playback continues. Ref 1.6 step 2. T-45a-040.
- **EC-07** Window closed (`isFinishing`) while playing: `deletePendingEpisodes()` then `stop`. Ref 1.6 step 4. T-45a-040.
- **EC-08** `onDestroy` before the mpv event loop drains: events after destroy are ignored (`isExiting = true` is set first). Ref 1.8 step 1. T-45a-039.
- **EC-09** `onResume` with `isExiting` false: no volume change. Ref 1.5. T-45a-046.
- **EC-10** Screen rotation or configuration change: `isChangingConfigurations` true, no `onSaveInstanceNonConfigurationChange`. Ref 1.10. Desktop: no-op (no rotation).

### 2.2 mpv initialisation and options

- **EC-11** `pref_gpu_next` true: `vo` = `gpu-next` (Kotlin) → Linux render path needs verification (see `p1-rust-design.md` section 9.1). T-45a-003.
- **EC-12** `pref_video_debanding = CPU` and `use_yuv420p = true`: the Kotlin overwrites `vf`. Rust result must be `gradfun=radius=12,format=yuv420p`. T-45a-004.
- **EC-13** `pref_video_debanding = GPU`: option `deband` = `yes`; no `vf` change. T-45a-004.
- **EC-14** `verbose_logging = true`: `msg-level` = `all=v`; log observer level `info` (`PlayerActivity.kt:425`). T-45a-005, T-45a-008.
- **EC-15** `pref_player_speed` stored as float with no fractional part (e.g. `2`): option string is `2.0`. T-45a-033.
- **EC-16** `pref_audio_delay` = `-250` (ms): option `audio-delay` = `-0.25`. T-45a-031.
- **EC-17** `pref_audio_volume_boost_cap` = 200: `volume-max` = `300`; `volumeBoostCap` observed `value - 100` = 200. T-45a-031.
- **EC-18** `pref_subtitles_font_size` = 0 or negative: the Kotlin passes the value unchanged. Proposal: clamp to 1..200 in the Rust settings UI (not in this scope). T-45a-030.
- **EC-19** Colour with alpha: `#AARRGGBB` written with `toHexString` of the signed Int (e.g. `-1` → `ffffffff`). Rust uses `format!("#{:08X}", argb as u32)`, identical output. T-45a-030.
- **EC-20** `pref_sub_pos` above 100: passed as-is (mpv accepts 0..150). T-45a-030.
- **EC-21** `pref_override_subtitles_ass = true`: `sub-ass-override=force` and `sub-ass-justify=yes`. T-45a-030.
- **EC-22** Cache limit: `demuxer-max-bytes` and `demuxer-max-back-bytes` both 64 MiB. T-45a-005.
- **EC-23** `pref_mpv_conf` contains `vo=...` or `hwdec=...`: user values in `mpv.conf` override Rust defaults only if mpv reads the file after the options. Document the precedence: options set through the API win over `mpv.conf` (mpv applies API options first). Proposal: write the user file and document that API defaults are overridden by `mpv.conf` only for keys not set by the API. T-45a-026.
- **EC-24** `pref_mpv_conf` invalid syntax: mpv logs an error and continues with defaults. No Rust-side validation. T-45a-026.
- **EC-25** `mpv_scripts = false`: the three script directories are still deleted (clean slate). Ref 2.5 step 2. T-45a-027.
- **EC-26** `mpv_scripts = true` and the user directory is missing: `listFiles()` returns null → no copy, no error. T-45a-027.
- **EC-27** `subfont.ttf` already present with the same length: not copied. T-45a-028.
- **EC-28** Asset copy fails (disk full): log error, continue. T-45a-028.
- **EC-29** Font directory is empty or missing: `sub-fonts-dir` still set to the (empty) directory. T-45a-029.

### 2.3 Loading a video

- **EC-30** `loadfile` with `video == null`: no command. Ref 3.1 step 1. T-45a-011.
- **EC-31** Local file path with spaces, Unicode, or a leading `-`: the path is passed as a single argument, not split. Rust `command(&["loadfile", path, "replace", "0", opts])` (no shell). T-45a-010, T-45a-011.
- **EC-32** `data:` URI containing `,` or `;`: `schemeSpecificPart` kept intact. T-45a-010.
- **EC-33** Protocol URL (`http`, `https`, `rtmp`, `udp`, `ftp`, ...): passed through unchanged. The protocol list is taken from mpv at runtime (`protocol-list` property) in Rust. T-45a-010.
- **EC-34** `content://` URI on Linux (from a Kotlin-sourced local source or a backup): `unknown scheme: content`, no load. T-45a-010, T-45a-081.
- **EC-35** `resumePosition` 0 and `seen` true with `preserve_watching_position` false: start 0. T-45a-012.
- **EC-36** `preserve_watching_position` true and seen and last position = total: start at total, mpv reaches end immediately and `eof-reached` fires; with autoplay off nothing else happens. T-45a-012.
- **EC-37** Episode reloaded (same episode, not loading): start = current `time-pos`. If `time-pos` is null the start is not set. T-45a-012.
- **EC-38** HTTP header value containing `,`: escaped as `\,`. T-45a-014.
- **EC-39** Header with duplicate key: first value kept. T-45a-014.
- **EC-40** Offline episode: no `http-header-fields`. T-45a-014.
- **EC-41** `Video.mpvArgs` value containing `"`: produces broken option string in Kotlin. Rust escapes it. T-45a-011.
- **EC-42** `Video.mpvArgs` empty: `videoOptions` = empty string, `loadfile` gets an empty options argument. T-45a-011.
- **EC-43** Metadata map missing `ANIYOMI_MPV_ARGS`: no-op. T-45a-013.
- **EC-44** Metadata entry without `=` (e.g. `vo`): Kotlin aborts the rest; Rust skips that entry and logs at warn. T-45a-013.
- **EC-45** Metadata entry with `=` inside the value (e.g. `a=b=c`): key `a`, value `b=c` (split with limit 2). T-45a-013.
- **EC-46** Metadata JSON invalid: log `Failed to read video metadata`, playback continues. T-45a-013.

### 2.4 Tracks, chapters, titles

- **EC-47** `aid` property returns `auto` (not yet selected): ignored. T-45a-015.
- **EC-48** `sid` returns `no`: track getter returns -1, UI shows subtitles off. T-45a-015.
- **EC-49** `sid` returns a non-integer string (e.g. `"abc"`): `toIntOrNull` gives -1 (no crash). Kotlin `trackId` (not the delegate) would throw `NumberFormatException` on `toInt()`. Rust returns "no change". T-45a-015.
- **EC-50** External subtitle with empty language: `sub-add <url> auto ` (empty lang). Kotlin passes it through. Rust passes the empty string. T-45a-016.
- **EC-51** Empty audio and subtitle track lists: `onFinishLoadingTracks` called directly (`PlayerActivity.kt:1346-1348`). T-45a-016, T-45a-084.
- **EC-52** Timestamp with empty name and type `Other`: name stays empty. T-45a-017.
- **EC-53** Timestamp with empty name and type `Opening`: localised `ChapterType` name. T-45a-017.
- **EC-54** Episode number 1.5: media title `E1.5`; 12.0: `E12`; 7: `E07`. T-45a-018.
- **EC-55** Anime title containing `%`: passed as a format argument (no printf injection in Rust, since the message is built with `format!`). T-45a-018.
- **EC-56** `chapter-list` observed before `FILE_LOADED`: `loadChapters` runs with no chapters. Part 2 handles; part 1 forwards. T-45a-084.

### 2.5 Observers and events

- **EC-57** `eof-reached=true` with autoplay off: nothing. T-45a-036.
- **EC-58** `eof-reached=true` with autoplay on and no next episode: `Next Episode not found!` is suppressed (`autoPlay=true`). T-45a-037.
- **EC-59** `MPV_EVENT_SEEK` during episode load: `isLoading` true until cleared by part 2. T-45a-019.
- **EC-60** `pause` flag event when `player.paused` disagrees with the event value: the double check suppresses the action. T-45a-019.
- **EC-61** `video-params/aspect` below 0.001 (audio-only file): `getVideoOutAspect` returns 0; orientation/PiP Rational must not divide by zero. T-45a-034, T-45a-048.
- **EC-62** PiP Rational with `width.toInt() == 0`: Kotlin `Rational` throws `ArithmeticException`. Rust: skip the aspect hint when width is 0. T-45a-048.
- **EC-63** `video-params/h` is absent: no PiP aspect hint. T-45a-048.
- **EC-64** Log line containing `HTTP error`: stored; appended to the next end-file error only once (cleared after use). T-45a-008, T-45a-009.
- **EC-65** End-file error with no message and no stored HTTP error: `Error: File ended`. T-45a-009.

### 2.6 Keys and input

- **EC-66** Key auto-repeat (`repeatCount > 0`): swallowed, not sent. T-45a-024.
- **EC-67** Dead key (combining accent): not sent. T-45a-024.
- **EC-68** Modifier-only press (Shift alone): not sent. T-45a-024.
- **EC-69** `ACTION_MULTIPLE` (Android legacy): ignored. Rust: no equivalent; ignore.
- **EC-70** Unmapped non-printable key: not sent; debug log only on first press (`repeatCount == 0`). T-45a-024.
- **EC-71** `onKeyUp` with null event: Kotlin NPE (`event!!`). Rust: no event, no action. T-45a-025.
- **EC-72** Space while a text field has focus (part 2 search): key handling must not steal it. Proposal: player key controller checks focus owner. T-45a-025.
- **EC-73** Modifier order: `shift+ctrl+alt+meta+KEY`. T-45a-024.

### 2.7 Audio, focus, noisy output (dropped or P2)

- **EC-74** Audio focus loss on Linux: no event. Documented loss. T-45a-042.
- **EC-75** Default output sink removed while playing: pause and release keep-awake (P2). T-45a-082.
- **EC-76** `pref_audio_config` stored as unknown name: `getEnum` returns the default `AutoSafe`. T-45a-031.

### 2.8 PiP, orientation, mini-player

- **EC-77** Mini-player open and main window closed: mini-player closes too (its owner is the window). T-45a-048.
- **EC-78** `pip_replace_with_previous` true: first button is previous (disabled at index 0). T-45a-048.
- **EC-79** Last episode in playlist: Next disabled in the mini-player and `pip_next` ignored. T-45a-048.
- **EC-80** Orientation preference stored as `ReverseLandscape`: dropped; Kotlin maps to a portrait value (defect). T-45a-035.

### 2.9 Screenshot, share, art

- **EC-81** Share requested with no current anime or episode: no-op (`?: return`). T-45a-050.
- **EC-82** Screenshot save error: log only, no toast. T-45a-049.
- **EC-83** Set-as-art `AddToLibraryFirst`: message `Please add the entry to your library before doing this`. T-45a-051.

### 2.10 External players

- **EC-84** `external_player_preference` empty: opens with the desktop default app. T-45a-055.
- **EC-85** Preset binary not installed: error `Player not found: <program>`, no spawn. T-45a-055.
- **EC-86** Custom command template with unmatched `{`: left as-is, spawn fails with a clear error. T-45a-056.
- **EC-87** Title with `"`: passed as a single argv element, no shell. T-45a-057.
- **EC-88** Start position in the external player: `position` in ms; mpv preset converts to `--start=<s>`. T-45a-057, T-45a-062.
- **EC-89** External player result: none on Linux; progress polling only for mpv IPC. T-45a-061.
- **EC-90** IPC socket path clash (two external mpv launches): unique socket per launch (`$XDG_RUNTIME_DIR/kurobayomi-mpv-<pid>-<n>.sock`). T-45a-061.
- **EC-91** External player is closed before the first poll: last known position may be 0; nothing saved (no completion). T-45a-061.
- **EC-92** `position` 0 with `duration` 0 on completion: Kotlin treats it as completion (`currentPosition == duration && duration == 0`). Rust: keep the same rule and log. T-45a-061.
- **EC-93** Web Video Caster preset: dropped. T-45a-060.
- **EC-94** Local file (on device): direct path, no FileProvider. T-45a-068.
- **EC-95** Local-source episode (`LocalAnimeSource.ID`): always on device. T-45a-068.

### 2.11 Progress, history, deletion, tracking

- **EC-96** `incognito_mode` true: no history and no progress write, even on completion. T-45a-063.
- **EC-97** `totalSeconds <= 0`: no progress write (history still written in the Kotlin `onActivityResult` path: step 6 is unconditional). T-45a-063.
- **EC-98** Progress reaches exactly 85%: `lastSecondSeen >= totalSeconds * 0.85` → seen. T-45a-064.
- **EC-99** Episode already seen: `seen` stays true; tracker update is sent only when the pre-update state was seen (behaviour 16.10). T-45a-066.
- **EC-100** `remove_after_read_slots = -1`: nothing deleted. T-45a-065.
- **EC-101** `remove_after_read_slots = 0`: deletes the current episode if seen (Kotlin). Document; keep. T-45a-065.
- **EC-102** `remove_after_read_slots = 2` with current at index 1: `getOrNull(-1)` → null → nothing. T-45a-065.
- **EC-103** Anime sorting `EPISODE_SORTING_ALPHABET` (0x600): Kotlin throws `NotImplementedError` inside `launchIO` (uncaught). Rust: log error, skip deletion, do not crash. T-45a-065.
- **EC-104** Tracker of a different type (not `AnimeTracker`): skipped. T-45a-066.
- **EC-105** Tracker logged out: skipped. T-45a-066.
- **EC-106** Offline when the update fires: stored in the delayed store and job scheduled. T-45a-067.
- **EC-107** Tracker `lastEpisodeSeen` already >= episode number: no update. T-45a-066.

### 2.12 Local HTTP server

- **EC-108** Video URL `http://localhost:1` exactly (placeholder): matches; rewritten to the real port. T-45a-070.
- **EC-109** Video URL `http://localhost:12345`: does not match (digit after 1). T-45a-070.
- **EC-110** `copyHttpServer` on a URL with two matches: both replaced (regex global replace). T-45a-070.
- **EC-111** Source with no `AnimeHttpSource`: `createHttpServer` not called; no server. T-45a-069.
- **EC-112** Second server start while one is running: first stopped, new port (port changes). Any cached URL in the player must be rebuilt. T-45a-069.
- **EC-113** Bind failure: Kotlin reports running with port 0 (defect). Rust returns an error, toast `Failed to start http server`. T-45a-069, T-45a-070.
- **EC-114** Server stopped while mpv streams from it: mpv gets `HTTP error`; the stored HTTP error text is shown at end-file. T-45a-009.

### 2.13 Torrent

- **EC-115** `pref_torrserver_enable` false: torrent URLs go to the normal path (mpv fails with an error). T-45a-078.
- **EC-116** Engine already running (`echo` non-empty): no restart, trackers not re-pushed. T-45a-072.
- **EC-117** Engine fails to start (`startServer` returns -1): nothing reported (Kotlin). Rust: error toast `Failed to start torrent server` (proposal text) and no load. T-45a-072.
- **EC-118** `wait(10)` timeout: about 11 polls; returns false; Kotlin continues with a port that may not answer. Rust: fail the start. T-45a-072.
- **EC-119** `magnet:` URL without `index=`: index 0. T-45a-074.
- **EC-120** `index=abc`: index 0. T-45a-074.
- **EC-121** `index=2&dn=...`: index 2 (substring up to `&`). T-45a-074.
- **EC-122** `content://` torrent upload: file read to memory, uploaded with title as filename. T-45a-074.
- **EC-123** Play link for file index with no matching file id: name falls back to torrent title. T-45a-075.
- **EC-124** Torrent without `hash` (null): the Kotlin prints `null` into the link (defect). Rust: fail the link build with an error. T-45a-075.
- **EC-125** Filename with `/` in `file_stats.path`: basename used (`File(path).name`). T-45a-075.
- **EC-126** Filename with spaces and non-ASCII: encoded as UTF-8 with `URLEncoder` rules (space as `+`). T-45a-075.
- **EC-127** `ProxyMode.None` with a proxy URL stored: URL not sent. T-45a-077.
- **EC-128** Tracker list with trailing newline: empty entry removed in Rust (Kotlin keeps an empty line joined with `,\n`). Proposal: drop empty entries. T-45a-076.

### 2.14 Notifications and services

- **EC-129** HTTP server notification action `Close` pressed: server stops; notification removed. T-45a-071.
- **EC-130** Torrent notification `Stop`: stops engine. T-45a-079.
- **EC-131** App exit while servers run: servers stop with the app (no survival). T-45a-080.

### 2.15 Source URL and file helpers

- **EC-132** `getFileName` on a cursor with no rows: Kotlin reads index -1 (defect). Rust: basename of the path, or `None`. T-45a-081.
- **EC-133** `openContentFd` fails: null. Desktop: Drop. T-45a-081.

---

## 3. Race conditions and concurrency

- **RC-01** `fileLoaded` (IO thread) vs `onPause` (main thread) both mutate `isExiting`. Rust: all reads and writes on the GTK main thread; `fileLoaded` posts results back. T-45a-039.
- **RC-02** `loadfile` issued twice (double click on next): the second command replaces the first; state must be keyed by a load generation id so the first `FILE_LOADED` does not apply stale options. T-45a-011, T-45a-084.
- **RC-03** `setVideo` on IO and a concurrent episode switch: Kotlin has no lock. Rust: a single `load` task per window; a newer request cancels the older (`tokio::select!` on a cancel token). T-45a-011.
- **RC-04** `copyFontsDirectory` (IO) sets `sub-fonts-dir` after the file loads: a subtitle using the font can render before the copy. Rust: no copy (T-45a-029 proposal), so no race.
- **RC-05** Progress save (IO) while the window closes: `deletePendingEpisodes` races with the save. Rust: await the save before closing, with a 2 s bound (proposal). T-45a-040.
- **RC-06** Tracker updates in `async` with `awaitAll`: a slow tracker delays nothing else (Kotlin awaits all before returning). Rust: spawn detached, log results. T-45a-066.
- **RC-07** `HttpServerService` restart: a request in flight to the old port fails. Acceptable; the window reloads the URL. T-45a-069.
- **RC-08** Torrent `wait` loop polls `echo` every second; `stop` during the loop: the loop keeps polling until its budget. Rust: cancel on stop. T-45a-072.
- **RC-09** `TorrentServerService.start` called twice in quick succession: two `startServer` calls, the second sees a non-empty `echo` after the first completes, or both start. Rust: one start per engine instance (mutex). T-45a-072.
- **RC-10** `observedProps` events arriving while `isExiting`: ignored (Kotlin). Rust: same gate, plus the PLAYBACK_RESTART exception (behaviour 6.4). T-45a-039.
- **RC-11** Event delivery after `mpv_terminate_destroy`: no further calls; drop the wakeup source before destroy. T-45a-002.
- **RC-12** `MpvHandle` used from a non-main thread: compile-time prevention via `!Send` wrapper (`PhantomData<*const ()>`). T-45a-002.

---

## 4. Failure modes and expected handling

| Failure | Kotlin behaviour | Rust behaviour | Task |
|---|---|---|---|
| libmpv create fails | not handled | error dialog, close window | T-45a-002 |
| libmpv render context fails | not handled | error dialog, keep window with message | T-45a-002 |
| `loadfile` returns error | not checked | error pill with mpv message | T-45a-011 |
| Source `createHttpServer` throws | toast `Failed to start http server` | same | T-45a-070 |
| Local server bind fails | reports running with port 0 (defect) | error and toast | T-45a-069 |
| External player preset not installed | Android: intent resolves to chooser or fails silently | error pill `Player not found: <program>` | T-45a-055 |
| External spawn fails | not applicable | error pill with OS error | T-45a-055 |
| Torrent engine start fails | silent | error pill `Failed to start torrent server` (proposal text) | T-45a-072 |
| Torrent API returns non-2xx | exception, silent | error pill with HTTP status | T-45a-073 |
| Torrent JSON decode fails | exception, silent | error pill | T-45a-073 |
| Progress DB write fails | exception in `launchIO` (uncaught) | logged, toast `Could not save progress` (proposal) | T-45a-063 |
| Tracker update fails | logged INFO | logged INFO | T-45a-066 |
| Metadata JSON fails | logged ERROR | logged ERROR | T-45a-013 |
| Asset copy fails | logged ERROR | logged ERROR | T-45a-028 |
| Font copy fails | exception on IO, silent | logged, continue | T-45a-029 |
| Screenshot save fails | logged ERROR | logged ERROR, error pill (proposal) | T-45a-049 |
| aniSkip lookup fails | part 2 | part 2 | area:45-player-core |
| Init fails | error + close | error + close | T-45a-052 |
| Hoster list empty | error + close | error + close | T-45a-037 |
| No next episode | toast (manual) | short pill (manual) | T-45a-037 |

---

## 5. New tests to write (Rust)

All tests are plain `#[test]` unit tests unless marked. No GTK, no libmpv, no network in unit tests.

### 5.1 `src/player/options.rs`

- `startup_defaults_match_golden`: `PlayerPrefs::default()` produces the 43 settings of `p1-data-and-config.md` section 4 in order (fixture `tests/fixtures/mpv/golden_defaults.txt`).
- `debanding_cpu_with_yuv420p_joins_vf` (EC-12): result `vf` = `gradfun=radius=12,format=yuv420p`.
- `debanding_gpu_sets_deband` (EC-13).
- `gpu_next_selects_vo` (EC-11): `vo` = `gpu-next`.
- `hwdec_flag_false_is_no` (`try_hwdec = false` gives `no`).
- `speed_formats_with_point_zero` (EC-15): `2.0`.
- `audio_delay_ms_to_seconds` (EC-16): `-250` gives `-0.25`.
- `volume_boost_cap_adds_100` (EC-17).
- `subtitle_colors_hex` (EC-19): `-1` gives `#FFFFFFFF`; `0` gives `#00000000`; `-16777216` gives `#FF000000`.
- `subtitle_delay_formats_seconds`: `1500` gives `1.5`.
- `font_default_sans_serif`.
- `ass_override_only_when_enabled` (EC-21).
- `cache_limits_are_64_mib` (EC-22).
- `filters_default_zero` and `filters_write_names`.

### 5.2 `src/player/keys.rs`

- `plain_letter_is_char` (`a` gives `a`).
- `arrow_keys_are_uppercase` (`Left` gives `LEFT`).
- `modifiers_in_order` (EC-73): Shift+Ctrl+Alt+Super+`a` gives `shift+ctrl+alt+meta+a`.
- `repeat_is_swallowed` (EC-66): returns `Swallow`.
- `dead_key_is_ignored` (EC-67).
- `modifier_only_is_ignored` (EC-68).
- `unknown_nonprintable_is_none` (EC-70).
- `hash_becomes_sharp` (proposal).
- `media_keys_map` (table in `p1-rust-design.md` section 8).

### 5.3 `src/player/bridge.rs`

- `routes_user_data_prefix`: `user-data/aniyomi/pause` routed; `user-data/current-anime/intro-length` routed to the int handler; unknown routed to debug-ignore (EC-related, T-45a-020).
- `pause_values` (`pause`, `unpause`, `pauseunpause`) and an invalid value is ignored.
- `seek_with_text_splits_on_pipe` (`10|Skip intro` gives value `10`, text `Skip intro`).
- `int_picker_six_fields` (`title|%ds|0|255|1|user-data/current-anime/intro-length`).
- `switch_episode_values` (`p`, `n`).
- `toggle_ui_values` (`show`, `hide`, `toggle`).
- `panel_values` (`subtitle_settings`, `subtitle_delay`, `audio_delay`, `video_filters`).

### 5.4 `src/player/source_url.rs`

- `file_scheme_gives_path`.
- `data_scheme_keeps_payload` (EC-32): `data:video/mp4;base64,AAAA,BBB` gives `data://video/mp4;base64,AAAA,BBB`.
- `protocol_passthrough` (EC-33) with `http`, `https`, `rtmp`, `udp`.
- `content_scheme_rejected` (EC-34).
- `unknown_scheme_is_none`.
- `spaces_preserved` (EC-31).

### 5.5 `src/player/external.rs` (argv builder, pure)

- `no_preset_uses_default_opener` (EC-84).
- `mpv_preset_argv` with title, start position, header and subtitle: `["mpv", "--title=Show - Ep", "--start=12.5", "--http-header-fields=Referer: x", "--sub-file=<url>", "<url>"]` (exact order fixed by the implementation; test asserts it).
- `vlc_preset_argv` with the subtitle as a separate argument.
- `custom_template_placeholders` (`{url}`, `{title}`, `{position}`); unmatched `{` left as-is (EC-86).
- `title_with_quotes_is_one_argv` (EC-87).
- `start_position_rule` (EC-62 section, 16.3): seen and not preserve gives 1 ms; seen, preserve, watched gives 1 ms; seen, preserve, not watched gives `lastSecondSeen`; unseen gives `lastSecondSeen`. Table test with four rows.
- `subtitle_locale_match` (EC-58 section): matching language selects that URL; no match selects first; empty list selects nothing.
- `mime_by_extension` (`mp4`, `mkv`, `m3u8`, other).
- `binary_missing_is_error` (EC-85): `which`-style lookup on a temp `PATH`.

### 5.6 `src/player/progress.rs` (uses an in-memory DB, async test with `tokio::test`)

- `incognito_skips_all_writes` (EC-96).
- `zero_total_skips_progress` (EC-97).
- `completion_threshold_85_percent` (EC-98): exactly 85% sets seen; 84.9% does not.
- `already_seen_stays_seen`.
- `history_upsert_twice_updates_last_seen` (animehistory `ON CONFLICT`).
- `delete_slots_minus_one_disabled` (EC-100).
- `delete_slots_zero_targets_current` (EC-101).
- `delete_slots_out_of_range_no_op` (EC-102).
- `alphabet_sorting_logs_and_skips` (EC-103): no panic, no delete.
- `tracker_skipped_when_logged_out` (EC-105).
- `tracker_skipped_when_not_newer` (EC-107).
- `offline_queues_delayed_update` (EC-106).

### 5.7 `src/services/local_server.rs`

- `binds_loopback_ephemeral` (port > 0; listens on 127.0.0.1 only).
- `stop_releases_port`.
- `restart_changes_port_and_stops_old` (EC-112).
- `bind_failure_returns_error` (EC-113): occupy the port range in the test? Use a `TcpListener` bound to `127.0.0.1:0` and then a second start with fixed port injection (test-only constructor).
- `range_request_served` (`[integration]` `hyper` client sends `Range: bytes=0-9`, expects `206`).

Rewrite-target of `usesHttpServer` / `copyHttpServer` (EC-108, EC-109, EC-110):

- `uses_placeholder_port_exact` (`http://localhost:1/x` true; `http://localhost:12345/x` false).
- `copy_replaces_all_matches` (two URLs in one string).

### 5.8 `src/services/torrent.rs`

- `play_link_directory_index_1` (fixture `test-torrent-directory`, index 1: basename `3.1`; expected `http://127.0.0.1:<port>/stream/3.1?link=863eb76f1cd0643d9fd2eb4a7da866c3752991b5&index=1&play`).
- `play_link_single_file_index_0`: no matching id (fixture single file has id null) → name is torrent title `test-torrent-file`, link hash `104701de4b710ee9720ee249bf220384659e8c38`.
- `play_link_name_encoding_space_and_unicode` (EC-126).
- `play_link_missing_hash_errors` (EC-124).
- `magnet_index_default_zero` (EC-119), `magnet_index_parses` (EC-121), `magnet_index_bad_is_zero` (EC-120).
- `tracker_list_join` (default list, 26 entries, joined with `,\n`) (section 5 of data doc).
- `tracker_list_drops_empty_entries` (EC-128, proposal).
- `proxy_url_only_when_not_none` (EC-127).
- `wait_budget_10_is_11_polls` (with an injected clock).
- `wait_default_is_20_polls`.

### 5.9 `src/player/controller.rs` (uses a fake `MpvHandle` and fake `PlayerModel`)

- `file_loaded_order` (T-45a-084): recorder asserts `metadata, title, chapters, tracks, skip_intro` order (orientation dropped).
- `playback_restart_clears_exiting_before_gate` (proposal, behaviour 6.4): after pause, `PlaybackRestart` clears `is_exiting`; a property event before restart is dropped.
- `events_dropped_while_exiting` (RC-10).
- `pause_flag_double_check` (EC-60).
- `start_position_rule` (3.2 table, four rows, EC-35 and EC-37).
- `load_generation_drops_stale_file_loaded` (RC-02).
- `eof_with_autoplay_changes_episode` (EC-57, EC-58).
- `aid_auto_ignored` (EC-47).
- `sid_non_integer_no_change` (EC-49).
- `error_then_close_on_init_failure` (EC-04).

### 5.10 `src/player/window.rs` and `mini.rs` (GTK, `#[ignore]` by default, run with `xvfb-run` or a Wayland session)

- `window_closes_on_missing_ids` (EC-01).
- `mini_controls_order_and_sensitivity` (EC-78, EC-79).
- `idle_inhibit_on_play_and_release_on_pause` (T-45a-041).
- `fullscreen_follows_preference` (T-45a-044).

### 5.11 Integration (manual, with a real mpv and file)

See section 7.

---

## 6. Fixtures

| Path | Content |
|---|---|
| `tests/fixtures/mpv/golden_defaults.txt` | the 43 settings of data doc section 4, one per line, `kind<TAB>name<TAB>value` |
| `tests/fixtures/mpv/golden_debanding_cpu.txt` | same with `vf` = `gradfun=radius=12,format=yuv420p` |
| `tests/fixtures/keys/gtk_keys.txt` | pairs `gtk_name<TAB>mods<TAB>expected` for the key table |
| `tests/fixtures/bridge/*.txt` | one protocol message per line: `property<TAB>value<TAB>expected_action` |
| `tests/fixtures/torrent/test-torrent-file.torrent` | copied from `core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-file.torrent` |
| `tests/fixtures/torrent/test-torrent-directory.torrent` | copied from `core/common/src/test/resources/aniyomi/core/common/torrent/test-torrent-directory.torrent` |
| `tests/fixtures/torrent/trackers_default.txt` | the 26 tracker lines, data doc section 5 |
| `tests/fixtures/external/argv_mpv.txt` | expected argv for the mpv preset |
| `tests/fixtures/external/argv_vlc.txt` | expected argv for VLC |
| `tests/fixtures/video/video_basic.json` | a `SerializableVideo` JSON with one audio track, one subtitle track, two `mpvArgs`, headers, one timestamp |
| `tests/fixtures/video/video_placeholder.json` | a `Video` with `videoUrl = http://localhost:1/x` |
| `tests/fixtures/metadata/ANIYOMI_MPV_ARGS.json` | `{"ANIYOMI_MPV_ARGS": "vf=scale=1280:-2;sub-pos=95"}` |

For the torrent fixtures the Kotlin parser output is the expected `Torrent` value (`TorrentHelpersTest`). The Rust tests can use the same values without re-parsing (the parser itself is area 13).

---

## 7. Manual integration checks (GL and real mpv)

Run on CachyOS, KDE Plasma, both Wayland and X11 sessions (`XDG_SESSION_TYPE`).

1. Open a local MP4 from the library: video renders, pause starts paused, Space toggles, time moves. Covers T-45a-002, T-45a-019, T-45a-024.
2. Open a local MKV with two audio tracks and an external SRT: `audio-add`/`sub-add` tracks appear in the track list. Covers T-45a-016.
3. Set `pref_video_debanding = CPU` and `use_yuv420p = true`. With `verbose_logging` on, the startup log must list the `vf` option as `gradfun=radius=12,format=yuv420p`, and the live `vf` property must read the same. Covers T-45a-004 (EC-12).
4. Set `pref_gpu_next = true`: video renders without a black frame. Covers EC-11.
5. Resume position: play to 60 s, close, reopen: starts at about 60 s. Covers T-45a-012.
6. Media keys: `playerctl -p kurobayomi play-pause` toggles playback (MPRIS). Covers T-45a-043.
7. Idle inhibit: `systemd-inhibit --list` shows the app while playing, and not when paused. Covers T-45a-041.
8. Fullscreen toggle with double-click and Escape. Covers T-45a-044.
9. External mpv launch: spawns `mpv` with the title and start position, progress saved on exit at the reported time. Covers T-45a-055 to T-45a-061.
10. Local server: open an online source that uses `usesHttpServer()`; the stream plays through `http://127.0.0.1:<port>`. Covers T-45a-069, T-45a-070.
11. Torrent: with `pref_torrserver_enable` on and a magnet URL, the engine starts and the link plays. Covers T-45a-072 to T-45a-078.
12. Unplug the audio output while playing: paused. Covers T-45a-082.

---

## 8. Known gaps (not testable without the external library)

- BaseMPVView (`aniyomi-mpv-lib` 1.18.n) behaviour is inferred from call sites. Tests cover the Rust side only; the mpv behaviour checks are in section 7.
- `KeyMapping.map` (Android keycode to mpv key name) is an external table. The Rust key table (`p1-rust-design.md` section 8) is a proposal; section 5.2 tests it.
- `Utils.PROTOCOLS` (external) is replaced by the runtime `protocol-list` property. The test in 5.4 uses a fixed list from mpv's documentation at the time of writing; re-check on libmpv upgrades.
- TorrServer (Go library) behaviour is covered only by the HTTP contract in `p1-data-and-config.md` section 6.
