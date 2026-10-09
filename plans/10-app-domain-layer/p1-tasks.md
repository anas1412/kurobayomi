# Area 10 - implementation tasks (part 1 of 1)

Scope: `eu.kanade.domain` (app-level domain layer): `DomainModule`, entry/item update + sync interactors, source/extension listing, tracking orchestration. Rust modules named below are in `p1-rust-design.md` section 1 (crate `kuro-domain`, `src/app/`). Edge-case ids `EC-nn` and test names are in `p1-edge-cases-and-tests.md`; algorithm text is in `p1-behavior.md` and the three topic files.

### T-10a-001 — Domain container replacing DomainModule and SYDomainModule
- Priority: P0
- Effort: M
- Depends: area:01-foundation-app-bootstrap, area:02-preferences-system, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/DomainModule.kt, app/src/main/java/eu/kanade/domain/SYDomainModule.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - `Domain::new(DomainDeps)` builds without panicking using in-memory fakes from `kuro-testkit`; `Domain::start().await` returns when the delayed tracking queue is empty.
  - The crate `kuro-domain` has no dependency on `gtk4`, `gio` or `glib` (checked by `cargo tree -p kuro-domain | grep -E "gtk|glib|gio"` printing nothing).
  - Every in-scope row of `p1-topic-domainmodule-registry.md` (owner `area:10-app-domain-layer`, 47 rows plus the SY row) has a public accessor on `Domain`; accessors are cheap (`Clone` of `Arc` handles, no IO).
  - The singletons listed in the registry as owned by this area (`DelayedTrackingStore` manga and anime, `SourcePreferences`, `TrackPreferences`) exist once per `Domain`.

### T-10a-002 — Reactive helpers: combine_latest, pref_stream, distinct_until_changed, non_cancellable
- Priority: P0
- Effort: S
- Depends: T-10a-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt, app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionsByType.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt
- Acceptance:
  - `combine3` emits nothing until all three inputs produced a value, then exactly once per later input change (test with three `watch` channels and a counter).
  - `pref_stream(&pref)` yields the current value first, then each change; `distinct_until_changed` suppresses equal consecutive items.
  - `non_cancellable(fut)` keeps running after the awaiting future is dropped (test: drop the outer future, observe the side effect within 100 ms).
  - The helpers are `Send` and usable from `glib::spawn_future_local` through `ui_stream` without blocking the main loop (compile-time test in the GUI crate).

### T-10a-003 — Viewer flag helpers and SetMangaViewerFlags
- Priority: P0
- Effort: S
- Depends: T-10a-001, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt
- Acceptance:
  - `set_flag(v, flag, mask) == (v & !mask) | (flag & mask)` for the 12 vectors of test `flags::set_flag_matches_kotlin`.
  - `SetMangaViewerFlags::set_reading_mode(id, flag)` changes only bits 0-2 and `set_orientation` only bits 3-5 of `mangas.viewer`; values 1-5 and 0x08-0x30 are accepted exactly as the enums' `flagValue`.
  - Parallel calls to both setters on one manga leave both bits set (EC-23), using a single atomic `UPDATE` statement.
  - `Manga::reading_mode()` returns `viewer & 0x7` and `reader_orientation()` returns `viewer & 0x38` as raw integers; unknown stored values map to DEFAULT in the enum conversion.

### T-10a-004 — Anime viewer flags: skip-intro, intro-disable, next airing
- Priority: P0
- Effort: S
- Depends: T-10a-003
- Sources: app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt
- Acceptance:
  - Masks equal `INTRO 0xFF`, `AIRING_EPISODE 0xFFFF00`, `AIRING_TIME 0xFFFFFFFF<<24`, `DISABLE 1<<56` (test `flags::anime_viewer_roundtrip`).
  - `set_skip_intro_length(id, 0)` sets bit 56 and length 0; `set_skip_intro_length(id, 30)` clears bit 56 and stores 30; `300` stores 44 (EC-21).
  - `set_next_episode_airing(id, 12, 1_700_000_000)` reads back `(12, 1_700_000_000)`; `(0, 0)` clears both fields (EC-22); the two fields are written in one statement.
  - Effective intro length used by the player: disabled bit -> 0; stored length 0 -> the `defaultIntroLength` preference; else stored length.

### T-10a-005 — Entry model mappers (manga and anime)
- Priority: P0
- Effort: S
- Depends: T-10a-001, area:07-domain-entries-items, area:15-source-api-manga, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt
- Acceptance:
  - `Manga::to_s_manga`, `copy_from(&SManga)`, `SManga::to_domain_manga(source_id)` and anime twins implement every field rule of `p1-behavior.md` section 2.2, including the genre rules (EC-19, EC-20): a blank non-null `genre` clears genres in `copy_from`; `get_genres` splits on the two-character `", "`, trims, drops blanks, de-duplicates in first-seen order.
  - `to_s_manga` joins genres with `", "`, does not copy `update_strategy`; `to_domain_manga` yields the `Manga::create()` defaults of section 2.2 for every field not copied.
  - `copy_from` keeps `initialized = other.initialized && self.initialized`.
  - `has_custom_cover` / `has_custom_background` delegate to `CoverCache`.

### T-10a-006 — Chapter and episode mappers
- Priority: P0
- Effort: S
- Depends: T-10a-005
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt, app/src/main/java/eu/kanade/domain/items/episode/model/Episode.kt
- Acceptance:
  - `Chapter::to_s_chapter` and `copy_from_s_chapter` copy `url, name, date_upload, chapter_number (f32 widened to f64 with `as f64`), scanlator` where the scanlator is `ifBlank{null}` and trimmed (chapters) / only blank-to-null (episodes).
  - `Episode` mappers also carry `fillermark, summary (blank to null), preview_url (blank to null), memo`.
  - A chapter number `12.1f32` survives `to_s_chapter(copy_from_s_chapter(x))` bit-identically (f32 round trip).
  - There is no function equivalent to `toDbChapter` / `toDbEpisode` (see T-10a-035).

### T-10a-007 — UpdateManga and UpdateAnime interactors
- Priority: P0
- Effort: M
- Depends: T-10a-005, area:05-data-repos-entries-items, area:07-domain-entries-items, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt, app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt
- Acceptance:
  - `update_from_source` implements the five ordered cover branches of `p1-behavior.md` section 3 with `FakeCovers` recording `delete_from_cache(manga, false)` calls (tests `update_manga::cover_branches`, EC-16, EC-17).
  - Title is written only for non-favourites with a non-empty remote title (EC-18); the update sets `initialized = true`, `status`, `update_strategy`, and writes `genre` only when `get_genres()` is `Some`.
  - `update_favorite(id, true)` sets `date_added` to the current epoch ms and `false` sets 0 (EC-25); `update_last_update`, `update_cover_last_modified` (anime also `update_background_last_modified`) write the current epoch ms.
  - `update_fetch_interval(entry, now, None)` computes the window with `FetchInterval::get_window(now)` and writes `next_update` and `calculate_interval`.
  - The anime `update_from_source` equivalent additionally applies the same rules to `background_url` / `background_last_modified` with the background cache.

### T-10a-008 — SyncChaptersWithSource (planner and applier)
- Priority: P0
- Effort: L
- Depends: T-10a-006, T-10a-007, T-10a-016, area:05-data-repos-entries-items, area:07-domain-entries-items, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, domain/src/main/java/tachiyomi/domain/items/chapter/interactor/ShouldUpdateDbChapter.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterRecognition.kt, data/src/main/java/tachiyomi/data/items/chapter/ChapterSanitizer.kt
- Acceptance:
  - `plan_chapter_sync` is a pure function; test `sync_chapters::worked_example` reproduces the worked example of `p1-topic-chapter-episode-sync.md` section 3 exactly (dates 1_000_003/2/1, c3b read+bookmark+dateFetch 500, return `[/c5, /c4]`).
  - Empty non-local source list returns `DomainError::NoChapters` and writes nothing; local source with an empty list proceeds (EC-01).
  - No-change sync performs no write unless `manual || fetch_interval == 0 || next_update < window.0` (EC-09, EC-10) and never touches `last_update` in that case.
  - Duplicate URLs (EC-03), deleted-twin lowest-fetch map (EC-04), duplicate-read marking with preference value `"new"` and its override by deleted-twin state (EC-05), `max_seen_upload_date` rules (EC-06), exclusion filter on the returned list only (EC-12) all have passing tests.
  - A source chapter whose scanlator became null is reported as needing update every run but the stored scanlator stays (EC-08); a chapter with `12.1f32` is not updated on an identical second sync.
  - `prepare_new_chapter` is applied only to HTTP-style sources; download folder rename is requested before the DB write and its failure does not abort the sync (EC-11).
  - Delete, insert, update, fetch-interval and last-update writes run in one DB transaction; a failing insert rolls back the delete.
  - Per-entry mutex prevents two concurrent syncs of the same manga from interleaving (race 2 in the edge-case file).

### T-10a-009 — SyncEpisodesWithSource
- Priority: P0
- Effort: L
- Depends: T-10a-008, area:07-domain-entries-items, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt, domain/src/main/java/tachiyomi/domain/items/episode/interactor/ShouldUpdateDbEpisode.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeRecognition.kt, data/src/main/java/tachiyomi/data/items/episode/EpisodeSanitizer.kt
- Acceptance:
  - Same planner contract as T-10a-008 with `seen` and the preference value `"new_episode"`; `NoEpisodes` error on empty non-local source.
  - `ShouldUpdateDbEpisode` compares `scanlator, name, date_upload, episode_number, source_order, summary, fillermark, preview_url, memo` (memo by JSON structural equality).
  - Updated rows write `name, episode_number, scanlator, summary, source_order, memo` always, `date_upload` only when non-zero, `fillermark` only while stored false, `preview_url` only while stored null/blank (test `sync_episodes::fillermark_sticky_and_preview_fill`).
  - No scanlator exclusion on the returned list.

### T-10a-010 — SyncSeasonsWithSource
- Priority: P1
- Effort: M
- Depends: T-10a-007, area:05-data-repos-entries-items, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt, domain/src/main/java/tachiyomi/domain/items/season/service/SeasonRecognition.kt, domain/src/main/java/tachiyomi/domain/items/season/interactor/ShouldUpdateDbSeason.kt, domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt
- Acceptance:
  - `SeasonRecognition::parse_season_number` passes the case list of test `season_recognition::cases`, implemented with `fancy-regex` for the look-behind `basic` pattern.
  - New seasons are created through `network_to_local` (insert on miss), updated seasons change only `title, season_number, season_source_order, memo`, removed seasons only get `parent_id = NULL` (EC-14).
  - Empty non-local source list -> `DomainError::NoSeasons` (message key `no_seasons_error`).
  - Returns all source seasons (not only new ones); the no-change path recomputes the fetch interval under the same condition as T-10a-008; a season equal to the parent is skipped with a warning (EC-13).

### T-10a-011 — SyncRelatedAnimeWithSource
- Priority: P2
- Effort: M
- Depends: T-10a-005, area:05-data-repos-entries-items, area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt, domain/src/main/java/tachiyomi/domain/entries/anime/repository/AnimeRelationRepository.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeRelationGroup.kt
- Acceptance:
  - Source without related-anime support or missing source returns immediately without DB access.
  - Unless `force_refresh`, a stored `fetched_at` younger than 604_800_000 ms (7 days) short-circuits; exactly 7 days old refreshes (test `related::ttl_and_cap`).
  - Each group is `distinct_by(url)`, minus the parent's url, capped at 10, mapped through `network_to_local` with the parent's source id; empty groups are dropped; `replace_relations(anime.id, groups, now)` is called once.

### T-10a-012 — Chapter and episode list filters and sorting
- Priority: P0
- Effort: M
- Depends: T-10a-005, area:07-domain-entries-items, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt, app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt
- Acceptance:
  - `downloaded_filter()` returns `EnabledIs` whenever `__APP_STATE_pref_downloaded_only` is true, otherwise maps flags `0x08 -> EnabledIs`, `0x10 -> EnabledNot`, else `Disabled` (anime seasons use `0x02` / `0x04`).
  - `chapters_filtered`, `episodes_filtered`, `seasons_filtered` follow the OR-lists of `p1-behavior.md` section 2.1.
  - `apply_filters` passes `filters::tri_state_matrix` (all 27 combinations plus fillermark for episodes), treats local entries as always downloaded, and sorts by source order (inverted for descending), number, upload date or collated name; an unknown sort value returns an error instead of panicking.
  - The same function serves the GUI row type through the `FilterRow` trait; no Compose-specific overload exists.

### T-10a-013 — SetReadStatus
- Priority: P0
- Effort: S
- Depends: T-10a-015, area:05-data-repos-entries-items, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Marking read updates only unread chapters and leaves `last_page_read`; marking unread updates chapters with `read || last_page_read > 0` and sets `last_page_read = 0` (EC-26).
  - Returns `NoItems` when the filtered list is empty, `InternalError` when the batch update fails (EC-28, nothing partially written), `Success` otherwise.
  - Download deletion happens only for `read == true` with `pref_remove_after_marked_as_read_key` true, grouped by manga id (EC-27); the whole operation survives cancellation of the caller (EC-29).
  - Overloads by `manga_id` and by `&Manga` exist.

### T-10a-014 — SetSeenStatus
- Priority: P0
- Effort: S
- Depends: T-10a-013
- Sources: app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt
- Acceptance:
  - Same contract as T-10a-013 with `seen`, `last_second_seen` reset to 0 on unseen, `total_seconds` untouched, result `NoItems` mapped to `NoEpisodes` for the UI.
  - Overloads by `anime_id` and by `&Anime`.

### T-10a-015 — DeleteChapterDownload and DeleteEpisodeDownload
- Priority: P1
- Effort: S
- Depends: T-10a-001, area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt, app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt
- Acceptance:
  - `await_all(manga, chapters)` resolves the source with `sources.get(manga.source)`; if absent it logs a warning and returns without deleting (EC-27).
  - The call runs through `non_cancellable`.
  - `DownloadFacade::delete_chapters` is invoked once with the full slice.

### T-10a-016 — Excluded scanlators and available scanlators
- Priority: P1
- Effort: S
- Depends: T-10a-001, area:03-db-manga-schema-migrations, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/interactor/GetExcludedScanlators.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt, app/src/main/java/eu/kanade/domain/items/chapter/interactor/GetAvailableScanlators.kt, data/src/main/sqldelight/data/excluded_scanlators.sq
- Acceptance:
  - `set(manga_id, set)` inserts `set - current`, deletes `current - set` with `scanlator IN (...)`, all in one transaction; an empty remove set is a no-op; table and index exist exactly as `excluded_scanlators.sq` (FK cascade on manga delete).
  - `get_excluded(manga_id)` returns the set; `subscribe` re-emits after `set`.
  - `GetAvailableScanlators` returns distinct non-blank scanlator strings of the manga's chapters (blank strings dropped), one-shot and reactive.

### T-10a-017 — SourcePreferences registry section
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system, area:52-util-app
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - All 30 keys of `p1-data-and-config.md` section 4.1 exist with identical key strings, types and defaults; `trusted_extensions`, `has_filters_toggle_state`, `last_*_catalogue_source` use the `__APP_STATE_` prefix.
  - Default of `source_languages` is `{"all", "en", <system language subtag lower-cased>}` computed at first read; an unparsable locale yields `en`.
  - Enum preferences store the enum NAME and fall back to the default on unknown names (test with garbage values).
  - Commented-out SY keys (`eh_enable_source_blacklist`, `sources_tab_*`) are not registered.

### T-10a-018 — GetEnabledSources (manga and anime)
- Priority: P0
- Effort: M
- Depends: T-10a-002, T-10a-017, area:06-data-repos-history-track-source-misc, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt, domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt
- Acceptance:
  - `compute_enabled_sources` passes `sources::enabled_sources_cases` for EC-30..EC-33: language filter (local source id 0 always kept), hidden-id filter, Java-style case-insensitive name sort, pin flags `0b11`/`0`, last-used duplicate row with `is_used_last = true` and `pin` XOR `0b10`.
  - `Pins` reproduces `contains(Unpinned) == true` and XOR `minus` (EC-34).
  - Manga rows carry `is_excluded_from_data_saver`; anime rows do not.
  - The stream emits current value first and is `distinct_until_changed`.
  - `cmp_ignore_case_java` matches the committed JVM vectors in `fixtures/java_ci_order.json`.

### T-10a-019 — GetLanguagesWithSources and language ordering
- Priority: P0
- Effort: S
- Depends: T-10a-018, area:51-i18n
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - Output keys ordered by `(lang not in enabled)` then `lang_cmp` (`"all"` first, then native display name by UTF-16 code unit order); `zh-CN`/`zh-TW` display as `zh-Hans`/`zh-Hant` names (test `sources::language_order`).
  - Sources inside a language are ordered by `(disabled)` then case-insensitive name.
  - `lang_display_name` resolves every tag in the committed extension index fixture without falling back to the raw tag, except documented unknown ones.

### T-10a-020 — Toggle source, pin, language
- Priority: P0
- Effort: S
- Depends: T-10a-017
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSource.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt, app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSourcePin.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSourcePin.kt, app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt
- Acceptance:
  - `toggle_source(id, None)` flips membership in the hidden set; `Some(true)` removes the id (enable), `Some(false)` adds it; the list overload applies set difference / union of decimal strings.
  - `toggle_pin(source)` and `toggle_language(lang)` flip membership; each is a single atomic `Pref::update`.
  - Stored strings are decimal ids; ids that fail to parse in stored data are ignored by readers.

### T-10a-021 — Sources with favorite count and migration sorting
- Priority: P1
- Effort: M
- Depends: T-10a-018, area:51-i18n
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt, app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt, core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt
- Acceptance:
  - The local source (id 0) is excluded; `ALPHABETICAL` sorts stubs first then collated lower-cased names; `TOTAL` sorts stubs first then by count; `DESCENDING` reverses the whole comparator including the stub rule (test `sources::migration_sort`, EC-35).
  - `set_migrate_sorting(mode, direction)` writes `pref_migration_sorting` and `pref_migration_direction` as enum names.
  - Collation uses `icu_collator` for the system locale at tertiary strength.

### T-10a-022 — Incognito state and toggles
- Priority: P1
- Effort: S
- Depends: T-10a-017, area:20-extension-strategy-redesign
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaIncognitoState.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeIncognitoState.kt, app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaIncognito.kt, app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeIncognito.kt
- Acceptance:
  - `await(source_id)`: global flag true -> true; `None` -> false; unknown source/extension -> false (EC-41); else extension package in the incognito set.
  - `subscribe` combines the global flag, the set and `watch_extension_package`, and is `distinct_until_changed`.
  - `toggle_incognito(package, enable)` adds or removes the package name in `incognito_manga_extensions` / `incognito_anime_extensions`.

### T-10a-023 — Extensions by type, extension languages, extension sources
- Priority: P1
- Effort: M
- Depends: T-10a-002, T-10a-017, area:20-extension-strategy-redesign, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionsByType.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt, app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionLanguages.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionLanguages.kt, app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionSources.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt, app/src/main/java/eu/kanade/domain/extension/manga/model/MangaExtensions.kt, app/src/main/java/eu/kanade/domain/extension/anime/model/AnimeExtensions.kt
- Acceptance:
  - `compute_extensions_by_type` passes `extensions::by_type_cases`: installed list filtered by NSFW, sorted obsolete-first then case-insensitive name, partitioned into `updates` and `installed`; `untrusted` sorted by name without NSFW filter; `available` excludes installed and untrusted package names, applies NSFW and language filters, and expands to one row per enabled-language source with row key `<pkg>-<sourceId>` (EC-37, EC-38).
  - Manga variant keeps an extension with zero sources when its own `lang` is enabled; the anime variant drops it.
  - `showNsfwSource` is observed reactively (documented improvement over the one-time read).
  - Language list: distinct source languages (manga: extension `lang` when no sources), enabled first, then `lang_cmp`.
  - `labelAsName` equals `is_multi_source && !is_multi_lang_single_source` per the truth-table test `extensions::sources_label_mode`; `enabled` is `!hidden.contains(id.to_string())` and re-emits when the hidden set changes.

### T-10a-024 — TrustExtension (manga and anime)
- Priority: P1
- Effort: M
- Depends: T-10a-017, area:20-extension-strategy-redesign
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - `is_trusted(identity)` is true iff some repository/store signing key appears in `identity.fingerprints` or the string `"<name>:<version_code>:<last fingerprint>"` is in `__APP_STATE_trusted_extensions`; empty `fingerprints` returns false without panic (EC-39).
  - `trust(name, version_code, hash)` removes every entry starting with `"<name>:"` then inserts the new triple (EC-40); `revoke_all()` deletes the preference and leaves repository keys untouched.
  - Stored triple format is byte-identical to Aniyomi so backups restore.
  - One implementation serves manga and anime, parameterised by the key provider (`extension_repos` vs extension stores).

### T-10a-025 — Data-saver preferences and ToggleExcludeFromMangaDataSaver
- Priority: P2
- Effort: S
- Depends: T-10a-017
- Sources: app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleExcludeFromMangaDataSaver.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt, app/src/main/java/eu/kanade/domain/SYDomainModule.kt
- Acceptance:
  - `toggle_exclude_from_data_saver(source)` toggles the decimal id in `data_saver_excluded`.
  - The seven `data_saver_*` preferences and enum `DataSaver {NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT}` exist with the defaults of section 4.1; the enabled-sources stream sets `is_excluded_from_data_saver` from them.
  - Decision recorded: the image-proxy implementations (Bandwidth Hero, wsrv.nl, resmush.it) are NOT built in this task; selecting a mode other than NONE is stored but the reader area decides whether to implement the proxy (note in the settings UI: "not available" until then).

### T-10a-026 — TrackPreferences and AutoTrackState
- Priority: P1
- Effort: M
- Depends: area:02-preferences-system, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt
- Acceptance:
  - Keys, types, defaults of `p1-data-and-config.md` section 4.2 exist; per-tracker keys embed the tracker id; username/password/token keys use the `__PRIVATE_` prefix and are stored through the `SecretStore` (area:02 `SecretPref`), never in the plain preference file.
  - `set_credentials(tracker, user, pass)` writes username, password and resets `trackAuthExpired` to false.
  - `AutoTrackState` stores `ALWAYS`/`ASK`/`NEVER`, defaults to `ALWAYS`, unknown stored values fall back to `ALWAYS`; labels resolve through i18n keys `lock_always`, `default_category_summary`, `lock_never`.
  - `anilistScoreType` default is the string `POINT_10`.

### T-10a-027 — Track model and personal-field copy
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core, area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/domain/track/manga/model/MangaTrack.kt, app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt, app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt
- Acceptance:
  - One `Track` struct holds the fields of `p1-data-and-config.md` section 1.3; `id: Option<i64>`; DB mapping stores `None` as insert-new and `-1` is never persisted.
  - `copy_personal_from(&other)` copies exactly `last_progress, score, status, start_date, finish_date, private`.
  - Column mapping manga `last_chapter_read/total_chapters/started_reading_date/finished_reading_date` and anime `last_episode_seen/total_episodes/started_watching_date/finished_watching_date` is covered by a repository round-trip test for both kinds.

### T-10a-028 — TrackChapter and TrackEpisode
- Priority: P1
- Effort: M
- Depends: T-10a-027, T-10a-029, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt
- Acceptance:
  - For each stored track: skipped when the tracker is missing, logged out, or `number <= last_progress` (EC-43); eligible tracks run concurrently and one failure never blocks another (EC-44).
  - Online path: refresh, set progress, `update(.., did_read = true)`, store the row, remove the queue entry; any failure queues `(kind, track_id, number)` and calls `worker.schedule()` when `setup_job_on_failure` (unified rule, see tracking topic section 2; auth errors are not queued).
  - Offline (`Connectivity::is_online() == false`): queue and schedule without attempting the network (EC-45).
  - Errors are logged at info and not returned to the caller; the call survives caller cancellation.
  - Message keys used by callers are available: `trackers_updated_summary_manga` / `_anime` with the integer part of the number, `confirm_tracker_update` / `confirm_tracker_update_anime`, `action_ok`.

### T-10a-029 — Delayed tracking queue and retry worker
- Priority: P1
- Effort: M
- Depends: T-10a-001, area:01-foundation-app-bootstrap, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/domain/track/manga/store/DelayedMangaTrackingStore.kt, app/src/main/java/eu/kanade/domain/track/anime/store/DelayedAnimeTrackingStore.kt, app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt, app/src/main/java/eu/kanade/domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt
- Acceptance:
  - `tracking_queue.json` holds separate `manga` and `anime` maps; `add` keeps only the greater progress as `f32` (EC-46, EC-54); writes are atomic (a stray `.tmp` file from a crash is ignored and replaced).
  - Worker: waits until online, replays every item through `TrackChapter`/`TrackEpisode` with `setup_job_on_failure = false`, removes entries whose track no longer exists, stops when the queue is empty; otherwise sleeps 5, 10, 20 minutes and gives up after four runs (test `track::worker_backoff_schedule` with paused tokio time, EC-48).
  - `schedule()` while running restarts the backoff after the in-flight item completes; `Domain::start` schedules when the stored queue is non-empty.
  - Entries whose tracker is logged out after the final attempt are dropped (EC-47).

### T-10a-030 — AddTracks.bind and bindEnhancedTrackers
- Priority: P1
- Effort: M
- Depends: T-10a-027, T-10a-032, area:27-tracking-core, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt
- Acceptance:
  - `bind` order: tracker.bind(item, has_read) -> insert -> (if any read chapter) push the integer part of the longest read prefix when greater than stored progress -> set start date from the earliest history `readAt` when `start_date <= 0` (None dates sort first, EC-50) using the local-wall-clock-as-UTC conversion -> `sync_progress` (test `track::add_tracks_bind_pushes_prefix`: read {1,2,3,5} pushes 3).
  - The whole bind is non-cancellable and runs off the UI thread.
  - `bind_enhanced_trackers(entry, source)` iterates logged-in enhanced trackers whose `accept(source)` is true, binds the single match, inserts, syncs; a failure of one tracker logs a warning "Could not match manga: <title> with service <name>" and continues.
  - Remote setters failing do not abort the remaining steps (tracker area shows the toast).

### T-10a-031 — RefreshMangaTracks and RefreshAnimeTracks
- Priority: P1
- Effort: S
- Depends: T-10a-027, T-10a-032, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/RefreshMangaTracks.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt
- Acceptance:
  - Refreshes every stored track whose tracker is logged in, concurrently; each success stores the refreshed row and runs `sync_progress`.
  - Returns `Vec<(TrackerId, TrackError)>` of failures only; a logged-out tracker is skipped silently (EC-53); a panic in one future is captured as a failure.
  - Test `track::refresh_collects_failures` (3 trackers, one failing).

### T-10a-032 — SyncChapterProgressWithTrack and SyncEpisodeProgressWithTrack
- Priority: P1
- Effort: S
- Depends: T-10a-027, area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin
- Sources: app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/SyncEpisodeProgressWithTrack.kt
- Acceptance:
  - No-op unless the tracker is an enhanced tracker.
  - Marks local recognised chapters `<= remote progress` read; `last = max(remote, contiguous local read prefix computed on the pre-update state)`; order of effects: remote update first, then local chapter updates, then track row (EC-52: remote failure leaves local chapters unchanged, warning logged).
  - Both worked examples of the tracking topic section 7 pass.

### T-10a-033 — getComicInfo builder
- Priority: P2
- Effort: S
- Depends: area:14-source-local-and-metadata, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt
- Acceptance:
  - `get_comic_info` maps every field of `p1-behavior.md` section 2.3 including number formatting (`12.0 -> "12"`, `12.5 -> "12.5"`, negative -> absent), `Web` joined by a single space, genre/categories joined by `", "`, status text table, `SourceAniyomi = source name`, and nulls for inker/colorist/letterer/coverArtist/tags.
  - A golden `ComicInfo.xml` produced with the area:14 serializer equals the committed fixture.

### T-10a-034 — Source icon provider (replaces AnimeSource.icon and Source.icon)
- Priority: P2
- Effort: S
- Depends: area:20-extension-strategy-redesign, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/domain/source/manga/model/MangaSource.kt, app/src/main/java/eu/kanade/domain/source/anime/model/AnimeSource.kt
- Acceptance:
  - `SourceIcons::get(source_id)` returns a texture loaded from the cached extension icon file (`<cache>/ext-icons/<pkg>.png`), the app icon for the local source, and the symbolic fallback icon for unknown sources.
  - Loading happens off the main thread and the GUI receives the texture through the UI channel; a missing or corrupt file never panics.

### T-10a-035 — Android-only constructs dropped from this scope
- Priority: P3
- Effort: S
- Depends: T-10a-001
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt, app/src/main/java/eu/kanade/domain/items/episode/model/Episode.kt, app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt, app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt
- Acceptance:
  - `Chapter.toDbChapter` and `Episode.toDbEpisode` (legacy `ChapterImpl`/`EpisodeImpl` Android DB models, carrying a migration-cleanup comment in the Kotlin) have no Rust counterpart; the decision and the lost nothing statement are recorded in the module docs.
  - `androidx.work` jobs, `SharedPreferences`, `PackageInfo`, Injekt and Compose `ImageBitmap` are not referenced anywhere in `kuro-domain` (grep check in CI), with replacements as listed in `p1-rust-design.md` section 7.
  - The user loses nothing visible: WorkManager persistence across process death is replaced by `Domain::start` replay (T-10a-029).

### T-10a-036 — Golden and integration test suite for the domain layer
- Priority: P0
- Effort: M
- Depends: T-10a-008, T-10a-009, T-10a-012, T-10a-018, T-10a-028
- Sources: domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt, domain/src/test/java/tachiyomi/domain/entries/manga/interactor/MangaFetchIntervalTest.kt, domain/src/test/java/tachiyomi/domain/entries/anime/interactor/AnimeFetchIntervalTest.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - Every test named in `p1-edge-cases-and-tests.md` section 4 exists and passes under `cargo test -p kuro-domain` in under 60 seconds.
  - Fixtures of section 5 are committed, including the JVM-generated `java_ci_order.json`.
  - The integration test `domain_end_to_end` runs sync (new chapters) -> mark read -> track update (offline queue) -> go online -> worker replays, on an in-memory DB with fakes, and asserts final DB, queue and tracker state.

### T-10a-037 — DomainModule registry parity check
- Priority: P1
- Effort: S
- Depends: T-10a-001
- Sources: app/src/main/java/eu/kanade/domain/DomainModule.kt, app/src/main/java/eu/kanade/domain/SYDomainModule.kt
- Acceptance:
  - A test parses `p1-topic-domainmodule-registry.md` (embedded with `include_str!`) and fails when a row owned by `area:10-app-domain-layer` has no `Domain` accessor of the listed Rust name; rows owned by other areas must appear in a `REGISTRY_OTHER_AREAS` list that names the owning area, so a forgotten class cannot hide.
  - The test prints the count of rows per owner (this scope: 47 plus the SY row).
