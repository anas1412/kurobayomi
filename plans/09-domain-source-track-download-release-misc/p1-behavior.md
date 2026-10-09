# Area 09 — behavior specification

Scope: the 73 files listed in `p1-inventory.md`. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Labels: **OBSERVED** = what the Kotlin does (file:line cited). **Proposal:** = what Kurobayomi should do. Feature keys: `pref:<key>` = preference key; `sql:<table>` = table in `p1-data-and-config.md`.

## 0. Cross-cutting conventions (OBSERVED)

1. **Error policy is per interactor, not global.** Three patterns occur in scope:
   - **Swallow and log** (track interactors): every call wraps the repository call in `try { … } catch (e: Exception) { logcat(LogPriority.ERROR, e) }`. Reads return `null` (single) or `emptyList()` (list); writes return `Unit`. Files: `domain/src/main/java/tachiyomi/domain/track/anime/interactor/GetAnimeTracks.kt:13-29`, `…/track/anime/interactor/InsertAnimeTrack.kt:12-26`, `…/track/anime/interactor/DeleteAnimeTrack.kt:11-17`, and the manga twins.
   - **Typed `Result` sealed interface** (custom buttons, extension repos): `Success`, `InternalError(error)` and for some `Unchanged`, `InvalidUrl`, `RepoAlreadyExists`, `DuplicateFingerprint`, `Error`. Files: `domain/src/main/java/tachiyomi/domain/custombuttons/interactor/*.kt`, `domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt:65-71`.
   - **Propagate** (extension-store add returns `Result<Unit>` from Kotlin stdlib; repo-update awaitAll and delete/replace/rename throw).
2. **Cancellation.** Mutating custom-button interactors run inside `withNonCancellableContext` (`tachiyomi.core.common.util.lang`), so a navigation cancel never leaves a half-written batch. Extension-store service rethrows `CancellationException` before the generic catch (`data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt:60-61`).
3. **Dispatchers.** Network: `withIOContext` (`ExtensionRepoService.kt:22`). `StorageManager` owns `CoroutineScope(Dispatchers.IO)` (`domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:23`).
4. **Logging.** `logcat(LogPriority.ERROR, e) { "…" }` everywhere; `LogPriority.WARN` for expected SQL conflicts in `CreateMangaExtensionRepo.kt:38`.
5. **Proposal:** every interactor returns `Result<T, DomainError>` (see `p1-rust-design.md` section 5); the swallow-and-log behaviour of track interactors is preserved as an explicit `log_and_default()` helper, not as a silent `Ok`, so that the UI can still show a toast when a write fails (Proposal, see `p1-edge-cases-and-tests.md` E-TRK-03).

## 1. Source model (manga and anime)

### 1.1 Source value (OBSERVED)
- `Source` (manga) fields: `id: Long`, `lang: String`, `name: String`, `supportsLatest: Boolean`, `isStub: Boolean`, `pin: Pins = Pins.unpinned`, `isUsedLast: Boolean = false`, `isExcludedFromDataSaver: Boolean = false` (SY fork field, see T-09a-056). File: `domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:3-14`.
- `AnimeSource` identical minus `isExcludedFromDataSaver`: `domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:3-11`.
- `visualName` (both): if `lang.isEmpty()` then `name`, else `"$name (${lang.uppercase()})"`. Source.kt:16-20; AnimeSource.kt:13-17.
- `key` (both): a lambda `() -> String` returning `"$id-lastused"` when `isUsedLast`, else `"$id"`. Source.kt:22-27; AnimeSource.kt:19-24. The lambda is a property so the key is recomputed on every access; it is used as the list item key in the UI, so the "last used" entry and the regular entry of the same source are distinct keys.

### 1.2 Stub source (OBSERVED)
- `StubMangaSource(id, lang, name)` and `StubAnimeSource(id, lang, name)` implement the upstream `MangaSource` / `AnimeSource` interfaces. `isInvalid = name.isBlank() || lang.isBlank()`.
- Every operation throws `SourceNotInstalledException` (manga) / `AnimeSourceNotInstalledException` (anime): `getMangaDetails`, `getChapterList`, `getPageList` (StubMangaSource.kt:17-24); `getPopularAnime`, `getLatestUpdates`, `getSearchAnime`, `getAnimeEpisodeUpdate`, `getAnimeSeasonUpdate`, `getRelatedAnimeList`, `getHosterList`, `getVideoList` (StubAnimeSource.kt:25-57). Anime stub: `supportsLatest = false`, `supportsRelatedAnime = false`.
- `toString()`: `"$name (${lang.uppercase()})"` when valid, else `id.toString()` (StubMangaSource.kt:26-27; StubAnimeSource.kt:59-60).
- `from(source)` copies id/lang/name only (StubMangaSource.kt:30-32).
- Persistence of stubs: `upsertStubMangaSource(id, lang, name)`, `getStubMangaSource(id)`, `subscribeAllManga()` (MangaStubSourceRepository.kt:7-11); anime twin (AnimeStubSourceRepository.kt:7-12). Stubs are created when a source's extension is uninstalled but its library entries remain, so the entry still shows the source name.

### 1.3 Pins (OBSERVED, both kinds identical logic)
- `Pin.Unpinned = 0b00`, `Pin.Pinned = 0b01`, `Pin.Actual = 0b10` (`domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt:4-6`; anime uses `object`, manga uses `data object`).
- `Pins.unpinned = Pins(Pin.Unpinned)` (code 0). `Pins.pinned = Pins(Pin.Pinned, Pin.Actual)` (code 0b11) — the two flags together mean "pinned".
- `contains(pin)` = `pin.code and code == pin.code` (Kotlin precedence: infix `and` binds tighter than `==`, so this is `(pin.code and code) == pin.code`). Consequence: `Pins.unpinned.contains(Pin.Unpinned)` is `true` (0 == 0). Documented as quirk Q-PIN-1.
- `plus(pin)` = OR. `minus(pin)` = **XOR** (`Pins(code xor pin.code)`), so subtracting a flag that is not set *adds* it (quirk Q-PIN-2).
- `PinsBuilder.unaryPlus` ORs; `PinsBuilder.unaryMinus` is also an OR (`this@PinsBuilder.code = code or this@PinsBuilder.code`) — a copy-paste bug, quirk Q-PIN-3. Nothing in scope calls `-pin` in the builder; the Rust port must not reproduce it.
- Proposal: `Pins` is a `u8` newtype with `contains` = `(self.0 & p) == p`, `with(p)` = OR, `without(p)` = AND-NOT. Only `Pins::PINNED` (0b11) and `Pins::UNPINNED` (0) are constructed by app code; `without` is therefore never observed by users.

### 1.4 Browse routing (OBSERVED)
- `GetRemoteManga.subscribe(sourceId, query, filterList)` returns `repository.getPopularManga(sourceId)` if `query == QUERY_POPULAR` (`"eu.kanade.domain.source.manga.interactor.POPULAR"`), `repository.getLatestManga(sourceId)` if `query == QUERY_LATEST` (`"eu.kanade.domain.source.manga.interactor.LATEST"`), otherwise `repository.searchManga(sourceId, query, filterList)` (GetRemoteManga.kt:11-17). Anime twin with `"eu.kanade.domain.source.anime.interactor.POPULAR"` / `…LATEST` (GetRemoteAnime.kt:11-17, 20-21).
- The sentinel strings are passed as the *query* field, so the browse UI never has to carry a separate enum. Quirk Q-BR-1: a user who types the exact sentinel text into search gets the popular feed instead of a search. Proposal: an enum `BrowseQuery { Popular, Latest, Search(String) }`.

### 1.5 Source registry (OBSERVED contract, implementation outside scope)
- `MangaSourceManager`: `isInitialized: StateFlow<Boolean>`, `catalogueSources: Flow<List<CatalogueSource>>`, `get(sourceKey): MangaSource?`, `getOrStub(sourceKey): MangaSource`, `getOnlineSources(): List<HttpSource>`, `getCatalogueSources(): List<CatalogueSource>`, `getStubSources(): List<StubMangaSource>` (MangaSourceManager.kt:10-24).
- `AnimeSourceManager`: `isInitialized`, `sources: Flow<List<AnimeSource>>`, `get`, `getOrStub`, `getAll`, `getOnlineSources(): List<AnimeHttpSource>`, `getStubSources` (AnimeSourceManager.kt:9-23).
- `getOrStub(key)` semantics (implementation in area 17): return the installed source, otherwise a `StubXSource` built from the persisted stub row; the stub row is written when an installed source is first seen.
- Proposal: `SourceRegistry<K>` with a `tokio::sync::watch::Sender<bool>` for `is_initialized` and a `watch::Receiver<Vec<Arc<dyn Source>>>` for the list. `get_or_stub` returns `SourceHandle::Installed(Arc<dyn Source>)` or `SourceHandle::Stub(StubSource)`.

### 1.6 Source listing queries (OBSERVED contracts)
- `getOnlineMangaSources(): Flow<List<Source>>`, `getMangaSources()`, `getMangaSourcesWithFavoriteCount(): Flow<List<Pair<Source, Long>>>` (MangaSourceRepository.kt:14-18). Anime twins in AnimeSourceRepository.kt:13-17.
- `getMangaSourcesWithNonLibraryManga(): Flow<List<MangaSourceWithCount>>` used by `GetMangaSourcesWithNonLibraryManga.subscribe()` (GetMangaSourcesWithNonLibraryManga.kt:11-13). Each row is `(source, count)` where `count` = number of non-favorite manga rows referencing the source. `MangaSourceWithCount.id` / `.name` delegate to `source` (MangaSourceWithCount.kt:8-12).
- Anime non-library: `GetAnimeSourcesWithNonLibraryAnime.subscribe(): Flow<List<DeletableAnime>>` where `DeletableAnime(animeId, sourceId, fetchType)` (DeletableAnime.kt:5-9); children via `getDeletableChildren(parentId): List<Anime>` (GetAnimeSourcesWithNonLibraryAnime.kt:16-18). The grouping model `AnimeSourceWithIds(source, ids, orphaned)` has `count = ids.size` and exposes `id`, `name` from the source (AnimeSourceWithIds.kt:3-15). `orphaned` lists ids of anime whose parent (`parent_id`) no longer exists.
- Proposal: the non-library screen shows, per source, the count and a "Delete all non-library" action that deletes only rows with `favorite = 0` (the same predicate the query uses; verify in area 05).

### 1.7 Remote paging types (OBSERVED)
- `typealias SourcePagingSourceType = PagingSource<Long, SManga>` and `typealias AnimeSourcePagingSourceType = PagingSource<Long, SAnime>` — keys are `Long` page numbers (MangaSourceRepository.kt:10, AnimeSourceRepository.kt:9). The Android paging library is replaced by a Rust async page trait (`p1-rust-design.md` section 4.3).

## 2. Upcoming feeds (OBSERVED)

- Anime: `includedStatuses = setOf(SAnime.ONGOING, SAnime.PUBLISHING_FINISHED, SAnime.UPCOMING)` as Long = {1, 4, 7} (GetUpcomingAnime.kt:12-16). `subscribe()` returns `animeRepository.getUpcomingAnime(includedStatuses)` (line 19). The repository query is in area 05.
- Manga: `{SManga.ONGOING, SManga.PUBLISHING_FINISHED}` = {1, 4} (GetUpcomingManga.kt:12-15); returns `mangaRepository.getUpcomingManga(includedStatuses)` (line 18).
- Status constants (OBSERVED, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79` and `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54`): `UNKNOWN=0, ONGOING=1, COMPLETED=2, LICENSED=3, PUBLISHING_FINISHED=4, CANCELLED=5, ON_HIATUS=6, UPCOMING=7` (anime only has 7).
- Edge: a `COMPLETED` (2) or `CANCELLED` (5) title never appears in Upcoming, even if the user is waiting for new episodes. Observed; intended per the name ("upcoming").
- Proposal: keep the sets as data (`const UPCOMING_ANIME: &[i64] = &[1, 4, 7]; const UPCOMING_MANGA: &[i64] = &[1, 4];`).

## 3. Tracks (both kinds)

### 3.1 Read interactors (OBSERVED)
- `awaitOne(id)`: calls `trackRepository.getTrackByMangaId(id)` / `getTrackByAnimeId(id)`; on exception returns `null` (GetMangaTracks.kt:13-20; GetAnimeTracks.kt:13-20). **Quirk Q-TRK-1:** the parameter is the track row id, not the manga id. The data implementation maps it to `manga_syncQueries.getTrackById(id)` i.e. `WHERE _id = :id` (`data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt:14-16`). The name is misleading; the Rust API must be `get_track_by_row_id(row_id)`.
- `await(mangaId)`: `getTracksByMangaId(mangaId)` → list, on exception `emptyList()` (GetMangaTracks.kt:22-29).
- `subscribe(mangaId)`: `getTracksByMangaIdAsFlow(mangaId)` (GetMangaTracks.kt:31-33).
- `GetTracksPerManga.subscribe()`: `getMangaTracksAsFlow().map { it.groupBy { it.mangaId } }` → `Map<Long, List<MangaTrack>>` (GetTracksPerManga.kt:12-14). Anime twin groups by `animeId`.

### 3.2 Write interactors (OBSERVED)
- `InsertMangaTrack.await(track)` → `insertManga(track)`; `awaitAll(tracks)` → `insertAllManga(tracks)`; both swallow exceptions (InsertMangaTrack.kt:12-26).
- Data layer: `insertValues(vararg tracks)` runs in a single transaction `handler.await(inTransaction = true)` and calls the generated `insert` for each track (MangaTrackRepositoryImpl.kt:56-79). Columns written: `manga_id, sync_id (= trackerId), remote_id, library_id, title, last_chapter_read, total_chapters, status, score, remote_url, start_date, finish_date, private`.
- **Quirk Q-TRK-2 (important):** the table has `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE` (`data/src/main/sqldelight/data/manga_sync.sq:18`). Inserting a track for a (manga, tracker) pair that already has one *replaces* the row, so the `_id` changes and the `id` the UI held is stale. Proposal: `INSERT … ON CONFLICT(manga_id, sync_id) DO UPDATE SET …` to keep the row id stable (documented in T-09a-012).
- `DeleteMangaTrack.await(mangaId, trackerId)` → `delete(mangaId, trackerId)` → SQL `DELETE FROM manga_sync WHERE manga_id = :mangaId AND sync_id = :syncId` (manga_sync.sq:23-25). Errors swallowed.
- `update` SQL exists (manga_sync.sq:45-61) with `coalesce` semantics for every column, but no domain interactor in scope calls it; tracker code in area 27 writes updates via its own path. The Rust port keeps the partial-update SQL for area 27.

### 3.3 Track value semantics (OBSERVED)
- `lastChapterRead: Double` / `lastEpisodeSeen: Double` are stored as `REAL` (fractional progress allowed, e.g. 12.5). `totalChapters` / `totalEpisodes`: `INTEGER`. `status`, `score`: `Long` / `Double` — the meaning of each value is tracker-specific (area 27). `remoteId`: `INTEGER` (tracker's own id). `libraryId`: nullable. `startDate`, `finishDate`: `INTEGER` (epoch milliseconds convention is set by tracker code in area 27; stored as given). `private`: `INTEGER AS Boolean DEFAULT 0 NOT NULL`.
- `Serializable` is implemented only for backup transport (`java.io.Serializable`); see T-09a-054.

## 4. Download preferences (OBSERVED keys and defaults)

Full table in `p1-data-and-config.md` section 4. Behavioural notes:
- `numberOfDownloads` (`download_slots`, default 1) — UI offers `(1..5)` (SettingsDownloadScreen.kt:102-104); info string `AYMR.strings.download_slots_info`.
- `downloadSpeedLimit` (`download_speed_limit`, default 0) — unit KiB/s. Consumed at `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:525` as `bytesPerSecond(limit.toLong() * 1024)`. The throttler convention is that `0` disables throttling (throttler class is outside this scope; verify in area 23). Dialog `DownloadLimitDialog` writes the value on confirm (SettingsDownloadScreen.kt:62-77).
- `removeAfterReadSlots` (`remove_after_read_slots`, default -1): UI entries (SettingsDownloadScreen.kt:139-147): `-1` disabled (`MR.strings.disabled`), `0` last read chapter (`MR.strings.last_read_chapter`), `1` second to last (`MR.strings.second_to_last`), `2` third to last, `3` fourth to last, `4` fifth to last (`MR.strings.fourth_to_last`, `MR.strings.fifth_to_last`). Consumer: `ReaderViewModel.deleteChapterIfNeeded` (app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:503-519): returns if `-1`; target index = `indexOf(current) - slots`; `getOrNull` (so out-of-range is a no-op). Anime consumer: `PlayerViewModel.kt:1743` and `ExternalIntents.kt:508`.
- `autoDownloadWhileReading` (`auto_download_while_reading`, default 0) and `autoDownloadWhileWatching` (`auto_download_while_watching`, default 0): UI entries `listOf(0, 2, 3, 5, 10)` (SettingsDownloadScreen.kt:319-340): 0 = disabled, n = "next n unread chapters" / "next n unseen episodes". Consumers: `ReaderViewModel.kt:226` and `PlayerViewModel.kt:1112` (`downloadAheadAmount`).
- `removeAfterMarkedAsRead` (default false): applied in `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt:49` (`if (read && pref) …`) and `…/episode/interactor/SetSeenStatus.kt:49`.
- `removeBookmarkedChapters` (`pref_remove_bookmarked`, default false): when false the download manager keeps bookmarked items (`MangaDownloadManager.kt:424`, `AnimeDownloadManager.kt:422`).
- `downloadFillermarkedItems` (`pref_download_fillermarked`, default false): anime only; when false, filler episodes are skipped (`AnimeDownloadManager.kt:430`).
- `saveChaptersAsCBZ` (`save_chapter_as_cbz`, default true): `MangaDownloader.kt:425`.
- `splitTallImages` (`split_tall_images`, default true): no consumer in the scope; consumer is area 23.
- `downloadOnlyOverWifi` (`pref_download_only_over_wifi_key`, default true): consumed by `MangaDownloadJob.kt:59,72` and `AnimeDownloadJob.kt:59,72` (job constraint, observed changes flow).
- External downloader (`use_external_downloader` default false, `external_downloader_selection` default `""`): `AnimeDownloader.kt:442` and `AnimeDownloader.kt:726` (`pkgName`). Android intent to another app. Dropped on Linux, see T-09a-022.

## 5. Auto-download filter (FilterChaptersForDownload / FilterEpisodesForDownload)

### 5.1 Entry point `await(manga, newChapters)` (OBSERVED, FilterChaptersForDownload.kt:29-44)
```
if newChapters is empty
   OR !downloadNewChapters()        // pref download_new (default false)
   OR !manga.shouldDownloadNewChapters()
    return []
if !downloadNewUnreadChaptersOnly() // pref download_new_unread_chapters_only (default false)
    return newChapters
readNumbers = getChaptersByMangaId(manga.id)
    .filter { it.read && it.isRecognizedNumber }
    .map { it.chapterNumber }  .toSet()
return newChapters.filterNot { it.chapterNumber in readNumbers }
```
Anime twin (FilterEpisodesForDownload.kt:29-44): `downloadNewEpisodes()` (`download_new_episode`), `downloadNewUnseenEpisodesOnly()` (`download_new_unread_episodes_only`), `seen` instead of `read`, `episodeNumber` instead of `chapterNumber`.

Notes:
- `isRecognizedNumber` = `chapterNumber >= 0f` (Chapter.kt:19-20); `episodeNumber >= 0f` (Episode.kt:27-28). Unrecognized numbers are excluded from the "already read" set, so an unrecognized read chapter never suppresses a new chapter (intended).
- The unread-only filter compares **numbers**, not ids: if the source re-numbers an already-read chapter as a new chapter with the same number, it is suppressed (observed; kept for parity, Q-DL-1).
- `newChapters` items with unrecognized numbers (`< 0`) are *not* in the set, so they are always kept when unread-only is on, unless their own number equals a read number, which is impossible for negatives.

### 5.2 `shouldDownloadNewChapters()` decision table (OBSERVED, FilterChaptersForDownload.kt:52-67)
Input: `favorite` flag of the manga; `categories` = `getCategories.await(id).map { it.id }` and, if empty, `listOf(0L)` (`DEFAULT_CATEGORY_ID = 0L`, UNCATEGORIZED_ID is also 0L, `domain/src/main/java/tachiyomi/domain/category/model/Category.kt:16`); `included` = `downloadNewChapterCategories()` mapped to Long; `excluded` = `downloadNewChapterCategoriesExclude()` mapped to Long.

| Step | Condition | Result |
|---|---|---|
| 1 | `!favorite` | false |
| 2 | `included.isEmpty() && excluded.isEmpty()` | true (download from all categories) |
| 3 | `categories.any { it in excluded }` | false (excluded wins over included) |
| 4 | `included.isEmpty()` | true (only exclusions configured, manga not excluded) |
| 5 | else | `categories.any { it in included }` |

Category strings are stored as decimal strings in a StringSet; `toLong()` is used without a null check, so a non-numeric entry throws `NumberFormatException` (Q-DL-2; Proposal: `filter_map(parse().ok())`).

### 5.3 Consumers that are outside this scope
- Called from the library-update job (area 26) after new items are discovered; the returned list is then queued to the downloader (area 23/24).
- `downloadNewChapterCategories` and friends are edited in the settings screen (`SettingsDownloadScreen.getAutoDownloadGroup`, not in scope).

## 6. Extension repositories (manga)

### 6.1 Add repository: `CreateMangaExtensionRepo.await(indexUrl)` (OBSERVED, CreateMangaExtensionRepo.kt:17-25)
1. `indexUrl.toHttpUrlOrNull()` (OkHttp parsing, normalizes host/case); if null → `Result.InvalidUrl`.
2. `.toString()` then `takeIf { it.matches(repoRegex) }` where `repoRegex = """^https://.*/index\.min\.json$"""` (line 15). Mismatch → `InvalidUrl`. Note: `http://` is rejected; a `.` in `.*` matches anything including `/`.
3. `baseUrl = formatted.removeSuffix("/index.min.json")`.
4. `service.fetchRepoDetails(baseUrl)`: GET `"$repo/repo.json"` (ExtensionRepoService.kt:25); any exception → `null` → `InvalidUrl` (ExtensionRepoService.kt:30-33; CreateMangaExtensionRepo.kt:24).
5. `insert(repo)` → `repository.insertRepo(baseUrl, name, shortName, website, signingKeyFingerprint)`.
   - On `SaveExtensionRepoException` (wraps `SQLiteException`): `handleInsertionError(repo)`:
     a. `getRepo(repo.baseUrl)` non-null → `RepoAlreadyExists`.
     b. `getRepoBySigningKeyFingerprint(fp)` non-null → `DuplicateFingerprint(oldRepo = that row, newRepo = repo)`.
     c. else → `Error`.
   - Other exceptions are **not caught** here and propagate (Q-REPO-1).
   - Success → `Result.Success`.
6. The UI dialog handles `DuplicateFingerprint` by offering `ReplaceMangaExtensionRepo.await(newRepo)`.

### 6.2 Replace: `ReplaceMangaExtensionRepo.await(repo)` (OBSERVED)
- Calls `repository.replaceRepo(repo)` → SQL `ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name = :name, short_name = :short_name, website = :website WHERE signing_key_fingerprint = signing_key_fingerprint` (`data/src/main/sqldelight/data/extension_repos.sq:43-53`). Effect: the existing row with that fingerprint takes the new base_url; the old base_url row is therefore gone. Exceptions propagate (no try/catch in the interactor, ReplaceMangaExtensionRepo.kt:9-11).

### 6.3 Refresh: `UpdateMangaExtensionRepo.awaitAll()` / `await(repo)` (OBSERVED, UpdateMangaExtensionRepo.kt:15-29)
- `awaitAll()`: `coroutineScope { repository.getAll().map { async { await(it) } }.awaitAll() }` — all repos in parallel, unbounded concurrency.
- `await(repo)`: `newRepo = service.fetchRepoDetails(repo.baseUrl) ?: return` (silent no-op on failure). Update only if `repo.signingKeyFingerprint.startsWith("NOFINGERPRINT") || repo.signingKeyFingerprint == newRepo.signingKeyFingerprint`; then `upsertRepo(newRepo)` (ON CONFLICT(base_url)). A fingerprint change is **silently ignored** (the repo keeps its old metadata) — this is the TOFU guard against a hijacked repo.
- Quirk Q-REPO-2: an exception in one `async` (e.g. `SaveExtensionRepoException` from upsert) cancels the `coroutineScope` and rethrows, cancelling sibling refreshes. Proposal: per-repo `Result`, collect errors, continue (see E-REPO-06).

### 6.4 Delete / get / count (OBSERVED)
- Delete: `DELETE FROM extension_repos WHERE base_url = :base_url` (extension_repos.sq:55-57). Extensions installed from the repo are not removed here (area 18).
- `GetMangaExtensionRepo.subscribeAll()` / `getAll()`; `GetMangaExtensionRepoCount.subscribe()` = `SELECT COUNT(*)` as `Flow<Int>` (MangaExtensionRepoRepositoryImpl.kt:32-34).

### 6.5 Upsert SQL (OBSERVED, extension_repos.sq:31-41)
`ON CONFLICT(base_url) DO UPDATE SET name, short_name, website, signing_key_fingerprint` — `WHERE base_url = base_url` (always true for the conflicting row). If the new fingerprint collides with another row's UNIQUE fingerprint, SQLite raises a constraint error → `SaveExtensionRepoException` (Q-REPO-3).

## 7. Extension stores (anime)

### 7.1 Add store: `AddAnimeExtensionStore.invoke(indexUrl)` (OBSERVED)
→ `repository.insert(indexUrl)` = `service.fetch(indexUrl).mapCatching { upsert(it) }` (AnimeExtensionStoreRepositoryImpl.kt:19-21).

`AnimeExtensionStoreService.fetch(indexUrl)` (`data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt:28-68`):
1. GET `indexUrl` (`awaitSuccess`), body `decompressIfGzipped()` (gzip magic `0x1f8b` peeked as Short; OBSERVED lines 111-120).
2. Peek first byte:
   - `0x5B` (`[`): legacy **list**. Requires `indexUrl.endsWith("/index.min.json")`, else `IllegalArgumentException("Provided legacy store url is not valid")`. Sets `updatedIndexUrl = indexUrl.replace("/index.min.json", "/repo.json")`, GETs it and decodes `NetworkLegacyAnimeExtensionRepo`.
   - `0x7B` (`{`): try decode `NetworkLegacyAnimeExtensionRepo`; on `IllegalArgumentException` decode `NetworkAnimeExtensionStore` (current format).
   - other: protobuf `NetworkAnimeExtensionStore`.
3. If the decoded value is a `NetworkLegacyAnimeExtensionRepo` with `indexV2 != null` → recurse `fetch(networkStore.indexV2)`.
4. `networkStore.toExtensionStore(updatedIndexUrl)`.
5. Errors: `CancellationException` rethrown; any other `Exception` logged (`"Failed to add extension store '$updatedIndexUrl'"`) and returned as `Result.failure`.

Proposal: keep the same decision order; the Rust type is `enum StoreWire { LegacyList, LegacyRepo(...), Current(...), Proto(...) }` decided by first byte after gunzip.

### 7.2 Upsert (OBSERVED, AnimeExtensionStoreRepositoryImpl.kt:54-67)
`extension_store` upsert keyed by `index_url`; all columns from the model. `contactDiscord` nullable; `extensionListUrl` nullable.

**Quirk Q-STORE-1:** `upsert` uses `store.indexUrl` from the *decoded* model, which for a legacy store is the rewritten `…/repo.json` URL. If the user added `…/index.min.json`, the row written by `insert` has `index_url = …/repo.json`, a second row can appear beside the original, and `refreshAll` (which fetches `store.index_url`) keeps both. Observed; Proposal: store the user-entered URL as the key and keep the resolved URL in a separate column.

### 7.3 Refresh all (OBSERVED, AnimeExtensionStoreRepositoryImpl.kt:38-52)
- Reads all rows, then **sequentially** for each: `service.fetch(store.index_url).mapCatching { upsert(it) }.onFailure { logcat(ERROR) { "Failed to refresh extension store '${store.name} (${store.index_url})'" } }`.
- Outer `try/catch(Exception)` logs and returns.
- Implementation note: `UpdateAnimeExtensionStores.invoke()` just calls `refreshAll()` (UpdateAnimeExtensionStores.kt:8-10).

### 7.4 Fetch extensions across stores (OBSERVED, AnimeExtensionStoreRepositoryImpl.kt:69-88)
- `supervisorScope { getAll(extensionStoreMapper).map { async { service.getExtensions(store).onFailure { log } } }.awaitAll().flatMap { it.getOrDefault(emptyList()) } }` — parallel; a failing store contributes nothing; the supervisor prevents cancellation of siblings.
- Outer `try/catch(Exception)` → `emptyList()`.
- `AnimeExtensionStoreService.getExtensions(store)` (AnimeExtensionStoreService.kt:70-109):
  a. `store.extensionListUrl != null` → GET it; decode `NetworkAnimeExtensionStore.ExtensionList` (JSON if first byte `{`, else protobuf); `toAvailableExtensions(store)`.
  b. else if `!store.isLegacy` → GET `store.indexUrl`; decode `NetworkAnimeExtensionStore`; `.extensionList!!` (NPE if absent; caught as failure); `toAvailableExtensions(store)`.
  c. else (legacy) → `storeBaseUrl = store.indexUrl.removeSuffix("/repo.json")`; GET `"$storeBaseUrl/index.min.json"`; decode `List<NetworkLegacyAnimeExtension>`; map `toAvailableExtension(store, storeBaseUrl)`.
  Gzip handling identical to fetch.

### 7.5 Insert from preference (OBSERVED, AnimeExtensionStoreRepositoryImpl.kt:23-36)
Migration path for an old preference-based store entry: `upsert(indexUrl, name, badgeLabel = name, signingKey = "NO_SIGNING_KEY", contactWebsite = indexUrl, contactDiscord = null, isLegacy = false, extensionListUrl = null)`. No network call.

### 7.6 Remove (OBSERVED)
`DELETE FROM extension_store WHERE index_url = :indexUrl` (extension_store.sq:42-44). No cascade to installed extensions.

## 8. Custom buttons

### 8.1 Placeholder substitution (OBSERVED, CustomButton.kt:12-25)
For `getButtonContent(primaryId)`, `getButtonLongPressContent(primaryId)`, `getButtonOnStartup(primaryId)`:
```
isPrimary = if (primaryId == id) "true" else "false"
result = field.replace("$id", id.toString()).replace("$isPrimary", isPrimary)
```
Order matters: `$id` is replaced first. `$isPrimary` contains `$i` but not `$id` (no collision). Quirk Q-CB-1: `$identity` becomes `<id>entity` (plain text replacement, no word boundary). Proposal: keep literal replacement for parity; document the placeholder syntax in the settings help.

### 8.2 Seed (OBSERVED, custom_buttons.sq:13-30)
`INSERT OR IGNORE` row `_id = 1`, `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`, with the three Lua snippets quoted in `p1-data-and-config.md` section 3.3. Re-running the migration does nothing because of `OR IGNORE` on the primary key.

### 8.3 Create (OBSERVED, CreateCustomButton.kt:11-33)
1. Inside `withNonCancellableContext`: `customButtons = getAll()`; `nextSortIndex = customButtons.maxOfOrNull { it.sortIndex }?.plus(1) ?: 0`.
2. `insertCustomButton(name, nextSortIndex, content, longPressContent, onStartup)` — repository inserts `isFavorite = false` (CustomButtonRepositoryImpl.kt:23-35).
3. `Success`, or `InternalError(e)` after `logcat(ERROR)`.
- Race: there is no mutex on create, so two concurrent creates can read the same max and write duplicate `sortIndex` values (Q-CB-2). Proposal: take the same mutex as reorder.

### 8.4 Update (OBSERVED, UpdateCustomButton.kt:10-25)
- Single: `updatePartialCustomButton(update)` → `UPDATE … SET col = coalesce(:param, col) WHERE _id = :id` for all 6 mutable columns (custom_buttons.sq:45-53). Errors → `InternalError` (not logged, Q-CB-3).
- Batch: `updatePartialCustomButtons(list)` in one transaction (CustomButtonRepositoryImpl.kt:43-49).

### 8.5 Delete (OBSERVED, DeleteCustomButton.kt:12-35)
1. `deleteCustomButton(id)` (`DELETE … WHERE _id = :id`); failure → `InternalError`.
2. `getAll()` (ordered by `sortIndex`, custom_buttons.sq:32-35) → `updates = mapIndexed { i, b -> CustomButtonUpdate(id = b.id, sortIndex = i) }`.
3. batch update in a transaction; failure → `InternalError`. A failure in step 3 leaves a gap in sortIndex (the delete already committed). Gaps are harmless because only ordering matters.

### 8.6 Reorder (OBSERVED, ReorderCustomButton.kt:17-45)
Under `mutex` and `withNonCancellableContext`:
1. `customButtons = getAll().toMutableList()`.
2. `currentIndex = indexOfFirst { it.id == customButton.id }`; `-1` → `Result.Unchanged` (no write).
3. `customButtons.add(newIndex, customButtons.removeAt(currentIndex))`. `removeAt` shrinks the list to n-1, then `add(newIndex)` requires `0 <= newIndex <= n-1`. Any other value throws `IndexOutOfBoundsException` inside the `try`, which is caught → `InternalError(e)` (Q-CB-4). Valid `newIndex = n-1` moves to the end.
4. `updates = mapIndexed { index, b -> CustomButtonUpdate(id = b.id, sortIndex = index) }`; batch update → `Success`.

### 8.7 Toggle favorite (OBSERVED, ToggleFavoriteCustomButton.kt:17-44)
- If `customButton.isFavorite` (currently favorite): single update `isFavorite = false` for that id (no mutex, not in the `else` branch). Result: no favorite remains.
- Else (not favorite): under `mutex`: `getAll()`; build `updates` for **every** button `isFavorite = (btn.id == customButton.id)`; batch update. Result: exactly this button is favorite.
- Edge: if the id does not exist, all buttons are set to `isFavorite = false` and the result is `Success` (no error). Proposal: return `Unchanged` when the id is absent.
- Invariant (observed, not enforced by schema): at most one favorite. Observed reads (`CustomButton` flows) can show zero favorites after a delete of the favorite button. The primary button for the player is the favorite (area 24/46 consumer; Proposal: fall back to lowest `sortIndex`).

### 8.8 Read (OBSERVED)
`findAll` ordered by `sortIndex` (ties undefined; Proposal: `ORDER BY sortIndex, _id`).

## 9. Storage

### 9.1 Base directory preference (OBSERVED)
- Key `Preference.appStateKey("storage_dir")` = `"__APP_STATE_storage_dir"` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`, prefix `__APP_STATE_`). Default `folderProvider.path()` (`core/common/src/main/java/tachiyomi/core/common/storage/AndroidStorageFolderProvider.kt:22`). Linux replacement default: `$XDG_DATA_HOME/kurobayomi` (fallback `~/.local/share/kurobayomi`), see `p1-rust-design.md` section 4.12.

### 9.2 Initialization and change flow (OBSERVED, StorageManager.kt:25-65)
1. `baseDir = getBaseDir(prefs.get())`: `UniFile.fromUri(context, uri).takeIf { it?.exists() == true }` (lines 56-59). A missing folder yields `null`; every accessor then returns `null`.
2. `_changes: Channel<Unit>(UNLIMITED)`; `changes = _changes.receiveAsFlow().shareIn(scope, SharingStarted.Lazily, 1)` (replay 1).
3. `init`: `prefs.changes().drop(1).distinctUntilChanged().onEach { uri -> … }.launchIn(scope)`:
   - `drop(1)` skips the current value, so **the directory tree is not created at app start**; it is created on change or lazily on first accessor call.
   - `baseDir = getBaseDir(uri)`; if non-null: `createDirectory` for `autobackup`, `local`, `localanime`, `downloads` (then `DiskUtil.createNoMediaFile(it, context)`), and `mpv-config` (then `fonts`, `scripts`, `script-opts`, `shaders` inside it). Each `createDirectory` returns the existing or new folder.
   - `_changes.send(Unit)` (always, even if `baseDir` is null).

### 9.3 Accessors (OBSERVED, StorageManager.kt:61-95)
Each accessor is `baseDir?.createDirectory(NAME)` (or via `getMPVConfigDirectory()`), so **every call creates the directory if missing** and returns null when the base is unavailable:
| Accessor | Path under base |
|---|---|
| `getAutomaticBackupsDirectory` | `autobackup` |
| `getDownloadsDirectory` | `downloads` |
| `getLocalMangaSourceDirectory` | `local` |
| `getLocalAnimeSourceDirectory` | `localanime` |
| `getMPVConfigDirectory` | `mpv-config` |
| `getFontsDirectory` | `mpv-config/fonts` |
| `getScriptsDirectory` | `mpv-config/scripts` |
| `getScriptOptsDirectory` | `mpv-config/script-opts` |
| `getShadersDirectory` | `mpv-config/shaders` |
Constants: `AUTOMATIC_BACKUPS_PATH="autobackup"`, `DOWNLOADS_PATH="downloads"`, `LOCAL_SOURCE_PATH="local"`, `LOCAL_ANIMESOURCE_PATH="localanime"`, `MPV_CONFIG_PATH="mpv-config"`, `FONTS_PATH="fonts"`, `SCRIPTS_PATH="scripts"` (public), `SCRIPT_OPTS_PATH="script-opts"` (public), `SHADERS_PATH="shaders"`.

Note the mismatch Q-STO-1: `getLocalMangaSourceDirectory` uses folder `local`, and anime uses `localanime`. Both are kept (user data on disk).

### 9.4 Error handling (OBSERVED)
- An invalid or unreachable base → `baseDir = null`, accessors return null, no exception. Callers (downloader, backup, local source) must handle null; they show an error in their own areas.
- `createDirectory` failures are not caught in `init` (an exception there would cancel the `launchIn` collector). Proposal: catch, log, and emit a `StorageError` on the changes channel.

## 10. Backup preferences (OBSERVED)
- `backupInterval()` = `preferenceStore.getInt("backup_interval", 12)` (BackupPreferences.kt:10). Consumer `BackupCreateJob.kt:92`: `prefInterval ?: backupPreferences.backupInterval().get()`. Migration `EnableAutoBackupMigration.kt:17-18`: if the value is `0` it is set to `12` (so `0` means auto-backup disabled; the enabling switch sets 12).
- `lastAutoBackupTimestamp()` = `getLong(Preference.appStateKey("last_auto_backup_timestamp"), 0L)` (BackupPreferences.kt:12). Written by `BackupCreator.kt:150` with `Instant.now().toEpochMilli()`; displayed in `SettingsDataScreen.kt:195`.
- `backupFlags()` = `getStringSet("backup_flags", setOf("1","2","4","8"))` (BackupPreferences.kt:14-17). **No caller exists outside the domain module** (grep of the whole tree for `backupFlags` finds only the declaration). Decision: keep the key for format parity; the Linux UI in area 21 may use it, see T-09a-053.
- Flag tags (PreferenceValues.kt:3-9): `"1"` categories, `"2"` chapters, `"4"` history, `"8"` track, `"10"` settings, `"20"` extension settings, `"40"` extensions. They are compared as opaque strings; the decimal-looking values are *not* parsed as numbers (Q-BK-1).

## 11. Application release check

### 11.1 Entry `await(arguments)` (OBSERVED, GetApplicationRelease.kt:19-46)
1. `now = Instant.now()`.
2. Throttle: if `!forceCheck && now.isBefore(Instant.ofEpochMilli(lastChecked.get()).plus(3, DAYS))` → `NoNewUpdate` (no network).
3. `release = service.latest(arguments) ?: return NoNewUpdate`. **Note:** `lastChecked` is *not* updated on this path, so the next call retries immediately (observed).
4. `lastChecked.set(now.toEpochMilli())` (even when no update is found).
5. `isNewVersion(...)` true → `NewUpdate(release)`, else `NoNewUpdate`.
- `OsTooOld` is declared but never produced (the producer is commented out in `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt:17`; the UI branch at `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:225` is therefore dead). Drop (T-09a-046).
- Preference key: `Preference.appStateKey("last_app_check")` = `__APP_STATE_last_app_check`, default `0` (GetApplicationRelease.kt:15-17).

### 11.2 Version comparison `isNewVersion` (OBSERVED, GetApplicationRelease.kt:48-76)
```
newVersion = versionTag.replace(Regex("[^\\d.]"), "")          // strips r/v prefix
if isPreview:
    return newVersion.toInt() > commitCount                     // "r1234" -> 1234
else:
    oldVersion = versionName.replace(Regex("[^\\d.]"), "")
    newSemVer = newVersion.split(".").map(toInt)
    oldSemVer = oldVersion.split(".").map(toInt)
    for index in oldSemVer.indices:                              // iterates OLD only
        if newSemVer[index] > oldSemVer[index]: return true      // may throw IOOBE
    return false
```
Observed defects (all in the current code):
- Q-REL-1: not lexicographic. `old=1.0.0`, `new=0.9.9` → index 0: `0 > 1` false; index 1: `9 > 0` true → **true** (a downgrade is reported as an update).
- Q-REL-2: extra segments in the new tag are ignored. `old=1.2`, `new=1.2.1` → false.
- Q-REL-3: `newSemVer` shorter than `oldSemVer` (`old=1.2.3`, `new=1.2`) → `newSemVer[2]` throws `IndexOutOfBoundsException`, uncaught in `await` → the check fails with an exception.
- Q-REL-4: preview path `toInt()` throws for a tag such as `r1.2`.
- Proposal (T-09a-043): compare as `semver::Version`, parsing the tag after stripping a leading `v`; preview: parse digits as `u64`, return `None` (no update) on parse failure with a logged warning.

### 11.3 Latest release fetch (OBSERVED, ReleaseServiceImpl.kt:18-43)
- GET `https://api.github.com/repos/${arguments.repository}/releases/latest` → `GithubRelease` (`tag_name` → `version`, `body` → `info`, `html_url` → `releaseLink`, `assets[]` of `{name, browser_download_url}`) (`data/src/main/java/tachiyomi/data/release/GithubRelease.kt:9-29`).
- `info`: each GitHub mention `@username` matching `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` (IGNORE_CASE; ReleaseServiceImpl.kt:58-59) is rewritten to `[@username](https://github.com/username)`.
- Download link: `BUILD_TYPES = ["arm64-v8a", "armeabi-v7a", "x86_64", "x86"]`; map each asset name to the first BUILD_TYPE whose `"-$type"` is a substring (`find` returns null when none); choose `map[Build.SUPPORTED_ABIS[0]] ?: map[null]` (ReleaseServiceImpl.kt:37-43). Linux: match on `std::env::consts::ARCH` (`x86_64`, `aarch64`) and the packaging suffix chosen in T-09a-042.
- If no asset matches, `latest` returns `null` → `NoNewUpdate` without updating `lastChecked`.

## 12. Messages and strings (consumer side)

The scope files contain no `MR.strings.*` / `AYMR.strings.*` references. The keys below are the consumer strings for the settings this scope defines (observed in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt`), listed so Kurobayomi's settings labels can match:
- `AYMR.strings.pref_download_slots`, `AYMR.strings.download_slots_info` (slots).
- `AYMR.strings.pref_category_delete_chapters` (group), `AYMR.strings.pref_remove_after_marked_as_read`, `AYMR.strings.pref_remove_after_read`, `MR.strings.disabled`, `MR.strings.last_read_chapter`, `MR.strings.second_to_last`, `MR.strings.third_to_last`, `MR.strings.fourth_to_last`, `MR.strings.fifth_to_last`.
- `MR.strings.download_ahead` (group), `AYMR.strings.auto_download_while_watching` (watching preference title), plurals `AYMR.plurals.next_unseen_episodes` and `MR.plurals.next_unread_chapters` (the reading preference title is set in the block at SettingsDownloadScreen.kt:332-340, not captured in this pass; Kurobayomi uses its own label "Download ahead while reading").
- Remaining labels (wifi, CBZ, split tall images, bookmarked, filler, external downloader) are in the same file and belong to area 23/24 settings.
No user-visible string is produced by a scope interactor itself; errors surface as `Result` values to the caller.

## 13. Concurrency, timings and limits (summary)

| Item | Value | Where |
|---|---|---|
| Release check throttle | 3 days (`plus(3, ChronoUnit.DAYS)`) | GetApplicationRelease.kt:20-25 |
| Repo refresh concurrency | unbounded (`async` per repo) | UpdateMangaExtensionRepo.kt:15-19 |
| Extension store refresh | sequential | AnimeExtensionStoreRepositoryImpl.kt:40-48 |
| Extension store fetch-all | parallel, supervisorScope | AnimeExtensionStoreRepositoryImpl.kt:72-82 |
| Storage scope | `Dispatchers.IO`, unlimited channel | StorageManager.kt:23,27 |
| Custom-button mutex | one per interactor instance (Reorder, Toggle) | ReorderCustomButton.kt:15, ToggleFavoriteCustomButton.kt:15 |
| Download slots | 1..5 (UI) | SettingsDownloadScreen.kt:102-104 |
| Download-ahead | 0, 2, 3, 5, 10 | SettingsDownloadScreen.kt:319-340 |
| Download speed | KiB/s, ×1024 bytes/s | MangaDownloader.kt:525 |
| Backup interval default | 12 | BackupPreferences.kt:10 |
| Remove-after-read slots | -1..4 | SettingsDownloadScreen.kt:139-147 |
| Repo URL regex | `^https://.*/index\.min\.json$` | CreateMangaExtensionRepo.kt:15 |
| Fingerprint placeholder | `NOFINGERPRINT` prefix | UpdateMangaExtensionRepo.kt:24 |
| Default category id | 0 (`UNCATEGORIZED_ID`) | Category.kt:16 |

## 14. Interactions with other areas (contracts this scope relies on)

- **area:07-domain-entries-items** owns `Chapter.isRecognizedNumber`, `Episode.isRecognizedNumber`, `GetChaptersByMangaId`, `GetEpisodesByAnimeId` (used by the filters).
- **area:08-domain-library-category-history-updates** owns `GetMangaCategories`, `GetAnimeCategories`, `Category.UNCATEGORIZED_ID`.
- **area:05/06** own the SQL behind `MangaExtensionRepoRepositoryImpl`, `AnimeExtensionStoreRepositoryImpl`, `CustomButtonRepositoryImpl`, the track repositories, the `animesources`/`mangas` queries for non-library listings.
- **area:12-network-stack** owns `NetworkHelper`, `GET`, `awaitSuccess`, `parseAs`.
- **area:13-core-util-storage-archive-torrent** owns `PreferenceStore`, `Preference.appStateKey`, `FolderProvider`, `DiskUtil`, `UniFile`.
- **area:21 / 22** own backup create/restore and consume `BackupPreferences` and the flag tags.
- **area:23 / 24** own the downloader, job constraints (wifi), throttler, CBZ writer, remove-after-read execution.
- **area:27-tracking-core** owns tracker logic and the `update` SQL for `manga_sync` / `anime_sync`.
- **area:26-library-update-jobs** calls the download filters after discovering new items.
- Upstream areas for the Kotlin source-api types (`SManga`, `SAnime`, `CatalogueSource`, `AnimeHttpSource`, `FetchType`, `FilterList`, `AnimeFilterList`) are referenced by name only.
