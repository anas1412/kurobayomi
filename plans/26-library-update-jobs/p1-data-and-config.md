# Area 26 - Library update jobs: data models, preferences, files, endpoints, DB

Source root: `/home/blackbox/aniyomi-src`. Each field lists type, nullability and default/notes. "DB column" is the SQLDelight column; the Kotlin field is the domain name.

---

## 1. Domain models read by the jobs

### 1.1 LibraryManga (`domain/src/main/java/tachiyomi/domain/library/manga/LibraryManga.kt`)

| Field | Type | Null | Source | Notes |
|---|---|---|---|---|
| `manga` | `Manga` | no | `M.*` of `mangas` | full manga row |
| `category` | `Long` | no | `coalesce(MC.category_id, 0)` | `0` = no category; one row per membership |
| `totalChapters` | `Long` | no | `coalesce(C.total, 0)` | chapters excluding excluded scanlators |
| `readCount` | `Long` | no | `coalesce(C.readCount, 0)` | `sum(read)` |
| `bookmarkCount` | `Long` | no | `coalesce(C.bookmarkCount, 0)` | |
| `latestUpload` | `Long` | no | `coalesce(C.latestUpload, 0)` | ms |
| `chapterFetchedAt` | `Long` | no | `coalesce(C.fetchedAt, 0)` | ms |
| `lastRead` | `Long` | no | `coalesce(C.lastRead, 0)` | ms |
| derived `id` | `Long` | | `manga.id` | |
| derived `unreadCount` | `Long` | | `totalChapters - readCount` | |
| derived `hasBookmarks` | `Boolean` | | `bookmarkCount > 0` | |
| derived `hasStarted` | `Boolean` | | `readCount > 0` | |

SQL source: `data/src/main/sqldelight/view/libraryView.sq`. Base filter `WHERE M.favorite = 1`. Chapter aggregation `GROUP BY chapters.manga_id` with `LEFT JOIN excluded_scanlators ... WHERE excluded_scanlators.scanlator IS NULL` and `LEFT JOIN history ON chapters._id = history.chapter_id`.

### 1.2 LibraryAnime (`domain/src/main/java/tachiyomi/domain/library/anime/LibraryAnime.kt`)

| Field | Type | Notes |
|---|---|---|
| `anime` | `Anime` | |
| `category` | `Long` | `coalesce(MC.category_id, 0)` from `animes_categories`; `-1L` for season children (`SeasonAnime.toLibraryAnime`) |
| `totalCount` | `Long` | depends on `fetch_type` (see 1.6) |
| `seenCount` | `Long` | |
| `bookmarkCount` | `Long` | |
| `fillermarkCount` | `Long` | |
| `latestUpload` | `Long` | |
| `episodeFetchedAt` | `Long` | |
| `lastSeen` | `Long` | |
| derived `unseenCount` | `Long` | `totalCount - seenCount` |
| derived `hasStarted` | `Boolean` | `seenCount > 0` |

SQL: `data/src/main/sqldelightanime/view/animelibView.sq`.

### 1.3 SeasonAnime (`domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt`)

`anime`, `totalCount`, `seenCount`, `bookmarkCount`, `fillermarkCount`, `latestUpload`, `fetchedAt`, `lastSeen`. Derived: `seen = totalCount == seenCount`, `unseenCount`, `hasStarted = seenCount > 0`, `hasBookmarks`, `hasFillermarks`. `toLibraryAnime()` maps to `LibraryAnime(category = -1L, episodeFetchedAt = fetchedAt, ...)`.

### 1.4 Manga (relevant fields) (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`)

| Field | Kotlin type | DB column | Default for new | Notes |
|---|---|---|---|---|
| `id` | Long | `_id` | | PK |
| `source` | Long | `source` | | source id |
| `favorite` | Boolean | `favorite` | | library membership, `favorite_modified_at` trigger |
| `lastUpdate` | Long | `last_update` | 0 | ms; time chapter list last changed |
| `nextUpdate` | Long | `next_update` | 0 (`Manga.kt:112`) | ms epoch of next scheduled check |
| `fetchInterval` | Int | `calculate_interval` (INTEGER DEFAULT 0 NOT NULL) | 0 (`Manga.kt:113`) | `>0` computed days; `<0` manual fixed override (abs); `0` not computed |
| `updateStrategy` | `UpdateStrategy` | `update_strategy` INTEGER DEFAULT 0 | `ALWAYS_UPDATE` (`Manga.kt:124`) | enum section 2.1 |
| `status` | Long | `status` INTEGER NOT NULL | | `SManga` status; `COMPLETED = 2` |
| `title` | String | `title` | | |
| `thumbnailUrl` | String? | `thumbnail_url` | | |
| `coverLastModified` | Long | `cover_last_modified` | | ms |
| `initialized` | Boolean | `initialized` | | |
| `chapterFlags` | Long | `chapter_flags` | | bitfield, not read here |
| `dateAdded` | Long | `date_added` | | ms; 0 when un-favourited |

`expectedNextUpdate: Instant?` = `nextUpdate` (ms) unless `status == COMPLETED` (`Manga.kt:37-40`).

### 1.5 Chapter (relevant fields) (`domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt`)

| Field | Type | DB column | Notes |
|---|---|---|---|
| `id` | Long | `_id` | |
| `mangaId` | Long | `manga_id` | |
| `url` | String | `url` | identity key for sync |
| `name` | String | `name` | sanitised by `ChapterSanitizer.sanitize(title)` |
| `scanlator` | String? | `scanlator` | excluded via `excluded_scanlators` |
| `read` | Boolean | `read` | |
| `bookmark` | Boolean | `bookmark` | |
| `chapterNumber` | Float / Double | `chapter_number REAL NOT NULL` | `-1` = unrecognised; `isRecognizedNumber = chapterNumber >= 0f` (`Chapter.kt:19-20`) |
| `sourceOrder` | Long | `source_order INTEGER NOT NULL` | 0 = newest on source |
| `dateFetch` | Long | `date_fetch INTEGER NOT NULL` | ms; new rows get `now + N` (see behaviour 5.4) |
| `dateUpload` | Long | `date_upload INTEGER NOT NULL` | ms; 0 = unknown, replaced by `maxSeenUploadDate` or `now` |
| `lastPageRead` | Long | `last_page_read` | not used here |
| `lastModifiedAt`, `version` | Long | | sync bookkeeping |

### 1.6 Episode (relevant fields) (`domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt`)

As chapter, with: `animeId` (`anime_id`), `seen` (`seen`), `bookmark`, `fillermark` (`fillermark`), `episodeNumber` (`episode_number REAL NOT NULL`; `isRecognizedNumber = episodeNumber >= 0f`), `sourceOrder`, `dateFetch`, `dateUpload`, `summary` (`summary`), `previewUrl` (`preview_url`), `memo` (`memo`, JsonObject), `lastSecondSeen`, `totalSeconds`.

### 1.7 Anime (relevant fields) (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`)

| Field | Kotlin type | DB column | Default | Notes |
|---|---|---|---|---|
| `id`, `source`, `favorite` | Long, Long, Boolean | `_id`, `source`, `favorite` | | |
| `lastUpdate` | Long | `last_update` | | ms |
| `nextUpdate` | Long | `next_update` | | ms |
| `fetchInterval` | Int | `calculate_interval` INTEGER DEFAULT 0 NOT NULL | 0 | same semantics as manga |
| `updateStrategy` | `AnimeUpdateStrategy` | `update_strategy` INTEGER AS AnimeUpdateStrategy NOT NULL DEFAULT 0 | `ALWAYS_UPDATE` | |
| `status` | Long | `status` | | `SAnime.COMPLETED = 2` |
| `fetchType` | `FetchType` | `fetch_type` INTEGER AS FetchType NOT NULL DEFAULT 1 | `Episodes` (1) | 0 = Seasons, 1 = Episodes (see 2.2) |
| `parentId` | Long? | `parent_id` | null | non-null for season children |
| `seasonNumber` | Double | `season_number` | | |
| `seasonSourceOrder` | Long | `season_source_order` NOT NULL | | |
| `seasonFlags` | Long | `season_flags` | | |
| `title`, `thumbnailUrl`, `backgroundUrl`, `coverLastModified`, `backgroundLastModified` | | | | |
| `memo` | JsonObject | `memo` | | |

Indexes (`data/src/main/sqldelightanime/dataanime/animes.sq:43-46`): `animelib_favorite_index ON animes(favorite) WHERE favorite = 1`, `animes_parent_id ON animes(parent_id)`, `animes_fetch_type ON animes(fetch_type)`.

### 1.8 Season rows

Season children are rows in `animes` with `parent_id` set and `fetch_type` `Episodes` (typically) and `favorite = 0`. View `animeseasonsView` (`data/src/main/sqldelightanime/view/animeseasonsView.sq`) selects `M.* WHERE M.parent_id IS NOT NULL` and computes `totalCount`, `seenCount`, `latestUpload`, `episodeFetchedAt`, `lastSeen`, `bookmarkCount`, `fillermarkCount`. Query `getAnimeSeasonsById: SELECT * FROM animeseasonsView WHERE parent_id = :parentId`.

### 1.9 Notification-side models

- `Manga` / `Anime` (title, id, source) and `Chapter[]` / `Episode[]` arrays are passed by value to the notifier. `Array<Chapter>` in `Pair<Manga, Array<Chapter>>` (`CopyOnWriteArrayList<Pair<Manga, Array<Chapter>>>`).
- Failure list: `CopyOnWriteArrayList<Pair<Manga, String?>>` (message nullable).

### 1.10 Update result models

- `RemoteAnimeEpisodeUpdate(anime: Anime, newEpisodes: List<Episode>)` (`mihon.domain.source.models`; not read, inferred from `UpdateAnimeFromRemote.kt:82`).
- `RemoteAnimeSeasonUpdate(anime, newSeasons)` (same).
- `MangaUpdate(id, nextUpdate, fetchInterval)`, `MangaUpdate(id, lastUpdate)`, `AnimeUpdate(id, nextUpdate, fetchInterval)`, `AnimeUpdate(id, title, coverLastModified, backgroundLastModified, author, artist, description, genre, thumbnailUrl, backgroundUrl, status, updateStrategy, initialized, memo)`, `MangaUpdate(id, title, coverLastModified, author, artist, description, genre, thumbnailUrl, status, updateStrategy, initialized)` (`UpdateManga.kt:63-75`).

---

## 2. Enums and stored values

### 2.1 UpdateStrategy / AnimeUpdateStrategy

| Kotlin | Values | Stored (`update_strategy`) | Source |
|---|---|---|---|
| `eu.kanade.tachiyomi.source.model.UpdateStrategy` | `ALWAYS_UPDATE`, `ONLY_FETCH_ONCE` | 0 = ALWAYS_UPDATE (DB default), 1 = ONLY_FETCH_ONCE (ordinal; mapper not read, assumed ordinal, verify in T-26a-004) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt:53-65` |
| `eu.kanade.tachiyomi.animesource.model.AnimeUpdateStrategy` | `ALWAYS_UPDATE`, `ONLY_FETCH_ONCE` | same | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt:10-23` |

Doc semantics: `ALWAYS_UPDATE` = included unless excluded by restrictions; `ONLY_FETCH_ONCE` = automatically skipped during library updates.

### 2.2 FetchType (anime)

`eu.kanade.tachiyomi.animesource.model.FetchType`: `Seasons` (calls `getSeasonList`), `Episodes` (calls `getEpisodeList`). Stored as `fetch_type` INTEGER: `0 = Seasons`, `1 = Episodes` (proven by the CASE branches `WHEN 1 THEN ES.* / WHEN 0 THEN ASS.*` in `animelibView.sq:4-31`). Ordinal mapping consistent with declaration order (`Seasons` first). Fetch type is fixed after initialisation (doc comment).

### 2.3 Status

`SManga.COMPLETED = 2` (`source-api/.../source/model/SManga.kt:50`), `SAnime.COMPLETED = 2` (`source-api/.../animesource/model/SAnime.kt:74`). Other status constants are not used by this area.

### 2.4 Device restriction tokens (LibraryPreferences companion, `LibraryPreferences.kt:398-406`)

| Constant | Literal |
|---|---|
| `DEVICE_ONLY_ON_WIFI` | `"wifi"` |
| `DEVICE_NETWORK_NOT_METERED` | `"network_not_metered"` |
| `DEVICE_CHARGING` | `"ac"` |

### 2.5 Entry restriction tokens (`LibraryPreferences.kt:403-406`)

| Constant | Literal | Meaning |
|---|---|---|
| `ENTRY_NON_COMPLETED` | `"manga_ongoing"` | skip completed (despite the name, this is "skip COMPLETED") |
| `ENTRY_HAS_UNVIEWED` | `"manga_fully_read"` | skip entries with unread/unseen items |
| `ENTRY_NON_VIEWED` | `"manga_started"` | skip entries not started (only when `totalChapters > 0`) |
| `ENTRY_OUTSIDE_RELEASE_PERIOD` | `"manga_outside_release_period"` | skip if `nextUpdate` beyond window upper bound |

Anime uses the same literals (`AnimeLibraryUpdateJob` imports the same constants).

### 2.6 Mark-duplicate tokens (`LibraryPreferences.kt:408-411`)

`MARK_DUPLICATE_CHAPTER_READ_NEW = "new"`, `MARK_DUPLICATE_CHAPTER_READ_EXISTING = "existing"`, `MARK_DUPLICATE_EPISODE_SEEN_NEW = "new_episode"`, `MARK_DUPLICATE_EPISODE_SEEN_EXISTING = "existing_episode"`.

### 2.7 DataSaver enum (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:118-123`)

`NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` (stored by enum name via `getEnum`).

### 2.8 Other enums on the pass path

- `WorkInfo.State.RUNNING` (AndroidX), used by `isRunning` / `stop`.
- `ExistingWorkPolicy.KEEP`, `ExistingPeriodicWorkPolicy.UPDATE`, `BackoffPolicy.LINEAR`, `NetworkType.UNMETERED | CONNECTED`, `ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC` (Android-only; mapped in section 6).
- `NetworkCapabilities.TRANSPORT_WIFI`, `NET_CAPABILITY_NOT_METERED` (Android-only).

---

## 3. Preferences used by this area (keys, types, defaults)

Keys are from `LibraryPreferences.kt` unless noted. "app state" = `Preference.appStateKey(...)` (not user-visible).

| Key | Type | Default | Read/written by | Meaning |
|---|---|---|---|---|
| `pref_library_update_interval_key` | Int | `0` | `setupTask` | hours between automatic passes; 0 = off |
| `library_update_restriction` | StringSet | `{"wifi"}` | `setupTask`, `doWork` (manga) | device restrictions (2.4) |
| `library_update_manga_restriction` | StringSet | `{"manga_fully_read","manga_ongoing","manga_started","manga_outside_release_period"}` | queue filter (both kinds; name says manga but used by anime too) | entry restrictions (2.5) |
| `library_update_categories` | StringSet | `{}` | manga queue | include categories (ids as strings) |
| `library_update_categories_exclude` | StringSet | `{}` | manga queue | exclude categories |
| `animelib_update_categories` | StringSet | `{}` | anime queue | include |
| `animelib_update_categories_exclude` | StringSet | `{}` | anime queue | exclude |
| `auto_update_metadata` | Boolean | `false` | manga/anime pass | also refresh details during automatic pass |
| `pref_update_season_on_library_update` | Boolean | `false` | anime queue | include season children |
| `library_update_last_timestamp` (app state, `Preference.appStateKey("library_update_last_timestamp")`) | Long | `0` | pass start | last pass start in ms |
| `library_unread_updates_count` | Int | `0` | pass | new manga item count (badge) |
| `library_unseen_updates_count` | Int | `0` | pass | new anime item count (badge) |
| `mark_duplicate_read_chapter_read` | StringSet | `{}` | chapter sync | contains `"new"` to auto-mark duplicates read |
| `mark_duplicate_seen_episode_seen` | StringSet | `{}` | episode sync | contains `"new_episode"` |
| `download_new` | Boolean | `false` | FilterChaptersForDownload | auto-download new chapters (DownloadPreferences) |
| `download_new_unread_chapters_only` | Boolean | `false` | FilterChaptersForDownload | skip chapters whose number was read |
| `download_new_episode` | Boolean | `false` | FilterEpisodesForDownload | auto-download new episodes |
| `download_new_unread_episodes_only` | Boolean | `false` | FilterEpisodesForDownload | |
| `download_new_chapter_categories` / `..._exclude` | StringSet | `{}` (not read; assumed) | FilterChaptersForDownload | DownloadPreferences `downloadNewChapterCategories()` |
| `download_new_episode_categories` / `..._exclude` | StringSet | `{}` (not read; assumed) | FilterEpisodesForDownload | |
| `data_saver` | enum name | `NONE` | DataSaver factory | proxy mode |
| `data_saver_excluded` | StringSet | `{}` | DataSaver factory | excluded source ids (as strings) |
| `data_saver_server` | String | `""` | BandwidthHero | proxy base URL (trailing `/` trimmed) |
| `ignore_jpeg` | Boolean | `false` | all proxies | skip jpg/jpeg |
| `ignore_gif` | Boolean | `true` | all proxies | skip gif |
| `data_saver_image_quality` | Int | `80` | all proxies | quality |
| `data_saver_image_format_jpeg` | Boolean | `false` | BandwidthHero (`"1"/"0"`) and wsrv.nl | convert to jpg |
| `data_saver_color_bw` | Boolean | `false` | BandwidthHero | grayscale |
| `data_saver_downloader` | Boolean | `true` | not verified | per the key name, gates the data saver for downloads; the read site was not verified in this area |
| `auto_clear_chapter_cache` | Boolean | `false` | not used here | |

Keys read but owned elsewhere: `display_*` not relevant here.

Restore path: backups carry preferences; `PreferenceRestorer.kt:45-46` calls `setupTask` after restore (see area:22-backup-restore).

---

## 4. Files, folders and formats

| Item | Location (Android) | Format | Writer | Notes |
|---|---|---|---|---|
| Update error log | `context.createFileInCacheDir("aniyomi_update_errors.txt")` (cache dir, `eu/kanade/tachiyomi/util/system/ContextExtensions.kt:104`) | UTF-8 text, format in behaviour section 7 | `writeErrorFile` (manga and anime) | overwritten per run; exposed via FileProvider URI (`getUriCompat`) |
| Cover cache | `MangaCoverCache` / `AnimeCoverCache` (external) | JPEG files | `UpdateManga.awaitUpdateFromSource` (delete), metadata job via `prepUpdateCover` | external |
| Background cache | `AnimeBackgroundCache` (external) | | `UpdateAnimeFromRemote` delete | external |
| Download cache | `MangaDownloadCache` / `AnimeDownloadCache` (external) | in-memory index of download folders | read by filters | external |
| WorkManager DB | app-private (WorkManager internal) | SQLite | WorkManager | not user-accessible; replaced by tokio in Rust |

Rust target paths (proposal, see p1-rust-design.md): `$XDG_CACHE_HOME/kurobayomi/update_errors.txt` (log), `$XDG_DATA_HOME/kurobayomi/kurobayomi.sqlite` (state), `$XDG_CONFIG_HOME/kurobayomi/settings.json` (prefs, if not already in area:02-preferences-system).

---

## 5. Network endpoints

### 5.1 Source API (external, invoked by this area; contract owned by area:15-source-api-manga/16/17)

| Call | Used by | Input | Output | Errors |
|---|---|---|---|---|
| `MangaSource.getMangaDetails(SManga)` / `HttpSource` | manga pass (only if `auto_update_metadata`), manga metadata job | manga url, source | `SManga` (title, thumbnail_url, author, artist, description, genre, status, update_strategy, background?) | throws; per-entry catch |
| `MangaSource.getChapterList(SManga)` | manga pass | url | `List<SChapter>` newest first | throws; empty -> `NoChaptersException` in sync |
| `AnimeSource.getAnimeEpisodeUpdate(SAnime, List<SEpisode>, fetchDetails, fetchEpisodes)` | anime pass, anime metadata job | | `RemoteAnimeEpisodeUpdate`-like object with `anime: SAnime`, `episodes: List<SEpisode>` | external, not read |
| `AnimeSource.getSeasonList` (season path) | not called by this area directly (season updates are in other flows) | | | external |

### 5.2 Image proxies (DataSaver)

| Proxy | Method | URL template (exact) | Auth | Response |
|---|---|---|---|---|
| Bandwidth Hero | GET (image fetch) | `"$dataSavedServer/?jpg=$format&l=$quality&bw=$colorBW&url=$escapedUrl"`, `$escapedUrl = URLEncoder.encode(imageUrl, "utf-8")`, `format`/`colorBW` are `"1"`/`"0"`, `quality` Int | none | image bytes |
| wsrv.nl | GET (image fetch) | `"https://wsrv.nl/?url=$imageUrl"` + `"&q=$quality&n=-1"` or `"&output=jpg&q=$quality&n=-1"` or `"&output=jpg&q=$quality"` or `"&output=webp&q=$quality"` (suffix rules in behaviour 10.4) | none | image bytes |
| reSmush.it | GET (metadata) then image | `"http://api.resmush.it/ws.php?img=$imageUrl&qlty=$quality"` | none | body contains `"dest":"<url>",`; extracted with `substringAfter("\"dest\":\"").substringBefore("\",")` |

### 5.3 Help and info URLs

| Name | URL |
|---|---|
| Manga troubleshooting | `https://aniyomi.org/help/guides/troubleshooting` |
| Anime troubleshooting | `https://aniyomi.org/docs/guides/troubleshooting/` |
| Large bulk updates FAQ | `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads` |

No authentication is used by any endpoint in this area.

---

## 6. Android API -> Rust/Linux mapping (data side)

| Android | Rust/Linux replacement |
|---|---|
| WorkManager periodic work with constraints | tokio `interval`-style scheduler persisted in SQLite (`library_jobs` table, see 7.3) |
| `NetworkRequest` / `NET_CAPABILITY_NOT_METERED` / `TRANSPORT_WIFI` | NetworkManager D-Bus `org.freedesktop.NetworkManager` `Connectivity`, `Metered` property (`NMMetered` values: `0` = unknown, `1` = yes, `2` = no, `3` = guess yes, `4` = guess no; treat `0` and `3` as metered when checking "not metered"); Wi-Fi detection via `Devices` with `DeviceType == 2` active connection. Crate: `zbus` |
| `BatteryManager` "charging" constraint | UPower D-Bus `org.freedesktop.UPower` `OnBattery` property (charging = not `OnBattery`), crate `zbus` |
| "battery not low" (always on) | UPower `DisplayDevice.Percentage` >= 15 is an approximation; decision in p1-rust-design (default: not enforced on desktop, documented) |
| `setForeground` / foreground service | not needed; a GTK app keeps running. Optional: `org.freedesktop.portal.Background` request for background running (see p1-rust-design) |
| `NotificationCompat` / `NotificationManager` | freedesktop Notifications (`org.freedesktop.Notifications`), crate `notify-rust`; actions via `notify-rust` action callbacks |
| Cache dir FileProvider URI | `file://` URI from `glib::filename_to_uri` / `gio::File::for_path` |
| `Injekt` singletons | explicit `Arc<AppState>` dependency injection |
| coil image loader (notification icon) | `gdk-pixbuf` load + circle-crop via `cairo`, scaled to 192 px |
| Locale number formatting (`NumberFormat.getPercentInstance`) | manual `format!("{}%", (cur * 100 / total) as floor)` with `RoundingMode.DOWN` semantics (floor) |

---

## 7. Database tables and columns touched

### 7.1 Read

| Table / view | Columns | Operation | Owner |
|---|---|---|---|
| `libraryView` (view over `mangas`, `chapters`, `excluded_scanlators`, `history`, `mangas_categories`) | all | `library` query | area:03-db-manga-schema-migrations / area:05-data-repos-entries-items |
| `animelibView` (view over `animes`, `episodestatsView`, `animehistorystatsView`, `animes_categories`, `animeseasonstatsView`) | all | `animelib` query | area:04-db-anime-schema-migrations / area:05-data-repos-entries-items |
| `animeseasonsView` / `getAnimeSeasonsById` | `parent_id`, `fetch_type`, `favorite`, `season_source_order`, counts | season children | area:04-db-anime-schema-migrations |
| `chapters` | `_id`, `manga_id`, `url`, `name`, `scanlator`, `read`, `bookmark`, `chapter_number`, `source_order`, `date_fetch`, `date_upload` | `getChaptersByMangaId(applyScanlatorFilter)`, `getChapterByMangaId` | area:03-db-manga-schema-migrations |
| `episodes` | equivalent for anime | `getEpisodesByAnimeId`, `getEpisodeByAnimeId` | area:04-db-anime-schema-migrations |
| `excluded_scanlators` | `manga_id`, `scanlator` | `getExcludedScanlators` | area:03-db-manga-schema-migrations |
| `mangas` | `_id`, `favorite`, `next_update`, `last_update`, `calculate_interval`, `update_strategy`, `status`, `title`... | `getManga` | area:03-db-manga-schema-migrations |
| `animes` | equivalent | `getAnime`, `getAnimeById` | area:04-db-anime-schema-migrations |
| `DownloadPreferences` values | | filters | area:09-domain-source-track-download-release-misc |

### 7.2 Written

| Table | Column(s) | Writer (this area) | Statement / path |
|---|---|---|---|
| `mangas` | `next_update`, `calculate_interval` | `UpdateManga.awaitUpdateFetchInterval` -> `MangaRepository.updateManga(MangaUpdate)` | `mangas.sq` update `calculate_interval = coalesce(:calculateInterval, calculate_interval)` (line 176) |
| `mangas` | `last_update` | `awaitUpdateLastUpdate` | |
| `mangas` | `title`, `cover_last_modified`, `author`, `artist`, `description`, `genre`, `thumbnail_url`, `status`, `update_strategy`, `initialized` | `UpdateManga.awaitUpdateFromSource` (metadata) | |
| `chapters` | insert (`addAllChapters`), update (`updateChapter.awaitAll`), delete (`removeChaptersWithIds`) | `SyncChaptersWithSource` | `chapters.sq` line 100 insert, lines 112-115 update fields `chapter_number`, `source_order`, `date_fetch`, `date_upload` |
| `animes` | `next_update`, `calculate_interval`, `last_update`, metadata fields, `title`, `background_url`, `background_last_modified`, `memo` | anime sync | `animes.sq` line 190 `calculate_interval = coalesce(...)` |
| `episodes` | insert, update (`summary`, `memo`, `fillermark`, `preview_url`, ...), delete | `SyncEpisodesWithSource` | `episodes.sq` lines 95-111 |
| app state | `library_update_last_timestamp`, `library_unread_updates_count`, `library_unseen_updates_count` | pass | preference store (not SQL) |

### 7.3 New table (Rust proposal)

```sql
CREATE TABLE library_job_run (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL CHECK (kind IN ('manga_update','anime_update','manga_metadata','anime_metadata')),
  trigger       TEXT NOT NULL CHECK (trigger IN ('auto','manual','category','covers')),
  category_id   INTEGER,              -- NULL = all categories
  started_at    INTEGER NOT NULL,     -- ms
  finished_at   INTEGER,              -- ms, NULL while running
  state         TEXT NOT NULL CHECK (state IN ('running','success','failure','cancelled','skipped_already_running')),
  total         INTEGER NOT NULL DEFAULT 0,
  done          INTEGER NOT NULL DEFAULT 0,
  failed        INTEGER NOT NULL DEFAULT 0,
  new_items     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX library_job_run_kind_state ON library_job_run(kind, state);
```
Purpose: single-writer lock (a row with `state='running'` blocks a second pass of the same kind), progress for the UI, crash recovery (rows left `running` at startup are set to `cancelled`). Proposal only; see p1-rust-design.md.

---

## 8. Application-level state machine (data)

Pass states (Rust proposal, mirrors WorkManager): `ENQUEUED -> RUNNING -> {SUCCEEDED | FAILED | CANCELLED | RETRY_SCHEDULED}`. Stored in `library_job_run.state`. `RETRY_SCHEDULED` is represented by in-memory timers (not persisted).

---

## 9. Constants summary (data-level)

Repeated from behaviour section 14 for convenience:
- Concurrency: 5 source-groups in parallel.
- Interval: `pref_library_update_interval_key` hours; flex 10 min; linear backoff 10 min.
- Fetch interval: min 1, max 28, default 7, window ±1 day, chapter window 3/10 by 8 items, increase-when-over 10.
- Notification: summary chop 45; progress chop 40; list truncation 5; icon 192 px; warning threshold 60; download action limits 15 chapters / 10 episodes; warning timeout 30 000 ms.
- Error file name `aniyomi_update_errors.txt`.
