# Area 09 — Domain: source, track, download, release, storage, backup, custom buttons, extension repos/stores — implementation tasks (part 1 of 1)

All paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`). Ids run T-09a-001 to T-09a-061. Companion documents: `p1-inventory.md` (every scope file mapped to tasks), `p1-behavior.md` (algorithms and quirks, with file:line), `p1-data-and-config.md` (models, SQL, preference keys, endpoints), `p1-rust-design.md` (crate layout and signatures), `p1-edge-cases-and-tests.md` (quirk register Q-*, edge cases E-*, tests TC-*), and the topic deep dives `p1-topic-extension-repos-and-stores.md`, `p1-topic-download-policy.md`, `p1-topic-source-model-pins-stubs.md`.

Coverage map: all 73 scope files are referenced by at least one task (see the "Tasks" column of `p1-inventory.md`). Build and manifest files are dropped by T-09a-055. Android-only parts (SAF, NoMedia, external downloader intent, SY data-saver flag, Serializable) are dropped by T-09a-050, T-09a-022, T-09a-056 and T-09a-054 and are listed in the drop table of `p1-rust-design.md` section 7.

Dependency rule: `Depends:` tokens use `area:<id>` with the exact plan folder name (checked against `plans/`); prose mentions of other areas also use `area:<id>` with the exact folder name.

Priority guide for this area: P0 = needed to browse installed sources, show source names for uninstalled sources, and read download and storage settings that the reader and downloader depend on; P1 = parity features (tracks, extension repos and stores, auto-download, backup preferences, non-library listings); P2 = long tail (custom buttons, release check, upcoming feeds, migration of old store entries); P3 = Android-only drops with the replacement stated.

## Foundation and source model

### T-09a-001 — kuro-domain module skeleton for source, track, download, extension, release, storage, backup
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap, area:08-domain-library-category-history-updates
- Sources: domain/build.gradle.kts, domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt, domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - `cargo build -p kuro-domain` succeeds with the module tree of `p1-rust-design.md` section 1 (`source`, `track`, `download`, `extension_repo`, `extension_store`, `custom_buttons`, `release`, `storage`, `backup`, `upcoming`), each `mod.rs` present
  - the crate has no dependency on `gtk`, `glib`, `gio` or `adw` (checked with `cargo tree -p kuro-domain | grep -E "gtk|glib|gio|adw"` printing nothing)
  - `DomainError` and `RepoStoreError` exist with the variants listed in `p1-rust-design.md` section 3

### T-09a-002 — Source value model: visual name, key and the last-used key
- Priority: P0
- Effort: S
- Depends: T-09a-001
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSourceWithIds.kt
- Acceptance:
  - `Source::visual_name` returns `name` when `lang` is empty, else `"{name} ({LANG_UPPER})"`; unit tests TC-SRC-01 pass
  - `Source::key` returns `"{id}"` and, when `is_used_last`, `"{id}-lastused"`; TC-SRC-02 passes
  - the anime model has the same fields minus `isExcludedFromDataSaver`; the manga model keeps the field only as the dropped SY placeholder described in T-09a-056
  - `pin` defaults to `Pins::UNPINNED` and `is_used_last` to `false` in both constructors

### T-09a-003 — Pins bit flags with the Kotlin quirks made explicit
- Priority: P1
- Effort: S
- Depends: T-09a-001
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt
- Acceptance:
  - `Pin` codes are `Unpinned = 0b00`, `Pinned = 0b01`, `Actual = 0b10`; `Pins::PINNED` is `0b11`; `Pins::UNPINNED` is `0b00`
  - `contains` reproduces `(flag & self) == flag` (so `UNPINNED.contains(Unpinned)` is true, Q-PIN-1); `is_pinned()` is `self == PINNED`
  - `without` is AND-NOT (not XOR; Q-PIN-2 is fixed and documented); TC-PIN-01 through TC-PIN-06 pass
  - `from_bits` masks to two bits (E-PIN-01)

### T-09a-004 — Source registry: installed sources, stubs, initialization state, listing traits
- Priority: P0
- Effort: M
- Depends: T-09a-002, area:06-data-repos-history-track-source-misc
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/service/MangaSourceManager.kt, domain/src/main/java/tachiyomi/domain/source/anime/service/AnimeSourceManager.kt, domain/src/main/java/tachiyomi/domain/source/manga/repository/MangaSourceRepository.kt, domain/src/main/java/tachiyomi/domain/source/anime/repository/AnimeSourceRepository.kt
- Acceptance:
  - `SourceRegistry` exposes `is_initialized` as a `watch` receiver that is `false` until the extension loader finishes, then `true` and never reverts
  - `get(key)` returns the installed source or `None`; `get_or_stub(key)` returns `SourceHandle::Installed` or `SourceHandle::Stub` (E-SRC-05)
  - `online()` returns only HTTP-backed sources; `stubs()` returns persisted stub rows
  - traits `MangaSourceRepo` and `AnimeSourceRepo` declare the methods listed in `p1-rust-design.md` section 4.1; a fake implementation compiles in tests

### T-09a-005 — Stub sources: uninstalled-source placeholders and their error behaviour
- Priority: P0
- Effort: M
- Depends: T-09a-002
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt
- Acceptance:
  - every remote operation on a `StubSource` returns `DomainError::SourceNotInstalled` (manga: details, chapter list, page list; anime: popular, latest, search, episode update, season update, related, hoster list, video list); TC-SRC-04
  - `display_name()` returns `"{name} ({LANG})"` when neither name nor lang is blank, else the numeric id (Q-SRC-1); TC-SRC-03
  - anime stub reports `supports_latest = false` and `supports_related = false`
  - `StubSource::from(source)` copies id, lang and name only

### T-09a-006 — Browse routing: popular, latest and search as an enum, not sentinel strings
- Priority: P0
- Effort: S
- Depends: T-09a-004
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetRemoteManga.kt, domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt
- Acceptance:
  - `BrowseQuery::{Popular, Latest, Search(String)}` is the only input the browse screens use; the browse list calls `popular`, `latest` or `search` accordingly
  - `BrowseQuery::from_legacy` maps the four sentinel strings (`eu.kanade.domain.{source}.{kind}.interactor.{POPULAR|LATEST}`) and returns `Search` for anything else; TC-SRC-05
  - typing the sentinel text into search performs a search (Q-BR-1 is fixed in the UI path)

### T-09a-007 — Non-library listings: counts per source, deletable anime and orphaned ids
- Priority: P1
- Effort: M
- Depends: T-09a-004, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetMangaSourcesWithNonLibraryManga.kt, domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetAnimeSourcesWithNonLibraryAnime.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/MangaSourceWithCount.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSourceWithIds.kt, domain/src/main/java/tachiyomi/domain/source/anime/model/DeletableAnime.kt
- Acceptance:
  - `sources_with_non_library()` emits `(source, count)` where count counts only non-favorite rows; a source with zero non-library rows is omitted unless it has orphaned ids (E-SRC-09)
  - `SourceWithIds::count()` equals `ids.len()`; `id` and `name` delegate to the source
  - `get_deletable_children(parent_id)` returns the children of that parent only
  - `DeletableEntry` carries `FetchType::{Seasons, Episodes}` as in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt`

### T-09a-008 — Stub persistence: upsert, read and subscribe for manga and anime stubs
- Priority: P1
- Effort: S
- Depends: T-09a-005, area:04-db-anime-schema-migrations
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/repository/MangaStubSourceRepository.kt, domain/src/main/java/tachiyomi/domain/source/anime/repository/AnimeStubSourceRepository.kt
- Acceptance:
  - `upsert_stub(id, lang, name)` inserts or updates; `get_stub(id)` returns `None` for unknown ids; `subscribe_all` emits after each upsert
  - a stub row survives an app restart (round-trip test with an on-disk database)

### T-09a-009 — Upcoming feeds: status sets for manga and anime
- Priority: P2
- Effort: S
- Depends: T-09a-001, area:05-data-repos-entries-items
- Sources: domain/src/main/java/mihon/domain/upcoming/anime/interactor/GetUpcomingAnime.kt, domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt
- Acceptance:
  - `UPCOMING_MANGA_STATUSES = [1, 4]` and `UPCOMING_ANIME_STATUSES = [1, 4, 7]`; TC-UP-01 passes
  - the status constants `UNKNOWN=0 ONGOING=1 COMPLETED=2 LICENSED=3 PUBLISHING_FINISHED=4 CANCELLED=5 ON_HIATUS=6` (manga and anime) and `UPCOMING=7` (anime) are defined once in `kuro-domain::upcoming`

## Tracks

### T-09a-010 — Track models for manga and anime with exact field types
- Priority: P1
- Effort: M
- Depends: T-09a-001
- Sources: domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt, domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt
- Acceptance:
  - `MangaTrack` has `id, manga_id, tracker_id, remote_id: i64`, `library_id: Option<i64>`, `title: String`, `last_chapter_read: f64`, `total_chapters: i64`, `status: i64`, `score: f64`, `remote_url: String`, `start_date: i64`, `finish_date: i64`, `private: bool`; `AnimeTrack` replaces the chapter fields with `last_episode_seen: f64` and `total_episodes: i64`
  - serde round-trip of both types preserves every field (no `Serializable` equivalent is needed, see T-09a-054)

### T-09a-011 — Track interactors with the log-and-default error policy and per-entry grouping
- Priority: P1
- Effort: M
- Depends: T-09a-010, area:06-data-repos-history-track-source-misc
- Sources: domain/src/main/java/tachiyomi/domain/track/manga/interactor/GetMangaTracks.kt, domain/src/main/java/tachiyomi/domain/track/manga/interactor/InsertMangaTrack.kt, domain/src/main/java/tachiyomi/domain/track/manga/interactor/DeleteMangaTrack.kt, domain/src/main/java/tachiyomi/domain/track/manga/interactor/GetTracksPerManga.kt, domain/src/main/java/tachiyomi/domain/track/anime/interactor/GetAnimeTracks.kt, domain/src/main/java/tachiyomi/domain/track/anime/interactor/InsertAnimeTrack.kt, domain/src/main/java/tachiyomi/domain/track/anime/interactor/DeleteAnimeTrack.kt, domain/src/main/java/tachiyomi/domain/track/anime/interactor/GetTracksPerAnime.kt
- Acceptance:
  - `by_row_id` returns `None` on error; `by_entry` returns an empty list on error; the error is logged once with `tracing::error!` (TC-TRK-05)
  - `insert` and `delete` never return an error to the caller in the parity path; the Result-returning service method (T-09a-059) is used by new UI code
  - `tracks_per_entry()` groups by `manga_id` or `anime_id`; entries without tracks are absent (TC-TRK-07)
  - `by_row_id` is named so that it cannot be confused with an entry lookup (Q-TRK-1)

### T-09a-012 — Track upsert keeps the row id stable
- Priority: P1
- Effort: S
- Depends: T-09a-011
- Sources: data/src/main/sqldelight/data/manga_sync.sq, data/src/main/sqldelightanime/dataanime/anime_sync.sq, data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt
- Acceptance:
  - inserting a second track for the same (entry, tracker) pair updates the row in place: `_id` is unchanged (TC-TRK-02 with the Proposal path); the Kotlin REPLACE semantics are documented as Q-TRK-2 and not reproduced
  - `insert_all` with duplicate pairs in one batch leaves one row holding the last values (TC-TRK-06)
  - the migration that creates the table keeps `UNIQUE (entry_id, sync_id)`

### T-09a-013 — Track repository trait and SQL contract for manga and anime
- Priority: P1
- Effort: S
- Depends: T-09a-010, area:03-db-manga-schema-migrations
- Sources: domain/src/main/java/tachiyomi/domain/track/manga/repository/MangaTrackRepository.kt, domain/src/main/java/tachiyomi/domain/track/anime/repository/AnimeTrackRepository.kt, data/src/main/sqldelight/data/manga_sync.sq, data/src/main/sqldelightanime/dataanime/anime_sync.sq
- Acceptance:
  - `TrackRepo<T>` declares the methods of `p1-rust-design.md` section 4.4 for both kinds
  - `delete(entry_id, tracker_id)` is `DELETE … WHERE entry_id = ? AND sync_id = ?` and succeeds with zero rows affected (TC-TRK-03)
  - `PRAGMA foreign_keys = ON` is verified on every connection (E-TRK-05): a test deletes a manga and asserts its tracks are gone

## Download policy

### T-09a-014 — Download preferences: the 23 keys with their exact defaults
- Priority: P0
- Effort: S
- Depends: T-09a-001, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - `DownloadPrefs` exposes the 23 typed accessors of `p1-rust-design.md` section 4.5 with the keys and defaults of `p1-data-and-config.md` section 4.1
  - TC-DL-11 asserts every default and key string (23 assertions)
  - category sets are `BTreeSet<i64>` and their keys are the six names in `categoryPreferenceKeys`

### T-09a-015 — Auto-download filter for manga chapters
- Priority: P1
- Effort: M
- Depends: T-09a-014, area:07-domain-entries-items
- Sources: domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt
- Acceptance:
  - `filter_new` returns an empty list when `download_new` is false, the list is empty, or the entry is not eligible (behavior 5.1 order); TC-DL-03
  - when `download_new_unread_chapters_only` is false, every new chapter is returned unchanged
  - eligibility is computed by `should_download_new` (T-09a-018)

### T-09a-016 — Auto-download filter for anime episodes
- Priority: P1
- Effort: M
- Depends: T-09a-014, area:07-domain-entries-items
- Sources: domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt, domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt
- Acceptance:
  - the same algorithm as T-09a-015 with `download_new_episode` and `download_new_unread_episodes_only`, comparing `seen` and `episodeNumber`
  - implemented by the same generic function (`ReadState` and `NewItem` traits); the test suite runs both kinds over one fixture table

### T-09a-017 — Unread-only de-duplication by recognised chapter number
- Priority: P1
- Effort: S
- Depends: T-09a-015
- Sources: domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt, domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt
- Acceptance:
  - only read items with `number >= 0` are in the suppression set (TC-DL-04, TC-DL-05)
  - numbers compare by exact float bits (`to_bits`); `10.5` suppresses `10.5` but not `10.0` (TC-DL-06)
  - the re-numbering behaviour (Q-DL-1) is kept and documented in the function's doc comment

### T-09a-018 — Category include and exclude decision table
- Priority: P1
- Effort: S
- Depends: T-09a-014
- Sources: domain/src/main/java/mihon/domain/items/chapter/interactor/FilterChaptersForDownload.kt, domain/src/main/java/mihon/domain/items/episode/interactor/FilterEpisodesForDownload.kt
- Acceptance:
  - `should_download_new` implements the five-row table of behavior 5.2 exactly, with the default category `0` used for an entry with no categories (E-DL-06)
  - TC-DL-01 covers every row; TC-DL-02 covers `favorite = false`
  - non-numeric category strings are skipped on read (Q-DL-2 Proposal, TC-DL-10)

### T-09a-019 — Remove-after-read slot arithmetic
- Priority: P1
- Effort: S
- Depends: T-09a-014
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `remove_after_read_slots` values `-1` (disabled), `0` (last read), `1..4` ("second to last" to "fifth to last") select the chapter at `index - slots` in the sorted chapter list, or nothing when out of range (TC-DL-07)
  - any negative value other than `-1` is treated as disabled (E-DL-04, TC-DL-08)
  - the UI options are `-1, 0, 1, 2, 3, 4` in that order

### T-09a-020 — Remove-after-marked-read, bookmarked, filler and exclude-category preferences
- Priority: P1
- Effort: S
- Depends: T-09a-014
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt
- Acceptance:
  - `remove_after_marked_as_read` (default false), `pref_remove_bookmarked` (false), `pref_download_fillermarked` (false), `remove_exclude_categories` and `remove_exclude_anime_categories` (empty sets) are typed prefs with the keys of `p1-data-and-config.md` section 4.1
  - a test asserts that the category-delete path strips the deleted id from all six category sets (fixes the upstream bug in `DeleteMangaCategory`, see area:08-domain-library-category-history-updates)

### T-09a-021 — Download slots, speed limit and Wi-Fi-only policy values
- Priority: P0
- Effort: S
- Depends: T-09a-014
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `number_of_downloads` reads are clamped to `1..5` (E-DL-01)
  - `download_speed_limit` is KiB/s; `bytes_per_second = limit as i64 * 1024` with no overflow (TC-DL-09); `0` means unthrottled; negative values are treated as `0` (E-DL-02)
  - `download_only_over_wifi` defaults to `true`; the Linux network check is a `gio::NetworkMonitor` query (replacement stated, implemented in area:23-downloads-manga/area:24-downloads-anime, verified here by a fake monitor)

### T-09a-022 — External downloader preferences: dropped on Linux
- Priority: P3
- Effort: S
- Depends: T-09a-014
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt
- Acceptance:
  - the `use_external_downloader` and `external_downloader_selection` keys are read only for backup-format parity; the UI does not show the option
  - the decision and the loss ("no third-party Android download apps") are recorded in the `p1-topic-download-policy.md` drop table

### T-09a-023 — CBZ packaging and tall-image splitting preferences
- Priority: P1
- Effort: S
- Depends: T-09a-014
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `save_chapter_as_cbz` defaults to `true` and `split_tall_images` to `true`; both are typed prefs (TC-DL-11)
  - the downloader (area:23-downloads-manga) reads them through `DownloadPrefs`; this task only proves the typed accessors

## Extension repositories (manga)

### T-09a-024 — Extension repo model and the save error type
- Priority: P1
- Effort: S
- Depends: T-09a-001
- Sources: domain/src/main/java/mihon/domain/extensionrepo/model/ExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/exception/SaveExtensionRepoException.kt
- Acceptance:
  - `ExtensionRepo` has `base_url, name, short_name: Option<String>, website, signing_key_fingerprint`
  - `RepoStoreError::Save` wraps a `rusqlite::Error` and is produced only by insert and upsert on constraint or SQLite failure

### T-09a-025 — extension_repos table and repository semantics
- Priority: P1
- Effort: M
- Depends: T-09a-024, area:03-db-manga-schema-migrations
- Sources: data/src/main/sqldelight/data/extension_repos.sq, data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/repository/MangaExtensionRepoRepository.kt
- Acceptance:
  - the migration creates `extension_repos(base_url TEXT PRIMARY KEY, name, short_name, website, signing_key_fingerprint UNIQUE)` exactly as in `p1-data-and-config.md` section 3.1
  - `insert`, `upsert` (ON CONFLICT(base_url)), `replace` (ON CONFLICT(signing_key_fingerprint)), `delete`, `get`, `get_by_fingerprint`, `count` pass a table test with a duplicate-fingerprint case
  - `subscribe_all` emits after every write

### T-09a-026 — Add manga extension repo: URL validation and conflict classification
- Priority: P1
- Effort: M
- Depends: T-09a-025, T-09a-027
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt
- Acceptance:
  - `validate_index_url` accepts only `^https://.*/index\.min\.json$` after URL normalisation; TC-REPO-01 and TC-REPO-02 pass
  - `add_manga_repo` returns `InvalidUrl` when repo.json cannot be fetched or parsed (E-REPO-04), `RepoAlreadyExists` for a repeated base URL, `DuplicateFingerprint{old,new}` for a shared fingerprint, `Success` otherwise, and `Error` for any other insert failure (Q-REPO-1 fixed: all DB errors are classified, not only `SaveExtensionRepoException`)
  - TC-REPO-03 and TC-REPO-05 pass

### T-09a-027 — repo.json fetch and DTO mapping
- Priority: P1
- Effort: S
- Depends: T-09a-024, area:12-network-stack
- Sources: domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt, domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoDto.kt
- Acceptance:
  - `GET <base>/repo.json` decodes `{"meta":{"name","shortName","website","signingKeyFingerprint"}}`; `shortName` may be null
  - any HTTP, JSON or network error yields `None` (logged at error level, message "Failed to fetch repo details")
  - fixture `repo.json` decodes to the expected `ExtensionRepo` (fixture list in `p1-edge-cases-and-tests.md` section 5)

### T-09a-028 — Refresh manga repos concurrently with the fingerprint guard
- Priority: P1
- Effort: M
- Depends: T-09a-026
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/UpdateMangaExtensionRepo.kt
- Acceptance:
  - repos refresh in parallel using a `JoinSet` capped at 4 concurrent fetches (Proposal, `p1-edge-cases-and-tests.md` section 8); one repo failing does not cancel the others (Q-REPO-2 fixed; TC-REPO-08)
  - a repo is updated only when its stored fingerprint starts with `NOFINGERPRINT` or equals the fetched fingerprint (TC-REPO-06); a changed fingerprint is skipped and logged as a warning (E-REPO-07)
  - a fetch returning `None` leaves the row untouched

### T-09a-029 — Replace, delete, get and count for manga repos
- Priority: P1
- Effort: S
- Depends: T-09a-025
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/ReplaceMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/DeleteMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/GetMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/GetMangaExtensionRepoCount.kt
- Acceptance:
  - replace on a fingerprint collision changes the base URL of the existing row (E-REPO-09, TC-REPO-05)
  - delete of an absent base URL returns Ok (TC-REPO-07)
  - `get_all`, `subscribe_all` and `count` watch return the same rows as the table

## Extension stores (anime)

### T-09a-030 — Anime extension store model
- Priority: P1
- Effort: S
- Depends: T-09a-001
- Sources: domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt
- Acceptance:
  - `AnimeExtensionStore` has `index_url, name, badge_label, signing_key, contact: Contact{website, discord: Option<String>}, is_legacy: bool, extension_list_url: Option<String>` with the exact types of `p1-data-and-config.md` section 1.4

### T-09a-031 — Add an anime extension store: wire-format sniffing and legacy handling
- Priority: P1
- Effort: M
- Depends: T-09a-030, area:12-network-stack
- Sources: domain/src/main/java/mihon/domain/extension/anime/interactor/AddAnimeExtensionStore.kt, domain/src/main/java/mihon/domain/extension/anime/repository/AnimeExtensionStoreRepository.kt, data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt
- Acceptance:
  - `gunzip_if_needed` gunzips on magic `0x1f 0x8b` (TC-STO-02); `sniff` returns LegacyList for `[`, LegacyRepo or CurrentJson for `{` (decode fallback), Proto otherwise (TC-STO-01)
  - a `[` body whose URL does not end in `/index.min.json` fails with "Provided legacy store url is not valid" (TC-STO-03)
  - one `indexV2` recursion is followed; a deeper chain fails with `Invalid` (E-STO-07, TC-STO-04)
  - the row is keyed by the user-entered URL and the resolved URL is stored in a separate column (Q-STORE-1 fixed)
  - protobuf bodies return `Err(Invalid("protobuf store: schema owned by area:19-extensions-anime"))` until area:19-extensions-anime supplies the schema

### T-09a-032 — Refresh all anime stores sequentially with per-store error logging
- Priority: P1
- Effort: M
- Depends: T-09a-031
- Sources: domain/src/main/java/mihon/domain/extension/anime/interactor/UpdateAnimeExtensionStores.kt, data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt
- Acceptance:
  - stores are refreshed one after another in `index_url` order; a failing store logs "Failed to refresh extension store '<name> (<index_url>)'" and the loop continues (TC-STO-05)
  - a store removed during refresh is not re-created (E-STO-09, Proposal re-check before upsert)
  - the function returns `Ok(())` when at least one store was attempted

### T-09a-033 — Aggregate extensions across stores in parallel with failure isolation
- Priority: P1
- Effort: M
- Depends: T-09a-031, area:12-network-stack
- Sources: data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt, data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt
- Acceptance:
  - `fetch_extensions` runs one task per store with a `JoinSet`; failing stores contribute nothing (TC-STO-06); zero stores means zero network calls (E-STO-06)
  - the three branches of `getExtensions` are implemented in the order: explicit `extension_list_url`, then non-legacy `index_url`, then legacy `<base>/index.min.json` (behavior 7.4)
  - a missing `extensionList` in a non-legacy store is an error, not a panic (E-STO-04)

### T-09a-034 — Remove, get, subscribe and count for anime stores
- Priority: P1
- Effort: S
- Depends: T-09a-030, area:04-db-anime-schema-migrations
- Sources: domain/src/main/java/mihon/domain/extension/anime/interactor/RemoveAnimeExtensionStore.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/GetAnimeExtensionStores.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/GetAnimeExtensionStoreCountAsFlow.kt
- Acceptance:
  - `remove(index_url)` deletes the row and does not touch extensions (TC-STO-08)
  - `subscribe_all` and `count` emit after insert and remove
  - the `extension_store` table matches `p1-data-and-config.md` section 3.2 column for column

### T-09a-035 — Migration of preference-based store entries (no network)
- Priority: P2
- Effort: S
- Depends: T-09a-031
- Sources: data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt
- Acceptance:
  - `insert_from_preference(index_url, name)` writes `badge_label = name`, `signing_key = "NO_SIGNING_KEY"`, `contact_website = index_url`, `contact_discord = NULL`, `is_legacy = false`, `extension_list_url = NULL` (TC-STO-07)
  - an existing row whose signing key is not `NO_SIGNING_KEY` is not overwritten (E-STO-08, Proposal)

## Custom buttons

### T-09a-036 — Custom button model and placeholder rendering
- Priority: P2
- Effort: S
- Depends: T-09a-001
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButtonUpdate.kt
- Acceptance:
  - `render` replaces `$id` then `$isPrimary` with `"true"` or `"false"` (TC-CB-01); the substring quirk `$identity` (Q-CB-1) is covered by TC-CB-02
  - the three rendered forms (content, long-press content, on-startup) are produced from one function
  - `CustomButtonUpdate` has an `id` and six optional fields in the order of `p1-data-and-config.md` section 1.5

### T-09a-037 — custom_buttons table, seed row and repository
- Priority: P2
- Effort: M
- Depends: T-09a-036, area:04-db-anime-schema-migrations
- Sources: data/src/main/sqldelightanime/dataanime/custom_buttons.sq, data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/custombuttons/repository/CustomButtonRepository.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/GetCustomButtons.kt
- Acceptance:
  - the table and the seed row (`_id = 1`, `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`, three Lua snippets exactly as in `p1-data-and-config.md` section 3.3) are created by `INSERT OR IGNORE` (TC-CB-10)
  - `get_all` orders by `sortIndex` (ties by `_id`, Proposal); `subscribe_all` emits after each write
  - the Lua snippets are stored byte-for-byte, including the tab characters inside `onStartup`

### T-09a-038 — Create, update (single and batch) and delete with compaction
- Priority: P2
- Effort: M
- Depends: T-09a-037
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/interactor/CreateCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/UpdateCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/DeleteCustomButton.kt, domain/src/main/java/tachiyomi/domain/custombuttons/exception/SaveCustomButtonException.kt
- Acceptance:
  - create assigns `sortIndex = max + 1` or `0`, inserts with `is_favorite = false`, and runs under the ops mutex (Q-CB-2 fixed; TC-CB-11); empty names are rejected (E-CB-09)
  - single update applies `coalesce` semantics for all six mutable columns and logs errors at error level (Q-CB-3 fixed)
  - batch update runs in one transaction
  - delete removes the row, then rewrites `sortIndex` to `0..n-1` in one transaction; deleting the last button skips the transaction (E-CB-02); TC-CB-04 passes
  - the `SaveCustomButtonException` case maps to `DomainError::Db`

### T-09a-039 — Reorder custom buttons under a mutex with index validation
- Priority: P2
- Effort: S
- Depends: T-09a-038
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ReorderCustomButton.kt
- Acceptance:
  - unknown id returns `Unchanged` with zero writes (TC-CB-06)
  - `new_index` must be in `0..=n-1`; otherwise `InvalidIndex` (Q-CB-4 fixed; TC-CB-07); `n-1` moves to the end (TC-CB-05)
  - after a successful move every button's `sortIndex` equals its position (TC-CB-05)
  - reordering to the same index writes nothing (E-CB-03, Proposal)

### T-09a-040 — Single-favourite toggle with unknown-id handling
- Priority: P2
- Effort: S
- Depends: T-09a-038
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ToggleFavoriteCustomButton.kt
- Acceptance:
  - toggling a non-favourite makes it the only favourite (TC-CB-08)
  - toggling the favourite off leaves zero favourites; the player's primary button falls back to the lowest `sortIndex` (E-CB-05, Proposal)
  - an unknown id returns `Unchanged` (Q-CB-5 fixed; TC-CB-09)
  - both paths take the mutex (RC-06 fixed)

### T-09a-041 — Custom button runtime contract for the player (Lua in mpv)
- Priority: P2
- Effort: M
- Depends: T-09a-036, area:04-db-anime-schema-migrations
- Sources: domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt, data/src/main/sqldelightanime/dataanime/custom_buttons.sq
- Acceptance:
  - the rendered `on_startup` snippet is passed to the player's Lua host with the primary flag resolved for the playing anime (area:24-downloads-anime/area:46-player-controls-ui player code)
  - a Lua snippet that fails to parse is reported to the user with the button name, and the button stays in the list (no deletion)
  - the parse check of the rendered `if false then … end` form succeeds (TC-CB-03, optional `mlua` test)

## Release check

### T-09a-042 — Latest-release check with a three-day throttle and GitHub source
- Priority: P2
- Effort: M
- Depends: T-09a-001, area:12-network-stack
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt, domain/src/main/java/tachiyomi/domain/release/service/ReleaseService.kt, data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt, data/src/main/java/tachiyomi/data/release/GithubRelease.kt
- Acceptance:
  - `GET https://api.github.com/repos/{repository}/releases/latest` is decoded with `tag_name`, `body`, `html_url`, `assets[].name`, `assets[].browser_download_url`
  - the check is skipped when less than three days have passed since `__APP_STATE_last_app_check` unless `force` is set (TC-REL-05, E-REL-02)
  - `last_checked` is written after a non-null release is returned, not when the fetch fails or returns no asset (Q-REL-5)
  - the Linux asset rule (`choose_asset`) picks the asset whose name contains `linux` and the host architecture token; no match returns `None` (TC-REL-04)
  - a 403 or other HTTP error returns `Err` and leaves `last_checked` unchanged (TC-REL-06)

### T-09a-043 — Version comparison with semver (fixes four Kotlin defects)
- Priority: P2
- Effort: S
- Depends: T-09a-042
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - release comparison uses `semver::Version`; TC-REL-01 passes including: `1.0.0` vs `0.9.9` is not newer (Q-REL-1), `1.2.1` vs `1.2` is newer (Q-REL-2), `1.2` vs `1.2.3` does not panic (Q-REL-3)
  - tags are stripped of a leading `v` and any non-digit, non-dot characters before parsing; two-part tags parse as `x.y.0`

### T-09a-044 — Preview build numbering (`r<commit count>`) comparison
- Priority: P2
- Effort: S
- Depends: T-09a-043
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt
- Acceptance:
  - preview tags parse as `u64` after stripping non-digits; `r2000` vs `commit_count = 2000` is not newer, `r2001` is newer (TC-REL-02)
  - an unparsable preview tag returns an error that the caller logs; no update is offered (Q-REL-4 fixed)

### T-09a-045 — Release model, mention linkifier and architecture-based asset choice
- Priority: P2
- Effort: S
- Depends: T-09a-042
- Sources: domain/src/main/java/tachiyomi/domain/release/model/Release.kt, data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt
- Acceptance:
  - `Release { version, info, release_link, download_link }` non-null; `info = null` in JSON becomes `""` (E-REL-06)
  - `linkify_mentions` rewrites `@user` to `[@user](https://github.com/user)` with the Kotlin regex (case-insensitive, max 39 characters); TC-REL-08 passes
  - `choose_asset` uses `std::env::consts::ARCH` in place of `Build.SUPPORTED_ABIS[0]` (TC-REL-04)

### T-09a-046 — Drop the unused OsTooOld result
- Priority: P3
- Effort: S
- Depends: T-09a-042
- Sources: domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt
- Acceptance:
  - the Rust `CheckResult` has only `NewUpdate` and `NoNewUpdate`
  - the drop reason is recorded: the producer is commented out upstream (`AppUpdateChecker.kt:17`), so no user can reach that UI branch

### T-09a-047 — Base storage directory preference with a path-safe encoding
- Priority: P0
- Effort: S
- Depends: T-09a-001, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt, core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
  - the key is `__APP_STATE_storage_dir`; the default is `$XDG_DATA_HOME/kurobayomi` (fallback `~/.local/share/kurobayomi`) via `directories::ProjectDirs`
  - a non-UTF-8 path round-trips through the preference (E-STO-12, TC-STO-13) using an OS-string-safe encoding (base64 of the bytes)
  - the Kotlin default `folderProvider.path()` is replaced, and the Android content-URI form is rejected with `StorageUnavailable`

### T-09a-048 — Storage directory tree: creation on change, lazy creation on access, and explicit startup check
- Priority: P0
- Effort: M
- Depends: T-09a-047
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - `set_base(path)` creates `autobackup`, `downloads`, `local`, `localanime`, `mpv-config/{fonts,scripts,script-opts,shaders}` (TC-STO-09)
  - `open(configured)` does not create the tree (TC-STO-10, Q-STO-2 documented); the app's first-run path calls `ensure_tree()` explicitly so the tree exists at startup (Proposal)
  - folder names match `local` for manga and `localanime` for anime exactly (Q-STO-1 kept for disk compatibility)

### T-09a-049 — Storage change notification (replaces the Kotlin shared flow)
- Priority: P0
- Effort: S
- Depends: T-09a-048
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - `subscribe()` returns a `watch::Receiver<u64>`; each real base change bumps the value once (TC-STO-12: setting the same path emits nothing)
  - the latest base path is available synchronously through `base()`; no event is lost when a subscriber starts late

### T-09a-050 — Storage platform adaptation: local paths only, no media-scanner marker
- Priority: P3
- Effort: S
- Depends: T-09a-048
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - SAF / UniFile content URIs are not supported; the settings picker (`gtk::FileDialog::select_folder`) returns a local path and remote (`sftp`, `smb`) locations are refused with `StorageUnavailable` (T-09a-051 covers the error)
  - `DiskUtil.createNoMediaFile` is not reproduced; the drop reason (the `.nomedia` marker only stops Android's media scanner; Linux desktop indexers do not read it, so nothing is lost on Linux) is recorded in `p1-rust-design.md` section 7

### T-09a-051 — Storage error handling: unavailable base, read-only and remote locations
- Priority: P1
- Effort: S
- Depends: T-09a-048
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - a missing base returns `DomainError::StorageUnavailable` from every accessor (TC-STO-11) and never panics (Q-STO-3 fixed)
  - a read-only base returns `DomainError::Io` (E-STO-11; `DomainError::Io` variant added)
  - `set_base` on a path that is not a directory returns `Invalid("not a directory")`
  - concurrent `set_base` calls are serialised by a mutex (E-STO-14)

### T-09a-052 — Backup preferences: interval, last automatic backup time and default flags
- Priority: P1
- Effort: S
- Depends: T-09a-001, area:02-preferences-system
- Sources: domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt, app/src/main/java/mihon/core/migration/migrations/EnableAutoBackupMigration.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt
- Acceptance:
  - `backup_interval` defaults to `12`; `0` means disabled (TC-BK-01); the enable migration sets 12 when the value is 0
  - `__APP_STATE_last_auto_backup_timestamp` defaults to `0` and is written with epoch milliseconds
  - `backup_flags` defaults to the set `{"1","2","4","8"}`; TC-BK-02 passes

### T-09a-053 — Backup flag tags and the unused-flag decision
- Priority: P1
- Effort: S
- Depends: T-09a-052
- Sources: domain/src/main/java/tachiyomi/domain/backup/service/PreferenceValues.kt, domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt
- Acceptance:
  - `BackupFlag` has the seven variants and `tag()` values `"1","2","4","8","10","20","40"`; `from_tag("3")` is `None` (TC-BK-03)
  - tags are never OR-ed together (Q-BK-1); unknown tags are dropped on write (E-BK-03, TC-BK-04)
  - `backupFlags` has no caller outside the domain module (Q-BK-2); the key is kept for backup-format parity and area:21-backup-format-and-create decides its UI

### T-09a-054 — Serializable models: serde replaces java.io.Serializable
- Priority: P3
- Effort: S
- Depends: T-09a-010
- Sources: domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt, domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt
- Acceptance:
  - `Serializable` is not reproduced; models derive `serde::Serialize` and `Deserialize` where a backup needs them
  - the drop reason (JVM object serialization is not a file format, no binary compatibility is required) is recorded in `p1-rust-design.md` section 7

### T-09a-055 — Gradle build script and Android manifest: dropped
- Priority: P3
- Effort: S
- Depends: T-09a-001
- Sources: domain/build.gradle.kts, domain/src/main/AndroidManifest.xml
- Acceptance:
  - the Cargo workspace replaces the Gradle module; each dependency in `domain/build.gradle.kts` has a stated replacement in `p1-rust-design.md` section 8 (`unifile` → std::fs/gio, sqldelight paging → rusqlite keyset paging, coroutines → tokio, kotlinx.serialization → serde, compose stablemarker → none)
  - the empty manifest has no Rust counterpart; no file is created for it

### T-09a-056 — SY data-saver flag on the manga source: dropped
- Priority: P3
- Effort: S
- Depends: T-09a-002
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt
- Acceptance:
  - `is_excluded_from_data_saver` is not in the Rust `Source`; the loss is stated: a source cannot be excluded from the image data saver (the data saver itself is area:34-images-covers-cache, if implemented)
  - the backup format keeps no field for it (backups from forks that carry it are read and the field ignored)

## Tests and cross-cutting

### T-09a-057 — Port the GetApplicationReleaseTest cases
- Priority: P2
- Effort: S
- Depends: T-09a-042, T-09a-043
- Sources: domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt
- Acceptance:
  - TC-REL-P1 (`When has update but is preview expect new update`), TC-REL-P2 (`When has update expect new update`), TC-REL-P3 (`When has no update expect no new update`) and TC-REL-P4 (`When now is before three days expect no new update`, service not called) pass in `crates/kuro-domain/tests/release.rs`
  - the fake `ReleaseSource` counts calls; the clock is injected

### T-09a-058 — Test harness: in-memory database, fake HTTP server, temp directories
- Priority: P1
- Effort: S
- Depends: T-09a-001, area:03-db-manga-schema-migrations
- Sources: domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt
- Acceptance:
  - dev-dependencies `wiremock`, `tempfile` and `tokio` `test-util` are declared in `kuro-domain`'s `Cargo.toml` only
  - an in-memory `kuro-db` connection with all migrations applied is created by one helper; the fixtures of `p1-edge-cases-and-tests.md` section 5 exist under `crates/kuro-domain/tests/fixtures/`

### T-09a-059 — Error and logging policy table, and the Result-returning service surface
- Priority: P1
- Effort: S
- Depends: T-09a-001
- Sources: domain/src/main/java/tachiyomi/domain/track/manga/interactor/GetMangaTracks.kt, domain/src/main/java/tachiyomi/domain/custombuttons/interactor/CreateCustomButton.kt, domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt
- Acceptance:
  - `p1-behavior.md` section 0 lists, per interactor, whether it swallows, returns a typed result, or propagates; the Rust code matches the table
  - `TrackService` offers both `*_or_default` (parity) and `Result` (new UI) methods; `log_and_default` is the only swallow helper (TC-TRK-05)
  - the custom-button `InternalError` cases carry the source error and log it at `error` level

### T-09a-060 — Remote page trait and paging contract for browse lists
- Priority: P0
- Effort: M
- Depends: T-09a-004, area:06-data-repos-history-track-source-misc
- Sources: domain/src/main/java/tachiyomi/domain/source/manga/repository/MangaSourceRepository.kt, domain/src/main/java/tachiyomi/domain/source/anime/repository/AnimeSourceRepository.kt
- Acceptance:
  - `PageSource<T>::load(page)` uses 1-based page numbers (Kotlin `PagingSource<Long, _>` keys) and returns `(items, next)`; `next = None` ends paging
  - an empty page with a `Some` next stops after one empty page (E-SRC-10)
  - a source-not-installed error surfaces as `DomainError::SourceNotInstalled` on the first page request (E-SRC-04)

### T-09a-061 — Topic deep dives and drop/keep decisions recorded for review
- Priority: P2
- Effort: S
- Depends: T-09a-001
- Sources: domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/UpdateMangaExtensionRepo.kt, domain/src/main/java/mihon/domain/extension/anime/interactor/UpdateAnimeExtensionStores.kt, domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt
- Acceptance:
  - the topic files `p1-topic-extension-repos-and-stores.md`, `p1-topic-download-policy.md` and `p1-topic-source-model-pins-stubs.md` exist, each with a drop-or-keep table for the parts of its subsystem that are Android-only
  - every "Proposal:" in those files is either linked to a task id in this list or marked as a decision for the user
