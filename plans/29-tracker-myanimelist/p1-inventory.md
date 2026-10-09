# Area 29-tracker-myanimelist, part 1: file inventory

Scope command: `python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 29-tracker-myanimelist 1`.
It prints 11 files, 1014 LOC in total. All paths are relative to `/home/blackbox/aniyomi-src`.

Port decision vocabulary (same as area:27-tracking-core):
- **Port**: same behaviour, rewritten in Rust with the same rules.
- **Rewrite**: same user-visible behaviour, different mechanism.
- **Replace-with-crate**: an Android/Kotlin library call is replaced by a named crate or GIO API.
- **Drop**: no Linux equivalent is needed. Reason and lost value are in `p1-behavior.md`.

Target crates follow `plans/01-foundation-app-bootstrap/p1-rust-design.md` section 1 (`kuro-core`, `kuro-net`, `kuro-db`, `kuro-platform`) and area:27-tracking-core (`kuro-track`). The MAL module is proposed as `kuro-track::mal`. Area:27 owns the generic tracker trait. This area implements it for MyAnimeList.

Priority note: tracking is P1 parity (area:27-tracking-core rules). No MAL task is P0. Android-only items are P3.

| # | Path | LOC | Declared names (every top-level class, object, interface, enum, typealias, top-level fun, companion member) | Purpose | Port decision | Target Rust module | Priority | Task ids |
|---|---|---|---|---|---|---|---|---|
| 1 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeList.kt | 287 | `class MyAnimeList(id: Long)`; companion: `const val READING`, `WATCHING`, `COMPLETED`, `ON_HOLD`, `DROPPED`, `PLAN_TO_READ`, `PLAN_TO_WATCH`, `REREADING`, `REWATCHING`, `private const val SEARCH_ID_PREFIX`, `SEARCH_LIST_PREFIX`, `private val SCORE_LIST`; members `json`, `interceptor`, `api`, `supportsReadingDates`, `getLogo`, `getLogoColor`, `getStatusListManga`, `getStatusListAnime`, `getStatusForManga`, `getStatusForAnime`, `getReadingStatus`, `getWatchingStatus`, `getRereadingStatus`, `getRewatchingStatus`, `getCompletionStatus`, `getScoreList`, `indexToScore`, `displayScore` (x2), private `add` (x2), `update` (x2), `delete` (x2), `bind` (x2), `searchManga`, `searchAnime`, `refresh` (x2), `login(username,password)`, `login(authCode)`, `logout`, `getIfAuthExpired`, `setAuthExpired`, `saveOAuth`, `loadOAuth` | Tracker facade for MAL: status tables, score model, progress rules, bind/update/refresh, search routing, login, logout, token persistence | Port (rewrite as trait impl) | `kuro-track::mal::tracker` | P1 | T-29a-001, T-29a-003, T-29a-006, T-29a-013, T-29a-023, T-29a-024, T-29a-025, T-29a-026, T-29a-027, T-29a-028, T-29a-029, T-29a-042, T-29a-047, T-29a-048, T-29a-056, T-29a-057, T-29a-059 |
| 2 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt | 430 | `class MyAnimeListApi(trackId, client, interceptor)`; members `json`, `authClient`, `getAccessToken`, `getCurrentUser`, `search`, `searchAnime`, `getMangaDetails`, `getAnimeDetails`, `updateItem` (x2), `deleteMangaItem`, `deleteAnimeItem`, `findListItem` (x2), `findListItems`, `findListItemsAnime`, private `getListPage`, private `parseMangaItem`, private `parseAnimeItem`, private `parseDate`, private `convertToIsoDate`; companion: `private const val CLIENT_ID`, `BASE_OAUTH_URL`, `BASE_API_URL`, `LIST_PAGINATION_AMOUNT`, `private var codeVerifier`, `fun authUrl()`, `fun mangaUrl(id)`, `fun animeUrl(id)`, `fun refreshTokenRequest(oauth)`, private `getPkceChallengeCode()` | All HTTP calls to MAL OAuth and MAL API v2, JSON to model mapping, date conversion, PKCE challenge | Rewrite (reqwest + serde, same URLs and form fields) | `kuro-track::mal::api` | P1 | T-29a-005, T-29a-007, T-29a-009, T-29a-010, T-29a-011, T-29a-012, T-29a-013, T-29a-014, T-29a-015, T-29a-016, T-29a-017, T-29a-018, T-29a-019, T-29a-020, T-29a-021, T-29a-022, T-29a-037, T-29a-038, T-29a-039, T-29a-042, T-29a-043, T-29a-055, T-29a-058 |
| 3 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListInterceptor.kt | 83 | `class MyAnimeListInterceptor(myanimelist)`; members `json`, `oauth` (var), `tokenExpired` (get), `intercept`, `setAuth`, private `refreshToken`; top-level `class MALTokenRefreshFailed : IOException`, `class MALTokenExpired : IOException` | Adds Bearer header to every API request, refreshes an expired access token under a lock, flags "login expired" on a 401 from the refresh endpoint | Rewrite (tower/reqwest middleware wrapper around one `MalAuth` service) | `kuro-track::mal::auth` | P1 | T-29a-004, T-29a-007, T-29a-008, T-29a-009, T-29a-034, T-29a-035, T-29a-043, T-29a-053, T-29a-054 |
| 4 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListUtils.kt | 38 | top-level `fun MangaTrack.toMyAnimeListStatus()`, `fun AnimeTrack.toMyAnimeListStatus()`, `fun getStatus(status: String?)` | Maps local status codes to MAL wire strings and wire strings back to local codes | Port (pure functions, table-driven) | `kuro-track::mal::status` | P1 | T-29a-002, T-29a-042 |
| 5 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALAnime.kt | 26 | `data class MALAnime`, `data class MALAnimeCovers` | Anime details response (`/v2/anime/{id}?fields=...`) | Port (serde struct) | `kuro-track::mal::dto` | P1 | T-29a-017, T-29a-036, T-29a-041, T-29a-042 |
| 6 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALList.kt | 48 | `data class MALListAnimeItem`, `data class MALListAnimeItemStatus`, `data class MALListMangaItem`, `data class MALListMangaItemStatus` | `my_list_status` response for a single entry, manga and anime | Port (serde struct) | `kuro-track::mal::dto` | P1 | T-29a-018, T-29a-020, T-29a-021, T-29a-036, T-29a-041, T-29a-042 |
| 7 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALManga.kt | 26 | `data class MALManga`, `data class MALMangaCovers` | Manga details response | Port (serde struct) | `kuro-track::mal::dto` | P1 | T-29a-016, T-29a-036, T-29a-041, T-29a-042 |
| 8 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALOAuth.kt | 25 | `data class MALOAuth` (with private `adjustedExpiresIn`, fun `isExpired()`) | OAuth token response, persisted as JSON, plus expiry rule (`expires_in - 60`) | Port | `kuro-track::mal::auth::token` | P1 | T-29a-007, T-29a-041, T-29a-042 |
| 9 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALSearch.kt | 18 | `data class MALSearchResult`, `data class MALSearchResultNode`, `data class MALSearchResultItem` | `/v2/manga` and `/v2/anime` search response (ids only) | Port | `kuro-track::mal::dto` | P1 | T-29a-011, T-29a-012, T-29a-036, T-29a-041, T-29a-042 |
| 10 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALUser.kt | 8 | `data class MALUser` | `/v2/users/@me` response (username only) | Port | `kuro-track::mal::dto` | P1 | T-29a-010, T-29a-041, T-29a-042 |
| 11 | app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/dto/MALUserListSearch.kt | 25 | `data class MALUserSearchResult`, `data class MALUserSearchItem`, `data class MALUserSearchPaging`, `data class MALUserSearchItemNode` | `/v2/users/@me/mangalist` page (ids, titles, paging.next) used by `my:` search | Port | `kuro-track::mal::dto` | P1 | T-29a-014, T-29a-015, T-29a-036, T-29a-041, T-29a-042 |

## Files referenced but outside this scope (read for exact behaviour)

These are not scope files. They are listed so the reader knows where each MAL hook is called.

| Path | Why it matters |
|---|---|
| app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt | `supportsPrivateTracking = false`, `logout()` clears credentials, `isLoggedIn` = username and password both non-empty |
| app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt, AnimeTracker.kt | Default `setRemote*` methods and `register`; `update(track, didRead = false)` default |
| app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt | Interface: id, name, logo, completion status, login/logout, credentials |
| app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt | `val myAnimeList = MyAnimeList(1L)`; `get(id)` |
| app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt | 50-byte random verifier, base64url without padding |
| app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt | Deep-link handler for `aniyomi://myanimelist-auth` |
| app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt | Loading screen and return to MainActivity |
| app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt | Login row (`openInBrowser(MyAnimeListApi.authUrl(), forceDefaultBrowser = true)`), logout dialog |
| app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackInfoDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt | Delete dialog, "Also remove from %s" checkbox, `DeletableAnimeTracker.delete` and `DeletableMangaTracker.delete` |
| app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt | Converts MAL id to AniList id through an AniList GraphQL `Media(idMal:...)` query |
| app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt | AniSkip lookup uses `track.remoteId` directly for MAL (`PlayerViewModel.kt:2044`) |
| app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt, MangaTrackSearch.kt | Search result holders filled by `MyAnimeListApi` |
| app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt, manga/MangaTrack.kt | DB-facing track interfaces, `copyPersonalFrom` |
| app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt | `MangaTrack.create(serviceId)` |
| app/src/main/java/eu/kanade/domain/track/anime/interactor/TrackEpisode.kt | Episode-watched path: `refresh`, then `update(track, true)`, then local insert |
| app/src/main/java/eu/kanade/domain/track/manga/interactor/TrackChapter.kt | Chapter-read path: `refresh`, then `update(track, true)`, then local insert |
| app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt | Calls `bind(item, hasSeenEpisodes)` when a title is added to a tracker |
| app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt | Calls `bind(item, hasReadChapters)`; then syncs local progress with `setRemoteLastChapterRead` |
| app/src/main/java/eu/kanade/domain/track/anime/interactor/RefreshAnimeTracks.kt, app/src/main/java/eu/kanade/domain/track/manga/interactor/RefreshMangaTracks.kt | Call `refresh` for each track |
| app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt | MAL credential and token preference keys, auto-update prefs |
| core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt | `awaitSuccess()` and `parseAs<T>()` |
| core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt | `GET`, `POST`, `PUT`, `DELETE` builders and default cache-control |
| core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt | Shared OkHttp client: 30 s connect and read, 2 min call timeout, 5 MiB cache |
| app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:175 | Shared `Json` with `ignoreUnknownKeys = true` |
| app/src/main/AndroidManifest.xml (TrackLoginActivity intent filter) | `aniyomi` scheme, host `myanimelist-auth` |
| i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml | Status labels, logout and delete dialog strings |
| app/src/main/res/drawable-nodpi/ic_tracker_mal.webp | MAL logo |

## Task ids for the outside files

- Settings and login UI (`SettingsTrackingScreen.kt`, `PreferenceItem.kt`, `TrackLoginActivity.kt`, `BaseOAuthLoginActivity.kt`): T-29a-029, T-29a-031, T-29a-044, T-29a-045, T-29a-047, T-29a-050, T-29a-051, T-29a-052.
- Tracking info dialogs (`AnimeTrackInfoDialog.kt`, `MangaTrackInfoDialog.kt`): T-29a-031, T-29a-056.
- Interactor callers (`TrackChapter.kt`, `TrackEpisode.kt`, `AddAnimeTracks.kt`, `AddMangaTracks.kt`, `RefreshAnimeTracks.kt`, `RefreshMangaTracks.kt`): T-29a-048, T-29a-056.
- Shared preferences (`TrackPreferences.kt`, `ReaderViewModel.kt`, `PlayerViewModel.kt`, `ExternalIntents.kt`): T-29a-004, T-29a-057.
- Consumers of MAL ids (`AniChartApi.kt`, `PlayerViewModel.kt:2044`): T-29a-032, T-29a-033.
- Tracker base types (`BaseTracker.kt`, `MangaTracker.kt`, `AnimeTracker.kt`, `Tracker.kt`, `TrackerManager.kt`): T-29a-024, T-29a-028, T-29a-048, T-29a-059.
- Logo (`ic_tracker_mal.webp`): T-29a-030. Manifest intent filter (`AndroidManifest.xml`): T-29a-044. Backup exclusion (`Preference.kt`): T-29a-046.
- Network helpers (`NetworkHelper.kt`, `Requests.kt`, `OkHttpExtensions.kt`, `HttpException.kt`, `AppModule.kt`): T-29a-009, T-29a-034, T-29a-036.
- Search holders (`MangaTrackSearch.kt`, `AnimeTrackSearch.kt`): T-29a-016, T-29a-017, T-29a-049.
- Track DB models (`MangaTrack.kt`, `AnimeTrack.kt` for DB and domain): T-29a-019, T-29a-023.
- PKCE helper (`PkceUtil.kt`): T-29a-005.

## Scope coverage check

Every path printed by the scope command appears verbatim in the table above. Run the self-check in the task brief to confirm.
