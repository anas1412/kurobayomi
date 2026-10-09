# 45-player-core (part 2 of 2) — Edge cases, failure modes, races, tests

Repo root: `/home/blackbox/aniyomi-src`. Each edge case has an ID `E-nn` that other documents reference. "Kotlin behaviour" is OBSERVED; "Rust rule" is the PROPOSAL the port must implement unless a task says otherwise.

Upstream unit tests: none of the 12 scope files has a `*Test.kt` counterpart in this part (checked: `find` for `*Test.kt` under `app/src/test` mentioning `PlayerViewModel`, `EpisodeLoader`, `HosterLoader`, `TrackSelect`, `ChapterUtils`, `AniSkipApi` returned nothing). Therefore section 4 lists new tests to write.

## 1. Edge cases

### Initialisation and identity

- **E-01 — `init` fast path returns cached result.** Kotlin: when `needsInit(animeId, episodeId)` is false, `init` returns `defaultResult` (`currentHosterList`, `qualityIndex`, position null) with success(true) (PlayerViewModel.kt:1259-1260). Rust rule: same; position must still be read from the session file, not returned as null, so a recreated window resumes at the saved position (data §6).
- **E-02 — anime not found.** `getAnime.await` returns null → `Result.success(false)` (not failure, :1323). The activity then shows nothing. Rust rule: `Ok(false)`-equivalent `SessionState::AnimeMissing`, shown as "not found" error panel with a back button.
- **E-03 — AniSkip uses only the first tracker.** `aniSkipResponse` returns from inside `map` (non-local return), so only `getTracks(animeId)[0]` is consulted; a Kitsu/Shikimori/Bangumi first track yields null even if a MAL/AniList track is second (PlayerViewModel.kt:2041-2052). Rust rule: choose the first track whose tracker resolves to a MAL id (MAL remote id, or AniList→MAL lookup returning non-zero) and query AniSkip once. Document as an intentional fix in the changelog (T-45b-021).
- **E-04 — `mixed-ed` makes the whole AniSkip response fail.** The request includes `types[]=mixed-ed` but `SkipType` has no such constant and no enum coercion is configured (`Json { ignoreUnknownKeys = true; explicitNulls = false }`, AppModule.kt:174-176). If any result has `skipType = "mixed-ed"`, decoding throws and `getResult` returns null (AniSkipApi.kt:41). Not observed in the sample fetched for this plan. Rust rule: decode `skipType` as a string, map unknown values to `ChapterType::Other` and keep the interval (or drop only that item). Keep the request types as-is for parity.
- **E-05 — AniSkip client is private and never closes the body.** `AniSkipApi` builds `OkHttpClient()` per instance (AniSkipApi.kt:18) and reads `body.string()` without closing on the success path (:27). Rust rule: use the shared `reqwest::Client` from area:12, read with `.text().await`, which closes the body.
- **E-06 — volume boost double-apply at system max.** At system max with mpv at 100 and `change < 0`, the code lowers the system volume (:769) and then falls through to set mpv to `max(100, 100 + change) = 100` and returns (:770-773). Net effect: system volume drops one step, mpv unchanged. Rust rule: when `volume_mpv == 100 && change < 0` lower system volume and return; when `volume_mpv > 100` lower mpv first. Test T-45b-031.
- **E-07 — decoder value not in enum throws.** `getDecoderFromValue(MPVLib.getPropertyString("hwdec"))` at construction (:204) uses `first { }` (PlayerEnums.kt:89-91). mpv can report `hwdec-current` values such as `vaapi-copy`, `nvdec`, `vdpau`, which are not in the Kotlin enum (Android-only list). Rust rule: map unknown values to `Decoder::Auto` (display "Auto") and keep the raw string for display.
- **E-08 — error hoster cannot be retried.** `onHosterClicked` ignores `Error` (PlayerViewModel.kt:1576). Rust rule: allow retry by re-running `loadHosterVideos(force=true)`; flagged as intentional improvement.

### Playlist and navigation

- **E-09 — countdown not reset across consecutive intro chapters.** `waitingSkipIntro` is only reset when a chapter of type `Other` is reached (:2075). If netflix countdown reached -1 in one intro chapter, the next intro chapter shows only the normal label until an `Other` chapter passes. Rust rule: reset the counter whenever `current_chapter` changes (chapter identity by index).
- **E-10 — countdown is per call, not per second.** The counter decrements on every `setChapter` call (:2091). `setChapter` runs on each whole-second `time-pos` (PlayerActivity.kt:668-671) and also at PlayerActivity.kt:1304 and 1383 (AniSkip merge, and track/initial load). Those extra calls advance the countdown. Rust rule: decrement only on the tick path; keep a separate `initial_sync` call that does not decrement. Test: T-45b-022.
- **E-11 — `seekTo` silently ignores out-of-range.** `seekTo` returns if `position !in 0..(duration ?: 0)` (:748). If duration is 0 (before file load), every seek is ignored, including chapter selection. Rust rule: same range check, but when `duration == 0` queue the seek and apply on `FileLoaded` (proposal; prevents lost chapter jumps when a user taps a chapter during load).
- **E-12 — `loadEpisode` failure keeps the previous episode's hoster list.** `currentHosterList` is only assigned on success (:1635-1639); the catch logs "Error getting links" and then returns `EpisodeLoadResult(hosterList = currentHosterList, …)` with the OLD list (:1643-1650). The episode id is also not updated on failure (:1641 not reached), so the UI shows the new title with the old videos. Rust rule: on failure return `Err(PlayerError::Other)` and keep the previous session untouched, but show the new episode title only after success.
- **E-13 — download-ahead re-requested every second.** `downloadNextEpisodes` runs on every tick once past 35% (:1682-1684) and calls `downloadEpisodes` each time (:1731). Rust rule: throttle to once per episode per session (store a `download_ahead_started: HashSet<episode_id>`), and let the download manager dedupe as well (area:24).
- **E-14 — completion repeats and can overwrite seen.** After `seconds >= total * progress`, `updateEpisodeProgressOnComplete` runs on every tick (:1674-1677). Its coroutine sets `currentEp.seen = true` asynchronously, while `saveWatchingProgress` (same tick, :1686) may write the object before that assignment and persist `seen = false` when the episode was not previously marked. Rust rule: set `seen = true` synchronously on the in-memory object before scheduling any save; run completion side effects once per episode (`completed_episode_ids` set) and keep progress saves on every tick.
- **E-15 — stale bookmark overwrite.** `bookmarkEpisode` writes the DB but not the in-memory `currentEpisode.bookmark` (:1805-1813). The next progress save writes `episode.bookmark` (:1780) and reverts the change. Rust rule: update in-memory state and DB together.
- **E-16 — incognito evaluated once.** `incognitoMode` is `by lazy` on first access (:1111) and captures `currentAnime.source` at that time; switching anime in the same ViewModel keeps the first value. Rust rule: recompute on each episode switch.
- **E-17 — two different incognito flags.** Progress uses per-source incognito (`GetAnimeIncognitoState`, :1111, :1673, :1775, :1793), while tracker sync uses the global `basePreferences.incognitoMode()` (:1948). Rust rule: keep both semantics explicitly named (`incognito_source` and `incognito_global`) and test the combination table in T-45b-027.
- **E-18 — `episodeToDownload` is dead.** Only assigned `null` (:1748); `onCleared` re-queues it when non-null (:1100-1101), so this never runs. Rust rule: drop the field and the re-queue; document in the changelog as a removed no-op.
- **E-19 — double-tap accumulator resets on consecutive back taps.** `seekByWithText` sets the amount to 0 when `value < 0 && amount < 0` (:987), so the second back tap shows 0 instead of -20. Kotlin behaviour in the overlay; Rust rule: accumulate (`amount + value`) and reset only after the overlay timeout. Mark as intentional fix (T-45b-023).
- **E-20 — thumbnail throttle drops the final request.** Requests within 2 s of the last fetch return without retry (:585-586), so the last scrub position may never load its tile. Rust rule: keep the latest pending request and fire it when the throttle window ends (trailing-edge throttle).
- **E-21 — empty token in track language lists.** `split(",").filter(String::isNotEmpty).map(String::trim)` keeps whitespace-only tokens (`" "` trims to `""`), and `"".contains…` matches everything (TrackSelect.kt:243-255). A blacklist containing `""` therefore excludes all tracks. Rust rule: trim first, then filter empty.
- **E-22 — regex alternation not grouped.** `Regex("""\b${isO3}|${language}\b""")` matches `\beng` OR `en\b` anywhere in the tag (TrackSelect.kt:281). Example: tag `"xen"` matches `en`. Rust rule: match whole tokens: `^(?:iso3|iso2)$` on the normalised tag (lowercase, `-` split, first subtag), and document the change in T-45b-017.
- **E-23 — rendezvous event channel.** `Channel<Event>()` (:1108) suspends the sender until the UI collects; a missing collector blocks save-image completion. Rust rule: unbounded `async_channel`, never blocks the producer.
- **E-24 — non-favourite anime cannot store a skip length.** `setAnimeSkipIntroLength` returns unless `anime.favorite` (:2001). The value still applies to the mpv session. Rust rule: same; the settings UI shows "Add to library to save" hint.
- **E-25 — smooth-seek asymmetry.** Chapter jumps use exact seeks regardless of `pref_player_smooth_seek`, while skip-intro and double-tap follow the pref (behavior §8.2). Rust rule: keep, and document in the setting help text.
- **E-26 — video filter conflicts.** Part 1 writes `vf` twice: debanding CPU sets `gradfun=radius=12`, then `use_yuv420p` overwrites `vf` with `format=yuv420p` (AniyomiMPVView.kt:120-130 with `setOptionString("vf", …)` each time). With both enabled, gradfun is silently dropped. Rust rule: build one `vf` chain string from all enabled filters (`format=yuv420p`, `gradfun=radius=12`, `brightness/saturation/...` from video filters), comma-joined. Flag as fixed.
- **E-27 — intro length silently truncated to 8 bits.** `setFlag(flag, ANIME_INTRO_MASK)` does `flag and 0xFF` (SetAnimeViewerFlags.kt:19 and `setFlag` at its bottom). A value 300 is stored as 44 with no error. The UI slider is bounded upstream (part 1), but a Lua `intro-length` write is not. Rust rule: clamp to 0..=255 before writing and reject larger values with a toast.
- **E-28 — tile crop bounds.** `createThumbnail` uses `Bitmap.createBitmap(tile, x, y, w, h)` without bounds checks (:611). A malformed `TileInfo` throws `IllegalArgumentException` in the UI thread. Rust rule: clamp the rectangle to the tile size; if empty, drop the thumbnail.
- **E-29 — shutdown saves may be lost.** Kotlin `onCleared` saves progress on the ViewModel scope (:1097-1103) and the process may die. Rust rule: on window close, run the final save synchronously on the main thread with a 2 s deadline (`block_on` with `timeout`), then shut down the runtime with `shutdown_timeout(3 s)`.
- **E-30 — negative or out-of-range tile index.** `thumbInfo.imageTileUrls[info.imageIndex]` (:593) throws if the source returns an index beyond the URL list; `TileInfo.imageIndex` is an `Int` and could be negative. Rust rule: validate index < urls.len() when building `ThumbnailInfo`; drop invalid tiles at construction.
- **E-31 — subtitle list has no "Off" entry.** Audio list gets `VideoTrack(-1, "Off", null)` (:425-426), subtitles do not (:423). Kotlin behaviour kept by default; the Rust port adds an "Off" row for subtitles behind a flag `subtitle_off_row` (decision to confirm with the user; the default preserves Kotlin).

### Hoster and video resolution

- **E-32 — `"null"` sentinel collision.** A genuine URL string `"null"` is treated as lazy (Video.kt:69, EpisodeLoader.kt:182). Rust rule: use `Option<String>` for `video_url` internally and map only the legacy constructor's null to `None`, so a server returning the literal string `"null"` is not re-resolved forever.
- **E-33 — `checkHasHosters` reflection.** Walks superclasses and reads `declaredMethods` (EpisodeLoader.kt:60-78). A class that inherits `getHosterList` from a base class not in the stop list is detected only if it is declared on a class in the chain (`declaredMethods` is per class). Rust rule: trait method `has_hoster_list` (see design §4).
- **E-34 — local hoster `!!`.** `videoFile!!.uri` (EpisodeLoader.kt:133) throws if the file was removed after listing; the catch returns an empty list (:140-141), giving "Hoster list is empty". Rust rule: return `Err(NotFound)` with message key `NoHosters`.
- **E-35 — `selectBestVideo` ignores ERROR and LOAD_VIDEO.** A preferred video in ERROR state is skipped even if it is the only preferred one; the fallback then picks any non-empty URL (HosterLoader.kt:32-50). Rust rule: same, documented.
- **E-36 — `getBestVideo` checks `!initialized` only in the first pass.** The fallback loop (HosterLoader.kt:116-142) does not check `initialized`, so an already-resolved video is re-resolved. Rust rule: skip `initialized` videos in the fallback too (they were already tried in the same call).
- **E-37 — `loadVideo` recursion depth.** Recursive call on failure (:1500) is bounded by the number of video slots, but on a large hoster list (dozens of videos) each step awaits network. Rust rule: loop with an explicit state machine (design §9).
- **E-38 — preferred-video race.** `hasFoundPreferredVideo` is an `AtomicBoolean` compare-and-set; if the saved selection loads and fails, the flag is reset to false (:1405-1407), allowing a later hoster to load its preferred video. Two hosters can both start loading in parallel before either finishes, so two `loadVideo` calls can run concurrently and race on `_selectedHosterVideoIndex`. Rust rule: serialize `load_video` calls through a single `tokio::sync::Mutex` (session-level) so only one load is in flight; cancel the previous one on user selection.
- **E-39 — `updatePausedState` / `setPausedState` ordering.** `loadVideo` captures pause and sets pause; `onFinishLoadingTracks` applies the captured state. If tracks never load (no track-list event, e.g. audio-only file), `setPausedState` is never called and playback stays paused (Kotlin relies on `PlayerActivity.kt:1344-1347` fallback). Rust rule: call `set_paused_state` from the `FileLoaded` handler as well, idempotently.
- **E-40 — onVideoClicked on Loading video.** Clicking a video whose state is `LoadVideo` is not blocked (only ERROR is). A second `loadVideo` can start while the first is in flight. Rust rule: ignore clicks when `is_loading_episode` is true, unless it is a deliberate re-selection (then cancel the first).

### Tracks and chapters

- **E-41 — `loadChapters` `!!` on missing count.** `chapter-list/count` null → NPE (:481). Rust rule: treat as zero chapters.
- **E-42 — `loadTracks` NPE swallowed.** The NPE catch (:438-441) returns without updating lists, so an mpv teardown leaves old tracks. Rust rule: on mpv handle gone, clear lists.
- **E-43 — mergeChapters adjacency compares original neighbour.** Step 7 compares against `combined[index-1]` in the un-filtered list (ChapterUtils.kt:216), so two consecutive empty-name segments where the first was itself removed still drop the second. Rust rule: identical semantics, with a unit test that pins the behaviour.
- **E-44 — mpv chapters within 1 s of an external start are dropped.** `abs(start diff) < 1.0` (ChapterUtils.kt:173). Accepted behaviour; test it.
- **E-45 — duration null uses -2.0 sentinel.** `isNotLastChapter` becomes true for every end when duration is unknown (ChapterUtils.kt:158). Rust rule: same sentinel; flagged.

### Persistence and DB

- **E-46 — progress save writes stale flags.** `saveEpisodeProgress` writes `seen`, `bookmark`, `fillermark` from the in-memory object (:1776-1785), which can differ from the DB if the user changed them elsewhere (e.g. episode list screen). Rust rule: write only the fields this tick changed (`last_second_seen`, `total_seconds`, and `seen` only when set by completion). Test T-45b-026.
- **E-47 — history upsert on every second.** `saveEpisodeHistory` upserts `last_seen = now` every tick (:1796). This keeps the history row's timestamp at the last second watched, which is intended, but it writes once per second. Rust rule: coalesce history writes to every 5 seconds plus on pause/close.
- **E-48 — `setAnimeSkipIntroLength` reads then writes.** The skip-length value is read (`getAnimeSkipIntroLength`) and compared before the write, but the write uses `getAnime.await` after the update (:2005-2006) and is not transactional. Rust rule: single update, then reload.
- **E-49 — DB errors swallowed by `UpdateEpisode`.** `await` and `awaitAll` catch all exceptions and only log (UpdateEpisode.kt:12-27). Progress can be lost silently. Rust rule: keep swallow-and-log for progress (parity) but surface a warning toast after 3 consecutive failures.
- **E-50 — `markDuplicateSeen` uses filtered playlist.** Duplicates hidden by the filter are not marked (:1697). Rust rule: parity; documented.

### Screenshots, files and sharing

- **E-51 — screenshot filenames collide within the same second.** `"${millis}_mpv_screenshot_tmp.png"` (:1831) is unique per millisecond, but `saveImage` names the final file from `prettyTime` seconds, so two saves in the same second produce the same name (ImageSaver may overwrite or suffix — area:13). Rust rule: append a counter when the target exists.
- **E-52 — `renameTo` failure is silent.** `tempFile.renameTo(newFile)` return value is ignored (:1839); the subsequent `exists()` check returns null if the rename failed and the temp file is still present. Rust rule: `std::fs::rename` with error propagation; delete the temp file on failure.
- **E-53 — malformed Lua payloads throw.** `launch_int_picker` destructuring of six fields (:901), `seek_to_with_text` split (:924), and `toInt()` calls (:925-932) throw on malformed input inside the mpv observer thread. Rust rule: parse with `Result`, log a warning with the property name, clear the property, and ignore the command.

### Concurrency

- **E-54 — `runBlocking` on the main path.** `initEpisodeList` and `checkTrackers` block the calling thread (:1337, :1353). If called from the main thread while a dispatcher is busy this can deadlock. Rust rule: async await; never block the GTK thread.
- **E-55 — `Dispatchers.Main.immediate` vs timers.** `startTimer` uses `viewModelScope.launch` (Main) and `delay(1000)`; long GC pauses delay ticks, so the countdown drifts. Rust rule: schedule with `glib::timeout_add_local` and compute remaining from `Instant`, so display never drifts.

## 2. Failure modes table

| Failure | Where | Kotlin result | Rust result |
|---|---|---|---|
| AniSkip network error | `getResult` | null, no UI message | `None`, log at debug; no toast |
| AniList network error | `getMalIdFromAL` | 0 (then AniSkip called with 0) | `None` → skip lookup (fix of E-03) |
| Source resolve exception (`resolveVideo`) | `getResolvedVideo` | null → next video | `None` → next video; `CancellationException` rethrown |
| Hoster list exception | `getHostersOnHttp` (not caught) → `init` `Result.failure` | error screen | `PlayerError::Other` error panel |
| Download list build failure | `getHostersOnDownloaded` | empty list | empty list (parity) → "Hoster list is empty" |
| Thumbnail tile fetch failure | `updateSeekPos` | logged | logged; no thumbnail |
| mpv handle destroyed mid-call | `loadTracks` NPE | silent | clear track lists |
| Screenshot file missing | `takeScreenshot` | null → UI shows error | `None` → toast "Failed to take screenshot" (new key, proposal) |
| `updateEpisode` DB error | `UpdateEpisode.await` | swallowed | swallowed + warning after 3 failures (E-49) |
| Tracker sync error | `TrackEpisode` | not caught (launchNonCancellable) | logged; no retry |
| Process killed during playback | `savedState` | restored on recreate | session file (design §13) |

## 3. Race conditions (catalogue)

1. **Hoster load vs. user selection.** User taps a video while `loadHosters` still resolves others → two `loadVideo`s (E-38, E-40). Fix: session mutex and cancel-on-select.
2. **Progress completion vs. save.** Completion and save on the same tick can invert `seen` (E-14). Fix: synchronous in-memory update first.
3. **Episode switch vs. in-flight progress save.** `saveCurrentEpisodeWatchingProgress` (PlayerActivity:201) runs before `init`/`loadEpisode` changes `currentEpisode`; a save launched after the switch would write the new episode's `last_second_seen` with the old position. Kotlin avoids this by capturing `currentEpisode.value` at call time (`saveWatchingProgress(episode)` takes the object). Rust rule: capture the `EpisodeRow` by value at call time; never read `store.current_episode` inside async saves.
4. **Chapter list vs. AniSkip merge.** AniSkip merge (PlayerActivity:1296-1305) reads chapters and updates them; a `chapter-list` observer arriving during the merge overwrites it. Fix: apply the merge on the main thread from the latest chapter list, not from a captured copy.
5. **Thumbnail tile cache vs. episode switch.** `resetState` clears the cache (:1029) while a tile fetch is in flight; the late result then inserts an old-episode tile at index 0. Rust rule: tag tiles with the `episode_id`/`video_url` generation and drop mismatches.
6. **Sleep timer vs. pause.** The timer pauses at the end regardless of user action (E-55 analysis). Rust rule: parity; also cancel when the user closes the window.
7. **Lua property re-entry.** `handleLuaInvocation` clears the property at the end (:960), which fires another observer event with an empty value (ignored by the `ifEmpty` guard). Rust rule: parity; guard against the empty echo.
8. **`onSecondReached` while `isLoadingEpisode`.** Returns early (:1659); progress for that second is lost. Accepted (parity); the next tick covers it.

## 4. Tests to write (new; the scope has no upstream tests)

Each test lists the function under test, the input fixture, and the expected result derived from the Kotlin code. Unit tests live next to the module (`#[cfg(test)]`), integration tests in `tests/player_*.rs`.

### 4.1 `select_best_video` (HosterLoader)
- T1 `preferred_beats_first_valid`: hosters `[H0: [v(url=a, pref=false)], H1: [v(url=b, pref=true)]]`, all QUEUE → `(1, 0)`.
- T2 `preferred_error_is_skipped`: H0 `[v(pref=true, state=ERROR)]`, H1 `[v(url=b)]` → `(1, 0)`.
- T3 `lazy_url_is_selectable`: H0 `[v(url="null")]` → `(0, 0)` (E-32 documents this).
- T4 `none_selectable`: all ERROR → `(-1,-1)`.
- T5 `load_video_state_excluded`: `LOAD_VIDEO` not selected.

### 4.2 `filter_episode_list`
- T6 `filter_unseen`: flags `0x2`, episodes seen/unseen → only unseen kept.
- T7 `filter_downloaded_not`: flags `0x10`, downloaded `is_downloaded` closure → only not downloaded kept.
- T8 `selected_always_kept`: flags `0x4` (show seen), selected episode unseen → kept and appended last.
- T9 `missing_selected_is_error`: selected id absent → `EpisodeNotFound`.
- T10 `combined_filters_are_conjunctive_per_clause`: flags `0x2 | 0x20` (unseen AND bookmarked) with one episode failing each → correct OR-of-clauses semantics: an episode is removed if it violates ANY active clause.

### 4.3 `build` (playlist)
- T11 `source_order_inverted`: anime sorting SOURCE, `sortDescending=false`, sourceOrder 1,2,3 → output order 3,2,1 (E-level quirk, §2.6 behavior).
- T12 `number_ascending`: sorting NUMBER → ascending episode_number.
- T13 `downloaded_only`: pref on → only downloaded kept.

### 4.4 `TrackSelect` (`preferred`)
- T14 `first_pref_language_with_match_wins`: prefs `["ja","en"]`, tracks `[en-track, ja-track]` → ja-track.
- T15 `whitelist_beats_order`: lang en, whitelist `["Signs"]`, tracks `["English", "English Signs"]` → "English Signs".
- T16 `blacklist_removes`: blacklist `["Commentary"]` → commentary removed.
- T17 `empty_pref_uses_system_locale`: prefs `""` → system locale (inject locale) match.
- T18 `token_trim_regression` (E-21): blacklist `"Commentary, "` → must NOT exclude everything.
- T19 `regex_anchored` (E-22): language `"xen"` must not match `en`.
- T20 `no_match_returns_none`.
- T21 `audio_ignores_whitelist`: audio mode uses empty whitelist/blacklist.

### 4.5 `merge_chapters` (ChapterUtils)
- T22 `external_start_at_zero_snaps`: first stamp start 0.4 → 0.0.
- T23 `end_marker_added_when_not_adjacent`: stamp end 90, next start 120 → end marker (index -1) at 90.
- T24 `end_marker_skipped_at_duration`: stamp end equals duration ±1 → no end marker.
- T25 `mpv_chapter_dropped_within_one_second_of_external`.
- T26 `empty_adjacent_names_dedup` (E-43).
- T27 `null_duration_sentinel` (E-45): end 1400, duration None → end marker present.
- T28 `external_color_only_for_known_type`: `Other` → unspecified color; `Opening` → 0xFFD8BBDF.

### 4.6 `SkipIntroMachine`
- T29 `netflix_countdown_five_then_seek` (behavior §6.8): calls at t=0..5 produce waits 5,4,3,2,1,0; seek on the sixth call; labels "don't skip" for 5..1; toast on the first call only.
- T30 `cancel_during_countdown` → `waiting == -1`, label is skip label.
- T31 `per_second_not_per_call` (E-10): extra sync call at t=0 does not advance.
- T32 `reset_on_new_chapter` (E-09): after -1 in chapter A, chapter B intro starts at 5.
- T33 `manual_mode_label`: label equals `player_skip_action(chapterTypeName)`.
- T34 `auto_skip_seeks_immediately`.
- T35 `other_chapter_clears_label`.

### 4.7 `aniskip` (client and selection)
- T36 `decode_live_sample`: the JSON from design §1.5 decodes; `found=true`; two results; labels "Ending" and "Opening"; `ChapterType` Ending and Opening.
- T37 `not_found_is_none`: `{"found":false,"results":null,...}` → None.
- T38 `unknown_skip_type_tolerated` (E-04): `skipType:"mixed-ed"` → item kept as Other, not whole response failure.
- T39 `anilist_zero_is_none` (E-03 fix): `idMal:null` → no AniSkip request.
- T40 `first_tracker_selection` (E-03): tracks `[Kitsu, MAL]` → MAL id used.

### 4.8 `seek` (actions)
- T41 `seek_by_flags`: `preciseSeek=false` → `"relative"`; `true` → `"relative+exact"`.
- T42 `seek_to_flags` (E-25): default precise → `"absolute"`; `false` → `"absolute+keyframes"`.
- T43 `seek_to_out_of_range_ignored` (E-11) and queued when duration 0.
- T44 `double_back_accumulates` (E-19): -10 then -10 → -20.
- T45 `double_tap_dispatch_table`: left/center/right × every enum value, including center no-op for Seek and Switch.

### 4.9 `volume` (actions)
- T46 `boost_up_uses_mpv`: system at max, mpv 100, change +1 → mpv 101.
- T47 `boost_down_at_100` (E-06): system max, mpv 100, change -1 → system down one step, mpv stays 100.
- T48 `cap_limit`: mpv at `volume-max` → no further change.
- T49 `set_mpv_volume_shows_slider_only_on_change`.

### 4.10 `aspect`
- T50 `fit_sets_panscan_zero_and_override_minus_one`.
- T51 `crop_panscan_one`.
- T52 `stretch_uses_monitor_ratio` (test with injected geometry 1920x1080 → 1.7777…).

### 4.11 `progress` and `completion`
- T53 `completion_threshold_085`: total 1000 s, at 850 s → completion fires; at 849 → not.
- T54 `completion_once` (E-14): many ticks after threshold → one completion side effect.
- T55 `seen_set_before_save` (E-14): save payload has `seen=true` at the completion tick.
- T56 `incognito_no_progress_without_tracker`: incognito true, no trackers → no DB write (`shouldTrack` false), but history also suppressed.
- T57 `incognito_with_tracker_writes_progress`: incognito true, tracker present → progress written, history not.
- T58 `history_upsert_shape`: `AnimeHistoryUpdate(episodeId, seenAt)`.
- T59 `delete_after_read_slots`: slots 0 → current episode queued; slots 1 → previous queued; -1 → none.
- T60 `download_ahead_once` (E-13): only once per episode; requires current and next downloaded.
- T61 `download_ahead_range_035`: below 35% no download.
- T62 `bookmark_updates_memory` (E-15).

### 4.12 `loadHosters` / `loadVideo` (session, with fakes)
- T63 `saved_selection_loads_first`: hosters with saved (1,0) → loads (1,0) first, no fallback call.
- T64 `saved_selection_fails_then_best`: (1,0) fails → fallback selects best.
- T65 `no_videos_error`: all hosters empty → `NoAvailableVideos` key.
- T66 `loading_hoster_defers`: a hoster still `Loading` and no selectable video → `load_video` returns false, selection reset to (-1,-1), no error.
- T67 `cancel_resets_states`: cancel token → all hosters back to `Idle`.
- T68 `single_load_in_flight` (E-38): two concurrent loads serialized.

### 4.13 `lua` dispatcher
- T69 `show_text_round_trip`.
- T70 `toggle_ui_table` (every row of behavior §12).
- T71 `empty_value_not_cleared` (guard) vs `value_cleared_after_dispatch`.
- T72 `malformed_picker_is_ignored` (E-53): no panic, property cleared.
- T73 `seek_by_text_split_limit2`: `"10|Skipped 10 s"` → text keeps `|` after the first.

### 4.14 `settings`
- T74 `all_defaults_match_source`: a table test over every key in `p2-data-and-config.md` §4 asserting the default value (catches typos).
- T75 `speed_presets_count_12`.
- T76 `audio_channels_values`, `subtitle_justification_values`, `border_style_values` (exact strings).

### 4.15 Flags and masks
- T77 `intro_length_mask_roundtrip`: write 85 → read 85; write 0 → disable bit set, read 0.
- T78 `intro_length_clamp` (E-27): write 300 → rejected (not truncated to 44).
- T79 `episode_flags_masks`: each mask constant equals the source value (table).

## 5. Fixtures

- `fixtures/anime_flags.json` — episode_flags/viewer values for T6-T10 and T77-T79.
- `fixtures/episodes_20.json` — 20 episodes, mixed seen/bookmark/filler/download states, sourceOrder 1..20, episode_number 1..20 plus two 12.5 and -1 (unrecognised).
- `fixtures/tracks_mixed.json` — mpv-like track list: `[{"type":"audio","id":1,"lang":"jpn","title":"Japanese"}, {"type":"audio","id":2,"lang":"eng","title":"English"}, {"type":"sub","id":1,"lang":"eng","title":"English Signs"}, {"type":"sub","id":2,"lang":"eng","title":"English Commentary"}, {"type":"sub","id":3,"lang":"jpn","title":"Japanese"}]`.
- `fixtures/aniskip_21_1.json` — the live response recorded in design §1.5 (verbatim).
- `fixtures/aniskip_not_found.json` — `{"found":false,"results":null,"message":"Not found","statusCode":404}`.
- `fixtures/aniskip_mixed_ed.json` — results containing `skipType: "mixed-ed"` (synthetic, for E-04).
- `fixtures/anilist_mal.json` — `{"data":{"Media":{"idMal":21}}}`; `fixtures/anilist_null.json` — `{"data":{"Media":{"idMal":null}}}`.
- `fixtures/chapters_mpv.json` — `[{"title":"Intro","time":0},{"title":"Part 1","time":300}]`, duration 1420.
- Fake `AnimeSourceApi` (in-memory, configurable hoster/video results, optional delays, optional failures).
- Fake `PlayerDeps` with recorded writes (for T46-T62).
- Fake mpv (records commands and property writes; no libmpv).
- A fake `SystemVolume` (records set/get).
- A fake clock (`Instant` injected) for the throttle and countdown tests (T29-T32, E-20).

## 6. Manual test script (desktop, once the GL surface exists)
1. Open an anime with three episodes; play episode 1 to 85 %; confirm "seen" flag after the threshold and history row updated.
2. Enable netflix skip intro with a known AniSkip entry; confirm the countdown labels 5..1 and the seek at 0.
3. Double-click left twice; confirm overlay shows -20 (E-19).
4. Scrub the seek bar quickly; confirm the thumbnail updates to the final position (E-20).
5. Select a hoster in the quality sheet that is in ERROR; confirm the row is not clickable.
6. Set sleep timer 3 s; confirm pause after 4 s and the pill.
7. Kill the process during playback; relaunch; confirm position restored (session file).
