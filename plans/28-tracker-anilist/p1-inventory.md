# area:28-tracker-anilist, part 1 — inventory

Repo root for every path below: `/home/blackbox/aniyomi-src` (repo-relative paths, Apache-2.0 source, read-only).
Scope command: `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 28-tracker-anilist 1` prints 13 files, 1 510 LOC in total (375+589+61+59+20+83+21+82+17+20+99+26+58; per-file counts equal `wc -l`).

All 13 files were read in full. Callers and callees outside the scope were read where needed for exact behaviour (listed in the behaviour doc, section "Cross-area call sites").

There are no `*Test.kt` files for AniList in the repository (`app/src/test` contains only `mihon/core/migration/MigratorTest.kt`, which does not touch tracking). Test plan is in `p1-edge-cases-and-tests.md`.

## Port-decision legend

- **Port**: behaviour carried over 1:1 into Rust, same constants and wire format.
- **Rewrite**: behaviour carried over, implementation replaced for Rust idioms (e.g. the expiry arithmetic is corrected, see behaviour doc).
- **Replace-with-crate**: Android/Kotlin library replaced (OkHttp → reqwest, kotlinx.serialization → serde, injekt → constructor injection).
- **Drop**: Android-only glue with no desktop equivalent beyond the replacement named in the task.

## Summary table

| # | Repo-relative path | LOC | Declared names (every top-level and nested name) | Purpose | Port decision (reason) | Target Rust module / crate | Priority | Task ids |
|---|---|---|---|---|---|---|---|---|
| 1 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/Anilist.kt | 375 | `class Anilist(id: Long)`; `companion object` with `const val` READING, WATCHING, COMPLETED, ON_HOLD, DROPPED, PLAN_TO_READ, PLAN_TO_WATCH, REREADING, REWATCHING, POINT_100, POINT_10, POINT_10_DECIMAL, POINT_5, POINT_3; private `json`, `interceptor` (lazy), `api` (lazy), `scorePreference`; overrides `supportsReadingDates`, `supportsPrivateTracking`; `init` block; functions `getLogo`, `getLogoColor`, `getStatusListManga`, `getStatusListAnime`, `getStatusForManga`, `getStatusForAnime`, `getReadingStatus`, `getWatchingStatus`, `getRereadingStatus`, `getRewatchingStatus`, `getCompletionStatus`, `getScoreList`, `get10PointScore` (manga and anime overloads), `indexToScore`, `displayScore` (manga and anime overloads), private `add` (manga and anime overloads), `update` (manga and anime overloads), `delete` (manga and anime overloads), `bind` (manga and anime overloads), `searchManga`, `searchAnime`, `refresh` (manga and anime overloads), `login(username, password)`, `login(token)`, `logout`, `saveOAuth`, `loadOAuth` | Tracker facade: status/score mapping, progress-driven status transitions, bind/refresh/delete entry points, login/logout, OAuth persistence | Rewrite (behaviour ported; expiry init and delete-branch bug fixed; GraphQL in Rust) | `kurobayomi-core::tracker::anilist::{tracker, status, score}` | P0 | T-28a-002, T-28a-003, T-28a-004, T-28a-005, T-28a-006, T-28a-008, T-28a-009, T-28a-011, T-28a-012, T-28a-013, T-28a-021, T-28a-024, T-28a-025, T-28a-026, T-28a-027, T-28a-028, T-28a-036, T-28a-043, T-28a-044 |
| 2 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt | 589 | `class AnilistApi(val client: OkHttpClient, interceptor: AnilistInterceptor)`; private `json`, `authClient` (with `rateLimit(permits = 85, period = 1.minutes)`); functions `addLibManga`, `updateLibManga`, `deleteLibManga`, `addLibAnime`, `updateLibAnime`, `deleteLibAnime`, `search`, `searchAnime`, `findLibManga`, `findLibAnime`, `getLibManga`, `getLibAnime`, `createOAuth`, `getCurrentUser`, private `createDate`; `companion object` with `private const val` CLIENT_ID, API_URL, BASE_URL, BASE_MANGA_URL, BASE_ANIME_URL; `fun mangaUrl`, `fun animeUrl`, `fun authUrl(): Uri` | All GraphQL transport: queries, mutations, auth URL, rate limiting | Replace-with-crate (OkHttp → reqwest, kotlinx.serialization → serde) and Rewrite (error handling for GraphQL `errors`) | `kurobayomi-core::tracker::anilist::api` | P0 | T-28a-014, T-28a-015, T-28a-016, T-28a-017, T-28a-019, T-28a-020, T-28a-021, T-28a-022, T-28a-023, T-28a-008, T-28a-011 |
| 3 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistInterceptor.kt | 61 | `class AnilistInterceptor(val anilist: Anilist, private var token: String?) : Interceptor`; private `var oauth: ALOAuth?` (setter multiplies `expires` by 1000 and subtracts 60 000); `override fun intercept(chain)`; `fun setAuth(oauth: ALOAuth?)` | Adds `Authorization: Bearer <token>` and `User-Agent`; throws when not authenticated; expiry check | Rewrite (expiry unit bug, NPE ordering bug) | `kurobayomi-core::tracker::anilist::api::auth` | P0 | T-28a-009, T-28a-010 |
| 4 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistUtils.kt | 59 | top-level `fun MangaTrack.toApiStatus()`, `fun AnimeTrack.toApiStatus()`, `private val preferences: TrackPreferences`, `private fun Double.toApiScore()`, `fun DomainMangaTrack.toApiScore()`, `fun DomainAnimeTrack.toApiScore()` | Local status int → AniList MediaListStatus string; local 100-point score → user score string for display | Port (pure mapping); the `preferences` injection becomes a function parameter | `kurobayomi-core::tracker::anilist::status`, `::score` | P0 | T-28a-007, T-28a-006 |
| 5 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAddEntry.kt | 20 | `data class ALAddEntryResult(data)`, `data class ALAddEntryData(@SerialName("SaveMediaListEntry") entry)`, `data class ALAddEntryEntry(id: Long)` | Response of `SaveMediaListEntry` mutation (add) | Replace-with-crate (serde structs) | `kurobayomi-core::tracker::anilist::dto::add_entry` | P0 | T-28a-037, T-28a-020 |
| 6 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALAnime.kt | 83 | `data class ALAnime(...)` with `fun toTrack(): AnimeTrackSearch`; `data class ALUserAnime(...)` with `fun toTrack(): AnimeTrack`, `private fun toTrackStatus()` | Anime search result model and user-list entry model | Port (mapping) | `kurobayomi-core::tracker::anilist::dto::anime` | P0 | T-28a-018, T-28a-019, T-28a-037 |
| 7 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALFuzzyDate.kt | 21 | `data class ALFuzzyDate(year: Int?, month: Int?, day: Int?)` with `fun toEpochMilli(): Long` | AniList fuzzy date → epoch millis at local start of day; 0 on any failure | Port (chrono replaces java.time) | `kurobayomi-core::tracker::anilist::dto::fuzzy_date` | P0 | T-28a-023, T-28a-037 |
| 8 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALManga.kt | 82 | `data class ALManga(...)` with `fun toTrack(): MangaTrackSearch`; `data class ALUserManga(...)` with `fun toTrack(): MangaTrack`, `private fun toTrackStatus()` | Manga search result model and user-list entry model | Port (mapping) | `kurobayomi-core::tracker::anilist::dto::manga` | P0 | T-28a-018, T-28a-019, T-28a-037 |
| 9 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALOAuth.kt | 17 | `data class ALOAuth(accessToken, tokenType, expires, expiresIn)` with `@SerialName` `access_token`, `token_type`, `expires_in`; top-level `fun ALOAuth.isExpired()` | Persisted OAuth token record and expiry test | Rewrite (expiry semantics corrected) | `kurobayomi-core::tracker::anilist::auth` | P0 | T-28a-009, T-28a-037 |
| 10 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearch.kt | 20 | `data class ALSearchResult(data: ALSearchPage)`, `data class ALSearchPage(@SerialName("Page") page: ALSearchMedia)`, `data class ALSearchMedia(media: List<ALSearchItem>)` | Search response envelope | Port (serde) | `kurobayomi-core::tracker::anilist::dto::search` | P0 | T-28a-016, T-28a-017, T-28a-037 |
| 11 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALSearchItem.kt | 99 | `data class ALSearchItem(...)` with `fun toALManga(): ALManga`, `fun toALAnime(): ALAnime`; `data class ALItemTitle(userPreferred)`; `data class ItemCover(large)`; `data class ALStaff(edges)`; `data class ALStaffEdge(role, id: Int, node)`; `data class ALStaffNode(name)`; `data class ALStaffName(userPreferred?, native?, full?)` with `operator fun invoke(): String?`; `data class ALStudios(edges)`; `data class ALStudiosEdge(isMain, node)`; `data class ALStudiosNode(name)` | Shared media node used by search and user-list responses; staff and studio sub-models | Port (serde; `!!` NPE sites documented as edge cases) | `kurobayomi-core::tracker::anilist::dto::media` | P0 | T-28a-018, T-28a-037 |
| 12 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUser.kt | 26 | `data class ALCurrentUserResult(data: ALUserViewer)`, `data class ALUserViewer(@SerialName("Viewer") viewer: ALUserViewerData)`, `data class ALUserViewerData(id: Int, mediaListOptions: ALUserListOptions)`, `data class ALUserListOptions(scoreFormat: String)` | Viewer query response (user id and score format) | Port (serde) | `kurobayomi-core::tracker::anilist::dto::viewer` | P0 | T-28a-011, T-28a-037 |
| 13 | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/ALUserList.kt | 58 | `data class ALUserListEntryQueryResult(data: ALUserListEntryPage)`, `data class ALUserListEntryPage(@SerialName("Page") page: ALUserListMediaList)`, `data class ALUserListMediaList(mediaList: List<ALUserListItem>)`, `data class ALUserListItem(...)` with `fun toALUserManga(): ALUserManga`, `fun toALUserAnime(): ALUserAnime` | `mediaList` response for a single (user, media) lookup | Port (serde) | `kurobayomi-core::tracker::anilist::dto::user_list` | P0 | T-28a-019, T-28a-037 |

## Per-file notes (what a reader must know without opening the Kotlin)

### 1. Anilist.kt
- The class is a `BaseTracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`) and implements `MangaTracker`, `AnimeTracker`, `DeletableMangaTracker`, `DeletableAnimeTracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt`, `AnimeTracker.kt`, `DeletableMangaTracker.kt`, `DeletableAnimeTracker.kt`). The framework (setRemote*, updateRemote, register) is documented by area:27-tracking-core, not here. This document describes only the AniList overrides and the behaviour they produce.
- Constructed once in `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:28` as `Anilist(ANILIST)` with `ANILIST = 2L`.
- The `init` block reads `anilistScoreType` and on `ClassCastException` (an int stored by API v1) calls `logout()` and deletes the preference.

### 2. AnilistApi.kt
- Every network call is `withIOContext { ... authClient.newCall(POST(API_URL, body = ...)).awaitSuccess() ... }`. `authClient` = base client + `AnilistInterceptor` + rate limiter.
- `search` and `searchAnime` also go through `authClient`, so searching requires a logged-in token.

### 3. AnilistInterceptor.kt
- Holds the only copy of the token used for requests (`token`), separate from the persisted record (`ALOAuth` in `track_token_2`).

### 4. AnilistUtils.kt
- `Double.toApiScore()` reads the score format from `TrackPreferences.anilistScoreType()` at call time, not from the `Anilist` instance.
- The POINT_5 and POINT_3 branches of `toApiScore` are unreachable from `displayScore` (which handles those formats first). Documented as dead code in edge cases.

### 5-13. DTOs
- All DTOs are `@Serializable` (kotlinx.serialization). Field names are the AniList GraphQL field names exactly; Kotlin names differ only where `@SerialName` is given.
- Several DTO classes contain mapping functions that build the app's `MangaTrack`/`AnimeTrack` (`tachiyomi.domain`-free `data.database.models` classes). These mappings are the authoritative conversion from AniList data to local rows.

## Cross-area call sites (outside scope, read for exactness)

| Call site | What it does with AniList | Owner area |
|---|---|---|
| app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt | Calls `refresh`, then `update(track, true)`, persists, removes delayed queue item | area:27-tracking-core |
| app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt | Same for manga; queues to `DelayedMangaTrackingStore` on any exception | area:27-tracking-core |
| app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt | `bind` on register; pushes local progress and start date | area:27-tracking-core |
| app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt | `refresh` for every logged-in tracker | area:27-tracking-core |
| app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2045 | `AniSkipApi().getMalIdFromAL(track.remoteId)` for AniSkip | area:28-tracker-anilist (task T-28a-034) |
| app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt | Direct GraphQL `Media(id:)`/`Media(idMal:)` calls for airing data | area:28-tracker-anilist (task T-28a-035) |
| app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt | OAuth redirect handler (`anilist-auth`) | area:28-tracker-anilist (task T-28a-040) |
| app/src/main/AndroidManifest.xml:250-260 | Intent filter scheme `aniyomi`, host `anilist-auth` | area:28-tracker-anilist (task T-28a-040) |
| app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt:171-180 | Login opens `AnilistApi.authUrl()` in the browser; logout dialog | area:28-tracker-anilist (task T-28a-047) |

## Port summary by Rust crate

| Rust module | Source files it replaces |
|---|---|
| `tracker::anilist::status` | AnilistUtils.kt (status part), Anilist.kt constants and status lists |
| `tracker::anilist::score` | Anilist.kt score functions, AnilistUtils.kt score part |
| `tracker::anilist::auth` | AnilistInterceptor.kt, ALOAuth.kt, Anilist.kt login/logout/saveOAuth/loadOAuth |
| `tracker::anilist::api` | AnilistApi.kt (+ rate limiter) |
| `tracker::anilist::dto::*` | all dto/*.kt |
| `tracker::anilist::service` | Anilist.kt update/bind/refresh/delete logic |
