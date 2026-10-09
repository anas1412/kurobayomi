# Area 10 - edge cases, failure modes, tests (part 1 of 1)

Kotlin paths repo-relative to `/home/blackbox/aniyomi-src`. Edge-case ids `EC-nn` are referenced from p1-tasks.md.

## 1. Upstream tests

No `*Test.kt` exists under `app/src/main/java/eu/kanade/domain/` or `app/src/test/` for this scope (`app/src/test/java/mihon/core/migration/MigratorTest.kt` belongs to area:11). The upstream tests that exercise helpers THIS area calls, to be ported by their owning areas and re-run by T-10a-036 as integration inputs:
- `domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt` (number parsing inside the chapter sync, area:07),
- `domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt`, `domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt` (interval/next-update computed after every sync, area:07),
- `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt` (area:07),
- `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt` (area:08).
All interactors in this area are therefore new-test territory.

## 2. Edge cases and failure modes

### Sync (chapters, episodes, seasons)
- EC-01 Source returns an empty list for a non-local source: throw `NoChapters`/`NoEpisodes`/`NoSeasons`; DB untouched. For the local source an empty list is legal and removes nothing special (every db row becomes "removed" and is deleted, see EC-02).
- EC-02 Source returns a list that lacks every existing chapter (site reorganised URLs): all db chapters are removed and all source chapters are new; read/bookmark state is recovered ONLY through matching chapter numbers (>= 0). Chapters with unrecognised number (-1) lose state. Test with a fixture of 5 chapters renamed from `/old/N` to `/new/N`.
- EC-03 Two source chapters with the same URL: first wins, second dropped before `sourceOrder` is assigned.
- EC-04 Two removed db chapters with the same number: the map keeps the entry of the LOWEST `dateFetch`; read/bookmark sets are unions.
- EC-05 A new chapter whose number equals a read db chapter that is NOT removed (alternate scanlation): marked read only if preference contains `"new"`; if it is also a number of a removed chapter the removed twin's state overrides (read may flip back to false).
- EC-06 Source chapter with `date_upload == 0` listed first: gets `now`; the next chapters with real dates raise `maxSeenUploadDate`; an older chapter without date later gets that max (never newer than the maximum seen so far, but possibly newer than its real age).
- EC-07 `prepareNewChapter` mutates the name so that it now equals a name already used: no uniqueness is enforced; two rows with different URLs and equal names coexist.
- EC-08 `ShouldUpdateDbChapter` always true because the source scanlator vanished (null vs stored text): row re-written each sync, scanlator never cleared (coalesce). Test asserts the list is non-empty on every run and the DB value unchanged.
- EC-09 `manualFetch = false`, `manga.fetchInterval != 0`, `manga.nextUpdate >= fetchWindow.first`, no changes: NO write at all (neither fetch interval nor last_update).
- EC-10 No changes but `fetchWindow = (0, 0)` and `manga.nextUpdate (> 0) < 0` is false: interval not recomputed unless `fetchInterval == 0` or `manualFetch`.
- EC-11 Download rename: db chapter has a downloaded folder, the source renamed the chapter so the directory name changes: `renameChapter` runs BEFORE the DB write. OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:380-407`): no old folder found -> silent return; new name equals old name -> return; `renameTo` fails -> `logcat(ERROR) "Could not rename downloaded chapter: ..."` and the sync continues (the folder keeps the old name, so the chapter shows as not downloaded). A `.cbz` file keeps the `.cbz` suffix. Rust: the `DownloadFacade::rename_chapter` contract is infallible-with-logging in the same way; an `io::Error` bubbling out must not abort the sync.
- EC-12 Exclusion filter: the returned list drops chapters whose scanlator is in the excluded set, but the chapters are still inserted. Anime has none.
- EC-13 Season sync: source returns the parent's own URL among seasons: it becomes its own child (`parent_id = own id`) - the Kotlin does not guard; Proposal: skip any season whose `url == anime.url && source == anime.source` and log WARN.
- EC-14 Season sync with removed seasons that are favourites in the library: only `parent_id` is cleared; the library entry stays.
- EC-15 Related anime: `source.supportsRelatedAnime == false` or source missing: return without touching the DB or the TTL; `forceRefresh = true` bypasses the TTL; group with all entries filtered out (only the parent's own URL) is dropped.

### Updates and flags
- EC-16 `awaitUpdateFromSource` with `remote.thumbnail_url == ""` or null: cover untouched and `thumbnailUrl` update null (column unchanged).
- EC-17 Local manga with a new cover URL: `coverLastModified = now` WITHOUT deleting the cache (branch 3 precedes `deleteFromCache`).
- EC-18 Favorite entry: title never overwritten by `awaitUpdateFromSource`.
- EC-19 `remote.genre` blank but non-null: `getGenres()` = null -> genre column unchanged in `awaitUpdateFromSource` (null = no change) but `Manga.copyFrom` clears it (EC-20).
- EC-20 `Manga.copyFrom(other)` with `other.genre = ""` sets `genre = null` in the in-memory copy.
- EC-21 `awaitSetSkipIntroLength(id, 300)` stores 44 and clears the disable bit; `0` sets the disable bit and length 0; negative values: `flag and 0xFF` of a negative Long (two's complement) stores `flag & 0xFF`, e.g. `-1 -> 255`. The Rust function must take `i64` and mask identically; the UI limits input to 0-255.
- EC-22 `awaitSetNextEpisodeAiring(id, (0, 0))`: clears both fields (stored as zeros). Episode above 65535 or airing time above `2^32 - 1` is truncated by the mask without error.
- EC-23 Concurrent `SetMangaViewerFlags.awaitSetReadingMode` and `awaitSetOrientation`: lost update in Kotlin; atomic SQL in Rust must show both bits set after parallel calls (test with `join!`).
- EC-24 `getMangaById` of a deleted manga inside `SetMangaViewerFlags`: Kotlin throws; Rust atomic UPDATE affects 0 rows and returns `Ok(false)` (documented deviation).
- EC-25 `awaitUpdateFavorite(id, false)`: `date_added = 0`; re-adding later sets a new `date_added`.

### Status setters
- EC-26 Mark read when all chapters already read: `NoChapters` (not Success); mark unread when chapters are unread but `last_page_read > 0`: they ARE updated (page reset).
- EC-27 `removeAfterMarkedAsRead` on and the source uninstalled: `deleteDownload` is a silent no-op (source lookup null).
- EC-28 `updateAllChapters` throws (DB locked): `InternalError`, no download deletion, no partial update (single transaction).
- EC-29 Caller cancelled (reader closed) during the update: the update and downloads deletion still complete (non-cancellable).

### Sources, extensions, preferences
- EC-30 `enabledLanguages` empty: only the local source remains in `GetEnabledSources`.
- EC-31 A source is both pinned and last used: two rows, the second with `isUsedLast = true`, `pin = 0b01`; list key `"<id>-lastused"` keeps keys unique.
- EC-32 `lastUsedSource` points to a disabled/unknown source: no duplicate row.
- EC-33 Same source name in different languages: stable sort keeps repository order.
- EC-34 `Pins.minus` on unpinned source yields `0b10` (documented quirk); never shown.
- EC-35 Migration list: stubs first ascending, last descending; local source excluded; equal counts keep input order (`sortedWith` is stable; `Collections.reverseOrder` of a comparator keeps stability).
- EC-36 Language sort: `"all"` first within its block (enabled block or disabled block); `zh-CN` and `zh-TW` map to `zh-Hans`/`zh-Hant` display names; an unknown tag falls back to the tag text itself.
- EC-37 `GetExtensionsByType`: `available` item whose extension has several sources in different enabled languages yields several rows with distinct pseudo package names `pkg-<sourceId>`; extension with sources in disabled languages only yields none (manga: unless it has zero sources and its own lang is enabled).
- EC-38 Obsolete installed extension with an update: appears in `updates` (partition by `hasUpdate`) sorted obsolete-first.
- EC-39 `TrustExtension.isTrusted` with empty fingerprints: Kotlin throws; Rust returns false.
- EC-40 `trust()` for a package already trusted at an older version: the older triple is removed (prefix `pkg:`); a package whose name is a prefix of another (`a.b` and `a.b:c` cannot happen because `:` is not allowed in package names) is safe.
- EC-41 `incognito` for a source whose extension is missing: false.
- EC-42 Data-saver ids and hidden ids are stored as decimal strings of SIGNED 64-bit ids (source ids can be negative? Source ids are derived from an MD5 prefix masked with `Long.MAX_VALUE`, so non-negative; the local source is `0`). Parse failures of stored strings are ignored (never matched).

### Tracking
- EC-43 `TrackChapter` with `chapterNumber <= track.lastChapterRead`: skipped, no network.
- EC-44 Several trackers: one failing must not block others; failures are logged only.
- EC-45 Offline manga: Kotlin still tries the network (no check) and queues on failure; Rust: `Connectivity::is_online() == false` short-circuits to queue.
- EC-46 Queue keeps the maximum progress per track; a lower later value is ignored.
- EC-47 Replay: track deleted meanwhile -> entry removed; replay of a track whose tracker was logged out -> `TrackChapter` skips it silently and the entry is NOT removed (stays forever; Proposal: drop entries whose tracker is logged out after the final attempt).
- EC-48 Worker attempts exceeded (4 runs): queue persists; next offline failure schedules again.
- EC-49 `bind` when the tracker returns a track with `id == null`: handled as id -1 for insert; `insertTrack` upserts by (entry id, tracker id).
- EC-50 `bind` start date: first history row without `readAt` makes `startDate` stay 0.
- EC-51 Enhanced sync with remote progress above the highest known chapter: all recognised chapters `<= remote` marked read.
- EC-52 Enhanced sync where `tracker.update` fails: chapters unchanged, WARN logged.
- EC-53 `RefreshTracks` for a tracker logged out: skipped, not reported as failure.
- EC-54 Float queue precision: queued 12.3 replays as 12.3000001907; the check `chapterNumber <= track.lastChapterRead` must compare in f64 with the replayed value to avoid infinite requeue (queue value > stored). Test: add(12.3), add(12.3) is ignored (equal).

## 3. Concurrency and races

1. Sync vs UI toggles (`SetReadStatus` while a refresh runs): sync's step 4 reads `dbChapters` once; a chapter marked read during the sync computation is overwritten only for fields in `updatedChapters` (`name, chapterNumber, scanlator, sourceOrder, dateUpload`), never `read`, so no loss. New chapters are inserted from the pre-sync snapshot: if the user marks a duplicate-number chapter read in between, the duplicate-read rule does not see it. Accepted.
2. Two syncs of the same entry in parallel (library job plus details refresh): both compute "new chapters" from the same snapshot and both insert -> duplicate rows with the same URL (`data/src/main/sqldelight/data/chapters.sq` has no UNIQUE constraint on `(manga_id, url)`, only the `chapters_unread_by_manga_index` index). Proposal: per-entry async mutex map `SyncLocks` in `Domain`, held for the planning+write section.
3. Delayed queue file written from two tokio tasks: protected by the store mutex; file replace is atomic.
4. `DelayedTrackingWorker::schedule` called while running: restarts (REPLACE semantics) by cancelling the running task via `CancellationToken`, but only AFTER the in-flight replay item finishes (do not cancel mid-request).
5. Preference stream emission order: `combine` emits when any input changes, producing intermediate states (e.g. enabling a language and a source in two writes); the UI debounces at 50 ms is NOT in Kotlin; Proposal: no debounce, but results are `distinct_until_changed`.

## 4. New tests to write (crate `kuro-domain`, `cargo test`)

Table-driven unless noted. Fixture helpers live in `kuro-testkit` (area:01): in-memory `Prefs`, `Db::open_in_memory`, `FakeSource`, `FakeDownloads`, `FakeTrackers`, a deterministic clock `FixedClock`.

| Test | Asserts |
|---|---|
| `flags::set_flag_matches_kotlin` | 12 vectors, e.g. `set_flag(0xFFFF, 0x2, 0x7) == 0xFFFA`, masks of both viewer layouts |
| `flags::anime_viewer_roundtrip` | `with_skip_intro(0,0)` has bit 56; `with_skip_intro(that, 30)` clears it and length 30; `with_airing(0, 12, 1_700_000_000)` reads back `(12, 1_700_000_000)`; masks equal `0xFFFFFFFF<<24`, `1<<56` |
| `flags::intro_length_wraps_to_8_bits` | EC-21 |
| `entry_model::copy_from_genre_rules` | EC-19, EC-20, `getGenres` splitting `"A, B, ,A"` -> `["A","B"]` |
| `entry_model::to_domain_defaults` | `create()` defaults of section 2.2 in p1-behavior.md |
| `update_manga::cover_branches` | five branches, with `FakeCovers` recording `delete_from_cache` |
| `update_manga::title_not_overwritten_for_favorite` | EC-18 |
| `update_manga::favorite_sets_date_added` | EC-25 |
| `sync_chapters::worked_example` | the worked example of p1-topic-chapter-episode-sync.md section 3 verbatim (dateFetch 1_000_003/2/1, returned list `[/c5, /c4]`, c3b read+bookmark+dateFetch 500) |
| `sync_chapters::empty_source_errors` / `local_empty_ok` | EC-01 |
| `sync_chapters::no_change_no_write` | EC-09/10 using a `RecordingRepo` |
| `sync_chapters::duplicate_urls_first_wins` | EC-03 |
| `sync_chapters::deleted_twin_map_keeps_lowest_fetch` | EC-04 |
| `sync_chapters::excluded_scanlators_filter` | EC-12 |
| `sync_chapters::f32_number_not_requeued` | a chapter with `12.1f32` is not "updated" on the second identical sync |
| `sync_chapters::scanlator_never_cleared` | EC-08 |
| `sync_episodes::fillermark_sticky_and_preview_fill` | section 5 rules |
| `sync_seasons::new_updated_removed` | parent_id cleared for removed, parse of season numbers |
| `season_recognition::cases` | `"Boku.no.Hero.Academia.S02.1080p-ITH"` with title `"Boku no Hero Academia"` -> 2.0, `"[FLE] Boku no Hero Academia Season 2 (BD Remux 1080p H.264 FLAC) [Dual Audio]"` -> 2.0, `"Show 2 special"` -> 2.97, `"Show 3.5"` -> 3.5, `"Show 1b"` -> 1.2, no number with hint `-1.0` -> -1.0, hint `-2.0` -> -2.0 |
| `related::ttl_and_cap` | 7-day TTL boundary (`now - last == TTL` refreshes), 10 per group, self-url filtered |
| `set_read_status::results` | `NoChapters`, `Success`, `InternalError`, download deletion only when pref set, grouped by manga |
| `set_seen_status::last_second_reset` | |
| `filters::tri_state_matrix` | all 27 combinations of unread x bookmarked x downloaded; downloaded-only override; local manga counts as downloaded |
| `filters::sort_modes_descending_source_order` | source sort direction inversion |
| `sources::enabled_sources_cases` | EC-30..EC-33, pinned+last-used, local source always present |
| `sources::pins_quirks` | `Pins::UNPINNED.contains(Pin::UNPINNED)`, XOR minus |
| `sources::cmp_ignore_case_java_vectors` | `"a" < "B"`, `"Z" > "a"`, `"Ǆ"`, supplementary char vs BMP (document equal to Java output captured in `fixtures/java_ci_order.json`, generated once by a 20-line Java program) |
| `sources::language_order` | EC-36 list |
| `sources::migration_sort` | EC-35 |
| `extensions::by_type_cases` | EC-37, EC-38, NSFW filter only on installed/available |
| `extensions::sources_label_mode` | `labelAsName` truth table over (1 source, n same-name, n distinct-name) |
| `trust::triples` | trust/replace/revoke, EC-39/40 |
| `incognito::state` | EC-41 |
| `track::track_chapter_skip_and_queue` | EC-43..46 with `FakeTrackers` |
| `track::delayed_store_max_and_namespacing` | EC-46, EC-54, manga/anime key separation, atomic file survives a crash (kill during write simulated by leaving a `.tmp`) |
| `track::worker_backoff_schedule` | with `tokio::time::pause()`: attempts at t=0, 5, 15, 35 minutes, then stop |
| `track::add_tracks_bind_pushes_prefix` | read {1,2,3,5}: remote gets 3; start date from history; zone shift test at a DST boundary using a fixed `UtcOffset` provider |
| `track::sync_progress_enhanced_only` | section 7 examples of p1-topic-tracking-orchestration.md |
| `track::refresh_collects_failures` | one failing tracker among three |
| `registry::every_row_has_a_counterpart` | T-10a-037 |

## 5. Fixtures needed

- `fixtures/chapters_worked_example.json` (db rows + source rows + expected plan) and an episode twin.
- `fixtures/sources.json` (30 sources over 8 languages, with pins, hidden ids, last-used) and `fixtures/expected_enabled_sources.json`.
- `fixtures/extensions.json` (installed 6 incl. obsolete + update + nsfw, untrusted 2, available 10 incl. multi-source and zero-source).
- `fixtures/java_ci_order.json`: pairs and the integer result of `String.CASE_INSENSITIVE_ORDER.compare` from a JVM, generated once (script in `tools/`, output committed).
- `fixtures/lang_names.json`: the CLDR native names for the tags used in the sort test.
- A tiny `FakeSource` implementing `prepare_new_chapter` to simulate extensions that rewrite names/dates.
- Golden `ComicInfo.xml` for `get_comic_info` (area:14 serializer) with number `12`, `12.5`, `-1`, categories, three URLs.
