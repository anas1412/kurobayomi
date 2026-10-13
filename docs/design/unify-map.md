# Unify map: one system with a kind flag

Source: seven per-area maps (data, downloads, entry page, GUI lists, sources,
backup/track/local, player/reader), read-only pass on 2026-10-12.
Line counts are estimates from those maps, not measured diffs.

## 1. Decision

- Inside the app there is ONE system. Every entry and item carries `kind` (anime | manga).
- Two copies exist only at the edges where an outside format really is two: .tachibk sections, aniyomi:// / tachiyomi:// links, AniList anime vs manga lists (a parameter).
- Parts that really differ stay two behind one shared interface: reader vs player, page fetch to .cbz vs ffmpeg to .mkv.
- ONE database file with one schema and a `kind` column. It replaces anime.db and manga.db.
- The schema starts fresh at version 1. Old data comes in only through backup import. Raw Aniyomi DB import is deleted.
- Must keep: Aniyomi/Tachiyomi backup restore, raw source ids, and the repo link format (index.min.json, already one format with a kind per source).

## 1b. The user's calls (2026-10-12)

The ten "Decide" items in section 5, settled:

1. Downloads at once: the user's setting (Settings > Downloads), for both kinds.
2. Bulk-download warning thresholds: 30 / 15.
3. A finished download whose file already exists: replace it.
4. Default list filters when a title opens: a session copy (the anime way); the row is written only when the user changes a filter.
5. Start/Resume reads the filtered, sorted list.
6. A genre chip searches the entry's own source.
7. Mark as read/seen follows the "delete after marked read" setting the same way from Library, Updates and the title page.
8. Download ahead at 25 %.
9. Escape leaves fullscreen first, then goes back (closes the reader or player); never while a text entry has focus.
10. A broken local metadata side file (chapters.json / episodes.json) is ignored, for both kinds.

Also decided: dialogs have no GTK header bar; the desktop frames them (`ui::modal`).

## 1c. Priority (user, 2026-10-12)

Correctness and drift, duplication, and UI consistency come first. Order: finish step 3 (lane E), wire the tracker
update for reader and player, then steps 4 to 12 one at a time, each fixing the drift items tied to it (section 5).
The UI consistency fixes from docs/audit/ui-shared.md run in parallel lanes on screens the current step does not touch.
The "never used" cleanup comes last. Every step ends with the full suite and real flows on the test display.

## 2. What stays two and why

| Stays two | Why |
|---|---|
| Manga reader viewers vs mpv player | Decoded pages with zoom/tiles/crop vs a libmpv render context with seek/tracks/skip |
| Page list + images vs hoster resolve + video | Source content really differs: `Pages` vs `Videos` strategy on one Source trait |
| Chapter pipeline (.cbz + ComicInfo) vs episode engine (ffmpeg .mkv, torrent) | Different artifact; both become one `UnitRunner` behind one queue |
| Open a downloaded unit (page list vs file URL) | Reader wants pages, player wants one file; the lookup is shared |
| Keyboard tables, settings sheets | Page turn/zoom vs seek/volume; only the key dispatch frame is shared |
| Local source content (ComicInfo/EPUB/archives vs video/ffmpeg frames/seasons) | Real format difference; the catalogue shell is shared |
| Anime-only features: seasons, airing, related, skip intro, summaries, custom buttons | No manga twin; nullable columns + anime strategy sections |
| Manga-only: scanlator exclusion sheet | No anime twin today (could later be enabled for both) |
| Viewer bit layouts (reading mode vs intro/airing) | Different meaning; one column + one masked write |
| Tracker media support (Simkl/Jellyfin anime, MangaUpdates/Komga/Kavita/Suwayomi manga) | Data: `Tracker::supports(kind)` |
| Recognition regexes (chapter vs episode number) | Parsing rules really differ |
| Upcoming statuses: manga [1,4], anime [1,4,7] | Real difference; `Kind::upcoming_statuses()` |
| .tachibk wire structs (BackupManga/BackupAnime, fields 1-106 vs 501-504) | Outside format; translated right after decode |
| Aniyomi pref key names per kind (sort, filters, default category, category id sets, repo lists) | Must round-trip in the backup prefs section; one accessor `prefs.x(kind)` inside |
| aniyomi:// and tachiyomi:// links, argv/.desktop action names, `manga=`/`anime=` ids | Outside format; parsed into one `DeepLink::{Library(kind), Entry(kind,id), ...}` |
| Tracker API calls (AniList ANIME/MANGA, MAL endpoints, status codes) | Remote API is two; kind is a parameter, as Kitsu/Shikimori/Bangumi already do |
| Two download roots (one folder per kind) | User-visible folder layout; chosen by `StorageManager.downloads_dir(kind)` |
| Two plasmoid packages | Two installable widgets; generated from one source tree |

## 3. The shared core

| Type / trait | Lives in | Replaces |
|---|---|---|
| `Kind { Manga, Anime }` (serde lowercase, `noun()`, `prefix()`, `upcoming_statuses()`, Aniyomi key helpers) | `kuro-platform::kind` (lowest crate, no internal deps), re-exported | ~14 enums: DbKind, CategoryKind x2, EntryKind x2, restore Kind, repo Kind, track Kind, feed Kind, nav Kind, BrowseKind, SourceKind, LocalKind, art::Owner, viewer::EntryKind, recipe Kind, app::source::Kind, plus `anime: bool` params |
| One schema `kurobayomi.db` v1: `entries(kind, source, url, ...; nullable fetch_type, parent_id, season_*, background_*, memo)` UNIQUE(kind, source, url); `items(entry_id, done, bookmark, progress, total_seconds NULL, fillermark, summary, preview_url, scanlator, memo)`; `history(item_id, last_at, duration_ms)`; `categories(kind, ...)`; `entry_categories`; `tracks(entry_id, tracker_id, last_item, total_items, ...)`; `stub_sources(kind, id)`; `excluded_scanlators`; `anime_relations`; `custom_buttons`; `download_queue(kind, unit_id, ...)`; `download_pending_delete`; `track_retry_queue`; one libraryView/historyView/updatesView with kind | `kuro-db` (one SchemaBundle, one heal, one `Db::open(paths)`) | schema_manga/schema_anime, two SQL trees, two bundles, AnimeDb |
| `Entry`, `Item`, `Track`, `EntryRepo`, `ItemRepo`, `TrackRepo`, `HistoryRepo` (+ one recorder with 12 h clamp), `StubSourceRepo` | `kuro-data` | Manga/Anime, Chapter/Episode, *TrackRepo, *HistoryRepo, *StubSourceRepo, kuro-data updates.rs |
| One item flag layout (anime superset) + one accessor/setter set; one `SortType` codec | `kuro-domain::entries` (codec moved low enough for kuro-data/kuro-prefs) | chapter_* / episode_* flag sets, 4 sort codecs |
| `SyncItems`, `SetDoneStatus`, `UpdateEntry`, one `image_action` rule, `CategoryService` (only write path), `UpdatesRepo` | `kuro-domain` | SyncChapters/SyncEpisodes, SetRead/SetSeen, UpdateManga/UpdateAnime, kuro-data create_*_category |
| `Queue` actor + `QueueItem{kind, UnitRef, EntryRef, SourceKey, Progress}` + `trait UnitRunner` (ChapterPipeline, EpisodeEngine) + one `Layout`, one `DownloadIndex` (owned by Shell), one `QueueStore`, one pending store, one `DeleteRules`, one notifier, full `DownloadEvent{kind, unit_id, entry_id}`; `QueueModel` as the only page model | `kuro-downloads` | DownloadManager/Downloader + AnimeQueue, both indexes, both layouts, view_model/page/queue_view |
| `EntryPage` + `EntryModel` + `EntryRunner` + `EntryStore` + `EntryBackend` + `DownloadPort`, with a `KindStrategy { open_item, extra_sections, overflow_extras, bulk_extras }` | `src/entry_core` (anime/ keeps only strategy add-ons) | the top-level vs `anime/` copies |
| `Vocab::of(kind)`, `kind_switch(build)` | `src/screens` / i18n | scattered wording arms, 3 switch widgets |
| `trait Tracker { supports(kind), status_list(kind), update(&Track), bind, search(kind, q), refresh, delete, enhanced() }` + one flow set over `TrackCtx { tracks, units, history }` | `kuro-track` | Manga/Anime traits x3, TrackRecord, 30+ fn pairs |
| `ItemSession` (writer actor, HistoryTimer, `start_position`, `on_completed` hooks, `build_item_list`, `incognito_for`, KeepAwake, DeleteQueue, download-ahead, crash restore, `locate()`), `MediaPage` frame | `src/media_session` (new module, shared by `src/reader` and `src/player_view`) | reader session/writer/chapters/nav + player store/neighbours + dead kuro-player session layer |
| `trait Source { kind(), listing/search/details/items, request_headers, content() -> Pages | Videos }`, `SEntry`, `SItem`, `EntriesPage`, one `Status` with `Other(i32)`, one `ConfigurableSource` (with on_settings_changed), one `ResolvableSource` | `kuro-source` | MangaSource/CatalogueSource/AnimeSource, SManga/SAnime, SChapter/SEpisode |
| `SourceHost`: `directory::current(kind)` over one source list; `instantiate(bytes, kind)`; one `WasmSource`, one `ScrapeSource{kind}`; one extension state, one trust store, one icon cache, one update check | `src/browse/directory` + `kuro-source-wasm` + `kuro-extensions` (repo.rs links) | 4 package systems, LOADED static, live_anime_sources, builtin_anime_sources |
| Backup edge: `EntryIn { kind, items, extras }` from BackupManga/BackupAnime; one `restore_entry`; `LibraryReader { entries(kind, q), categories(kind) }`; shared `BackupSourceRef/BackupHistoryRow/BackupTrack` | `kuro-backup` (the only Aniyomi-aware crate, plus link parsing in `src/shell/uri.rs`) | restore/manga.rs vs anime.rs, two creators |

Key constraint: source ids collide across kinds (local source is 0 for both; `generate_id` can repeat).
So everything source-keyed uses (kind, id): entries unique key, stub sources, hidden/pinned sets, lookups.
Entry ids become unique across kinds, so events like `TrackChanged { entry_id }` stop being ambiguous.

## 4. Steps

Rule for every step: `cargo build` + full `cargo test` green, app launches, and a real .tachibk restore
(one Aniyomi backup with both sections) still works. Real flows run on the test display.
Backup restore is re-pointed minimally in each step that changes what it writes to (steps 3, 4, 5);
step 12 does the cleanup.

| # | Step | What merges | Main files | ~Lines out | Risk | Verify (beyond build + tests) |
|---|---|---|---|---|---|---|
| 1 | Dead-code sweep | Delete copies with no app caller: legacy importers + `legacy-import` feature, AnimeDb, kuro-db restore.rs, kuro-data updates.rs, kuro-data delete/reorder/set_*_sort category fns, kuro-domain LibraryRepo (move its one test), kuro-player dead session layer (playlist, episodes, episode_list, completion, downloads, tracking, marks, desktop, model/state/view/window/session_*), downloads view_model.rs/page.rs/anime queue_view.rs, DeleteChapter/EpisodeDownload, crate restore_queue/RestoreRow, source factory.rs, kuro-prefs per-kind sort enums, unused HTTP/parsed source bases | kuro-db/{legacy,import,anime_legacy,anime,restore}.rs + sql/legacy, sql/anime/legacy, fixtures; kuro-data/updates.rs; kuro-player/src/*; kuro-downloads/{view_model,page,anime/queue_view}.rs; kuro-source/factory.rs; kuro-source-http/{http_source,parsed,anime_http,anime_parsed}.rs | ~8,000 | low | Launch; open a manga chapter and an anime episode; download queue page renders |
| 2 | One Kind | `kuro_platform::kind::Kind` everywhere; drop `anime: bool` params; one `Step`, one `now_ms` | every crate's local enum; src/feed, src/shell/nav, src/browse/route, queue_model, kuro-images art/viewer | ~400 | low | Anime/Manga switch in Library, Updates, Browse; deep link `--show-anime <id>` |
| 3 | One database | Fresh schema v1 (section 3), one bundle/heal/open, one statement module per table with kind bound; kuro-data per-kind repos kept as thin wrappers over the unified tables so callers compile; delete refuse_foreign/NeedsLegacyImport/TooOldForImport/LegacyFile; episodes get the url index; unified libraryView with favourites-first prefilter; kuro_lower in history search; restore re-pointed to new tables | kuro-db/{schema*,statements_*,views_anime,models,runtime,flags}.rs + sql/; kuro-data/*; src/app data wiring (`manga_db`/`anime_db` -> one `Arc<Db>`); kuro-backup restore driver | ~2,600 | high | Fresh profile; restore .tachibk; library shows both kinds with counts; local manga "X" and local anime "X" both appear; history search with non-ASCII manga title; open chapter + episode, progress saved |
| 4 | One entry/item model and domain | `Entry/Item/Track` + repos replace wrappers; one HistoryRepo + recorder (12 h clamp, incognito clear, anime duration); CategoryService only write path (duplicate/system checks for all callers); one UpdatesRepo; one flag layout (anime superset) with manga chapterFlags translated at the backup edge; one sort codec; prefs accessors take Kind; one set_viewer_bits + reset(kind); SyncItems; SetDoneStatus/UpdateEntry/network_to_local/image_action; delete_non_library for both kinds (reset history + orphan seasons) | kuro-data/*; kuro-domain/{entries,items,app,history,updates,category,library}/*; kuro-prefs/{library,category,import,download,sort}.rs; kuro-backup restore (flag translation) | ~5,500 | high | Library filters/sort per kind survive restart; restore backup, manga chapter sort order matches Aniyomi; create duplicate category name from Browse (refused); library update on both kinds; read 2 chapters, history shows time |
| 5 | One download system | One Queue actor + UnitRunner; one store (unreadable guard) in the unified DB; one restore with Waiting rows for both kinds; one Layout (NotFound-tolerant remove); one DownloadIndex on Shell with watcher and one snapshot; one pending delete with a flush that runs (reader delete_disk folded in); one DeleteRules (explicit delete forces remove_bookmarked); one notifier/events; one connectivity gate + ProcessHold; spawn_blocking + SeriesLocks for all disk work; IndexBackend answers both kinds and `download_host::install` is called; renames update the index | kuro-downloads/** ; src/downloads/*; src/updates/download_host.rs; src/shell/host.rs; src/library_update/renames.rs; src/reader/delete_disk.rs | ~3,300 | high | Queue 3 chapters + 2 episodes, pause/resume/reorder; quit mid-download, restart (anime row with missing source waits); network off stops both; delete-after-read; anime downloaded badge + filter in Library; Updates/Storage show anime size; Reindex downloads |
| 6 | One entry page | EntryPage/Model/Runner/Store/Backend/DownloadPort + KindStrategy; id-anchored Selection + keyboard map + F1 for both; domain filter engine for both; one Download-menu rule (skip_filtered, excludes queued); Notices; typed DetailError; LatestWins + 10 Hz throttle; one duplicate dialog; one add flow (bind_enhanced + defaults in one transaction); auto-download after manual refresh for both; one FAB/header rule | src/entry_core/*.rs, src/entry_core/anime/* (keep only seasons/airing/related/intro/summary/torrent/open-item) | ~3,500 | high | Open manga + anime from Library and Browse; filter then shift-range select; Download "next 5" twice (no double queue); add to library with categories + enhanced tracker; manual refresh; seasons grid, related, skip intro still work; scanlator sheet on manga |
| 7 | One list layer | Collapse kind arms in Library/Updates/History/Upcoming/Storage/Browse/migration/library update/feed/search provider/settings/tracker host; Vocab::of(kind); one kind_switch; one favourite store (after_add = default item flags + tracker bind; entry_categories for both); one migrate tx (deletes anime downloads too); one feed query; DeepLink enum by kind; `extension_repos` renamed from manga_extension_repos | src/library/*, src/updates/*, src/browse/*, src/library_update/*, src/feed/*, src/shell/{library_catalogue,deep_link_page,argv,intents,uri}.rs, src/settings_ui/* | ~1,900 | medium | Mark read from Library and from Updates (same result); migrate an anime between sources; feed CLI for both plasmoids; `.desktop` actions; library CSV export |
| 8 | One tracking layer | Track/TrackSearch with kind; one Tracker trait with `supports(kind)`; one flow set (ctx/sync/progress/remote/edit/remove/bind); per-tracker kind match for API + status ids (check completion id per kind for MAL); one repo seam; one fake set; TrackChanged carries a unique id | kuro-track/src/** | ~2,200 | medium | Bind AniList on one manga + one anime; set progress, score, dates; remove; retry queue drains after offline |
| 9 | One media session | ItemSession over the unified writer (off GTK thread, coalesced, flush barrier); HistoryTimer for both; start_position with preserve pref; on_completed hooks (track, delete-after, duplicates) for both; build_item_list honours entry filters/sort for next/prev; incognito_for (global + per-source pref, live follow); KeepAwake for playback; locate() prefers downloads; crash restore by (kind, entry, item, position); MediaPage Escape/fullscreen frame; one LoadingIndicator | new src/media_session/*; src/reader/{session,writer,chapters,nav,history_rules,delete_queue,download_ahead,keep_awake,restore,launch}.rs; src/player_view/{mod,store,neighbours,source}.rs; src/local_flow/open.rs | ~1,500 | medium | Watch past 85%: marked seen, tracker updated, file deleted if pref on; next episode skips filtered ones; network off: downloaded episode plays; screensaver stays off while playing; history shows watch time; Escape behaviour per decision |
| 10 | One source system | One Source trait + content strategy; SEntry/SItem/Status; one ConfigurableSource/ResolvableSource (anime deep links wired); one wire set (real anime status sent); one WasmSource + instantiate(kind); one ScrapeSource; `url_without_domain(raw, kind)`; directory::current(kind) everywhere (drop LOADED, live_anime_sources, builtin_anime_sources); one local catalogue helper + `apply_details`, `local_root(kind)` | kuro-source/**, kuro-source-wasm/src/{source,wire,anime_wire}.rs, kuro-source-scrape/src/{manga,anime,engine}.rs, kuro-local/*, src/recipe_sources.rs, src/browse/loader.rs, src/entry_core/anime/{route,remote}.rs, src/screens.rs | ~900 | high | Browse popular/search/filters on a manga and an anime repo source; open entry, refresh; read chapter, play episode; deep link to a source URL for each kind; local folders for both kinds |
| 11 | One plug-in host | repo.rs links are the one live format; one extension state/trust/icons/update check/toggles/logs; delete the manifest.toml loader, the kuro-source-wasm package/install/startup, and the anime .kuroext store/install/manager/stores_page/dialog (keep one .kuroext reader only if still wanted); Aniyomi legacy repo lists move to the backup/link edge; orphan scan uses directory names for every source | kuro-extensions/** (anime/* mostly removed), kuro-source-wasm/src/{manifest,package,verify,install,startup}.rs, kuro-domain/extension/*, src/ext_host.rs | ~6,000 | medium | Add a repo via aniyomi:// and tachiyomi:// links; install/update/remove a source of each kind; trusted_extensions pref round-trips through backup; no false orphan-folder log |
| 12 | Backup = the only Aniyomi-aware place | One restore_entry over EntryIn; one driver connection; categories splice kept at edge; shared BackupSourceRef/HistoryRow/Track; one LibraryReader; one installed-extensions walk; all Aniyomi names (read/seen, chapterFlags bits, time_read, category lists, pref keys, repo sections) translated here; grep proves no `anime_db`/`manga_db`/Aniyomi table names elsewhere | kuro-backup/src/{schema,restore/*,creator/*}.rs | ~600 | medium | Restore Aniyomi + Tachiyomi/Mihon backups; create a backup, restore it into a fresh profile, diff library/history/tracks/categories/prefs; Aniyomi opens our backup (if a device is available) |

## 5. Drift found

Behaviour differences that look like bugs. "Decide" = no right answer from code; pick once.

### Data (fixed in steps 3-4)
| Drift | Where | Step |
|---|---|---|
| Manga history search: ASCII `lower()`, query not lowered; anime uses kuro_lower | kuro-db/src/statements_manga.rs:562 vs statements_anime.rs:178 | 3 |
| animelibView aggregates every episode (no favourites prefilter); no episodes(url) index | kuro-db/sql/anime/views.sql:3-78 vs sql/migrations/34.sql | 3 |
| Live HistoryRecorder has no 12 h cap and keeps start in incognito; ReadTimer has both but is unused | kuro-data/src/history.rs:332,349 vs kuro-domain/src/history/recorder.rs:9,26 | 4 |
| Duplicate episode url picks an arbitrary row (manga orders by _id and warns) | kuro-data/src/episode.rs:138 vs chapter.rs:116 | 4 |
| Anime clear-non-library keeps reset history and does not orphan favourite seasons | kuro-data/src/anime.rs:711 vs manga.rs:492; logic only in statements_anime.rs:1648,1679 | 4 |
| Anime duplicate check scans favourites in Rust; SQL statement unused | kuro-data/src/anime.rs:542, statements_anime.rs:1697 | 4 |
| kuro-data create_*_category skips duplicate-name/system checks; debounce 50 ms vs 20 ms | kuro-data/src/category.rs:56,88 vs kuro-domain/src/category/service.rs:351, repo.rs:18 | 4 |
| Updates feed: live SQL orders only by datefetch, re-sorted in Rust; tiebreak only in dead copy | statements_manga.rs:564, kuro-data/src/updates.rs | 1, 4 |
| Cover refresh rule written twice (inline delete vs CacheAction) | kuro-domain/src/app/update.rs:29,174 vs source/remote_update.rs:246 | 4 |
| Episode flag constants defined twice; sort codes defined four times | kuro-db/src/schema_anime.rs:245, kuro-domain/src/entries/anime.rs:15; flags.rs library_sort, kuro-prefs sort.rs, kuro-domain library/sort.rs | 4 |
| Category id pref keys listed three times (5 vs 6 per kind) | kuro-prefs/src/{category,import,download}.rs | 4 |

### Downloads (step 5)
| Drift | Where |
|---|---|
| Anime restore runs before sources load; source-less rows deleted, stub-only rows fail to ERROR (manga keeps them waiting) | src/downloads/anime_queue.rs:84-111, anime/store.rs:90-111, anime_io.rs:79 vs src/downloads/mod.rs:98-140 |
| Max active: manga hard-coded 5 (pref ignored), anime reads pref, default 3. Decide | downloader.rs:508 vs anime_queue.rs:65-66, anime/manager.rs:74 |
| Bulk warning thresholds 30/15 vs 20/10. Decide | downloader.rs:31-32 vs anime/enqueue.rs:8-10 |
| Anime store has no unreadable guard: failed read + persist wipes the queue | anime/store.rs:72 vs store.rs:89-92 |
| Blocking disk/SQLite IO on the anime actor, no SeriesLocks; DeleteAnime pauses whole queue | anime/manager.rs:525-539, 593-632, 947 |
| Anime remove_path not NotFound-tolerant | anime/manager.rs:1013 vs manager.rs:590 |
| Downloaded check: index (manga) vs disk stat (anime), also on title pages | downloader.rs:330 vs anime/manager.rs:663,771; entry_core/backend.rs:244 vs anime/download_backend.rs:84 |
| Finalise collision: replace vs refuse + "(id)" name. Decide | pipeline.rs:192-201 vs anime/downloader.rs:158-172 |
| Anime actor emits only Warning/Error; app polls every second for "done" | anime/manager.rs:677,718,737,936; anime_queue.rs:154-164 |
| Anime queue never stops on network loss (network_changed never called) | anime/network.rs; no caller of AnimeQueue::network_changed |
| Start: manga retries ERROR + remembers network stop; anime requeues all; only manga stops on InvalidLocation | downloader.rs:380-431,628 vs anime/manager.rs:690-701 |
| ProcessHold and fs watcher manga-only; two index owners and snapshot formats | downloader.rs:394, watch.rs; shell/host.rs:82-96 vs updates/download_host.rs:137-158 |
| Updates/Storage: anime never "on disk", 0 bytes; install() never called so enqueue/delete from these screens is a no-op for both | src/updates/download_host.rs:102-105,127-130; library/actions.rs:177, misc/runner.rs:128 |
| Library anime download count always 0, item_downloaded always false | src/library/download_facts.rs:13-17, library/mount.rs:331-340 |
| Manga orphan folder names lack "(LANG)" | src/ext_host.rs:103 vs layout.rs:49 |
| Renames bypass both indexes; episode rename never updates AnimeIndex | src/library_update/renames.rs:104-107,131 |
| Pending deletes never flushed; reader has a third delete path | src/reader/delete_disk.rs; flush_pending_deletes has no caller |
| Manga command channel drops commands when full (try_send, 64) | manager.rs:226, downloader.rs:40 |
| Reindex invalidates only the manga index | src/settings_ui/maintenance.rs:190-200 |
| Migration never deletes old anime downloads | src/browse/migrate_live.rs:6-7,353 (step 7) |
| Sidebar waiting count checks only the manga queue | src/shell/sidebar.rs:58 |
| "Maximum downloads" setting governs only anime | src/settings_ui/downloads.rs:24 |
| download_queue.sql repeats the pending-delete DDL; anime layout root optional | kuro-downloads/sql/download_queue.sql; anime/layout.rs:60-80 |

### Entry page (step 6)
| Drift | Where |
|---|---|
| Anime refresh only sees builtin + recipe sources, not installed packages | src/entry_core/anime/route.rs:95 vs backend.rs:158 (step 10 finishes it) |
| Enhanced trackers bound after add only on manga | runner.rs:332-339 vs anime/runner.rs:486-530 |
| Auto-download after manual refresh only on anime | anime/runner.rs:415 vs runner.rs:224 |
| Library defaults: manga writes on every open, anime session copy. Decide (anime way is safer) | runner.rs:136 vs anime/model.rs:151-183 |
| Anime add_to_library is three separate writes (partial state on failure) | anime/store.rs:176-185 vs store.rs:227 |
| Anime explicit delete passes category rules, may keep a bookmarked episode | anime/download_backend.rs:108-118 vs backend.rs:266 |
| Anime Download menu can queue an already-queued episode | anime/downloads.rs:28 vs chapter_actions.rs:108 |
| skip_filtered honoured only on manga | chapter_actions.rs:103 vs anime/downloads.rs:30 |
| Anime rebuilds all rows on every progress event (no 10 Hz throttle) | anime/route.rs:1430 vs live.rs:26-40 |
| Anime cancel bypasses the runner | anime/route.rs:1447 vs runner.rs:300 |
| Anime selection anchors are view indexes: shift-range wrong after filter/sort | anime/selection.rs vs selection.rs:24 |
| No F1 help or list keys on anime | anime/route.rs:622-660 vs route.rs:37,493 |
| Manga missing-count shows false gaps under a filter | list_model.rs:181-190 vs anime/model.rs:279-290 |
| FAB reads filtered list on manga, unfiltered on anime. Decide | chapter_actions.rs:85 vs anime/model.rs:311 |
| Manga queues banners, against E-UI-3 "replace" | banner.rs via page.rs:7 vs message.rs:121 |
| Store errors as stringly DetailError::Other vs separate DetailsError | runner.rs:120 vs anime/details_error.rs |
| "Open in browser" anime-only | anime/overflow.rs:52 vs toolbar.rs:19 |
| Genre chips: global search (anime) vs own source (manga). Decide | anime/info.rs:47 vs header.rs:112 |
| Anime never deletes changed cover files (NoCoverFiles) | anime/remote.rs:131 |
| Manga page reimplements the domain filter engine | view.rs, flags.rs vs kuro-domain/src/app/items.rs:74 |
| Snapshots coalesced only on manga | runner.rs:50 |

### Lists, browse, library update (step 7)

Status 2026-10-13: done in substance. Step 4 already made the favourite store, the migration and the mark-done path one
each; step 7 added the cached-cover deletion for Browse, History and the entry page (`library::mount::clear_cached`),
the tracker send from Library and Updates mark-as-read (`progress::marked_items`), one global-search hint for both
kinds and a translatable "Unwatched". The remaining `Kind` arms in these areas are data tables (wording, help URLs,
lock names, layouts), not duplicated logic, and stay. Pin and hide of sources are cut by design. `browse::migrate_list`
is only built by tests (dead; the "never used" cleanup removes it). The drift table below is the pre-step-4 survey.
| Drift | Where |
|---|---|
| Manga add from Browse/History never applies default chapter flags | src/browse/repo_store.rs:47-108 vs 214 |
| Anime category chooser preselects nothing (no entry_categories) | repo_store.rs:162-241, browse/entries.rs:55 |
| Local manga covers deleted, local anime skipped | repo_store.rs:86-88 vs 146-155 |
| Tracker binding on add via two mechanisms | src/updates/history_add.rs:74-97 |
| Anime set_categories in migration is hand-written SQL | src/browse/migrate_live.rs:249-262 |
| has_repository still reads anime_extension_repos (nothing writes it) | src/library/view/mod.rs:649 |
| Pinned/hidden source prefs passed as empty sets (dead in GUI) | src/browse/mount.rs:62-71 |
| "Unwatched" hard-coded English; migrate empty text and search hint i18n only one kind | src/library/view/options.rs:51; browse/migrate_list.rs:58-62; search_screen.rs:198-200 |
| kind_switch copied verbatim, labels not translated | src/updates/mount.rs:52-60 vs screens.rs:148-155, browse/shell.rs:80 |
| Mark read: Updates removes read downloads, Library does not. Decide | src/library/actions.rs:51-70 vs src/updates/bulk.rs:35-76 |

### Tracking (step 8)

Added 2026-10-12 after the tracker wiring (src/entry_core/tracking/progress.rs: one `send` for reader, player and
title-page mark-as-read):
- Mark-as-read from Library and Updates does not reach the trackers yet (step 7 moves those screens onto `progress::marked`).
- kuro-track's track_chapter_with / track_episode_with also check `auto_update_track`, so mark-as-read with "Always" is
  blocked when the reader switch is off; Aniyomi checks only `auto_update_on_mark_read` there (step 8).
- No "Trackers updated" toast after a send (`auto_track::success_toast_shown` is unused) (step 8).
- The anime season and related tiles draw `ui::cover` placeholders: SeasonItem and the related rows carry no art (step 6).

| Drift | Where |
|---|---|
| TrackChanged{entry_id} has no kind; anime 7 and manga 7 collide | kuro-track/src/events.rs:12, ctx.rs:146,160 (fixed by step 3 ids) |
| AniList completion_status uses the anime id for manga; check per tracker (MAL ranges differ) | tracker/anilist/tracker_impl.rs:51, remote.rs:12,20, mal/status.rs |
| Unlink order differs (stale: the order is the tracker's, one for both kinds; dead `steps` removed) | src/entry_core/tracking/remove.rs |

### Player / reader (step 9)
| Drift | Where |
|---|---|
| Seen local episode opened from entry resumes near its end | src/local_flow/open.rs:47 |
| Next/prev ignores filters, sort, skip-read/dupes, downloaded-only; reads DB twice | src/player_view/neighbours.rs:19-34 |
| Downloaded episode of an online source is always streamed | src/player_view/mod.rs:645-660, source.rs |
| No screensaver inhibit during playback | src/player_view (none) vs src/reader/keep_awake.rs |
| Player writes episode + history on the GTK thread every 5 s | src/player_view/store.rs:113-139 |
| Anime history never records a duration | kuro-data history.rs:301 |
| Incognito read once per episode; per-source incognito pref read by neither | src/player_view/mod.rs:121,187; kuro-prefs/src/registry.rs:548 |
| preserve-watching-position hard-coded off | src/player_view/store.rs:20 |
| No completion side effects in the player (track, delete-after, duplicates) | src/player_view/store.rs:116 |
| Download-ahead 25% vs 35%, neither wired. Decide | src/reader/download_ahead.rs:9 vs kuro-player progress.rs |
| Escape closes reader, only leaves fullscreen in player; no text-entry guard. Decide | src/reader/state.rs, screen.rs:952-962 vs player_view/mod.rs:401 |
| warm_neighbours prefetches in source_order, not the session list | src/reader/launch.rs:83-93 |

### Sources and plug-ins (steps 10-11)

Added 2026-10-12 (found on the test display): `stub_sources` is never filled, so a backup made while the repository
sources are not loaded (always the case for `--run-backup`, which runs without the GUI) writes source ids as their
names, and the restore screen then lists missing sources as numbers. Aniyomi records every installed source (kind, id,
lang, name) in its sources table when the sources load. Fix in step 10: upsert every loaded source into stub_sources
(src/recipe_sources.rs load), and make LiveRegistry::name_of / stub_label fall back to the stub table.

| Drift | Where | Step |
|---|---|---|
| WireAnime always sends status 0; drops artist/update_strategy/initialized/preview_url/memo | kuro-source-wasm/src/anime_wire.rs:111 | 10 |
| Manga `details` op required, anime optional | kuro-source-wasm/src/source.rs:394 vs :334 | 10 |
| Anime ConfigurableSource lacks on_settings_changed | kuro-source/src/anime/traits.rs:171 vs source.rs:75 | 10 |
| Anime request_headers not reachable through the trait | kuro-source-wasm/src/source.rs:292 | 10 |
| Manga status: unknown codes and 7 read as Unknown | kuro-source/src/model/manga.rs:53 | 10 |
| Anime deep links unwired (ResolvableAnimeSource unused) | kuro-source/src/anime/traits.rs | 10 |
| Scrape engine applies the manga url rule to anime; two fns with same name, different rule | kuro-source-scrape/src/engine.rs:134 | 10 |
| Two identical anime-source lookups bypass the directory | src/screens.rs:129, src/entry_core/anime/route.rs:95 | 10 |
| Manga packages scanned with an empty trust map: never installed | src/ext_host.rs:217 | 11 |
| Orphan scan ignores repo/recipe sources | src/ext_host.rs:101-104 | 11 |
| Manga update counts always 0 (set_available never called) | kuro-extensions/src/registry.rs | 11 |
| Anime .kuroext sources listed but never instantiated | src/ext_host.rs:66-82 | 11 |
| tachiyomi:// add-repo has no handler | src/shell/uri.rs | 11 |

### Backup and local (steps 3, 10, 12)
| Drift | Where | Step |
|---|---|---|
| Anime restore never clears is_syncing; update_is_noop ignores it | kuro-backup/src/restore/anime.rs:87-118,209-244 vs manga.rs:75,127-160 | 12 (gone with one restore_entry) |
| Anime history duration dropped on restore and written as 0 on create | restore/anime.rs:366-391; creator/entries.rs:237 | 4 + 12 |
| Anime categories via hand SQL; existing_pairs via raw SQL | restore/anime.rs:37-41,120-129 | 12 |
| Bad chapters.json ignored, bad episodes.json fails the listing. Decide (ignore) | kuro-local/src/json_side.rs:118 vs :129 | 10 |
| Anime episode extensions case-sensitive; list still has "torrent" (removed 2026-10-12) | kuro-local/src/format.rs:12-18 | 10 |
| Anime details failure propagates, no per-URL lock (manga logs + locks) | kuro-local/src/manga/mod.rs:241-254 vs anime/mod.rs:166-168 | 10 |
| Custom cover refused for non-favourite anime, allowed for manga | kuro-images/src/invalidate.rs:168 vs kuro-local/src/images.rs:107 | 4 |

## 6. Totals

| Step | ~Lines out |
|---|---|
| 1 Dead-code sweep | 8,000 |
| 2 One Kind | 400 |
| 3 One database | 2,600 |
| 4 One entry/item model | 5,500 |
| 5 Downloads | 3,300 |
| 6 Entry page | 3,500 |
| 7 Lists | 1,900 |
| 8 Tracking | 2,200 |
| 9 Media session | 1,500 |
| 10 Sources | 900 |
| 11 Plug-in host | 6,000 |
| 12 Backup edge | 600 |
| **Total** | **~36,000** (per-area maps sum to ~37k; ~1k is counted in two areas) |

Pairs per verdict (183 pairs):

| Area | merge | keep-separate | already-shared | edge-adapter |
|---|---|---|---|---|
| data | 19 | 3 | 2 | 2 |
| downloads | 19 | 3 | 3 | 0 |
| entry page | 23 | 3 | 3 | 0 |
| GUI lists | 20 | 2 | 2 | 2 |
| sources | 23 | 1 | 6 | 1 |
| backup/track/local | 13 | 2 | 3 | 3 |
| player/reader | 19 | 5 | 1 | 0 |
| **total** | **136** | **19** | **20** | **8** |

Drift items: ~95, of which 10 are "Decide" calls to make before the step that touches them
(max active, warning thresholds, finalise collision, library defaults on open, FAB list, genre chip scope,
mark-read deletes downloads, download-ahead threshold, Escape in player, side-file error policy).
