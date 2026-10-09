# Area 11 app-migrations, part 1: data and configuration

Everything under OBSERVED is read from the Kotlin source; Proposal sections say what Kurobayomi stores. The area-11 scope has no network endpoints and no SQL schema of its own.

## 1. Framework data

OBSERVED:

| Item | Type | Value |
|---|---|---|
| `Migration.version` | `Float` | `-1f` (`ALWAYS`) or the build number at which the step becomes due |
| `BuildConfig.VERSION_CODE` | `Int` | `132` (`app/build.gradle.kts:22`; `versionName = "0.18.2.1"`) |
| stored build number | preference `__APP_STATE_last_version_code` | Int, default `0`; written only by the completion listener after a run whose result was true |
| `MigrationContext.dryrun` | `Boolean` | `false` in production |
| `Migrator.scope` | `CoroutineScope(Dispatchers.IO + Job())` | one per process |
| migrations registered | 44 | 3 always-run, 41 versioned (versions 7, 15, 19, 26, 44, 52, 54, 57, 59, 60, 61, 64, 64, 70, 71, 72, 75, 77, 81, 82, 84, 85, 86, 92, 93, 96, 97, 99, 105, 106, 113, 114, 116, 117, 120, 121, 123, 126, 127, 129, 130) |
| preference key prefixes | `__APP_STATE_` (app state, never in backups), `__PRIVATE_` (secrets, only with explicit consent) | `Preference.kt:27-48` |

Proposal for Kurobayomi:

| Item | Type | Value |
|---|---|---|
| `VERSION_CODE` | `u32` constant in `kuro_core::app_info` | starts at `1`; +1 per release that adds a migration |
| `last_version_code` | key in `state.toml` | `u32`, default `0` meaning "fresh profile"; never renamed |
| `Version` | `enum { Always, At(u32) }` | `Always < At(n)`, ordering as `-1f` |
| registered list | `static MIGRATIONS: &[Migration]` | the three always-run set-up steps; empty versioned list at `VERSION_CODE = 1` |

## 2. Preference keys read or written by the migrations

Types are the Android SharedPreferences primitive kinds (Int, Long, Float, String, Boolean, StringSet); "enum" means a String holding the enum constant name (`getEnum`/`serializer = { it.name }`). "Step" is the version(s) touching the key. "Kurobayomi" says what the Rust app does with the key: `typed` = a normal setting owned by area:02-preferences-system (see `plans/02-preferences-system/p1-topic-key-registry.md`), `normaliser` = only read by the backup-import normaliser (`p1-topic-legacy-normaliser.md`), `ignored` = dropped.

| Key | Kind | Default (current code) | Step(s) | Kurobayomi |
|---|---|---|---|---|
| `__APP_STATE_last_version_code` | Int | 0 | framework | typed (`state.toml`) |
| `backup_interval` | Int hours | 12 | ALWAYS, 84 | typed |
| `pref_library_update_interval_key` | Int hours | 0 | ALWAYS, 61, 71 | typed |
| `library_update_restriction` | StringSet | `{"wifi"}` | ALWAYS, 105 | typed |
| `library_update_manga_restriction` | StringSet | `{"manga_fully_read","manga_ongoing","manga_started","manga_outside_release_period"}` | 72 | typed |
| `pref_update_only_non_completed_key` | Boolean | true (migration default) | 72 | normaliser |
| `library_sorting_mode` | enum-like String `"<TYPE>,<DIRECTION>"` (earlier Int, then String) | `"ALPHABETICAL,ASCENDING"` | 44, 64, 81, 82 | typed + normaliser |
| `animelib_sorting_mode` | same for anime | `"ALPHABETICAL,ASCENDING"` | 44, 64, 81, 82 | typed + normaliser |
| `library_sorting_ascending` | Boolean (before 64), String (64 to 81) | true / `"ASCENDING"` | 64, 82 | normaliser |
| `pref_filter_downloaded_key`, `pref_filter_unread_key`, `pref_filter_completed_key` | Boolean | false | 52 | normaliser |
| `pref_filter_library_downloaded`, `_unread`, `_unseen`, `_started`, `_bookmarked`, `_completed`, `_tracked_<id>` | Int 0/1/2 | 0 | 99 | normaliser |
| `pref_filter_library_downloaded_v2`, `_unread_v2`, `_started_v2`, `_bookmarked_v2`, `_completed_v2`, `_tracked_<id>_v2` | enum TriState | `DISABLED` | 52, 99 | typed |
| `pref_filter_animelib_downloaded_v2`, `_unread_v2`, `_started_v2`, `_bookmarked_v2`, `_completed_v2`, `_tracked_<id>_v2` | enum TriState | `DISABLED` | none (see quirks item 8) | typed |
| `default_chapter_filter_by_read`, `default_chapter_filter_by_downloaded`, `default_chapter_filter_by_bookmarked`, `default_chapter_sort_by_source_or_number`, `default_chapter_display_by_name_or_number`, `default_chapter_sort_by_ascending_or_descending`, `default_episode_filter_by_seen`, `default_episode_filter_by_downloaded`, `default_episode_filter_by_bookmarked`, `default_episode_sort_by_source_or_number` | Long (Int before 85) | see section 3 | 85 | typed + normaliser |
| `default_category`, `default_anime_category` | Int | -1 | 129 | typed |
| `library_update_categories`, `animelib_update_categories`, `library_update_categories_exclude`, `animelib_update_categories_exclude` | StringSet of category ids | empty | 129 | typed |
| `remove_exclude_categories`, `remove_exclude_anime_categories`, `download_new_categories`, `download_new_anime_categories`, `download_new_categories_exclude`, `download_new_anime_categories_exclude` | StringSet | empty | 129 | typed |
| `source_languages` | StringSet | `{"all","en",<system language>}` | 70 | typed |
| `anime_extension_repos`, `extension_repos` | StringSet of URLs | empty | 7, 114 | normaliser |
| `extension_installer` | enum | `PACKAGEINSTALLER` (`LEGACY` on MIUI) | 75 | ignored (P3) |
| `secure_screen` | Boolean | false | 75 | normaliser |
| `secure_screen_v2` | enum `ALWAYS`/`INCOGNITO`/`NEVER` | `INCOGNITO` | 75 | typed |
| `enable_doh` | Boolean | false | 57 | normaliser |
| `doh_provider` | Int | -1 | 57 | typed |
| `pref_rotation_type_key` | Int 1..5 | 1 | 59, 60 | normaliser |
| `pref_default_viewer_key` | Int 1..5 | 1 | 60 | normaliser |
| `pref_default_orientation_type_key` | Int flag | `0x08` (FREE) | 60 | typed |
| `pref_default_reading_mode_key` | Int flag | `0x02` (RIGHT_TO_LEFT) | 60 | typed |
| `reader_tap` | Boolean | false (migration default) | 77 | normaliser |
| `reader_navigation_mode_pager`, `reader_navigation_mode_webtoon` | Int 0..5 | 0 | 77 | typed |
| `pref_theme_mode_key` | enum `LIGHT`/`DARK`/`SYSTEM` | `SYSTEM` | 86 | typed |
| `relative_time` | Int | 7 (migration default) | 106 | normaliser |
| `relative_time_v2` | Boolean | true | 106 | typed |
| `bottom_nav_style` | Int | 0 | 120 | normaliser |
| `default_home_tab_library` | Boolean | false | 120 | normaliser |
| `start_screen` | enum `ANIME`/`MANGA`/`UPDATES`/`HISTORY`/`BROWSE` | `ANIME` | 120 | typed |
| `bottom_rail_nav_style` | enum `MOVE_MANGA_TO_MORE`/`MOVE_UPDATES_TO_MORE`/`MOVE_HISTORY_TO_MORE`/`MOVE_BROWSE_TO_MORE` | `MOVE_HISTORY_TO_MORE` | 120 | typed |
| `pref_progress_preference` | Float (String before 92) | 0.85 | 92 | typed |
| `pref_skip_length_preference` | Int (String before 93) | 10 | 93 | typed |
| `pref_default_player_orientation_type_key` | enum `PlayerOrientation` (Int before 127, String digits before 93) | `SensorLandscape` | 93, 127 | typed (no desktop effect) |
| `pref_deband`, `pref_player_view_mode`, `gpu_next` | Int, Int, Boolean | 0, 1, false | 123 | normaliser |
| `pref_video_debanding` | enum `None`/`CPU`/`GPU` | `None` | 123 | typed |
| `pref_player_aspect_state` | enum `Crop`/`Fit`/`Stretch` | `Fit` | 123 | typed |
| `pref_gpu_next` | Boolean | false | 123 | typed |
| `pref_invert_playback_txt`, `pref_invert_duration_txt`, `pref_hwdec` | any | n/a | 123 (removed) | ignored |
| `pref_sub_select_conf` | String JSON | `""` | 126 | normaliser |
| `pref_subtitle_lang`, `pref_subtitle_whitelist`, `pref_subtitle_blacklist` | String (comma separated) | `""` | 126, 130 | typed |
| `pref_audio_lang` | String (comma separated) | `""` | 130 | typed |
| `pref_download_only`, `incognito_mode`, `last_catalogue_source`, `trusted_signatures`, `last_app_closed`, `library_update_last_timestamp`, `library_unseen_updates_count`, `last_used_category`, `last_app_check`, `last_ext_check`, `last_version_code`, `storage_dir` | various | n/a | 113 (renamed to `__APP_STATE_<key>`) | ignored (un-prefixed) |
| `__APP_STATE_trusted_signatures` | StringSet | n/a | 117 (removed) | ignored |
| `pref_mangasync_*`, `track_token_*` | String | n/a | 116 (renamed to `__PRIVATE_<key>`) | ignored |
| `automatic_ext_updates` | Boolean | n/a | 97 (removed) | ignored |

Other SharedPreferences files touched: `tracking_queue` (file name `"tracking_queue"`, `MODE_PRIVATE`; keys are tracker row ids as decimal strings, values Float last-chapter-read; step 93 rewrites it; area:27-tracking-core owns the queue).

## 3. Chapter and episode flag layouts (read by step 85 and stored as Long)

Manga (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:77-103`):

| Constant | Value | Mask |
|---|---|---|
| `SHOW_ALL` | `0x00000000` | |
| `CHAPTER_SORT_DESC` / `CHAPTER_SORT_ASC` | `0x0` / `0x1` | `CHAPTER_SORT_DIR_MASK = 0x1` |
| `CHAPTER_SHOW_UNREAD` / `CHAPTER_SHOW_READ` | `0x2` / `0x4` | `CHAPTER_UNREAD_MASK = 0x6` |
| `CHAPTER_SHOW_DOWNLOADED` / `CHAPTER_SHOW_NOT_DOWNLOADED` | `0x8` / `0x10` | `CHAPTER_DOWNLOADED_MASK = 0x18` |
| `CHAPTER_SHOW_BOOKMARKED` / `CHAPTER_SHOW_NOT_BOOKMARKED` | `0x20` / `0x40` | `CHAPTER_BOOKMARKED_MASK = 0x60` |
| `CHAPTER_SORTING_SOURCE` / `_NUMBER` / `_UPLOAD_DATE` / `_ALPHABET` | `0x0` / `0x100` / `0x200` / `0x300` | `CHAPTER_SORTING_MASK = 0x300` |
| `CHAPTER_DISPLAY_NAME` / `CHAPTER_DISPLAY_NUMBER` | `0x0` / `0x100000` | `CHAPTER_DISPLAY_MASK = 0x100000` |

Anime (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:217-255`): `SHOW_ALL 0x0`; `EPISODE_SORT_DESC 0x0`, `EPISODE_SORT_ASC 0x1`, mask `0x1`; `EPISODE_SHOW_UNSEEN 0x2`, `EPISODE_SHOW_SEEN 0x4`, mask `0x6`; `EPISODE_SHOW_DOWNLOADED 0x8`, `EPISODE_SHOW_NOT_DOWNLOADED 0x10`, mask `0x18`; `EPISODE_SHOW_BOOKMARKED 0x20`, `EPISODE_SHOW_NOT_BOOKMARKED 0x40`, mask `0x60`; `EPISODE_SHOW_FILLERMARKED 0x80`, `EPISODE_SHOW_NOT_FILLERMARKED 0x100`, mask `0x180`; `EPISODE_SORTING_SOURCE 0x0`, `_NUMBER 0x200`, `_UPLOAD_DATE 0x400`, `_ALPHABET 0x600`, mask `0x600`; `EPISODE_SHOW_PREVIEWS 0x0`, `EPISODE_SHOW_NOT_PREVIEWS 0x800`, mask `0x800`; `EPISODE_SHOW_SUMMARIES 0x0`, `EPISODE_SHOW_NOT_SUMMARIES 0x1000`, mask `0x1000`; `EPISODE_DISPLAY_NAME 0x0`, `EPISODE_DISPLAY_NUMBER 0x100000`, mask `0x100000`.
Defaults of the default-flag preferences (`LibraryPreferences.kt:172-224`): filters `SHOW_ALL` (0); sort-by `*_SORTING_SOURCE` (0); display `*_DISPLAY_NAME` (0); direction `*_SORT_DESC` (0).
Important: the migration only widens the Int to Long; it never interprets the bits.

## 4. Library sort values (steps 44, 64, 81, 82)

Stored form: `"<TYPE>,<DIRECTION>"`, `DIRECTION` is `ASCENDING` or `DESCENDING`; default `ALPHABETICAL,ASCENDING` (`MangaLibrarySort.default`, `AnimeLibrarySort.default`).
Manga `TYPE` names with their category-flag bits (`MangaLibrarySortMode.kt`; `Type.mask = 0b00111100`, `Direction.mask = 0b01000000`, `Ascending = 0b01000000`, `Descending = 0b00000000`): `ALPHABETICAL 0b000000`, `LAST_READ 0b000100`, `LAST_MANGA_UPDATE 0b001000`, `UNREAD_COUNT 0b001100`, `TOTAL_CHAPTERS 0b010000`, `LATEST_CHAPTER 0b010100`, `CHAPTER_FETCH_DATE 0b011000`, `DATE_ADDED 0b011100`, `TRACKER_MEAN 0b0100000`, `RANDOM 0b111100`.
Anime `TYPE` names (`AnimeLibrarySortMode.kt`): `ALPHABETICAL 0b000000`, `LAST_SEEN 0b000100`, `LAST_ANIME_UPDATE 0b001000`, `UNSEEN_COUNT 0b001100`, `TOTAL_EPISODES 0b010000`, `LATEST_EPISODE 0b010100`, `EPISODE_FETCH_DATE 0b011000`, `DATE_ADDED 0b011100`, `TRACKER_MEAN 0b0100000`, `AIRING_TIME 0b110000`, `RANDOM 0b111100`.
Decimal values of the type bits: `ALPHABETICAL 0`, `LAST_READ`/`LAST_SEEN 4`, `LAST_*_UPDATE 8`, `UNREAD_COUNT`/`UNSEEN_COUNT 12`, `TOTAL_* 16`, `LATEST_* 20`, `*_FETCH_DATE 24`, `DATE_ADDED 28`, `TRACKER_MEAN 32` (source literal `0b000100000`), `AIRING_TIME 48` (anime only), `RANDOM 60`; direction bit `ASCENDING 64`, `DESCENDING 0`. Category flags combine `type | direction`. These matter to area:07-domain-entries-items and area:08-domain-library-category-history-updates, not to the migrations; they are listed so the normaliser's valid-name set is exact.
Legacy Int table: see `p1-topic-legacy-normaliser.md` section 2 rule A1.

## 5. Enums with stored values referenced by migrations

| Enum | Stored form | Values (declaration order) | Source |
|---|---|---|---|
| `TriState` | name | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` | `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt` |
| `ReaderOrientation` (Int flag, not name) | `flagValue` | `DEFAULT 0x00`, `FREE 0x08`, `PORTRAIT 0x10`, `LANDSCAPE 0x18`, `LOCKED_PORTRAIT 0x20`, `LOCKED_LANDSCAPE 0x28`, `REVERSE_PORTRAIT 0x30`; `MASK = 0x38` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt` |
| `ReadingMode` (Int flag) | `flagValue` | `DEFAULT 0`, `LEFT_TO_RIGHT 1`, `RIGHT_TO_LEFT 2`, `VERTICAL 3`, `WEBTOON 4`, `CONTINUOUS_VERTICAL 5`; `MASK = 0x7` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt` |
| reader tap zones (Int index) | index | 0 default, 1 `l_nav`, 2 `kindlish_nav`, 3 `edge_nav`, 4 `right_and_left_nav`, 5 `disabled_nav` | `ReaderPreferences.kt:211-218` |
| `PlayerOrientation` | name | `Free`, `Video`, `Portrait`, `ReversePortrait`, `SensorPortrait`, `Landscape`, `ReverseLandscape`, `SensorLandscape` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:41-50` |
| `VideoAspect` | name | `Crop`, `Fit`, `Stretch` | `PlayerEnums.kt:52-56` |
| `Debanding` | name | `None`, `CPU`, `GPU` | `PlayerEnums.kt:93-97` |
| `SecureScreenMode` | name | `ALWAYS`, `INCOGNITO`, `NEVER` | `SecurityPreferences.kt:28-32` |
| `ExtensionInstaller` | name | `LEGACY`, `PACKAGEINSTALLER`, `SHIZUKU`, `PRIVATE` | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31-36` |
| `StartScreen` | name | `ANIME`, `MANGA`, `UPDATES`, `HISTORY`, `BROWSE` | `app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt` |
| `NavStyle` | name | `MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE` | `app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt` |
| `ThemeMode` | name | `LIGHT`, `DARK`, `SYSTEM` | `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt` |
| DoH provider ids (Int) | id | `CLOUDFLARE 1`, `GOOGLE 2`, `ADGUARD 3`, `QUAD9 4`, `ALIDNS 5`, `DNSPOD 6`, `360 7`, `QUAD101 8`, `MULLVAD 9`, `CONTROLD 10`, `NJALLA 11`, `SHECAN 12`, `LIBREDNS 13`; off = -1 | `core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt:12-24` |
| `LibraryPreferences` string constants | string | `wifi`, `network_not_metered`, `ac` (device); `manga_ongoing`, `manga_fully_read`, `manga_started`, `manga_outside_release_period` (items) | `LibraryPreferences.kt:399-406` |

## 6. Databases touched (step 7 only)

Manga database, table `extension_repos` (`data/src/main/sqldelight/data/extension_repos.sq`): `base_url TEXT NOT NULL PRIMARY KEY`, `name TEXT NOT NULL`, `short_name TEXT`, `website TEXT NOT NULL`, `signing_key_fingerprint TEXT UNIQUE NOT NULL`; query used: `upsert` (`INSERT ... ON CONFLICT(base_url) DO UPDATE SET name, short_name, website, signing_key_fingerprint`).
Anime database, table `extension_store` (`data/src/main/sqldelightanime/dataanime/extension_store.sq`): `index_url TEXT NOT NULL PRIMARY KEY`, `name TEXT NOT NULL`, `badge_label TEXT NOT NULL`, `signing_key TEXT NOT NULL`, `contact_website TEXT NOT NULL`, `contact_discord TEXT`, `is_legacy INTEGER AS Boolean NOT NULL`, `extension_list_url TEXT`; query `upsert` with `ON CONFLICT(index_url) DO UPDATE`.
Step 129 reads `categories` through `GetMangaCategories.await()` and `GetAnimeCategories.await()` (`SELECT ... FROM categories ORDER BY sort`, including the system row `_id = 0` inserted by `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)`, `categories.sq:10`).
Neither migration run executes DDL; schema upgrades are SQLDelight migrations (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations).

## 7. Files and folders touched

| Path (Android) | Step | Operation |
|---|---|---|
| `<cacheDir>/chapter_disk_cache` | 15 | recursive delete |
| `<externalCacheDir>/cover_disk_cache` | 19 | each file renamed into `<externalFilesDir>/covers/` |
| `<externalCacheDir>/chapter_disk_cache` | 26 | recursive delete |
| `<cacheDir>` (entire directory) | 113 | recursive delete |
| `shared_prefs/tracking_queue.xml` | 93 | entries rewritten |
| default `shared_prefs` file | most | key rewrites |
| WorkManager database | ALWAYS, 96, 97 | enqueue/cancel unique work |

Proposal: nothing on disk is touched by the Rust migration list at `VERSION_CODE = 1`. Files: `state.toml` (key `last_version_code`) and `config.toml` in `Paths::config()` (area:02-preferences-system). Cache deletion, when a future migration needs it, goes through `Paths::cache()` only.

## 8. Network endpoints

None in the scope. (`TrustExtensionRepositoryMigration` does not fetch anything: `insertFromPreference` writes placeholder rows and the repos are refreshed later by `AnimeExtensionStoreRepositoryImpl.refreshAll()`, area:19-extensions-anime.)

## 9. Scheduler constants

`BackupCreator` / `LibraryUpdate-auto` / `AnimeLibraryUpdate-auto`: see `p1-topic-scheduler-setup.md` section 2. Tag and name strings: `TAG_AUTO = "BackupCreator"`, `TAG_MANUAL = "BackupCreator:manual"`, `IS_AUTO_BACKUP_KEY = "is_auto_backup"`, `LOCATION_URI_KEY = "location_uri"`, `OPTIONS_KEY` (`BackupCreateJob.kt:131-135`); library `TAG = "LibraryUpdate"`, `WORK_NAME_AUTO = "LibraryUpdate-auto"`, `WORK_NAME_MANUAL = "LibraryUpdate-manual"` (`MangaLibraryUpdateJob.kt:423-425`); anime `TAG = "AnimeLibraryUpdate"`, `"AnimeLibraryUpdate-auto"`, `"AnimeLibraryUpdate-manual"` (`AnimeLibraryUpdateJob.kt:435-437`); cancelled legacy tags `"UpdateChecker"`, `"ExtensionUpdate"` (step 97).
