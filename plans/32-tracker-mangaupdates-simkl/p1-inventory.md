# Area 32: MangaUpdates + Simkl trackers, part 1 - file inventory

Repo root for every path: `/home/blackbox/aniyomi-src` (read-only). Paths below are repo-relative and were produced by `areas.py files 32-tracker-mangaupdates-simkl 1`. Total 22 files, 1,111 LOC in scope (sum of the scope command's LOC column: MangaUpdates 498, Simkl 613).

Abbreviations used in the table: MU = `mangaupdates`, SK = `simkl`, `dto/` = `.../dto/`. "Port" means translate the Kotlin logic to Rust; "Rewrite" means the Kotlin shape is replaced by a Rust-idiomatic one while the wire behaviour is kept; "Replace" means a Linux or crate equivalent replaces the Android/JVM API; "Drop" means not carried over.

Target Rust crate paths follow `plans/27-tracking-core/p1-rust-design.md` section 1: `crates/kuro-track/src/trackers/mangaupdates/` and `crates/kuro-track/src/trackers/simkl/`.

## Table

| # | Repo-relative path | LOC | Top-level declarations (all names) | Purpose | Port decision | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|---|
| 1 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdates.kt | 126 | class `MangaUpdates`; companion object: `READING_LIST`, `WISH_LIST`, `COMPLETE_LIST`, `UNFINISHED_LIST`, `ON_HOLD_LIST`, private `SCORE_LIST`; members `getLogo`, `getLogoColor`, `getStatusListManga`, `getStatusForManga`, `getReadingStatus`, `getRereadingStatus`, `getCompletionStatus`, `getScoreList`, `indexToScore`, `displayScore`, `update`, `delete`, `bind`, `searchManga`, `refresh`, private ext `MangaTrack.copyFrom`, `login`, `restoreSession` | Tracker service entry point; status/score tables; binds MangaTracker + DeletableMangaTracker | Port (rewrite as `MangaUpdatesTracker` implementing the traits) | `kuro-track/src/trackers/mangaupdates/mod.rs` | P1 | T-32a-001, T-32a-002, T-32a-003, T-32a-010, T-32a-013, T-32a-014, T-32a-015, T-32a-017, T-32a-019, T-32a-020, T-32a-022, T-32a-026, T-32a-064 |
| 2 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt | 197 | class `MangaUpdatesApi(interceptor, client)`; private `json`, `authClient`; members `getSeriesListItem`, `addSeriesToList`, `updateSeriesListItem`, `deleteSeriesFromList`, private `getSeriesRating`, private `updateSeriesRating`, `search`, `authenticate`; companion `BASE_URL`, `CONTENT_TYPE` | All HTTP calls to api.mangaupdates.com | Port | `kuro-track/src/trackers/mangaupdates/api.rs` | P1 | T-32a-011, T-32a-012, T-32a-013, T-32a-015, T-32a-016, T-32a-017, T-32a-018, T-32a-019, T-32a-020, T-32a-021, T-32a-060 |
| 3 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesInterceptor.kt | 31 | class `MangaUpdatesInterceptor(mangaUpdates)`; field `token`; members `intercept`, `newAuth` | Adds `Authorization: Bearer` and User-Agent; throws when no token | Port (reqwest default header wrapper) | `kuro-track/src/trackers/mangaupdates/auth.rs` | P1 | T-32a-011, T-32a-014 |
| 4 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUContext.kt | 11 | data class `MUContext(sessionToken, uid)` | Login response context | Port | `kuro-track/src/trackers/mangaupdates/dto.rs` | P1 | T-32a-012, T-32a-013 |
| 5 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUImage.kt | 10 | data class `MUImage(url, height, width)` | Cover image in search record | Port | `.../dto.rs` | P1 | T-32a-012, T-32a-021 |
| 6 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUListItem.kt | 22 | data class `MUListItem(series, listId, status, priority)`; extension `fun MUListItem.copyTo(track: MangaTrack): MangaTrack` | Series list item and its mapping into the track | Port | `.../dto.rs` + `.../mod.rs` (`copy_to`) | P1 | T-32a-012, T-32a-015, T-32a-020 |
| 7 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MULoginResponse.kt | 8 | data class `MULoginResponse(context)` | Login envelope | Port | `.../dto.rs` | P1 | T-32a-012, T-32a-013 |
| 8 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURating.kt | 15 | data class `MURating(rating)`; extension `fun MURating.copyTo(track)` (NOT referenced anywhere in scope; dead code) | Series rating response | Port (struct); the `copyTo` extension is Drop (dead code, documented in p1-behavior section 9) | `.../dto.rs` | P1 | T-32a-012, T-32a-015, T-32a-020 |
| 9 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MURecord.kt | 38 | data class `MURecord(...)`; extension `fun MURecord.toTrackSearch(id: Long): MangaTrackSearch` | Search hit record, mapping to search model | Port | `.../dto.rs` + `.../search.rs` | P1 | T-32a-012, T-32a-021, T-32a-023 |
| 10 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUSearch.kt | 13 | data class `MUSearchResult(results)`, data class `MUSearchResultItem(record)` | Search envelope | Port | `.../dto.rs` | P1 | T-32a-012, T-32a-021 |
| 11 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUSeries.kt | 9 | data class `MUSeries(id, title)` | Series reference in list item | Port | `.../dto.rs` | P1 | T-32a-012 |
| 12 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUStatus.kt | 9 | data class `MUStatus(volume, chapter)` | Progress inside list item | Port | `.../dto.rs` | P1 | T-32a-012, T-32a-015 |
| 13 | app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/dto/MUUrl.kt | 9 | data class `MUUrl(original, thumb)` | Image URLs | Port | `.../dto.rs` | P1 | T-32a-012, T-32a-021 |
| 14 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/Simkl.kt | 150 | class `Simkl(id)`; companion `WATCHING`, `COMPLETED`, `ON_HOLD`, `NOT_INTERESTING`, `PLAN_TO_WATCH`, private `SCORE_LIST`; private `add`; members `getScoreList`, `displayScore`, `update`, `bind`, `searchAnime`, `refresh`, `getLogo`, `getLogoColor`, `getStatusListAnime`, `getStatusForAnime`, `getWatchingStatus`, `getRewatchingStatus`, `getCompletionStatus`, `login(username,password)`, `login(code)`, `saveToken`, `restoreToken`, `logout` | Simkl tracker entry point | Port | `kuro-track/src/trackers/simkl/mod.rs` | P1 | T-32a-001, T-32a-002, T-32a-003, T-32a-030, T-32a-033, T-32a-034, T-32a-041, T-32a-044, T-32a-045, T-32a-046, T-32a-047 |
| 15 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt | 255 | class `SimklApi(client, interceptor)`; private `json`, `authClient`; members `addLibAnime`, private `addToList`, private `updateRating`, private `updateProgress`, private `buildProgressObject`, `updateLibAnime`, `searchAnime`, `findLibAnime`, `getCurrentUser`, `accessToken`, private `accessTokenRequest`; companion `CLIENT_ID`, private `CLIENT_SECRET`, private `BASE_URL`, private `API_URL`, private `OAUTH_URL`, private `LOGIN_URL`, `POSTERS_URL`, private `REDIRECT_URL`, `authUrl()` | All Simkl HTTP calls and OAuth URL | Port | `kuro-track/src/trackers/simkl/api.rs`, `.../oauth.rs` | P1 | T-32a-031, T-32a-032, T-32a-036, T-32a-038, T-32a-039, T-32a-040, T-32a-042, T-32a-043, T-32a-049, T-32a-060 |
| 16 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklInterceptor.kt | 35 | class `SimklInterceptor(val simkl)`; field `oauth`; members `intercept`, `newAuth` | Bearer token + simkl-api-key header | Port | `kuro-track/src/trackers/simkl/auth.rs` | P1 | T-32a-035, T-32a-034 |
| 17 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklUtils.kt | 21 | top-level `fun AnimeTrack.toSimklStatus()`, top-level `fun toTrackStatus(status: String)` | Status <-> Simkl string mapping | Port | `kuro-track/src/trackers/simkl/status.rs` | P1 | T-32a-030 |
| 18 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklOAuth.kt | 10 | data class `SimklOAuth(accessToken)` | Token response and stored token | Port | `kuro-track/src/trackers/simkl/dto.rs` | P1 | T-32a-032, T-32a-034 |
| 19 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSearch.kt | 44 | data class `SimklSearchResult(...)` with `fun toTrackSearch(fallbackType)`; data class `SimlkSearchResultIds(simklId)` (sic: "Siml k") | Search hit mapping | Port | `.../dto.rs` + `.../search.rs` | P1 | T-32a-036, T-32a-037 |
| 20 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSyncItem.kt | 73 | data class `SimklSyncResult(anime, tv, movies)` with `fun getFromType(type)`; data class `SimklSyncItem(show, movie, totalEpisodesCount, watchedEpisodesCount, userRating)` with `fun toAnimeTrack(typeName, type, statusString)` and `fun getFromType(typeName)`; data class `SimklSyncResultItem(title, ids)`; data class `SimklSyncResultIds(simkl)` | all-items payload and mapping to AnimeTrack | Port | `.../dto.rs` + `.../sync.rs` | P1 | T-32a-039, T-32a-044 |
| 21 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklSyncWatched.kt | 12 | data class `SimklSyncWatched(result, lastWatched, list)` | /sync/watched element | Port | `.../dto.rs` | P1 | T-32a-039 |
| 22 | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/dto/SimklUser.kt | 13 | data class `SimklUser(account)`, data class `SimklUserAccount(id)` | /users/settings | Port | `.../dto.rs` | P1 | T-32a-033 |

The table has 22 rows, one per scope path. Row count check: MangaUpdates 13 files (3 top-level + 10 dto) + Simkl 9 files (4 top-level + 5 dto) = 22.

## Declared-name cross-check

- MangaUpdates package: 13 files; 2 classes (`MangaUpdates`, `MangaUpdatesApi`), 1 class (`MangaUpdatesInterceptor`), 10 DTO classes/objects (`MUContext`, `MUImage`, `MUListItem`, `MULoginResponse`, `MURating`, `MURecord`, `MUSearchResult`, `MUSearchResultItem`, `MUSeries`, `MUStatus`, `MUUrl`) = 11 DTO data classes total (MUSearch.kt holds two).
- Simkl package: 9 files; classes `Simkl`, `SimklApi`, `SimklInterceptor`; top-level functions `toSimklStatus` (extension), `toTrackStatus`; DTO classes `SimklOAuth`, `SimklSearchResult`, `SimlkSearchResultIds`, `SimklSyncResult`, `SimklSyncItem`, `SimklSyncResultItem`, `SimklSyncResultIds`, `SimklSyncWatched`, `SimklUser`, `SimklUserAccount`.
- Top-level extension functions: `MUListItem.copyTo`, `MURating.copyTo`, `MURecord.toTrackSearch`, `SimklSearchResult.toTrackSearch` (member), `SimklSyncItem.toAnimeTrack` (member), `AnimeTrack.toSimklStatus`, `toTrackStatus`. `MangaTrack.copyFrom` is a private extension inside `MangaUpdates`.

## External files that define things these files use (read for exactness, not in scope)

These are NOT scope files but every behaviour above depends on them; the behaviour spec cites them with path:line.

- `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/track/DeletableMangaTracker.kt`, `DeletableAnimeTracker.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt`
- `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt`, `.../manga/MangaTrack.kt`
- `app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt`, `.../manga/model/MangaTrack.kt`
- `app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt`
- `app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt`
- `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`
- `app/src/main/AndroidManifest.xml` (intent filter for `aniyomi` scheme, host `simkl-auth`, lines 3-21 of the excerpt)
- `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt`
- `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt`, `OkHttpExtensions.kt`
- `core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt`
- `app/build.gradle.kts` (applicationId `xyz.jmir.tachiyomi.mi`, versionName `0.18.2.1`)

## Coverage summary

- Files: 22 scope paths, 22 rows.
- Tasks in `p1-tasks.md`: all `T-32a-NNN` (count is printed by the lint step).
- Every scope file maps to at least one task (see column "Task ids").
- Port: 22 files (logic translated; idioms change). Drop: one declaration only (`MURating.copyTo`, dead code, never called in scope), recorded in p1-behavior.md section 9 and task T-32a-062.
