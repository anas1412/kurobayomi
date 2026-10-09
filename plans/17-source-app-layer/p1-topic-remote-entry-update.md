# Topic: remote entry update (UpdateAnimeFromRemote)

Scope file: `mihon/domain/source/interactor/UpdateAnimeFromRemote.kt` (`app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt`, 215 lines). Models: `app/src/main/java/mihon/domain/source/models/RemoteAnimeEpisodeUpdate.kt`, `RemoteAnimeSeasonUpdate.kt`.

This is the one function that turns a source's view of an anime into database writes: metadata, cover and background cache invalidation, episode list sync, and season list sync. Every call site is listed in section 9.

## 1. Constructor and dependencies (OBSERVED, `UpdateAnimeFromRemote.kt:27-35`)

```kotlin
class UpdateAnimeFromRemote(
    private val sourceManager: AnimeSourceManager,
    private val episodeRepository: EpisodeRepository,
    private val animeRepository: AnimeRepository,
    private val syncEpisodesWithSource: SyncEpisodesWithSource,
    private val syncSeasonsWithSource: SyncSeasonsWithSource,
    private val coverCache: AnimeCoverCache,
    private val backgroundCache: AnimeBackgroundCache,
)
```

DI wiring: `app/src/main/java/eu/kanade/domain/DomainModule.kt:419` `addFactory { UpdateAnimeFromRemote(get(), get(), get(), get(), get(), get(), get()) }` (a new instance per injection).

Rust: `RemoteUpdater` (`p1-rust-design.md` section 9) takes `Arc<SourceRegistry>`, the episode and anime repositories (area 05/07), the sync services (area 07), and the cover and background caches (area 13).

## 2. Public entry points (OBSERVED)

| Function | Lines | Parameters (defaults) | Source resolution |
|---|---|---|---|
| `awaitEpisodesUpdate(anime, fetchDetails=false, fetchEpisodes=false, manualFetch=false, fetchWindow=Pair(0,0))` | `:36-52` | `source = sourceManager.getOrStub(anime.source)` | registry lookup, stub if not installed |
| `awaitEpisodesUpdate(source, anime, fetchDetails=false, fetchEpisodes=false, manualFetch=false, fetchWindow=Pair(0,0))` | `:54-87` | explicit source | none |
| `awaitSeasonsUpdate(anime, fetchDetails=false, fetchSeasons=false, manualFetch=false, fetchWindow=Pair(0,0))` | `:89-105` | `getOrStub(anime.source)` | registry |
| `awaitSeasonsUpdate(source, anime, fetchDetails=false, fetchSeasons=false, manualFetch=false, fetchWindow=Pair(0,0))` | `:107-140` | explicit | none |

Return type: `Result<RemoteAnimeEpisodeUpdate>` or `Result<RemoteAnimeSeasonUpdate>` (Kotlin `kotlin.Result`).

Rust: one function per kind with a `SourceHandle` argument (already resolved by the caller) and the same flags.

## 3. Episode update algorithm (OBSERVED, `:54-87`)

```
awaitEpisodesUpdate(source, anime, fetchDetails, fetchEpisodes, manualFetch, fetchWindow):
  try:
    episodes := episodeRepository.getEpisodeByAnimeId(anime.id) sorted ascending by sourceOrder     (:63-64)
    update := withIOContext {                                                                       (:65-72)
        source.getAnimeEpisodeUpdate(
            anime = anime.toSAnime(),
            episodes = episodes.map(Episode::toSEpisode),
            fetchDetails = fetchDetails,
            fetchEpisodes = fetchEpisodes)
    }                                                                                               -> SAnimeEpisodeUpdate(anime: SAnime, episodes: List<SEpisode>)
    awaitUpdateFromSource(anime, update.anime, manualFetch)      // return value DISCARDED (:73)
    newEpisodes := syncEpisodesWithSource.await(                                                   (:74-80)
        rawSourceEpisodes = update.episodes,
        anime = anime,                     // the ORIGINAL anime object, not the freshly updated one
        source = source,
        manualFetch = manualFetch,
        fetchWindow = fetchWindow)
    updatedAnime := animeRepository.getAnimeById(anime.id)                                         (:81)
    return success(RemoteAnimeEpisodeUpdate(anime = updatedAnime, newEpisodes = newEpisodes))      (:82)
  catch (e: Exception):
    logcat(LogPriority.ERROR, e)
    return failure(e)                                                                             (:83-86)
```

Observed consequences:
1. The episode sync receives the stale `anime` object. Fields read by the sync (`fetchInterval`, `nextUpdate`, `favorite`, `source`) are the pre-update values. Keep this behaviour for `fetchInterval` and `nextUpdate` (they are the scheduling fields and must not be read from the write that just happened); Proposal: pass the stale object explicitly and document why.
2. A failed metadata write does not stop the episode sync (the `Boolean` is dropped at `:73`). Proposal: check it and return `RemoteError::Db` (the sync is then not run). This is a deliberate change; the failure is otherwise invisible.
3. `SyncEpisodesWithSource.await` throws `NoEpisodesException` when `rawSourceEpisodes` is empty and the source is not local (`app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt:54-55`). That exception becomes `Result.failure`. The UI shows it (screen owner).
4. The re-read on `:81` is the only place where the returned anime carries the new metadata.

## 4. Season update algorithm (OBSERVED, `:107-140`)

Same shape:
- `seasons := animeRepository.getAnimeSeasonsById(anime.id)` sorted ascending by `it.anime.seasonSourceOrder` (`:116-117`).
- `source.getAnimeSeasonUpdate(anime = anime.toSAnime(), seasons = seasons.map { it.anime.toSAnime() }, fetchDetails, fetchSeasons)` (`:118-125`).
- `awaitUpdateFromSource(anime, update.anime, manualFetch)` (`:126`), result discarded.
- `syncSeasonsWithSource.await(rawSourceSeasons = update.seasons, anime, source, manualFetch, fetchWindow)` (`:127-133`). Empty list and not local → `NoSeasonsException` (`app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt:31-32`).
- `animeRepository.getAnimeById(anime.id)` (`:134`), then `success(RemoteAnimeSeasonUpdate(anime = updatedAnime, newSeasons = newSeasons))` (`:135`).

## 5. Metadata write (`awaitUpdateFromSource`, OBSERVED `:142-214`)

Inputs: `localAnime: Anime`, `remoteAnime: SAnime`, `manualFetch: Boolean`. Output: `Boolean` (from the repository; discarded by callers).

### 5.1 Title

```kotlin
val remoteTitle = try { remoteAnime.title } catch (_: UninitializedPropertyAccessException) { "" }
val title = if (remoteTitle.isNotEmpty() && !localAnime.favorite) remoteTitle else null
```
(`:147-159`.)

Rules:
- `title` is never written for library entries (`favorite == true`). A library entry keeps the user's title.
- A non-library entry takes the remote title whenever the remote title is non-empty, even on a normal refresh.
- `SAnime.title` is a Kotlin `lateinit`-style property: reading it before it was set throws `UninitializedPropertyAccessException`, mapped to `""`.

Rust: `fn title_update(local: &Anime, remote: Option<&str>) -> Option<String>` where `None` = not set, `Some("")` never occurs (empty is mapped to `None` before the call).

### 5.2 Cover last-modified (`:161-175`)

Decision table (evaluated top to bottom; first match wins):

| # | Condition | Result `coverLastModified` | Side effect |
|---|---|---|---|
| 1 | `remoteAnime.thumbnail_url.isNullOrEmpty()` | `null` (keep) | none |
| 2 | `!manualFetch && localAnime.thumbnailUrl == remoteAnime.thumbnail_url` | `null` (keep) | none |
| 3 | `localAnime.isLocal()` (source id == 0) | `Instant.now().toEpochMilli()` | none |
| 4 | `localAnime.hasCustomCover(coverCache)` (custom cover file exists for `id`) | `null` | `coverCache.deleteFromCache(localAnime, false)` |
| 5 | otherwise | `Instant.now().toEpochMilli()` | `coverCache.deleteFromCache(localAnime, false)` |

`deleteFromCache(anime, deleteCustomCover=false)` deletes the cached thumbnail file for `anime.thumbnailUrl` (`app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt:75-80`) and keeps the custom cover (the second argument is `false`).

Note on row 2: `manualFetch = true` bypasses the URL-equality check, so a manual refresh always rewrites the timestamp and deletes the cache (rows 4/5).

### 5.3 Background last-modified (`:177-191`)

Identical table with `background_url`, `localAnime.backgroundUrl`, `hasCustomBackground(backgroundCache)`, and `backgroundCache.deleteFromCache(localAnime, false)` (`AnimeBackgroundCache.kt:75-80`).

Timestamp unit: milliseconds since the Unix epoch (`Instant.toEpochMilli`). Consumed as a cache-busting value by the cover and background loaders (area 07; the consumer is not read in this area).

### 5.4 URL fields (`:193-194`)

`thumbnailUrl = remote.thumbnail_url?.takeIf { it.isNotEmpty() }`; `backgroundUrl` likewise. Null → keep (coalesce).

### 5.5 `AnimeUpdate` (`:196-213`)

Passed to `animeRepository.updateAnime`. Full field rule table is in `p1-data-and-config.md` section 2.12. Summary of unconditional writes (values never null, so always written):
- `status = remote.status.toLong()`; `UNKNOWN (0)` overwrites a previously known status (a source that does not report status clears it).
- `updateStrategy = remote.update_strategy`.
- `initialized = true`.
- `memo = remote.memo` (the source's JSON object; `{}` overwrites).

Conditional writes (null keeps the stored value through `coalesce`): `author`, `artist`, `description`, `genre` (from `getGenres()`; null when the source genre is blank), `thumbnailUrl`, `backgroundUrl`, `title`, cover/background timestamps.

Subtle: `author`, `artist` and `description` are passed through as-is, so a source that returns an EMPTY string overwrites the stored value (only null is kept).

### 5.6 `getGenres` rule (OBSERVED `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:49-52`)

`null` if `genre` is null or blank. Otherwise split on `", "`, trim each, drop blanks, `distinct()` (order preserved, first occurrence kept).

## 6. Error semantics (OBSERVED)

- Any `Exception` in the try block is logged at ERROR and returned as `Result.failure`.
- `CancellationException` is an `Exception`, so cancellation is reported as a failure, and the caller continues. Proposal: `RemoteError::Cancelled` returned without logging as an error; the function never swallows cancellation.
- `NoEpisodesException`, `NoSeasonsException`, source `SourceNotInstalled`, and DB exceptions all surface as `Result.failure`.
- The stub case: `getOrStub` returns a stub; the first call to `getAnimeEpisodeUpdate` throws `AnimeSourceNotInstalledException` (`domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:34-39`). Result: failure, no DB writes at all (the metadata write happens after the source call).

## 7. Concurrency (OBSERVED)

- The source call runs inside `withIOContext` (`tachiyomi.core.common.util.lang.withIOContext`, imported at `UpdateAnimeFromRemote.kt:16`; Dispatchers.IO).
- The DB reads and writes are suspend calls on the repository (SQLDelight handler, in a transaction for `partialUpdateAnime`).
- No locking: two concurrent refreshes of the same anime both run. Last writer wins for metadata; the episode sync is idempotent per the sync's own rules (area 07).

Proposal: a per-anime `tokio::sync::Mutex` in `RemoteUpdater` so two refreshes of one anime serialise (cheap, prevents duplicate cache deletes and duplicate episode inserts).

## 8. Rust implementation sketch (for the author)

```rust
pub fn apply_remote_metadata(local: &Anime, remote: &SAnimeView, flags: FetchFlags, now_ms: i64,
                             has_custom_cover: bool, has_custom_bg: bool) -> (AnimeUpdate, Vec<CacheAction>) {
    let cover = cover_last_modified(local, remote.thumbnail_url.as_deref(), flags.manual_fetch, has_custom_cover, now_ms);
    let bg = background_last_modified(local, remote.background_url.as_deref(), flags.manual_fetch, has_custom_bg, now_ms);
    let update = AnimeUpdate {
        id: local.id,
        title: title_update(local, remote.title.as_deref()),
        cover_last_modified: cover.timestamp(),
        background_last_modified: bg.timestamp(),
        author: remote.author.clone(),
        artist: remote.artist.clone(),
        description: remote.description.clone(),
        genre: remote.genres(),                     // Option<Vec<String>>, section 5.6
        thumbnail_url: non_empty(remote.thumbnail_url.as_deref()),
        background_url: non_empty(remote.background_url.as_deref()),
        status: Some(remote.status as i64),         // always written
        update_strategy: Some(remote.update_strategy),
        initialized: Some(true),
        memo: Some(remote.memo.clone()),            // always written
        ..AnimeUpdate::keep(local.id)
    };
    (update, vec![cover.action(), bg.action()])
}

pub enum CacheAction { Keep, Delete }
```

Ordering inside the write: (1) run `CacheAction::Delete` for cover and background (the file system step), (2) write the row. Kotlin deletes the cache inside the decision (before the write), so a failed write leaves the cache deleted, which is harmless: the next load re-downloads.

## 9. Call sites (OBSERVED, all from grep of `app/src/main/java`)

| Call site | Purpose |
|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt:87` | library refresh (background job) |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeMetadataUpdateJob.kt:41` | metadata-only refresh job |
| `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:150` | details screen refresh |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/season/MigrateSeasonSelectScreenModel.kt:47` | migration season selection |
| `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt:212` | migration search |
| `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreenModel.kt:30` | deep link open |

The job and screen owners are other areas; this area provides the function and the result types.

## 10. Test vectors

See `p1-edge-cases-and-tests.md` section 4 for the full table. Core cases:
1. Remote thumbnail `""`, local thumbnail `"a"` → cover keep, no delete.
2. Remote thumbnail `"b"`, local thumbnail `"b"`, `manual=false` → keep.
3. Same, `manual=true` → now, delete (row 5 because no custom cover).
4. Local source (id 0), remote `"b"` → now, no delete.
5. Favourite anime, remote title `"X"` → title `null`.
6. Non-favourite, remote title `""` → title `null`.
7. Remote `genre = ", ,A, A,B"` → `["A", "B"]`.
8. Remote `genre = null` → genres `null` (keep).
9. Remote `status = 0` → status written `0`.
10. Remote `author = ""` → author written as `""`.
