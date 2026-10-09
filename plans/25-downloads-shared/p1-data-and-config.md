# 25-downloads-shared — data models, preferences, storage, DB touch points

All paths repo-relative to `/home/blackbox/aniyomi-src/`.

---

## 1. Preference store contract (OBSERVED)

File: core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt (implementation; `getBoolean` at :39, `getStringSet` at :43) and InMemoryPreferenceStore.kt (:44, :50).

`DownloadPreferences` calls `preferenceStore.getBoolean(key, default)`, `getInt(key, default)`, `getString(key, default)`, `getStringSet(key, default)`. Each returns a `Preference<T>` (core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:9-23) with `key(): String`, `get(): T`, `set(value: T)`, `isSet(): Boolean`, `delete()`, `defaultValue(): T`, `changes(): Flow<T>`, `stateIn(scope): StateFlow<T>`. Helpers in the same file: `Preference<Boolean>.toggle()`, `Preference<Set<T>>.plusAssign/minusAssign`. Key prefixes `__PRIVATE_` and `__APP_STATE_` (Preference.kt:47-48) are reserved; none of the 23 keys in this scope use them. The value lives in SharedPreferences on Android.

Proposal (Rust): a `PrefStore` backed by the SQLite key-value table of area:02-preferences-system. Each key keeps the same name and the same default, so a restored backup (area:22) maps 1:1. Types map as: Boolean → `bool`, Int → `i64` (value stored as integer), String → `String`, StringSet → `BTreeSet<String>` (serialised as JSON array of strings in the DB, sorted).

---

## 2. Model types touched by the scope (OBSERVED)

### 2.1 Chapter (domain)
File: domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt. Fields used here:

| Field | Kotlin type | Notes |
|---|---|---|
| id | Long | |
| mangaId | Long | series key for deletes and "same series" actions |
| read | Boolean | |
| bookmark | Boolean | protected unless `pref_remove_bookmarked` |
| lastPageRead | Long (non-null) | `> 0` means "started" |
| dateUpload | Long | sort key "By upload date" |
| chapterNumber | Double | sort key "By chapter number" |

Other fields (not used in scope): dateFetch, sourceOrder, url, name, scanlator, lastModifiedAt, version, isRecognizedNumber.

### 2.2 ChapterUpdate (domain partial update)
File: domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt. Used fields: `id: Long`, `read: Boolean?`, `lastPageRead: Long?`. Mapper in SetReadStatus: `ChapterUpdate(read = read, lastPageRead = if (!read) 0 else null, id = chapter.id)`. A `null` field means "leave unchanged".

### 2.3 Episode (domain)
File: domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt.

| Field | Kotlin type | Notes |
|---|---|---|
| id | Long | |
| animeId | Long | series key |
| seen | Boolean | |
| bookmark | Boolean | |
| fillermark | Boolean | used by `pref_download_fillermarked` filtering (area:24) |
| lastSecondSeen | Long | `> 0` means "started" |
| dateUpload | Long | |
| episodeNumber | Double | |

### 2.4 EpisodeUpdate
File: domain/src/main/java/tachiyomi/domain/items/episode/model/EpisodeUpdate.kt. Used: `id: Long`, `seen: Boolean?`, `lastSecondSeen: Long?`. Mapper in SetSeenStatus: `EpisodeUpdate(seen = seen, lastSecondSeen = if (!seen) 0 else null, id = episode.id)`.

### 2.5 AnimeDownload (queue item)
File: app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt.
- `data class AnimeDownload(val source: AnimeHttpSource, val anime: Anime, val episode: Episode, val changeDownloader: Boolean = false, var video: Video? = null) : ProgressListener`.
- `enum class State(val value: Int)`: `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)`.

### 2.6 MangaDownload (queue item)
File: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt.
- `data class MangaDownload(val source: HttpSource, val manga: Manga, val chapter: Chapter)`, plus `var pages: List<Page>? = null`.
- `enum class State(val value: Int)`: `NOT_DOWNLOADED(0)`, `QUEUE(1)`, `DOWNLOADING(2)`, `DOWNLOADED(3)`, `ERROR(4)`.

Both enums have identical numeric values. Proposal (Rust): `#[repr(i64)] enum DownloadState { NotDownloaded = 0, Queue = 1, Downloading = 2, Downloaded = 3, Error = 4 }` with `TryFrom<i64>`.

### 2.7 Source grouping key
`groupBy { it.source }` in the queue models: the key is the source object; header uses `entry.key.id` (Long) and `entry.key.name` (String). Proposal: key by `source_id: i64`.

### 2.8 Queue header (UI model)
`AnimeDownloadHeaderItem(sourceId: Long, sourceName: String, count: Int)` and the same for manga (constructor used at AnimeDownloadQueueScreenModel.kt:119). Holds `subItems: MutableList<AnimeDownloadItem>`. `AnimeDownloadItem(download, header)` wraps a download (file: app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadItem.kt, owned by this area's UI).

---

## 3. Preference table (complete, all 23 accessors)

Keys and defaults OBSERVED from DownloadPreferences.kt. "Type" is the Kotlin accessor type. "Read by" from grep (see p1-behavior.md section 7).

| # | Accessor (Kotlin) | Key (exact string) | Type | Default | Read by (file) | Settings UI (area:47) |
|---|---|---|---|---|---|---|
| 1 | downloadOnlyOverWifi() | `pref_download_only_over_wifi_key` | Boolean | `true` | AnimeDownloadJob.kt, MangaDownloadJob.kt | SettingsDownloadScreen.kt |
| 2 | useExternalDownloader() | `use_external_downloader` | Boolean | `false` | AnimeDownloader.kt, AnimeScreenModel.kt, AnimeUpdatesScreenModel.kt | SettingsDownloadScreen.kt |
| 3 | externalDownloaderSelection() | `external_downloader_selection` | String | `""` | AnimeDownloader.kt | SettingsDownloadScreen.kt |
| 4 | saveChaptersAsCBZ() | `save_chapter_as_cbz` | Boolean | `true` | MangaDownloader.kt | SettingsDownloadScreen.kt |
| 5 | splitTallImages() | `split_tall_images` | Boolean | `true` | MangaDownloader.kt | SettingsDownloadScreen.kt |
| 6 | autoDownloadWhileReading() | `auto_download_while_reading` | Int | `0` | ReaderViewModel.kt | SettingsDownloadScreen.kt |
| 7 | autoDownloadWhileWatching() | `auto_download_while_watching` | Int | `0` | PlayerViewModel.kt | SettingsDownloadScreen.kt |
| 8 | removeAfterReadSlots() | `remove_after_read_slots` | Int | `-1` | ReaderViewModel.kt:509, PlayerViewModel.kt:1743, ExternalIntents.kt:508 | SettingsDownloadScreen.kt:139 |
| 9 | removeAfterMarkedAsRead() | `pref_remove_after_marked_as_read_key` | Boolean | `false` | SetReadStatus.kt:49, SetSeenStatus.kt:49, NotificationReceiver.kt | SettingsDownloadScreen.kt |
| 10 | removeBookmarkedChapters() | `pref_remove_bookmarked` | Boolean | `false` | AnimeDownloadManager.kt:422, MangaDownloadManager.kt:424 | SettingsDownloadScreen.kt |
| 11 | downloadFillermarkedItems() | `pref_download_fillermarked` | Boolean | `false` | AnimeDownloadManager.kt | SettingsDownloadScreen.kt |
| 12 | removeExcludeCategories() | `remove_exclude_categories` | StringSet | `emptySet()` | MangaDownloadManager.kt:411, DeleteMangaCategory.kt, CategoryPreferencesCleanupMigration.kt:16 | SettingsDownloadScreen.kt |
| 13 | removeExcludeAnimeCategories() | `remove_exclude_anime_categories` | StringSet | `emptySet()` | AnimeDownloadManager.kt:411, DeleteAnimeCategory.kt, CategoryPreferencesCleanupMigration.kt:16 | SettingsDownloadScreen.kt |
| 14 | downloadNewChapters() | `download_new` | Boolean | `false` | FilterChaptersForDownload.kt | SettingsDownloadScreen.kt |
| 15 | downloadNewEpisodes() | `download_new_episode` | Boolean | `false` | FilterEpisodesForDownload.kt | SettingsDownloadScreen.kt |
| 16 | downloadNewChapterCategories() | `download_new_categories` | StringSet | `emptySet()` | FilterChaptersForDownload.kt, DeleteMangaCategory.kt, CategoryPreferencesCleanupMigration.kt | SettingsDownloadScreen.kt |
| 17 | downloadNewEpisodeCategories() | `download_new_anime_categories` | StringSet | `emptySet()` | FilterEpisodesForDownload.kt, DeleteAnimeCategory.kt, CategoryPreferencesCleanupMigration.kt | SettingsDownloadScreen.kt |
| 18 | downloadNewChapterCategoriesExclude() | `download_new_categories_exclude` | StringSet | `emptySet()` | FilterChaptersForDownload.kt, DeleteMangaCategory.kt, CategoryPreferencesCleanupMigration.kt | SettingsDownloadScreen.kt |
| 19 | downloadNewEpisodeCategoriesExclude() | `download_new_anime_categories_exclude` | StringSet | `emptySet()` | FilterEpisodesForDownload.kt, DeleteAnimeCategory.kt, CategoryPreferencesCleanupMigration.kt | SettingsDownloadScreen.kt |
| 20 | numberOfDownloads() | `download_slots` | Int | `1` | **none found** (SettingsDownloadScreen.kt:102 only) | SettingsDownloadScreen.kt:102 (ListPreference, entries `1..5`) |
| 21 | downloadSpeedLimit() | `download_speed_limit` | Int | `0` | MangaDownloader.kt:525 | SettingsDownloadScreen.kt:62-73 |
| 22 | downloadNewUnreadChaptersOnly() | `download_new_unread_chapters_only` | Boolean | `false` | FilterChaptersForDownload.kt | SettingsDownloadScreen.kt |
| 23 | downloadNewUnseenEpisodesOnly() | `download_new_unread_episodes_only` | Boolean | `false` | FilterEpisodesForDownload.kt | SettingsDownloadScreen.kt |

Source lines in DownloadPreferences.kt: 47-50 (#1), 52 (#2), 54-57 (#3), 59 (#4), 61 (#5), 63 (#6), 64 (#7), 66 (#8), 68-71 (#9), 73 (#10), 75 (#11), 77-80 (#12), 81-84 (#13), 86 (#14), 87 (#15), 89-92 (#16), 93-96 (#17), 98-101 (#18), 102-105 (#19), 107 (#20), 108 (#21), 110 (#22), 111 (#23).

Notes on the table:
- #20 `download_slots` is written by the settings UI with entries 1..5 (`(1..5).associateWith { it.toString() }`, SettingsDownloadScreen.kt:103) but no code reads it in this tree. See edge case E-12.
- #21 is an integer in **KB/s**. The engine converts: `bytesPerSecond(value.toLong() * 1024)` (MangaDownloader.kt:525). A value of `0` is passed through unchanged; the zero-means-unlimited rule is inside `bytesPerSecond` (not in scope, area:23).
- #8 `-1` means disabled (ReaderViewModel.kt:510 `if (removeAfterReadSlots == -1) return`). Any other value `n` deletes the chapter at position `current - n` in the chapter list.
- #10 is inverted from its name: `false` (default) protects bookmarked items. Keep the key.

### 3.1 Key-name rules (OBSERVED)
- Keys are not namespaced consistently: some use `pref_` (`pref_download_only_over_wifi_key`, `pref_remove_after_marked_as_read_key`, `pref_remove_bookmarked`, `pref_download_fillermarked`), some do not. Proposal: keep the exact strings so backups restore; do not rename.
- Anime and manga variants are separate keys for category sets (`download_new_categories` vs `download_new_anime_categories`), and for `remove_exclude_categories` vs `remove_exclude_anime_categories`. The non-anime-prefixed keys belong to manga.
- The chapter/episode auto-download keys `download_new` and `download_new_episode` are the on/off switches; the matching `*_categories` / `*_categories_exclude` are the scope filter.

---

## 4. Category-set keys and the restore list

`DownloadPreferences.categoryPreferenceKeys` (companion, DownloadPreferences.kt:121-128) is a `setOf(...)` of exactly these six strings, in this order:
1. `remove_exclude_categories`
2. `remove_exclude_anime_categories`
3. `download_new_categories`
4. `download_new_anime_categories`
5. `download_new_categories_exclude`
6. `download_new_anime_categories_exclude`

Used in: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt:136 — `LibraryPreferences.categoryPreferenceKeys + DownloadPreferences.categoryPreferenceKeys`. These keys hold category **ids as strings** (each element is parsed with `String::toLong` in the delete filters). Proposal: store as `BTreeSet<i64>` in Rust, serialise as strings in backups for compatibility, and remap ids on restore the same way area:21/22 remaps categories.

---

## 5. Delete-path data (OBSERVED)

### 5.1 Inputs
- Episode list (`vararg episodes: Episode`) and `Anime` (for the anime path); chapter list and `Manga` (manga path).
- `anime.source: Long`, `manga.source: Long` (source id) → `sourceManager.get(id)`.

### 5.2 Category lookup
`getCategories.await(anime.id)` / `getCategories.await(manga.id)` returns `List<Category>`; only `.id` (Long) is used. Empty list → `[0L]`.

### 5.3 File layout touched
- Episode/chapter directories: found by `provider.findEpisodeDirs(episodes, anime, source): Pair<UniFile?, List<UniFile>>` (app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt:101) and `provider.findChapterDirs(chapters, manga, source)` (MangaDownloadProvider.kt:101). Returns `(seriesDir, itemDirs)`. If the series directory is not found it returns `null to emptyList()` (AnimeDownloadProvider.kt:102), so the delete loop is a no-op. Item directory names come from `getValidEpisodeDirNames(episode.name, episode.scanlator)` / `getValidChapterDirNames(chapter.name, chapter.scanlator)` (several candidate names per item, `mapNotNull`). Owned by area:23/24 (layout in p1-rust-design.md section 5 for the shared rule).
- Anime directory: `provider.findAnimeDir(anime.title, source)`; manga: `provider.findMangaDir(manga.title, source)`.
- Source directory: `provider.findSourceDir(source)`.
- Each deletion is `UniFile.delete()`.
- Cache: `cache.removeEpisodes(list, anime)`, `cache.removeChapters(list, manga)`, `cache.removeAnime(anime)`, `cache.removeManga(manga)`, `cache.removeSource(source)` (AnimeDownloadCache / MangaDownloadCache, owned by area:23/24).

### 5.4 Empty-directory cleanup rule (OBSERVED)
After item deletion: `if (dir?.listFiles()?.isEmpty() == true)` → delete the series directory (via `deleteAnime`/`deleteManga` with `removeQueued = false`). Then `sourceDir?.listFiles()?.isEmpty() == true` → delete source directory and `cache.removeSource(source)`.

Note: `listFiles()` returning `null` (not a directory / IO error) → the `== true` check is false → no cascade. Proposal: same.

### 5.5 Database writes in this area
None directly. The delete path writes only files and the in-memory/disk download cache. Seen/read state is written by SetSeenStatus / SetReadStatus through `EpisodeRepository.updateAllEpisodes` / `ChapterRepository.updateAllChapters` (area:05/07) before the delete trigger runs.

The Rust port adds no tables. If the download queue is persisted (owned by area:23/24), it is outside this area.

---

## 6. Filesystem and format facts (this area)

- No file formats are written here. CBZ and split-tall-images are engine behaviour (MangaDownloader, area:23); this area only reads the two flags.
- Directory names come from the provider (area:23/24); this area never builds a path itself.
- Deletion is permanent (no trash). Proposal: keep permanent deletion (same as Aniyomi) and show the confirmation the entry-details screens already show; no trash in Rust.

---

## 7. Network endpoints

None in this scope. Downloads and page fetches are in area:23/24 and area:12.

---

## 8. Android to Linux mapping for the data layer

| Android / Kotlin | Linux / Rust replacement |
|---|---|
| SharedPreferences via `PreferenceStore` | SQLite `preferences` table through area:02's `PrefStore` (rusqlite) |
| `Flow` on a `Preference` | `tokio::sync::watch::Sender<T>` per key, or GLib `notify::` on a settings GObject |
| `UniFile` (SAF) | `std::path::PathBuf` + `std::fs` (see p1-rust-design.md section 5) |
| `kotlinx.coroutines` `launchIO` | `tokio::task::spawn_blocking` for FS work |
| `Injekt.get()` DI | explicit constructor injection (`Arc<DownloadPrefs>`) |
