# Area 10 - behaviour specification (part 1 of 1)

Scope: the app-level domain layer `eu.kanade.domain` under `app/src/main/java/eu/kanade/domain/` (69 files, listed in p1-inventory.md). All paths are repo-relative to `/home/blackbox/aniyomi-src`. "OBSERVED" is what the Kotlin does, "Proposal:" is what the Rust port should do. Other areas are referred to as `area:<id>`.

Everything in this area is GTK-free business logic. It sits between the data layer (area:05, area:06 repositories), the pure domain layer (area:07, area:08, area:09), the source/extension runtime (area:17, area:18, area:19, area:20), trackers (area:27 and following), the download managers (area:23, area:24, area:25) and the UI screens (area:37 to area:42). Its callers are the screen models, the reader/player view models, the library update jobs and the backup restorer.

Deeper dives: p1-topic-chapter-episode-sync.md (the three sync algorithms with worked examples), p1-topic-tracking-orchestration.md (tracking, offline queue, enhanced trackers), p1-topic-source-and-extension-listing.md (source/extension listing, ordering, pins, incognito, trust), p1-topic-domainmodule-registry.md (all 178 + 1 registrations).

## 0. Conventions used by every interactor

OBSERVED:
- `suspend fun await(...)` is a one-shot call, `fun subscribe(...)`: `Flow<T>` is a reactive query that emits the current value first and again on every change of any input (kotlinx `combine` semantics: it emits only after every input flow has emitted at least once, then once per input change).
- `withNonCancellableContext { }` (`core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt:65`) = `withContext(NonCancellable, block)`: the body keeps running to completion even if the calling coroutine (a screen model, the reader activity being closed) is cancelled. It is used by SetReadStatus, SetSeenStatus, DeleteChapterDownload, DeleteEpisodeDownload, AddMangaTracks.bind, AddAnimeTracks.bind, TrackChapter, TrackEpisode.
- `withIOContext { }` = `withContext(Dispatchers.IO)`.
- Logging: `logcat(LogPriority.X, throwable)`; WARN in the sync/track interactors, ERROR in the set-status interactors, INFO for per-tracker failures in TrackChapter/TrackEpisode, DEBUG for queue operations.
- Preferences are read with `pref.get()` (blocking, in-memory) and observed with `pref.changes()` (a `Flow` that emits the current value first). `getAndSet { }` = `pref.set(block(pref.get()))`, not atomic.
- Timestamps are epoch milliseconds (`Instant.now().toEpochMilli()`), except tracker start/finish dates which are epoch milliseconds of a "UTC-shifted local date" (see section 12.5).
- Injekt: constructor arguments are resolved by type; see p1-topic-domainmodule-registry.md.

Proposal:
- `non_cancellable(fut)`: `tokio::spawn(fut).await` and `JoinHandle` is awaited with the result mapped; if the caller future is dropped the spawned task keeps running (this is the default behaviour of a spawned tokio task). Panics inside become `Err(JoinError)`, which the callers convert to the same error variant the Kotlin code returns.
- Reactive inputs: `Pref<T>::changes()` from area:02 plus `Subscription<T>` from area:05 plus a `watch::Receiver` for extension/source lists. `combine_latest` helper is specified in p1-rust-design.md section 5.
- `tracing::{error,warn,info,debug}!` replace the logcat priorities one to one.

## 1. DomainModule and SYDomainModule

Source: `app/src/main/java/eu/kanade/domain/DomainModule.kt`, `app/src/main/java/eu/kanade/domain/SYDomainModule.kt`; imported at `app/src/main/java/eu/kanade/tachiyomi/App.kt:101` and `:103`.

OBSERVED:
- `DomainModule.registerInjectables()` registers 178 items: 22 `addSingletonFactory` (repositories, `ReleaseService`, `AnimeExtensionStoreService`, the two stub-source repositories) and 156 `addFactory` (a new object on every `Injekt.get()`; every factory class is a stateless wrapper holding handles to repositories/preferences, so sharing one instance would be equivalent).
- The order of registration does not matter (lazy resolution by type). `DelayedAnimeTrackingStore`, `DelayedMangaTrackingStore` are registered in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:218-219`, `SourcePreferences` and `TrackPreferences` in `app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt:43,76`, not in DomainModule.
- Notable constructor arities in this scope: `SyncChaptersWithSource` 9, `SyncEpisodesWithSource` 8, `SyncSeasonsWithSource` 5, `UpdateAnimeFromRemote` 7 (area:17), `TrackChapter`/`TrackEpisode` 4, `AddMangaTracks`/`AddAnimeTracks` 4, `RefreshMangaTracks`/`RefreshAnimeTracks` 4, `SetReadStatus`/`SetSeenStatus` 4, `GetMangaIncognitoState`/`GetAnimeIncognitoState` 3, `UpdateManga`/`UpdateAnime` 2.
- `SYDomainModule` registers one factory: `ToggleExcludeFromMangaDataSaver(get())`. The "SY" markers inside `GetEnabledMangaSources` and `SourcePreferences` mark TachiyomiSY-derived features (data saver, per-source exclusion).
- The manga `GetExtensionSources` class is declared in the file `GetMangaExtensionSources.kt` (class name and file name differ).

Proposal: a plain `Domain` struct (p1-rust-design.md section 2) built once at startup by `Domain::new(deps: DomainDeps)`. Each registration in p1-topic-domainmodule-registry.md maps to either a field (singleton) or an accessor method (factory). No reflection, no global service locator (`Injekt.get<X>()` calls inside the Kotlin interactors, e.g. `AddMangaTracks.bind` calling `Injekt.get<GetMangaHistory>()`, become ordinary constructor fields).

## 2. Entry model helpers (Manga.kt, Anime.kt)

Sources: `app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt`, `app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt`.

### 2.1 Filter state with "downloaded only"

OBSERVED (`Manga.downloadedFilter`, `Anime.downloadedFilter`, `Anime.seasonDownloadedFilter`):
1. If `BasePreferences.downloadedOnly()` (key `__APP_STATE_pref_downloaded_only`, default `false`) is true, return `TriState.ENABLED_IS` regardless of the stored flags.
2. Otherwise map the raw masked flag: manga `CHAPTER_SHOW_DOWNLOADED (0x08)` -> `ENABLED_IS`, `CHAPTER_SHOW_NOT_DOWNLOADED (0x10)` -> `ENABLED_NOT`, anything else -> `DISABLED`. Anime `EPISODE_SHOW_DOWNLOADED (0x08)` / `EPISODE_SHOW_NOT_DOWNLOADED (0x10)`; seasons `SEASON_SHOW_DOWNLOADED (0x02)` / `SEASON_SHOW_NOT_DOWNLOADED (0x04)`.
`TriState` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt`): `DISABLED, ENABLED_IS, ENABLED_NOT`; `next()` cycles DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED. `applyFilter(filter, predicate)` (`domain/src/main/java/tachiyomi/domain/entries/TriState.kt`): `DISABLED` -> true, `ENABLED_IS` -> predicate(), `ENABLED_NOT` -> !predicate().

`Manga.chaptersFiltered()` = `unreadFilter != DISABLED || downloadedFilter != DISABLED || bookmarkedFilter != DISABLED` (uses the downloaded-only-aware `downloadedFilter`, so "downloaded only" mode makes the filter badge show as active). `Anime.episodesFiltered()` additionally ORs `fillermarkedFilter`. `Anime.seasonsFiltered()` ORs the six season filters (downloaded, unseen, started, completed, bookmarked, fillermarked).

### 2.2 Source-model mappers

OBSERVED (`Manga.toSManga`): copies `url, title, artist, author, description`, `genre = genre.orEmpty().joinToString()` (Kotlin default separator is `", "`), `status.toInt()`, `thumbnail_url`, `initialized`. It does NOT copy `update_strategy` (the SManga default `ALWAYS_UPDATE` applies) and there is no memo on manga.

OBSERVED (`Anime.toSAnime`): the same plus `background_url`, `fetch_type`, `season_number`, `memo`. Again no `update_strategy`.

OBSERVED (`Manga.copyFrom(other: SManga)`), used by `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt:125` (area:26-library-update-jobs); the backup restorers define their own private `copyFrom` and do not use it:
- `author = other.author ?: author`, same for `artist`, `description`.
- `genres = if (other.genre != null) other.getGenres() else genre`. `SManga.getGenres()` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:29`) returns `null` when `genre` is null or blank, else `genre.split(", ").map(trim).filterNot(blank).distinct()`. So a non-null but blank `other.genre` CLEARS the stored genres to null.
- `thumbnailUrl = other.thumbnail_url ?: thumbnailUrl` (an empty string overwrites).
- `status = other.status.toLong()` always, `updateStrategy = other.update_strategy` always, `initialized = other.initialized && initialized`.
- Anime additionally: `backgroundUrl = other.background_url ?: backgroundUrl`, `fetchType = other.fetch_type`, `seasonNumber = other.season_number`, `memo = other.memo` (always overwritten).

OBSERVED (`SManga.toDomainManga(sourceId)`): `Manga.create().copy(url, title, artist, author, description, genre = getGenres(), status = status.toLong(), thumbnailUrl, updateStrategy, initialized, source = sourceId)`. `Manga.create()` defaults (`domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:105`): `id -1, url "", title "", source -1, favorite false, lastUpdate 0, nextUpdate 0, fetchInterval 0, dateAdded 0, viewerFlags 0, chapterFlags 0, coverLastModified 0, artist/author/description/genre null, status 0, thumbnailUrl null, updateStrategy ALWAYS_UPDATE, initialized false, lastModifiedAt 0, favoriteModifiedAt null, version 0`. `Anime.create()` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:315`): the same plus `episodeFlags 0, backgroundLastModified 0, backgroundUrl null, fetchType Episodes, parentId null, seasonFlags 0, seasonNumber -1.0, seasonSourceOrder 0, memo {}`.

`hasCustomCover` = the custom cover file of the entry exists (`MangaCoverCache.getCustomCoverFile(id).exists()`, area:34); `Anime.hasCustomBackground` the same with the background cache.

### 2.3 getComicInfo (manga)

OBSERVED (`Manga.kt:95`): builds the `ComicInfo` XML model for a downloaded chapter, called from `MangaDownloader.createComicInfoFile` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:666`). Field mapping:
- `Title = chapter.name`; `Series = manga.title`.
- `Number`: `chapter.chapterNumber.takeIf { it >= 0 }` then `if (it % 1 == 0.0) it.toInt().toString() else it.toString()` (so `12.0` -> `"12"`, `12.5` -> `"12.5"`, `-1.0` -> absent).
- `Web = urls.joinToString(" ")` (urls come from the caller: tracker remote URLs, trimmed, non-blank, plus the chapter URL, distinct).
- `Summary = manga.description` (if non-null), `Writer = manga.author`, `Penciller = manga.artist`, `Translator = chapter.scanlator`, `Genre = manga.genre.joinToString()` (", ").
- `PublishingStatusTachiyomi = ComicInfoPublishingStatus.toComicInfoValue(manga.status)`: `1 Ongoing, 2 Completed, 3 Licensed, 4 "Publishing finished", 5 Cancelled, 6 "On hiatus"`, else (0 and everything unknown) `Unknown`.
- `CategoriesTachiyomi = categories.joinToString()` (", ") when the list is non-null; `SourceAniyomi = sourceName`.
- `Inker, Colorist, Letterer, CoverArtist, Tags` are always null. XML namespaces and the serializer are area:14 / area:23.

## 3. UpdateManga / UpdateAnime

Sources: `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt`, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt`.

All writes go through `MangaRepository.updateManga(MangaUpdate)` / `updateAllManga` (area:05). `MangaUpdate` fields that are `null` mean "leave unchanged" because the SQL uses `coalesce(:x, column)` (see `data/src/main/sqldelight/data/chapters.sq` `update:` for the pattern; the manga/anime tables use the same form). Consequence: a nullable text column (author, artist, description, thumbnail_url, scanlator) can never be set back to NULL through these update objects.

| Method | Effect |
|---|---|
| `await(update)` / `awaitAll(updates)` | pass-through, returns the repository Boolean |
| `awaitUpdateFetchInterval(manga, dateTime = now, window = fetchInterval.getWindow(dateTime))` | `repo.updateManga(fetchInterval.toMangaUpdate(manga, dateTime, window))`; sets `next_update` and `fetch_interval` |
| `awaitUpdateLastUpdate(id)` | `last_update = now` |
| `awaitUpdateCoverLastModified(id)` | `cover_last_modified = now` (anime also `awaitUpdateBackgroundLastModified` -> `background_last_modified = now`) |
| `awaitUpdateFavorite(id, favorite)` | `favorite = favorite`, `date_added = now if favorite else 0` |

`awaitUpdateFromSource(localManga, remoteManga, manualFetch, coverCache)` (manga only; the anime counterpart is `UpdateAnimeFromRemote.awaitUpdateFromSource` in `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt:142`, area:17, which additionally handles the background image with identical rules):
1. `remoteTitle = remoteManga.title`; reading an uninitialised `lateinit` title throws `UninitializedPropertyAccessException`, which is caught and treated as `""`.
2. `title = null` (no change) if `remoteTitle.isEmpty() || localManga.favorite`, else `remoteTitle`. So the title of a library entry is never overwritten by a refresh; non-library (browse cache) rows follow the source.
3. `coverLastModified` is chosen by the first matching branch of this ordered `when`:
   1. `remoteManga.thumbnail_url.isNullOrEmpty()` -> `null` (never lose an existing cover because the source returned nothing).
   2. `!manualFetch && localManga.thumbnailUrl == remoteManga.thumbnail_url` -> `null`.
   3. `localManga.isLocal()` (source id `0`, `LocalMangaSource.ID`) -> `now`.
   4. `localManga.hasCustomCover(coverCache)` -> `coverCache.deleteFromCache(localManga, false)` then `null` (the cached remote cover is dropped, the user's custom cover stays, the timestamp is not bumped).
   5. otherwise `coverCache.deleteFromCache(localManga, false)` then `now` (the `false` argument means "do not delete the custom cover").
4. `thumbnailUrl = remoteManga.thumbnail_url?.takeIf { it.isNotEmpty() }`.
5. One `MangaUpdate`: `id, title, coverLastModified, author = remote.author, artist = remote.artist, description = remote.description, genre = remote.getGenres(), thumbnailUrl, status = remote.status.toLong(), updateStrategy = remote.update_strategy, initialized = true`. Null fields do not change the column. Returns the repository result.

Proposal: `UpdateManga::await_update_from_source(local: &Manga, remote: &SManga, manual: bool) -> Result<bool>`, `CoverCache` is a trait owned by area:34 (`delete_from_cache(&Manga, delete_custom: bool)`, `has_custom_cover(id) -> bool`). `SManga.title` is `Option<String>` (None == uninitialised).

## 4. Viewer flags (SetMangaViewerFlags, SetAnimeViewerFlags)

Sources: `app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt`, `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt`. Bit layouts are in p1-data-and-config.md section 3.

OBSERVED common primitive: `Long.setFlag(flag, mask) = (this and mask.inv()) or (flag and mask)` (infix calls are left associative: `this and mask.inv() or (flag and mask)`).

`SetMangaViewerFlags`:
- `awaitSetReadingMode(id, flag)`: `manga = repo.getMangaById(id)` (throws if absent), then `updateManga(MangaUpdate(id, viewerFlags = manga.viewerFlags.setFlag(flag, ReadingMode.MASK = 0x7)))`.
- `awaitSetOrientation(id, flag)`: same with `ReaderOrientation.MASK = 0x38`.
- Callers: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:684` (reading mode, `ReadingMode.flagValue`) and `:723` (`orientation.flagValue.toLong()`).
- `Manga.readingMode` = `viewerFlags and 0x7` and `Manga.readerOrientation` = `viewerFlags and 0x38` (raw Long values; `ReadingMode.fromPreference` / `ReaderOrientation.fromPreference` map them to the enum, unknown -> DEFAULT).

`SetAnimeViewerFlags`:
- `awaitSetSkipIntroLength(id, flag)`: read the anime, then one update with `viewerFlags = anime.viewerFlags.setFlag(flag, ANIME_INTRO_MASK = 0xFF).setFlag(((flag == 0L).toLong()).addHexZeros(14), ANIME_INTRO_DISABLE_MASK = 0x100000000000000)`. `addHexZeros(n) = this * 16^n` computed in `Double` and converted back with `toLong()`; `1 * 16^14 = 2^56 = 0x100000000000000`. So length 0 sets the "disable skip-intro button" bit (comment in source: "Disable skip intro button if length is set to 0"), any other length clears it. The length is stored modulo 256 (`flag and 0xFF`): a value of 300 stores 44. Callers: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2005` and `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:371`. Reading back (`PlayerViewModel.getAnimeSkipIntroLength`, line 1983): disabled bit set -> `0`; stored length `<= 0` -> the preference `gesturePreferences.defaultIntroLength()`; else the stored length.
- `awaitSetNextEpisodeAiring(id, Pair(episode: Int, airingAt: Long))`: two separate read-modify-write updates, in this order:
  1. `awaitSetNextEpisodeToAir(id, episode.toLong().addHexZeros(2))` -> `setFlag(.., ANIME_AIRING_EPISODE_MASK = 0xFFFF00)` (episode shifted left 8 bits, 16-bit field).
  2. `awaitSetNextEpisodeAiringAt(id, airingAt.addHexZeros(6))` -> `setFlag(.., ANIME_AIRING_TIME_MASK = 0x0FFFFFFFF000000)` (airing time shifted left 24 bits, 32-bit field, epoch seconds from AniList `nextAiringEpisode.airingAt`, valid until 2106).
  Reading back: `nextEpisodeToAir = ((viewerFlags and 0xFFFF00) / 16^2).toInt()`, `nextEpisodeAiringAt = (viewerFlags and 0x0FFFFFFFF000000) / 16^6`. Callers: `AnimeScreenModel.kt:1541`, written after `AniChartApi.loadAiringTime` (`app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:22`).
- Concurrency quirk: both interactors are read-modify-write without a transaction; two concurrent flag setters (for example the reader saving the reading mode while the details screen saves nothing else on manga) can lose an update. For anime the intro-length and airing writes are on different screens and in practice do not overlap.

Proposal: implement the flag updates as one atomic SQL statement on the writer connection: `UPDATE mangas SET viewer = (viewer & ~:mask) | (:flag & :mask) WHERE _id = :id` (same for animes, with `:mask` and `:flag` as 64-bit integers), and fold the two airing writes into one statement. This keeps bit-exact results and removes the lost-update race. Column names come from area:03/area:04.

## 5. Excluded scanlators and available scanlators (manga only)

Sources: `app/src/main/java/eu/kanade/domain/entries/manga/interactor/GetExcludedScanlators.kt`, `app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetExcludedScanlators.kt`, `app/src/main/java/eu/kanade/domain/items/chapter/interactor/GetAvailableScanlators.kt`, SQL `data/src/main/sqldelight/data/excluded_scanlators.sq`.

OBSERVED:
- Table `excluded_scanlators(manga_id INTEGER NOT NULL, scanlator TEXT NOT NULL, FOREIGN KEY(manga_id) REFERENCES mangas(_id) ON DELETE CASCADE)` plus index `excluded_scanlators_manga_id_index ON excluded_scanlators(manga_id)`. No UNIQUE constraint.
- `GetExcludedScanlators.await(mangaId)` = `SELECT scanlator FROM excluded_scanlators WHERE manga_id = :mangaId` as a `Set<String>`; `subscribe` is the reactive version (re-emits on any change of the table).
- `SetExcludedScanlators.await(mangaId, excluded: Set<String>)` in ONE transaction: `current = SELECT ...`; `toAdd = excluded - current` -> `INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (:mangaId, :scanlator)` per item; `toRemove = current - excluded` -> single `DELETE FROM excluded_scanlators WHERE manga_id = :mangaId AND scanlator IN :scanlators` (an empty `toRemove` produces `IN ()` which matches nothing and is valid in SQLite).
- `GetAvailableScanlators`: `repository.getScanlatorsByMangaId(mangaId)` (distinct scanlators of the manga's chapters, area:05), then `mapNotNull { it.ifBlank { null } }.toSet()`: blank strings are dropped. `await` and `subscribe` variants.
- Effect of exclusion elsewhere: (a) `SyncChaptersWithSource` filters the returned "new chapters" list (section 6.1 step 17), (b) `GetChaptersByMangaId(applyScanlatorFilter = true)` hides the chapters in the details list and fetch-interval computation (area:07), (c) the backup creator stores the set (`MangaBackupCreator.kt:34`, area:21) and the restorer re-inserts it (`MangaRestorer.kt:426`, area:22).
- UI caller: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt:1133` (dialog `ScanlatorFilterDialog` at `MangaScreen.kt:302`).
- Anime has no equivalent (episodes have a scanlator column but no exclusion).

## 6. Syncing source lists with the database

Full algorithm walk-throughs with worked examples are in p1-topic-chapter-episode-sync.md; this section is the normative step list.

### 6.1 SyncChaptersWithSource.await

Source: `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt`. Signature `await(rawSourceChapters: List<SChapter>, manga: Manga, source: MangaSource, manualFetch: Boolean = false, fetchWindow: Pair<Long, Long> = Pair(0, 0)): List<Chapter>` returning the NEWLY ADDED chapters that should notify/auto-download.

Callers: `MangaScreenModel.kt:575` (details refresh), `MangaLibraryUpdateJob.kt:359` (`manualFetch=false`, with the job's fetchWindow), `MigrateMangaDialog.kt:219` and `DeepLinkMangaScreenModel.kt:69`.

1. If `rawSourceChapters.isEmpty() && !source.isLocal()` throw `NoChaptersException` (user message `MR.strings.no_chapters_error` = "No chapters found", shown via `app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:28`, in the library update error list `MangaLibraryUpdateJob.kt:295`, and `MangaScreenModel.kt:587`). Local sources may legitimately be empty.
2. `now = ZonedDateTime.now()`, `nowMillis = now.toEpochMilli()`.
3. `sourceChapters`: `rawSourceChapters.distinctBy { it.url }` (first occurrence wins) then `mapIndexed { i, sChapter -> Chapter.create().copyFromSChapter(sChapter).copy(name = ChapterSanitizer.sanitize(sChapter.name, manga.title)).copy(mangaId = manga.id, sourceOrder = i) }`. `copyFromSChapter`: `name, url, dateUpload = date_upload, chapterNumber = chapter_number (f32) widened to f64, scanlator = scanlator?.ifBlank{null}?.trim()`. The sanitizer is specified by area:05 (`p1-topic-item-sanitizer.md`).
4. `dbChapters = getChaptersByMangaId.await(manga.id)` (all chapters, scanlator filter NOT applied).
5. `removedChapters` = db chapters whose url is absent from `sourceChapters`.
6. `maxSeenUploadDate = 0`. For each `sourceChapter` in source order:
   1. `chapter = sourceChapter`.
   2. If `source is HttpSource`: `sChapter = chapter.toSChapter()`; `source.prepareNewChapter(sChapter, manga.toSManga())` (extension hook that may rewrite name/date/number, e.g. to parse dates); `chapter = chapter.copyFromSChapter(sChapter)`.
   3. `chapterNumber = ChapterRecognition.parseChapterNumber(manga.title, chapter.name, chapter.chapterNumber)` (area:07; a number already `-2.0` or `> -1.0` is trusted as is); `chapter = chapter.copy(chapterNumber = ...)`.
   4. `dbChapter = dbChapters.find { it.url == chapter.url }`.
   5. If none (NEW): if `chapter.dateUpload == 0L` then `dateUpload = if (maxSeenUploadDate == 0) nowMillis else maxSeenUploadDate`; else `maxSeenUploadDate = max(maxSeenUploadDate, sourceChapter.dateUpload)` and keep `chapter` (note: uses the pre-`prepareNewChapter` value). Append to `newChapters`.
   6. If found: `shouldUpdateDbChapter.await(db, chapter)` is true when any of `scanlator, name, dateUpload, chapterNumber, sourceOrder` differs (`domain/src/main/java/tachiyomi/domain/items/chapter/interactor/ShouldUpdateDbChapter.kt`). Then:
      - `shouldRenameChapter = downloadProvider.isChapterDirNameChanged(db, chapter) && downloadManager.isChapterDownloaded(db.name, db.scanlator, manga.title, manga.source)`; if true `downloadManager.renameChapter(source, manga, db, chapter)` (renames the downloaded directory/CBZ so the download is not orphaned; area:23).
      - `toChange = db.copy(name = chapter.name, chapterNumber = chapter.chapterNumber, scanlator = chapter.scanlator, sourceOrder = chapter.sourceOrder)`; if `chapter.dateUpload != 0L` then `toChange = toChange.copy(dateUpload = chapter.dateUpload)`. Append to `updatedChapters`. Read/bookmark/lastPageRead/dateFetch of the db row are preserved.
7. If `newChapters`, `removedChapters`, `updatedChapters` are all empty: `if (manualFetch || manga.fetchInterval == 0 || manga.nextUpdate < fetchWindow.first) updateManga.awaitUpdateFetchInterval(manga, now, fetchWindow)`; return `emptyList()` (no `last_update` bump).
8. Else build helper sets: `readChapterNumbers` = chapter numbers of db chapters with `read && isRecognizedNumber (number >= 0)`; for each removed chapter: add its number to `deletedChapterNumbers` (TreeSet), to `deletedReadChapterNumbers` if read, to `deletedBookmarkedChapterNumbers` if bookmarked; `deletedChapterNumberDateFetchMap = removed.sortedByDescending { dateFetch }.associate { number to dateFetch }` (later pairs overwrite earlier ones, so for duplicate numbers the entry with the SMALLEST `dateFetch` survives).
9. `markDuplicateAsRead = libraryPreferences.markDuplicateReadChapterAsRead().get().contains("new")` (key `mark_duplicate_read_chapter_read`, a string set with values `"new"` and `"existing"`, default empty).
10. Assign fetch dates and carry state. `itemCount = newChapters.size`; for each new chapter in list order: `chapter = toAdd.copy(dateFetch = nowMillis + itemCount--)` (first listed, i.e. newest per the "sources must list most recent first" rule, gets the highest value `now + N`; the last gets `now + 1`). Then:
    1. if `chapter.chapterNumber in readChapterNumbers && markDuplicateAsRead`: add url to `changedOrDuplicateReadUrls`, `chapter = chapter.copy(read = true)`.
    2. if `!chapter.isRecognizedNumber || chapter.chapterNumber !in deletedChapterNumbers`: keep as is (`return@map`).
    3. else `chapter = chapter.copy(read = number in deletedReadChapterNumbers, bookmark = number in deletedBookmarkedChapterNumbers)` (this OVERRIDES the read=true of step 10.1: the state of the removed twin wins), then if `deletedChapterNumberDateFetchMap[number]` exists use it as `dateFetch` (so a re-uploaded chapter does not appear as new in the Updates tab), add url to `changedOrDuplicateReadUrls`.
11. `if removedChapters.isNotEmpty()`: `chapterRepository.removeChaptersWithIds(ids)`.
12. `if updatedToAdd.isNotEmpty()`: `updatedToAdd = chapterRepository.addAllChapters(updatedToAdd)` (returns the inserted rows with their new ids).
13. `if updatedChapters.isNotEmpty()`: `updateChapter.awaitAll(updatedChapters.map { it.toChapterUpdate() })`. Because `ChapterUpdate` fields are nullable-coalesce, a source chapter whose scanlator became null keeps the old scanlator in the DB; since `ShouldUpdateDbChapter` compares `scanlator`, that chapter is "updated" again on every sync (harmless but it defeats the step-7 early return).
14. `updateManga.awaitUpdateFetchInterval(manga, now, fetchWindow)`.
15. `updateManga.awaitUpdateLastUpdate(manga.id)` (comment in source: `last_update` is the last time the chapter list changed at all).
16. `excludedScanlators = getExcludedScanlators.await(manga.id).toHashSet()`.
17. Return `updatedToAdd.filterNot { it.url in changedOrDuplicateReadUrls || it.scanlator in excludedScanlators }`: chapters that were marked read as duplicates, that inherited state from a removed twin, or that belong to an excluded scanlator are not reported as new (no notification, no auto-download).
Note: steps 11-15 are separate repository calls, NOT one transaction.

### 6.2 SyncEpisodesWithSource.await

Source: `app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt`. Same skeleton as 6.1 with these differences:
- Throws `NoEpisodesException` ("No episodes found", `MR.strings.no_episodes_error` in `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:516`) when empty and not local (`AnimeScreenModel.kt:350`, `AnimeLibraryUpdateJob.kt:310`).
- `Episode.copyFromSEpisode`: `name, url, dateUpload, episodeNumber = episode_number.toDouble(), fillermark, scanlator?.ifBlank{null}` (NOT trimmed, unlike chapters), `summary?.ifBlank{null}`, `previewUrl = preview_url?.ifBlank{null}`, `memo`. Sanitizer: `EpisodeSanitizer.sanitize(name, anime.title)`.
- `prepareNewEpisode(sEpisode, anime.toSAnime())` for `AnimeHttpSource`; number via `EpisodeRecognition.parseEpisodeNumber(anime.title, episode.name, episode.episodeNumber)`.
- `ShouldUpdateDbEpisode` compares: `scanlator, name, dateUpload, episodeNumber, sourceOrder, summary, fillermark, previewUrl, memo` (JSON object equality).
- On update: `toChange = db.copy(name, episodeNumber, scanlator, summary, sourceOrder, memo)`; then `if (episode.dateUpload != 0L) dateUpload = sourceEpisode.dateUpload`; `if (!toChange.fillermark) fillermark = sourceEpisode.fillermark` (a fillermark set by the user is never cleared by the source); `if (toChange.previewUrl.isNullOrBlank()) previewUrl = sourceEpisode.previewUrl`. Rename of downloads uses `isEpisodeDirNameChanged` / `isEpisodeDownloaded(name, scanlator, anime.title, anime.source)` / `renameEpisode`.
- Duplicate handling uses `seen` / `markDuplicateSeenEpisodeAsSeen` (key `mark_duplicate_seen_episode_seen`, value `"new_episode"` for new; constants `MARK_DUPLICATE_EPISODE_SEEN_NEW = "new_episode"`, `..._EXISTING = "existing_episode"`).
- There is no excluded-scanlator filtering: it returns `updatedToAdd.filterNot { it.url in changedOrDuplicateReadUrls }`.
- Everything else (fetch-date arithmetic, early return, deleted-twin inheritance, `awaitUpdateFetchInterval`, `awaitUpdateLastUpdate`) is identical. The caller is `UpdateAnimeFromRemote.awaitEpisodesUpdate` (`app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt:74`), which wraps failures in `Result.failure`.

### 6.3 SyncSeasonsWithSource.await

Source: `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt`. A season is itself an `Anime` row whose `parent_id` points at the parent anime. Signature `await(rawSourceSeasons: List<SAnime>, anime: Anime, source: AnimeSource, manualFetch = false, fetchWindow = Pair(0, 0)): List<Anime>` returns ALL source seasons mapped to local rows (not only new ones).
1. Empty and not local -> `NoSeasonsException` (`MR.strings.no_seasons_error` = "No seasons found", `AnimeScreenModel.kt:353`).
2. `sourceSeasons = raw.distinctBy { it.url }.mapIndexed { i, s -> networkToLocalAnime.await(s.toDomainAnime(source.id)).copy(parentId = anime.id, seasonSourceOrder = i) }`. `NetworkToLocalAnime.await` (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt`): looks the row up by `(url, source)`; absent -> `insertAnime` and return with the new id; present and not favourite -> returns the local row with `title` replaced by the source title; present and favourite -> the local row unchanged. This INSERTS rows for every unseen season immediately, before diffing.
3. `dbSeasons = getAnimeSeasonsByParentId.await(anime.id)` (rows with `parent_id = anime.id`, each wrapping `.anime`).
4. `removedSeasons` = db seasons whose url is not in `sourceSeasons`.
5. For each source season: `seasonNumber = SeasonRecognition.parseSeasonNumber(anime.title, season.title, season.seasonNumber)` (`domain/src/main/java/tachiyomi/domain/items/season/service/SeasonRecognition.kt`, see p1-topic-chapter-episode-sync.md section 6); then find the db season by url. Not found -> `newSeasons`; found and `ShouldUpdateDbSeason` (`title`, `seasonNumber`, `seasonSourceOrder` differ) -> `updatedSeasons += db.copy(title, seasonNumber, seasonSourceOrder, memo = season.memo)`.
6. If nothing new/removed/updated: `if (manualFetch || anime.fetchInterval == 0 || anime.nextUpdate < fetchWindow.first) updateAnime.awaitUpdateFetchInterval(anime, now, fetchWindow)`; return `sourceSeasons`.
7. `removedSeasons.isNotEmpty()` -> `animeRepository.removeParentIdByIds(ids)` (detach: the rows survive, their parent link is cleared; nothing is deleted).
8. `toUpdate = newSeasons.map(toAnimeUpdate) + updatedSeasons.map(toAnimeUpdate)`; `updateAnime.awaitAll(toUpdate)` writes the FULL field set of each in-memory season (including `parent_id`, `season_number`, `season_source_order`), then `updateAnime.awaitUpdateLastUpdate(anime.id)`. Unlike the chapter sync the fetch interval is not recomputed on change. Return `sourceSeasons`.

### 6.4 SyncRelatedAnimeWithSource.await

Source: `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt`; caller `AnimeScreenModel.kt:1479`.
1. `source = sourceManager.get(anime.source)?.takeIf { it.supportsRelatedAnime } ?: return`.
2. `now = now_millis`. Unless `forceRefresh`: `lastFetchedAt = relationRepository.getLastFetchedAt(anime.id)`; if non-null and `now - lastFetchedAt < TTL` return. `TTL = 7.days.inWholeMilliseconds = 604_800_000`.
3. `groups = source.getRelatedAnimeList(anime.toSAnime()).map { relation -> relation.name to relation.animes.distinctBy { it.url }.filterNot { it.url == anime.url }.take(MAX_PER_GROUP = 10).map { networkToLocalAnime.await(it.toDomainAnime(anime.source)).id } }.filter { (_, ids) -> ids.isNotEmpty() }`.
4. `relationRepository.replaceRelations(anime.id, groups, now)` (area:05 `AnimeRelationRepository`, replaces all rows of the anime and stores `fetched_at = now`). Errors from the source propagate to the caller (no try/catch here). Note the related rows are created through `networkToLocalAnime` (same insert-on-miss behaviour) with the PARENT's source id, not the related entry's.

## 7. Chapter/episode list filters

Sources: `app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt`, `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt`.

OBSERVED `List<Chapter>.applyFilters(manga, downloadManager)`: `isLocal = manga.isLocal()`; then in this order (filters are conjunctive): unread (`applyFilter(manga.unreadFilter) { !chapter.read }`), bookmarked (`chapter.bookmark`), downloaded (`applyFilter(manga.downloadedFilter) { downloadManager.isChapterDownloaded(chapter.name, chapter.scanlator, manga.title, manga.source) || isLocal }`), then `sortedWith(getChapterSort(manga))`. Local manga always counts as downloaded. The overload on `List<ChapterList.Item>` (UI items with a precomputed `isDownloaded`) does the same lazily via a `Sequence` and uses `it.isDownloaded || isLocal`.

`getChapterSort(manga, sortDescending = manga.sortDescending())` (`domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt`): by `manga.sorting`: `CHAPTER_SORTING_SOURCE (0x000)` -> descending: ascending `sourceOrder`; ascending: descending `sourceOrder` (source order 0 is the NEWEST chapter, so "descending" shows the newest first); `NUMBER (0x100)` -> by `chapterNumber`; `UPLOAD_DATE (0x200)` -> by `dateUpload`; `ALPHABET (0x300)` -> by `name.compareToWithCollator` (locale collator); any other value throws `NotImplementedError("Invalid chapter sorting method: ...")`. `sortDescending()` = `chapterFlags and CHAPTER_SORT_DIR_MASK(0x1) == CHAPTER_SORT_DESC(0x0)`.

Episodes: unseen (`!episode.seen`), bookmarked, fillermarked (`episode.fillermark`), downloaded, then `getEpisodeSort(anime)` (area:07: sorting constants `EPISODE_SORTING_SOURCE 0x0, NUMBER 0x200, UPLOAD_DATE 0x400, ALPHABET 0x600`). All filters are `TriState`s from the entry flags (p1-data-and-config.md section 3.3).

Proposal: one generic implementation `apply_filters<T: FilterRow>(rows, entry: &EntryFilters, is_downloaded: impl Fn(&T)->bool, is_local: bool)` where `FilterRow` exposes `is_read/is_bookmarked/is_fillermarked/number/source_order/upload_date/name`. The Compose UI-item overload is replaced by the same function over the GUI row type.

## 8. SetReadStatus / SetSeenStatus

Sources: `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt`, `app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt`.

OBSERVED `SetReadStatus.await(read: Boolean, vararg chapters: Chapter): Result` inside `withNonCancellableContext`:
1. Filter: `read = true` -> chapters with `!it.read`; `read = false` -> chapters with `it.read || it.lastPageRead > 0`. If the filtered list is empty return `Result.NoChapters`.
2. `chapterRepository.updateAllChapters(filtered.map { ChapterUpdate(id, read = read, lastPageRead = if (!read) 0 else null) })` in one repository transaction. Marking read keeps `last_page_read` untouched (null = unchanged); marking unread resets it to 0. Any `Exception` is logged at ERROR and returned as `Result.InternalError(e)`.
3. If `read && downloadPreferences.removeAfterMarkedAsRead().get()` (key `pref_remove_after_marked_as_read_key`, default `false`): group the updated chapters by `mangaId`; for each group `deleteDownload.awaitAll(mangaRepository.getMangaById(mangaId), *chapters)`.
4. Return `Result.Success`.
Overloads: `await(mangaId, read)` = all chapters of the manga (`getChapterByMangaId`); `await(manga, read)`. Result type: `Success | NoChapters | InternalError(Throwable)`.
`SetSeenStatus` is identical with `seen`, `EpisodeUpdate(seen = seen, lastSecondSeen = if (!seen) 0 else null)`, filter `seen = false` -> `it.seen || it.lastSecondSeen > 0`, `Result.NoEpisodes`. Note it does not touch `total_seconds`.
Callers: details screens (multi-select "mark read"), `ReaderViewModel`, `PlayerViewModel`, library/update screens.

## 9. DeleteChapterDownload / DeleteEpisodeDownload

Sources: `app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt`, `app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt`.
OBSERVED `awaitAll(manga, vararg chapters)` in `withNonCancellableContext`: `sourceManager.get(manga.source)?.let { downloadManager.deleteChapters(chapters.toList(), manga, source) }`. If the source is not installed (`get` returns null) NOTHING is deleted and no error is raised (the stub-source fallback `getOrStub` is not used). Result is `Unit`.
Proposal: keep the "source missing -> silent no-op" behaviour but log at `warn` so the user can find out why a download stayed on disk; consider resolving the directory name from the stored source name (area:23 keeps a source-name -> id cache) in a later improvement.

## 10. Source listing and preferences

Sources under `app/src/main/java/eu/kanade/domain/source/`. Full detail and ordering rules in p1-topic-source-and-extension-listing.md. Preference keys in p1-data-and-config.md section 4.

- `GetEnabledMangaSources/AnimeSources.subscribe()`: `combine(pinned ids, enabled languages, (disabled ids, last used id, [manga: data-saver excluded ids]), repository.getSources())`. Steps: keep sources with `lang in enabledLanguages || id == LocalSource.ID (0)`; drop ids whose decimal string is in the disabled set; sort by name with `String.CASE_INSENSITIVE_ORDER`; `flatMap` each source to `[source.copy(pin = pinned or unpinned [, isExcludedFromDataSaver])]` and, when `source.id == lastUsedSource`, a second copy `source.copy(isUsedLast = true, pin = source.pin - Pin.Actual)`; `distinctUntilChanged()`.
- `GetLanguagesWithMangaSources/AnimeSources.subscribe()`: `combine(enabledLanguages, disabledSources, repository.getOnlineMangaSources()` (anime: `getOnlineAnimeSources()`) `)`; sources sorted by `(id in disabled)` (enabled first) then name case-insensitively; `groupBy lang`; `toSortedMap(compareBy { it !in enabledLanguage }.then(LocaleHelper.comparator))`.
- `GetMangaSourcesWithFavoriteCount/GetAnimeSourcesWithFavoriteCount.subscribe()`: drops the local source, sorts with `migrationSortingMode` and `migrationSortingDirection`; stubs (`isStub`) sort FIRST in both modes.
- `ToggleMangaSource/ToggleAnimeSource.await`: the `enable` default parameter is `isEnabled(id)` which (naming bug) returns `true` when the id IS in the disabled set, so the no-argument toggle enables a disabled source and disables an enabled one; set variant adds/removes the decimal id strings from `hidden_catalogues` / `hidden_anime_catalogues`.
- `ToggleMangaSourcePin/ToggleAnimeSourcePin.await(source)`: add/remove the id in `pinned_catalogues` / `pinned_anime_catalogues`.
- `ToggleLanguage.await(language)`: add/remove in `source_languages`.
- `SetMigrateSorting.await(mode, direction)`: sets `pref_migration_sorting` and `pref_migration_direction` (two separate writes).
- `ToggleMangaIncognito/ToggleAnimeIncognito.await(extensions: String, enable)`: add/remove the extension package name in `incognito_manga_extensions` / `incognito_anime_extensions`.
- `GetMangaIncognitoState/GetAnimeIncognitoState`: `await(sourceId?)`: true if the global incognito flag is on; false if `sourceId == null`; else true iff the owning extension package is in the incognito set. `subscribe` is the reactive version with `distinctUntilChanged`.
- `ToggleExcludeFromMangaDataSaver.await(source)`: toggles `source.id.toString()` in `data_saver_excluded`.
- `AnimeSource.icon`, `Source.icon`: the extension's app icon as a Compose bitmap (Android drawable); see Replacement in p1-rust-design.md section 7.

## 11. Extension listing and trust

Sources under `app/src/main/java/eu/kanade/domain/extension/`. Details in p1-topic-source-and-extension-listing.md.

- `GetMangaExtensionsByType/GetAnimeExtensionsByType.subscribe()` -> `MangaExtensions/AnimeExtensions(updates, installed, available, untrusted)`: see the exact pipeline in the topic file. `showNsfwSource` is read ONCE when `subscribe()` is called (not observed).
- `GetMangaExtensionLanguages/GetAnimeExtensionLanguages.subscribe()`: distinct languages of the available extensions' sources (manga: an extension with no sources contributes its own `lang`), sorted by `(lang !in enabledLanguages)` then `LocaleHelper.comparator`.
- `GetExtensionSources` (manga) / `GetAnimeExtensionSources.subscribe(installedExtension)`: one item per source with `enabled = id.toString() !in disabledSources` and `labelAsName = isMultiSource && !isMultiLangSingleSource`.
- `TrustMangaExtension / TrustAnimeExtension`: `isTrusted(pkgInfo, fingerprints)`: the extension is trusted iff any signing key of the user's extension repos/stores appears in the package's signature `fingerprints`, or the string `"<package>:<versionCode>:<fingerprints.last()>"` is in the `trusted_extensions` set. `trust(pkgName, versionCode, signatureHash)`: removes every entry starting with `"<pkgName>:"` then adds the new triple. `revokeAll()` deletes the preference (called from `SettingsAdvancedScreen.kt:470-471`).

## 12. Tracking

Sources under `app/src/main/java/eu/kanade/domain/track/`. Full state machines in p1-topic-tracking-orchestration.md.

### 12.1 TrackPreferences, AutoTrackState
`TrackPreferences` keys and defaults are in p1-data-and-config.md section 5. `AutoTrackState {ALWAYS (MR.strings.lock_always "Always"), ASK (MR.strings.default_category_summary "Always ask"), NEVER (MR.strings.lock_never "Never")}`; stored by enum name under `pref_auto_update_manga_on_mark_read`, default `ALWAYS`. `setCredentials(tracker, username, password)` writes username, password and then resets `trackAuthExpired` to `false` (three writes).

### 12.2 TrackChapter.await / TrackEpisode.await (auto-update progress)
`await(context, mangaId, chapterNumber: Double, setupJobOnFailure = true)` in `withNonCancellableContext`:
1. `tracks = getTracks.await(mangaId)`; empty -> return.
2. For each track: `service = trackerManager.get(track.trackerId)`; skip when `service == null || !service.isLoggedIn || chapterNumber <= track.lastChapterRead`.
3. Otherwise run concurrently (`async`) with `runCatching`:
   - Manga: `updatedTrack = service.mangaService.refresh(track.toDbTrack()).toDomainTrack(idRequired = true)!!.copy(lastChapterRead = chapterNumber)`; `service.mangaService.update(updatedTrack.toDbTrack(), true)` (`didReadChapter = true`); `insertTrack.await(updatedTrack)`; `delayedTrackingStore.removeMangaItem(track.id)`. ANY exception: `delayedTrackingStore.addManga(track.id, chapterNumber)`; if `setupJobOnFailure` -> `DelayedMangaTrackingUpdateJob.setupTask(context)`; rethrow (swallowed by `runCatching`).
   - Anime: the same but guarded by `context.isOnline()`: online -> refresh/update/insert/remove-from-queue (a failure here is only logged, NOT queued); offline -> `delayedTrackingStore.addAnime(track.id, episodeNumber)` and `setupTask` if allowed.
4. `awaitAll`, collect `exceptionOrNull()` of each, log each at INFO.
Callers: `ReaderViewModel.kt:926`, `MangaScreenModel.kt:776,792`, `PlayerViewModel.kt:1955` (`episode.episode_number.toFloat -> toDouble`), `AnimeScreenModel.kt:890,907`, `DelayedMangaTrackingUpdateJob` / `DelayedAnimeTrackingUpdateJob` (with `setupJobOnFailure = false`).

### 12.3 Delayed queue and worker
`DelayedMangaTrackingStore` / `DelayedAnimeTrackingStore`: Android `SharedPreferences` file `tracking_queue` (BOTH stores use the same file name, keys are the track row ids as decimal strings, values are `Float`). `add*(trackId, lastRead)`: only writes if `lastRead > stored (default 0f)`. `remove*Item(trackId)`: removes the key. `get*Items()`: every entry becomes `DelayedTrackingItem(trackId = key.toLong(), lastChapterRead = value.toString().toFloat())`.
`Delayed*TrackingUpdateJob` (WorkManager `CoroutineWorker`): `if (runAttemptCount > 3) return Result.failure()`; for each queued item: `track = getTracks.awaitOne(trackId)`; if null remove the queue entry; else `track.copy(lastChapterRead = item.lastChapterRead.toDouble())` then `trackChapter.await(context, track.mangaId, track.lastChapterRead, setupJobOnFailure = false)`. Return `Result.success()` if the queue is empty afterwards else `Result.retry()`. `setupTask(context)`: unique one-time work, tag `"DelayedMangaTrackingUpdate"` / `"DelayedAnimeTrackingUpdate"`, constraint `NetworkType.CONNECTED`, `BackoffPolicy.EXPONENTIAL` with initial 5 minutes, `ExistingWorkPolicy.REPLACE`.
Bug to avoid: because the two stores share one file with colliding id spaces (manga and anime track tables are different databases), a manga job can drop or replay an anime entry and vice versa.

### 12.4 RefreshMangaTracks / RefreshAnimeTracks
`await(mangaId): List<Pair<Tracker?, Throwable>>` in `supervisorScope`: for every stored track whose tracker `isLoggedIn`, concurrently: `updated = service.mangaService.refresh(track.toDbTrack()).toDomainTrack()!!`, `insertTrack.await(updated)`, `syncChapterProgressWithTrack.await(mangaId, updated, service.mangaService)`; per-tracker failure is caught as `Throwable` and returned in the list (the others continue). Callers: `MangaTrackInfoDialog.kt:241`, `AnimeTrackInfoDialog.kt:241`, `MangaScreenModel.kt:798`, `AnimeScreenModel.kt:913`.

### 12.5 AddMangaTracks.bind / AddAnimeTracks.bind
(`tracker`, `item: <Db>Track`, `mangaId`), inside `withNonCancellableContext { withIOContext { } }`:
1. `allChapters = getChaptersByMangaId.await(mangaId)`; `hasReadChapters = any { it.read }` (episodes: `seen`).
2. `tracker.bind(item, hasReadChapters)` (remote add/lookup; the tracker may update `item` in place, area:27).
3. `track = item.toDomainTrack(idRequired = false) ?: return` (a track without id is allowed here: id -1).
4. `insertTrack.await(track)`.
5. If `hasReadChapters`:
   - `latestLocalReadChapterNumber = allChapters.sortedBy { number }.takeWhile { it.read }.lastOrNull()?.chapterNumber ?: -1.0` (longest read prefix by number).
   - if it is `> track.lastChapterRead`: `track = track.copy(lastChapterRead = latest)`; `tracker.setRemoteLastChapterRead(track.toDbTrack(), latest.toInt())` (the remote receives the integer part; the local row keeps the decimal).
   - if `track.startDate <= 0`: `firstReadChapterDate = GetMangaHistory.await(mangaId).sortedBy { it.readAt }.firstOrNull()?.readAt` (the history list entries have `readAt: Date?`; `Injekt.get<GetMangaHistory>()` is resolved inline); if present: `startDate = date.time.convertEpochMillisZone(ZoneId.systemDefault(), ZoneOffset.UTC)` i.e. `LocalDateTime.ofInstant(instant, systemZone).atZone(UTC).toEpochMilli()` (the local wall clock time reinterpreted as UTC, `app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt:29`); `track = track.copy(startDate)`; `tracker.setRemoteStartDate(track.toDbTrack(), startDate)`.
6. `syncChapterProgressWithTrack.await(mangaId, track, tracker)` (no-op unless the tracker is Enhanced).
Anime: identical with `seen`, `episodeNumber`, `seenAt`, `setRemoteLastEpisodeSeen`.

`bindEnhancedTrackers(manga, source)`: for every logged-in tracker that is `EnhancedMangaTracker` and `accept(source)`: try `service.match(manga)` -> `MangaTrack?`; if non-null set `track.manga_id = manga.id`, `(service as Tracker).mangaService.bind(track)`, `insertTrack.await(track.toDomainTrack(idRequired = false)!!)`, then `syncChapterProgressWithTrack.await(manga.id, track.toDomainTrack(false)!!, service.mangaService)`. Per-tracker `Exception` -> WARN "Could not match manga: <title> with service <service>". Callers: `BrowseMangaSourceScreenModel.kt:248` (when an entry is added to the library), `MangaScreenModel.kt:374`, `MangaHistoryScreenModel.kt:225`, anime equivalents at `BrowseAnimeSourceScreenModel.kt:240`, `AnimeScreenModel.kt:450`, `AnimeHistoryScreenModel.kt:220`.

### 12.6 SyncChapterProgressWithTrack / SyncEpisodeProgressWithTrack
`await(mangaId, remoteTrack, tracker)`: only for `EnhancedMangaTracker` (Kavita/Komga/Suwayomi/Jellyfin style, area:33); returns immediately otherwise.
1. `sortedChapters = getChaptersByMangaId.await(id).sortedBy { chapterNumber }.filter { isRecognizedNumber }`.
2. `chapterUpdates = sortedChapters.filter { number <= remoteTrack.lastChapterRead && !read }.map { it.copy(read = true).toChapterUpdate() }`.
3. `localLastRead = sortedChapters.takeWhile { it.read }.lastOrNull()?.chapterNumber ?: 0F` (computed on the PRE-update state, so it does not include chapters step 2 is about to mark).
4. `lastRead = max(remoteTrack.lastChapterRead, localLastRead.toDouble())`; `updatedTrack = remoteTrack.copy(lastChapterRead = lastRead)`.
5. In `try`: `tracker.update(updatedTrack.toDbTrack())` (remote write, `didReadChapter = false`), then `updateChapter.awaitAll(chapterUpdates)`, then `insertTrack.await(updatedTrack)`. Any `Throwable` is logged at WARN. If the remote update throws, the local chapters are NOT marked.

### 12.7 Track mappers
`MangaTrack.copyPersonalFrom(other)` (domain, in this scope): copies `lastChapterRead, score, status, startDate, finishDate, private` (all six, unconditionally). The tracker-layer `copyPersonalFrom(other, copyRemotePrivate = true)` on the Db model (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:37`) is area:27 and optionally skips `private`. `toDbTrack` / `toDomainTrack(idRequired = true)`: field-for-field mapping (domain `remoteUrl` <-> `tracking_url`, `startDate` <-> `started_reading_date` / `started_watching_date`, `libraryId`, `title`, `totalChapters` / `totalEpisodes`); `toDomainTrack` returns `null` when `id == null && idRequired`, uses id `-1` when `!idRequired`. Anime `trackerId = tracker_id.toLong()`.

## 13. Cross-area contracts this area depends on

| Dependency | Needed API | Area |
|---|---|---|
| `MangaRepository`, `AnimeRepository` | `get_by_id, update(update), update_all, remove_parent_id_by_ids, get_seasons_by_id` | area:05 |
| `ChapterRepository`, `EpisodeRepository` | `get_by_entry_id, add_all (returns rows with ids), update_all, remove_with_ids, get_scanlators_by_manga_id` | area:05 |
| `MangaFetchInterval` / `AnimeFetchInterval` | `get_window(dt)`, `to_update(entry, dt, window)` (MAX_INTERVAL 28, GRACE 1) | area:07 |
| `ChapterRecognition`, `EpisodeRecognition`, `SeasonRecognition` | number parsing | area:07 |
| `ChapterSanitizer`, `EpisodeSanitizer` | `sanitize(name, entry_title)` | area:05 |
| Download managers | `is_downloaded(name, scanlator, title, source)`, `is_dir_name_changed`, `rename`, `delete` | area:23, area:24, area:25 |
| Source manager | `get(source_id)`, `get_or_stub`, `prepareNew*` on HTTP sources | area:17, area:15, area:16 |
| Extension manager flows | `installed`, `untrusted`, `available`, `extension_package(source_id)` | area:18, area:19, area:20 |
| `TrackerManager` | `get(id)`, `logged_in_trackers()`, `Tracker::{manga,anime}_service`, `EnhancedTracker` | area:27 |
| History | `GetMangaHistory.await(manga_id)` (readAt), `GetAnimeHistory` | area:08 |
| Preferences | `BasePreferences, LibraryPreferences, DownloadPreferences` | area:02 |
