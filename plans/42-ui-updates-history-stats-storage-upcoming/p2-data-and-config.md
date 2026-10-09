# Area 42 (part 2): data models, preferences, files, endpoints, DB

Repo root: `/home/blackbox/aniyomi-src`. No network endpoints are touched directly by scope files. Downloads and library updates are triggered through other areas (see `p2-behavior.md` section 15).

---

## 1. Domain models (read by scope)

### 1.1 MangaUpdatesWithRelations
File: `domain/src/main/java/tachiyomi/domain/updates/manga/model/MangaUpdatesWithRelations.kt`

| Field | Kotlin type | Nullable | Source column (view `updatesView`) |
|---|---|---|---|
| mangaId | Long | no | `mangas._id AS mangaId` |
| mangaTitle | String | no | `mangas.title AS mangaTitle` |
| chapterId | Long | no | `chapters._id AS chapterId` |
| chapterName | String | no | `chapters.name AS chapterName` |
| scanlator | String | yes | `chapters.scanlator` |
| read | Boolean | no | `chapters.read` |
| bookmark | Boolean | no | `chapters.bookmark` |
| lastPageRead | Long | no | `chapters.last_page_read` |
| sourceId | Long | no | `mangas.source` |
| dateFetch | Long (epoch ms) | no | `chapters.date_fetch AS datefetch` |
| coverData | MangaCover | no | built in the repository (see 1.5) |

Not carried into the model: `chapters.date_upload` (used only in the WHERE clause), `mangas.favorite` (used in the WHERE clause), `mangas.thumbnail_url` and `cover_last_modified` (used to build `coverData`).

Mapper: `MangaUpdatesRepositoryImpl.mapUpdatesWithRelations(mangaId, mangaTitle, chapterId, chapterName, scanlator, read, bookmark, lastPageRead, sourceId, favorite, thumbnailUrl, coverLastModified, dateUpload, dateFetch)` (`data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt`). `dateUpload` is passed to the mapper but dropped.

### 1.2 AnimeUpdatesWithRelations
File: `domain/src/main/java/tachiyomi/domain/updates/anime/model/AnimeUpdatesWithRelations.kt`

| Field | Kotlin type | Nullable | Source column (view `animeupdatesView`) |
|---|---|---|---|
| animeId | Long | no | `animes._id AS animeId` |
| animeTitle | String | no | `animes.title AS animeTitle` |
| episodeId | Long | no | `episodes._id AS episodeId` |
| episodeName | String | no | `episodes.name AS episodeName` |
| scanlator | String | yes | `episodes.scanlator` |
| seen | Boolean | no | `episodes.seen` |
| bookmark | Boolean | no | `episodes.bookmark` |
| fillermark | Boolean | no | `episodes.fillermark` |
| lastSecondSeen | Long | no | `episodes.last_second_seen` (ms as used by `formatProgress`) |
| totalSeconds | Long | no | `episodes.total_seconds AS totalSeconds` (ms as used by `formatProgress`) |
| sourceId | Long | no | `animes.source` |
| dateFetch | Long (epoch ms) | no | `episodes.date_fetch AS datefetch` |
| coverData | AnimeCover | no | built in repository |

Mapper: `AnimeUpdatesRepositoryImpl.mapUpdatesWithRelations(...)` (`data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt`), `coverData = AnimeCover(animeId, sourceId, isAnimeFavorite = favorite, url = thumbnailUrl, lastModified = coverLastModified)`.

### 1.3 MangaUpdatesItem / AnimeUpdatesItem (presentation wrapper, `@Immutable data class`)
- `update: MangaUpdatesWithRelations` (or Anime variant), non-null.
- `downloadStateProvider: () -> MangaDownload.State` (or `AnimeDownload.State`), non-null lambda.
- `downloadProgressProvider: () -> Int`, non-null lambda, Int percent (download progress unit owned by area:23-downloads-manga, area:24-downloads-anime).
- `selected: Boolean = false`.

### 1.4 Screen state and UI models
- `MangaUpdatesScreenModel.State(isLoading: Boolean = true, items: PersistentList<MangaUpdatesItem> = [], dialog: Dialog? = null)`. Derived `selected: List<MangaUpdatesItem>`, `selectionMode: Boolean`.
- `Dialog` sealed interface: `DeleteConfirmation(toDelete: List<MangaUpdatesItem>)` (manga); anime adds `ShowQualities(episodeTitle: String, episodeId: Long, animeId: Long, sourceId: Long)`.
- `Event` sealed interface: `InternalError` (data object), `LibraryUpdateTriggered(started: Boolean)`.
- `MangaUpdatesUiModel` (presentation, `app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdateScreen.kt`): `Header(date: LocalDate)`, `Item(item: MangaUpdatesItem)`. Anime equivalent `AnimeUpdatesUiModel` in `presentation/updates/anime/AnimeUpdateScreen.kt`.
- `UpcomingMangaUIModel` (`mihon/feature/upcoming/manga/UpcomingMangaUIModel.kt`): `Header(date: LocalDate, mangaCount: Int)`, `Item(manga: Manga)`. Anime: `UpcomingAnimeUIModel.Header(date, animeCount)`, `Item(anime: Anime)`.
- `UpcomingMangaScreenModel.State(selectedYearMonth: YearMonth = YearMonth.now(), items: ImmutableList<UpcomingMangaUIModel> = [], events: ImmutableMap<LocalDate, Int> = {}, headerIndexes: ImmutableMap<LocalDate, Int> = {})`. Anime: same with `UpcomingAnimeUIModel`.

### 1.5 Covers
- `MangaCover(mangaId, sourceId, isMangaFavorite, url: String?, lastModified: Long)` and `AnimeCover(animeId, sourceId, isAnimeFavorite, url, lastModified)` are the cover handles passed to `ItemCover`. Cover cache keys and file layout belong to area:34-images-covers-cache.
- Upcoming rows use `upcoming.asMangaCover()` / `upcoming.asAnimeCover()` (extension functions on `Manga` and `Anime`, `domain/.../entries/*/model/`).

### 1.6 Entry models used by Upcoming
- `Manga` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`): fields used here: `id: Long`, `title: String`, `source: Long`, `status: Long`, `nextUpdate` (epoch ms, `Long`), `favorite: Boolean`, `thumbnailUrl`, `coverLastModified`. Derived `expectedNextUpdate: Instant?` (line 37): `nextUpdate.takeIf { status != SManga.COMPLETED.toLong() }?.let { Instant.ofEpochMilli(it) }`.
- `Anime` (`domain/.../entries/anime/model/Anime.kt`): same fields, `expectedNextUpdate` at line 50.

### 1.7 Chapter and episode update payloads (written by the actions)
- `ChapterUpdate(id: Long, read: Boolean? = null, lastPageRead: Long? = null, bookmark: Boolean? = null)` (the scope only passes these four named fields). Used as `ChapterUpdate(read, lastPageRead = if (!read) 0 else null, id)` and `ChapterUpdate(id, bookmark)`. A null field means "do not change" (`coalesce` in the update SQL).
- `EpisodeUpdate(id: Long, seen: Boolean? = null, lastSecondSeen: Long? = null, bookmark: Boolean? = null, fillermark: Boolean? = null)` (the scope only passes these named fields). Used as `EpisodeUpdate(seen, lastSecondSeen = if (!seen) 0 else null, id)`, `EpisodeUpdate(id, bookmark)`, `EpisodeUpdate(id, fillermark)`.
- Repository batch methods: `ChapterRepository.updateAllChapters(list)`, `EpisodeRepository.updateAllEpisodes(list)`, `UpdateChapter.awaitAll(list)`, `UpdateEpisode.awaitAll(list)`. Owned by area:05-data-repos-entries-items, area:07-domain-entries-items.

---

## 2. Enum values

| Enum | Values (numeric) | File |
|---|---|---|
| MangaDownload.State | NOT_DOWNLOADED(0), QUEUE(1), DOWNLOADING(2), DOWNLOADED(3), ERROR(4) | `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt:67` |
| AnimeDownload.State | NOT_DOWNLOADED(0), QUEUE(1), DOWNLOADING(2), DOWNLOADED(3), ERROR(4) | `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt:62` |
| ChapterDownloadAction | START, START_NOW, CANCEL, DELETE (no numeric values) | `app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt:42` |
| EpisodeDownloadAction | START, START_NOW, CANCEL, DELETE, SHOW_QUALITIES | `app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt:42` |
| SManga status | UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54` |
| SAnime status | UNKNOWN 0, ONGOING 1, COMPLETED 2, LICENSED 3, PUBLISHING_FINISHED 4, CANCELLED 5, ON_HIATUS 6, UPCOMING 7 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79` |

Status sets used by Upcoming: manga `{1, 4}` (ONGOING, PUBLISHING_FINISHED). Anime `{1, 4, 7}` (ONGOING, PUBLISHING_FINISHED, UPCOMING). Stored in `mangas.status` / `animes.status` as Long.

---

## 3. Database objects read by scope

### 3.1 View `updatesView` (manga)
File: `data/src/main/sqldelight/view/updatesView.sq`
```
CREATE VIEW updatesView AS
SELECT
    mangas._id AS mangaId,
    mangas.title AS mangaTitle,
    chapters._id AS chapterId,
    chapters.name AS chapterName,
    chapters.scanlator,
    chapters.read,
    chapters.bookmark,
    chapters.last_page_read,
    mangas.source,
    mangas.favorite,
    mangas.thumbnail_url AS thumbnailUrl,
    mangas.cover_last_modified AS coverLastModified,
    chapters.date_upload AS dateUpload,
    chapters.date_fetch AS datefetch
FROM mangas JOIN chapters
ON mangas._id = chapters.manga_id
WHERE favorite = 1
AND date_fetch > date_added
ORDER BY date_fetch DESC;

getRecentUpdates:
SELECT * FROM updatesView WHERE dateUpload > :after LIMIT :limit;

getUpdatesByReadStatus:
SELECT * FROM updatesView WHERE read = :read AND dateUpload > :after LIMIT :limit;
```
Join: `mangas JOIN chapters ON mangas._id = chapters.manga_id`. Filter: `mangas.favorite = 1 AND chapters.date_fetch > mangas.date_added`. Note `date_added` and `date_fetch` are unqualified in the view; they resolve to `mangas.date_added` and `chapters.date_fetch` because `mangas` has `date_added` and only `chapters` has `date_fetch`.

Used by: `MangaUpdatesRepositoryImpl.subscribeAllMangaUpdates(after, limit)` (`getRecentUpdates`), `subscribeWithRead` / `awaitWithRead` (`getUpdatesByReadStatus`). The scope uses only `getRecentUpdates` via `GetMangaUpdates.subscribe(instant)`.

### 3.2 View `animeupdatesView` (anime)
File: `data/src/main/sqldelightanime/view/animeupdatesView.sq`
```
CREATE VIEW animeupdatesView AS
SELECT
    animes._id AS animeId,
    animes.title AS animeTitle,
    episodes._id AS episodeId,
    episodes.name AS episodeName,
    episodes.scanlator,
    episodes.seen,
    episodes.bookmark,
    episodes.fillermark,
    episodes.last_second_seen,
    episodes.total_seconds AS totalSeconds,
    animes.source,
    animes.favorite,
    animes.thumbnail_url AS thumbnailUrl,
    animes.cover_last_modified AS coverLastModified,
    episodes.date_upload AS dateUpload,
    episodes.date_fetch AS datefetch
FROM animes JOIN episodes
ON animes._id = episodes.anime_id
WHERE favorite = 1
AND date_fetch > date_added
ORDER BY date_fetch DESC;

getRecentAnimeUpdates:
SELECT * FROM animeupdatesView WHERE dateUpload > :after LIMIT :limit;

getUpdatesBySeenStatus:
SELECT * FROM animeupdatesView WHERE seen = :seen AND dateUpload > :after LIMIT :limit;
```
Used by `AnimeUpdatesRepositoryImpl.subscribeAllAnimeUpdates` (`getRecentAnimeUpdates(after, limit, mapper)`).

Proposal for the Rust port (same semantics, explicit ordering):
```sql
SELECT mangas._id, mangas.title, chapters._id, chapters.name, chapters.scanlator,
       chapters.read, chapters.bookmark, chapters.last_page_read,
       mangas.source, chapters.date_fetch, mangas.thumbnail_url, mangas.cover_last_modified
FROM mangas JOIN chapters ON mangas._id = chapters.manga_id
WHERE mangas.favorite = 1
  AND chapters.date_fetch > mangas.date_added
  AND chapters.date_upload > ?1          -- after (ms)
ORDER BY chapters.date_fetch DESC
LIMIT ?2;                                  -- limit = 500
```
Anime: same with `animes`/`episodes`, `episodes.anime_id`, `episodes.seen`, `episodes.last_second_seen`, `episodes.total_seconds`, `episodes.fillermark`.

### 3.3 Queries for Upcoming
- `mangas.sq:124` `getUpcomingManga`: `SELECT * FROM mangas WHERE next_update >= :startOfDay AND favorite = 1 AND status IN :statuses ORDER BY next_update ASC;` The `:statuses` set is expanded to an `IN (?, ?, ...)` list by SQLDelight.
- `animes.sq:135` `getUpcomingAnime`: identical shape on `animes`.
- `:startOfDay` = `LocalDate.now().atStartOfDay(ZoneId.systemDefault()).toEpochSecond() * 1000` (milliseconds, from local midnight today).

Proposal (Rust): compute `start_of_day_ms` with chrono `Local::now().date_naive().and_hms_opt(0,0,0)` → `Local.from_local_datetime(...).single()` (or earliest on DST gap) → `timestamp_millis()`. Same semantic; document DST fallback.

### 3.4 Tables touched by writes triggered from scope (no schema change)
- `chapters`: `read`, `last_page_read`, `bookmark` (via `ChapterRepository.updateAllChapters` and `UpdateChapter.awaitAll`).
- `episodes`: `seen`, `last_second_seen`, `bookmark`, `fillermark` (via `EpisodeRepository.updateAllEpisodes` and `UpdateEpisode.awaitAll`).
- Read-only: `mangas`, `chapters`, `animes`, `episodes` (for getManga/getChapter/getAnime/getEpisode and the views).
- Download deletion (`removeAfterMarkedAsRead`) writes files and no DB rows here; the download manager owns that.

### 3.5 Index and performance notes
- `next_update` index: none in scope. Upcoming query scans `mangas`. Proposal: `CREATE INDEX IF NOT EXISTS idx_mangas_next_update ON mangas(favorite, next_update)` only if profiling shows need; ceiling is library size of a few thousand rows.
- `chapters.date_fetch`: no index in scope. Updates view scans the join. Proposal: index `(date_fetch)` on chapters if the list is slow with >50k chapters.

---

## 4. Preferences used by scope

Keys are the exact strings passed to the preference store.

| Preference accessor | Stored key | Type | Default | Scope use | Defined in |
|---|---|---|---|---|---|
| `LibraryPreferences.lastUpdatedTimestamp()` | `Preference.appStateKey("library_update_last_timestamp")` = `__APP_STATE_library_update_last_timestamp` (prefix `__APP_STATE_` from `Preference.kt:43-48`) | Long | `0L` | last updated line | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:39` |
| `LibraryPreferences.newMangaUpdatesCount()` | `library_unread_updates_count` | Int | `0` | reset to 0 on tab enter/leave | `LibraryPreferences.kt:148` |
| `LibraryPreferences.newAnimeUpdatesCount()` | `library_unseen_updates_count` | Int | `0` | reset to 0 on tab enter/leave | `LibraryPreferences.kt:149` |
| `DownloadPreferences.useExternalDownloader()` | `use_external_downloader` | Boolean | `false` | AUSM read once; passed to episode options sheet | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:14` |
| `DownloadPreferences.removeAfterMarkedAsRead()` | `pref_remove_after_marked_as_read_key` | Boolean | `false` | remove downloads when marked read/seen | `DownloadPreferences.kt:30-33` |
| `PlayerPreferences.alwaysUseExternalPlayer()` | `pref_always_use_external_player` | Boolean | `false` | Play externally/internally actions and row tap routing | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt:67-70` |
| `UiPreferences.relativeTime()` | `relative_time_v2` | Boolean | `true` | relative vs absolute date text | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:33` |
| `UiPreferences.dateFormat()` | `app_date_format` | String | `""` (empty = locale short format) | absolute date text | `UiPreferences.kt:35`, mapping at `:44` |

Preferences read by the scope, owned elsewhere:
- Tablet UI mode `TabletUiMode` (`UiPreferences.tabletUiMode()`) affects `prepareTabletUiContext`, not `isTabletUi()` in scope. Documented in area:36-ui-shell-navigation-security-webview.
- Nav style (`bottom_rail_nav_style`, area:36-ui-shell-navigation-security-webview) decides whether `fromMore` is true for these tabs.

### 4.1 Preferences that the Rust port needs new keys for
Proposal (Rust settings store, see area:02-preferences-system keys):
- `updates.window_months` = `3` (Int). Aniyomi hard-codes 3 months (`minusMonths(3)`); keep 3 as default; it is a Proposal to expose it.
- `updates.limit` = `500` (Int, not exposed; hard-coded as in Aniyomi).
- `upcoming.week_start` = `auto` (String: `auto` | `monday` | `sunday`). Proposal (Aniyomi uses locale only).
- `ui.two_panel_min_width` = `720` (Int, px). Proposal for the desktop breakpoint.

---

## 5. Files and folders on disk

Scope files do not write to disk directly. Indirect effects:
- Download deletion removes the downloaded chapter/episode folder via the download manager (area:23-downloads-manga, area:24-downloads-anime). The folder is found by name (`isChapterDownloaded(name, scanlator, mangaTitle, sourceId)`), so the on-disk layout is owned by area:23-downloads-manga, area:24-downloads-anime.
- Library update job writes nothing in scope.
- Cover thumbnails are read via `ItemCover` (area:34-images-covers-cache). Cache file names and paths are owned by area:34-images-covers-cache.

Proposal (Rust): no new files. Updates and Upcoming read the Kurobayomi SQLite database only.

---

## 6. Network endpoints

None are called from scope files. Network is used by the library update job (area:26-library-update-jobs and source APIs in area:15-source-api-manga, area:16-source-api-anime) and by downloads (area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared). In the Rust port, the Updates tab triggers the library update through the existing scheduler API (area:26-library-update-jobs Rust equivalent) and performs no HTTP itself.

External URL opened by the Upcoming help button: `https://aniyomi.org/docs/faq/updates/upcoming` (`core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:5`, constant `URL_HELP_UPCOMING`). Opened with `LocalUriHandler.openUri`. Rust: `gio::AppInfo::launch_default_for_uri` or `gtk::UriLauncher` (GTK 4.10+).

---

## 7. Localisation keys used by scope (MR and AYMR)

Values from `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR). Plurals from `i18n/src/commonMain/moko-resources/base/plurals.xml`.

| Key | Value (English base) | Used in |
|---|---|---|
| MR.strings.label_upcoming | Upcoming | upcoming title |
| MR.strings.upcoming_guide | Upcoming Guide | help button description |
| MR.strings.upcoming_calendar_prev | Previous Month | calendar prev description |
| MR.strings.upcoming_calendar_next | Next Month | calendar next description |
| MR.strings.information_no_recent | No recent updates | empty state |
| MR.strings.updates_last_update_info | Library last updated: %s | last-updated row |
| MR.strings.updates_last_update_info_just_now | Just now | relative time |
| MR.strings.relative_time_span_never | Never | relative time |
| MR.strings.not_applicable | N/A | null date |
| MR.strings.action_view_upcoming | View Upcoming Updates | toolbar |
| MR.strings.action_update_library | Update library | toolbar |
| MR.strings.action_select_all | Select all | selection toolbar |
| MR.strings.action_select_inverse | Select inverse | selection toolbar |
| MR.strings.updating_library | Updating library | snackbar |
| MR.strings.update_already_running | An update is already running | snackbar |
| MR.strings.internal_error | InternalError: Check crash logs for further information | snackbar |
| MR.strings.confirm_delete_chapters | Are you sure you want to delete the selected chapters? | dialog |
| AYMR.strings.confirm_delete_episodes | Are you sure you want to delete the selected episodes? | dialog |
| MR.strings.action_ok | OK | dialog |
| MR.strings.action_cancel | Cancel | dialog |
| MR.strings.unread | Unread | icon description |
| MR.strings.action_filter_bookmarked | Bookmarked | icon description |
| AYMR.strings.action_filter_fillermarked | Fillermarked | icon description |
| MR.strings.chapter_progress | Page: %1$d | manga progress |
| AYMR.strings.episode_progress | %1$s/%2$s | anime progress |
| MR.strings.relative_time_today | Today | relative date |
| MR.plurals.relative_time | one: Yesterday; other: %1$d days ago | relative date |
| MR.plurals.upcoming_relative_time | one: Tomorrow; other: In %1$d days | relative date |
| AYMR.strings.label_updates | Manga (the manga updates tab title in the aniyomi base file) | tab title |
| AYMR.strings.label_anime_updates | Anime | tab title |

Note on `MR.strings.label_updates`: in the scope file `MangaUpdatesTab.kt` the title is `AYMR.strings.label_updates`, and the aniyomi base file gives it the value "Manga". Keep the key, not the English text, as the translation key.

Also referenced (owned elsewhere): `MR.strings.relative_time_minutes` and `relative_time_hours` plurals (LocalDateTime formatter, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime), `AYMR.strings.relative_time_now`, `AYMR.plurals.upcoming_relative_time_hours`, `AYMR.plurals.upcoming_relative_time_minutes`, `MR.strings.action_filter_bookmarked`.

---

## 8. Data flow summary (fields to plumb)

```
Updates feed:  updatesView row -> MangaUpdatesWithRelations -> MangaUpdatesItem (+ download providers, selected)
               -> UiModel (Header(LocalDate of dateFetch) | Item) -> row widget

Upcoming feed: mangas row (favorite=1, status in {1,4}, next_update >= startOfDay ms)
               -> Manga.expectedNextUpdate (Instant) -> local date
               -> UpcomingRow(Header(date, count) | Item(manga)) -> events map, headerIndexes -> calendar dots and day jump
```
