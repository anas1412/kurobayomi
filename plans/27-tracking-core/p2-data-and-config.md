# area:27-tracking-core part 2 — data models, preference keys, storage, endpoints

Repo root: `/home/blackbox/aniyomi-src`. Scope files: see `p2-inventory.md`. Part-1 tables (tracker class list, status and score lists, `TrackPreferences` getters) are in `p1-data-and-config.md`; this file lists only what the six in-scope files read or write.

---

## 1. Domain track row (what the dialogs hold)

### 1.1 Anime: `tachiyomi.domain.track.anime.model.AnimeTrack`
Source: `domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20`. `data class ... : Serializable`.

| Field | Kotlin type | Null? | Stored column (`anime_sync`) | Meaning | Notes |
|---|---|---|---|---|---|
| `id` | Long | no | `_id` INTEGER PK | local row id | `toDomainTrack(idRequired = false)` uses `-1` when the id is missing (`app/src/main/java/eu/kanade/domain/track/anime/model/AnimeTrack.kt:33-34`). |
| `animeId` | Long | no | `anime_id` | owning entry | FK `animes(_id)` ON DELETE CASCADE. |
| `trackerId` | Long | no | `sync_id` | tracker id (see section 2) | Column name is `sync_id`. |
| `remoteId` | Long | no | `remote_id` | id on the tracker's site | Used by remote delete and remote update calls. |
| `libraryId` | Long? | yes | `library_id` | tracker-side list entry id (e.g. Shikimori `user_rates` id) | Used by Shikimori delete (`ShikimoriApi.kt:78`). |
| `title` | String | no | `title` | remote title shown in search prefill | |
| `lastEpisodeSeen` | Double | no | `last_episode_seen` REAL | progress (episodes) | Stored as REAL. The dialog truncates to Int (behavior 6). |
| `totalEpisodes` | Long | no | `total_episodes` INTEGER | total episodes, 0 when unknown | |
| `status` | Long | no | `status` INTEGER | tracker status id (section 2) | |
| `score` | Double | no | `score` REAL | tracker score as index or numeric per tracker | Part 1 `indexToScore`; `-1.0` in search objects means unset. |
| `remoteUrl` | String | no | `remote_url` TEXT | tracker page URL | Empty string when unknown. Used by open/copy. |
| `startDate` | Long | no | `start_date` INTEGER | epoch millis, local midnight of the chosen day; 0 = unset | Column name is `start_date`, Kotlin name in `toDbTrack` is `started_watching_date`. |
| `finishDate` | Long | no | `finish_date` INTEGER | epoch millis; 0 = unset | Column `finish_date`. |
| `private` | Boolean | no | `private` INTEGER DEFAULT 0 NOT NULL | hide from public list on the tracker | Default 0. |

### 1.2 Manga: `tachiyomi.domain.track.manga.model.MangaTrack`
Same fields with `mangaId` (`manga_id`), `lastChapterRead` (`last_chapter_read` REAL), `totalChapters` (`total_chapters` INTEGER). Table `manga_sync` (`data/src/main/sqldelight/data/manga_sync.sq`).

### 1.3 Table definitions (OBSERVED)
- Anime: `data/src/main/sqldelightanime/dataanime/anime_sync.sq:3-21`.
- Manga: `data/src/main/sqldelight/data/manga_sync.sq:3-21`.
- Both: `UNIQUE (<entry>_id, sync_id) ON CONFLICT REPLACE` and `FOREIGN KEY ... ON DELETE CASCADE`.
- Queries used by these files (all in the `.sq` files):
  - anime: `delete` (`DELETE FROM anime_sync WHERE anime_id = :animeId AND sync_id = :syncId`), `getAnimeTracks` (all rows), `getTrackByAnimeId` (`WHERE _id = :id`), `getTracksByAnimeId` (`WHERE anime_id = :animeId`), `insert` (named columns including `private`), `update` (every column wrapped in `coalesce(:param, column)`, `WHERE _id = :id`).
  - manga: same with `getTracks`, `getTrackById`, `getTracksByMangaId`.
- Observed consequence of `ON CONFLICT REPLACE`: inserting a row for an existing (entry, tracker) pair replaces it and gets a NEW `_id`. Rust must upsert by (entry_id, tracker_id) and keep the row id stable if any cache keys on it (defect noted in behavior section 3.4; see rust design section 6).
- Observed consequence of `coalesce` update: a null parameter keeps the stored value. Setting `start_date`/`finish_date` to `0` clears it, because `0` is not null. Setting `private` to `false` works.

### 1.4 Search objects (part 1 types used by the search dialog)
`eu.kanade.tachiyomi.data.track.model.AnimeTrackSearch` (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/AnimeTrackSearch.kt`):
| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | Long? | null | |
| `anime_id` | Long | 0 | set by `register` |
| `tracker_id` | Long | 0 | set by `create(serviceId)` |
| `remote_id` | Long | 0 | |
| `library_id` | Long? | null | |
| `title` | String | lateinit | |
| `last_episode_seen` | Double | 0.0 | |
| `total_episodes` | Long | 0 | |
| `score` | Double | -1.0 | unset marker |
| `status` | Long | 0 | |
| `started_watching_date` | Long | 0 | |
| `finished_watching_date` | Long | 0 | |
| `private` | Boolean | false | written by the confirm lambda (behavior 4.3) |
| `tracking_url` | String | lateinit | the URL used for pre-selection (behavior 4.2) |
| `authors` | List<String> | emptyList() | search-only |
| `artists` | List<String> | emptyList() | search-only |
| `cover_url` | String | "" | search-only |
| `summary` | String | "" | search-only |
| `publishing_status` | String | "" | search-only |
| `publishing_type` | String | "" | search-only |
| `start_date` | String | "" | search-only; raw string from tracker |

`equals`/`hashCode` use only `anime_id`, `tracker_id`, `remote_id` (`AnimeTrackSearch.kt:51-69`). This is what `find`/`contains` see.
Manga search object `MangaTrackSearch` has the same shape with `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date` and `manga_id`.

---

## 2. Enumerations and constants

### 2.1 Tracker ids (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:297-322`)
Constants inside the companion: `ANILIST = 2L`, `KITSU = 3L`, `KAVITA = 8L`, `SIMKL = 101L`, `JELLYFIN = 102L`.
Constructor ids: `myAnimeList = MyAnimeList(1L)`, `aniList = Anilist(ANILIST)`, `kitsu = Kitsu(KITSU)`, `shikimori = Shikimori(4L)`, `bangumi = Bangumi(5L)`, `komga = Komga(6L)`, `mangaUpdates = MangaUpdates(7L)`, `kavita = Kavita(KAVITA)`, `suwayomi = Suwayomi(9L)`, `simkl = Simkl(SIMKL)`, `jellyfin = Jellyfin(JELLYFIN)`.
Display names (`BaseTracker` constructor): "MyAnimeList", "AniList", "Kitsu", "Shikimori", "Bangumi", "Komga", "MangaUpdates", "Kavita", "Suwayomi", "Simkl", "Jellyfin" (e.g. `anilist/Anilist.kt:31`, `myanimelist/MyAnimeList.kt:28`, `shikimori/Shikimori.kt:29`, `kitsu/Kitsu.kt:30`, `bangumi/Bangumi.kt:24`, `simkl/Simkl.kt:20`, `jellyfin/Jellyfin.kt:20`).

Note: `Tracker.name` is what `track_error` and the delete dialog print (e.g. "AniList error: ...").

### 2.2 Entry kind
Two dialog families: anime (`AnimeTrackInfoDialog.kt`) and manga (`MangaTrackInfoDialog.kt`). Rust enum: `EntryKind { Anime, Manga }` (see rust design).

### 2.3 Status, score and progress values
Status id lists are tracker-specific and live in part 1 and areas 28-33. The dialogs read them through `getStatusListAnime()` / `getStatusListManga()` and `getStatusForAnime(id)` / `getStatusForManga(id)`, and `getScoreList()`.

### 2.4 Bit layouts
None in these files. No bit flags.

### 2.5 Enums
- `AutoTrackState` (`app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt`) is not read by these files (part 1). Not used here.
- `TabletUiMode` and `NavStyle` are in `UiPreferences` only and are not used by these files.

---

## 3. Preference keys read or written by the in-scope files

| Key (as stored) | Type | Default | Read/write | Where | Notes |
|---|---|---|---|---|---|
| `app_date_format` | String | `""` | read | A:104-106, M:104-106 via `UiPreferences.dateFormat()` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:35`) | `""` = `FormatStyle.SHORT` localized; otherwise a `DateTimeFormatter` pattern in `Locale.getDefault()`. |
| `__PRIVATE_pref_mangasync_username_<trackerId>` | String | `""` | read/write via `TrackPreferences.trackUsername(tracker)` | `BaseTracker.getUsername()` (`BaseTracker.kt:98`), `setCredentials` | Prefix `__PRIVATE_` (`Preference.kt:32-33`, `48`). Excluded from backups. |
| `__PRIVATE_pref_mangasync_password_<trackerId>` | String | `""` | read/write | `BaseTracker.getPassword()` (`BaseTracker.kt:100`) | Holds the **access token** for OAuth trackers (`saveCredentials(username, oauth.accessToken)`). Name is historical. Excluded from backups. |
| `__PRIVATE_pref_tracker_auth_expired_<trackerId>` | Boolean | `false` | read/write | `TrackPreferences.trackAuthExpired` (`TrackPreferences.kt:24-27`) | Set to false by `setCredentials` (`TrackPreferences.kt:32`), set true by MAL on expired token (`myanimelist/MyAnimeList.kt:270-274`). |
| `__PRIVATE_track_token_<trackerId>` | String (JSON) | `""` | read/write | `TrackPreferences.trackToken(tracker)` (`TrackPreferences.kt:35`) | Full OAuth JSON. Per tracker: `ALOAuth` (AniList), `MALOAuth` (MAL), `BGMOAuth` (Bangumi), `SMOAuth` (Shikimori), `SimklOAuth` (Simkl). Deleted on logout. |
| `anilist_score_type` | String | `"POINT_10"` (`Anilist.POINT_10`, `anilist/Anilist.kt:50`) | write on AniList login | `TrackPreferences.anilistScoreType()` (`TrackPreferences.kt:37`), `anilist/Anilist.kt:66` and `:353` | Written from the Viewer query `mediaListOptions.scoreFormat`. |

Notes:
- The four private keys are per tracker, so there are 11 × 4 possible entries. Keys for trackers that do not use OAuth (Kitsu, Komga, Kavita, MangaUpdates, Suwayomi, Jellyfin) use username/password instead of the token JSON.
- The Kotlin `Preference.privateKey(key)` wraps with `__PRIVATE_` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:32-34`).
- Backup excludes these keys (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt:82`). Consequence: a restored backup has the track rows but no tokens; the user must log in again.

### 3.1 Rust storage mapping (Proposal)
- Token JSON, username, auth-expired flag → `SecretStore` (platform trait, `p2-rust-design.md` section 7). Keys: `tracker/<trackerId>/oauth` (JSON), `tracker/<trackerId>/account` (username), no password field.
- `app_date_format` → the app config file managed by area:02-preferences-system (key name kept).
- `anilist_score_type` → app config (non-secret).

---

## 4. Files on disk
None are written by the in-scope files. The browser OAuth redirect has no file. Covers and track rows are DB only.

## 5. Network endpoints used by the login flow (OBSERVED)
All values are repo-relative to the cited files. Client ids are public values in the source. Client secrets exist in source but are NOT copied here; their locations are given so the Rust port reads them from configuration (see rust design section 7.3).

### 5.1 Browser authorization URLs (built by `authUrl()`)
| Tracker | Built in | Final URL shape (query order as built) | Response type |
|---|---|---|---|
| AniList | `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt:584-587` | `https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token` | implicit: token in URL fragment |
| MyAnimeList | `myanimelist/MyAnimeListApi.kt:392-396` | `https://myanimelist.net/v1/oauth2/authorize?client_id=686b980ff4240fccce7f6a654cea07ce&code_challenge=<verifier>&response_type=code` | code, PKCE, method implicit `plain` (no `code_challenge_method` sent) |
| Bangumi | `bangumi/BangumiApi.kt:279-285` | `https://bgm.tv/oauth/authorize?client_id=<Bangumi client id>&response_type=code&redirect_uri=aniyomi%3A%2F%2Fbangumi-auth` | code |
| Shikimori | `shikimori/ShikimoriApi.kt:261-266` | `https://shikimori.one/oauth/authorize?client_id=<Shikimori client id>&redirect_uri=aniyomi%3A%2F%2Fshikimori-auth&response_type=code` | code |
| Simkl | `simkl/SimklApi.kt:248-253` | `https://simkl.com/oauth/authorize?response_type=code&client_id=aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth` | code |

Redirect URIs in use: `aniyomi://bangumi-auth` (`BangumiApi.kt:275`), `aniyomi://shikimori-auth` (`ShikimoriApi.kt:256`), `aniyomi://simkl-auth` (`SimklApi.kt:246`). AniList and MAL authorize URLs do not send `redirect_uri`, so the redirect registered with the provider applies. The manifest intent filter (`AndroidManifest.xml:250-258`) handles all five hosts with scheme `aniyomi`.
The Linux port must keep the `aniyomi` scheme and these five hosts, because the provider client registrations hold these exact redirect URIs (see topic file).

### 5.2 Token exchange
| Tracker | Method, URL | Body encoding | Parameters (exact names) | Response parsed as | Source |
|---|---|---|---|---|---|
| AniList | none (implicit) | n/a | `access_token` from fragment | `ALOAuth` built locally by `createOAuth` | `AnilistApi.kt:515-517` |
| MyAnimeList | POST `https://myanimelist.net/v1/oauth2/token` | form | `client_id`, `code`, `code_verifier`, `grant_type=authorization_code` (no `redirect_uri`) | `MALOAuth` (`token_type`, `refresh_token`, `access_token`, `expires_in`, ...) | `MyAnimeListApi.kt:48-62` (`BASE_OAUTH_URL` = `https://myanimelist.net/v1/oauth2`, line 385) |
| Bangumi | POST `https://bgm.tv/oauth/access_token` | form | `grant_type=authorization_code`, `client_id`, `client_secret`, `code`, `redirect_uri` | `BGMOAuth` (`access_token`, `token_type`, `created_at`, `expires_in`, ...) | `BangumiApi.kt:236-256`, `OAUTH_URL` line 272 |
| Shikimori | POST `https://shikimori.one/oauth/token` | form (`FormBody`) | `grant_type=authorization_code`, `client_id`, `client_secret`, `code`, `redirect_uri` | `SMOAuth` | `ShikimoriApi.kt:236-250`, `OAUTH_URL` line 253 |
| Simkl | POST `https://api.simkl.com/oauth/token` | JSON object (`application/json`) | `code`, `client_id`, `client_secret`, `redirect_uri`, `grant_type=authorization_code` | `SimklOAuth` (`access_token`) | `SimklApi.kt:225-236`, `OAUTH_URL` line 242 |

Refresh (for completeness; part 1): MAL `refreshTokenRequest` (`MyAnimeListApi.kt:408-412`), Bangumi, Shikimori, Simkl have refresh requests in their API files.

### 5.3 Identity lookups after token exchange
| Tracker | Request | Field read | Source |
|---|---|---|---|
| AniList | POST `https://graphql.anilist.co/` (`API_URL`, `AnilistApi.kt:571`), GraphQL query `query User { Viewer { id mediaListOptions { scoreFormat } } }` | `Viewer.id` (Int) and `Viewer.mediaListOptions.scoreFormat` → returned as `Pair(viewer.id, scoreFormat)` (`AnilistApi.kt:519-549`). The stored username is `viewer.id.toString()` (a number, not a display name). | `anilist/AnilistApi.kt:519-549` |
| MyAnimeList | GET `https://api.myanimelist.net/v2/users/@me` (`MyAnimeListApi.kt:67-80`) | `MALUser.name` string (the display name, stored as username) | `MyAnimeListApi.kt:66-77` |
| Bangumi | GET `https://api.bgm.tv/v0/me` (`BangumiApi.kt:256-263`) | `username` (`BGMUser.username`) | `bangumi/dto/BGMUser.kt:7-9` |
| Shikimori | GET `https://shikimori.one/api/users/whoami` (`ShikimoriApi.kt:220-225`) | `id` Int (`SMUser.id`) | `shikimori/dto/SMUser.kt:6-8` |
| Simkl | GET `https://api.simkl.com/users/settings` (`SimklApi.kt:204-213`), `runBlocking` | `account.id` Int (`SimklUser.account.id`) | `simkl/dto/SimklUser.kt:6-13` |

For Shikimori and Simkl the stored username is the numeric id as a string (`user.toString()`), so the username field is not a human name.

### 5.4 Headers
- AniList: `Authorization: Bearer <token>` added by `AnilistInterceptor` (`setAuth(oauth)` at `AnilistInterceptor.kt:56`).
- MyAnimeList: `Authorization: Bearer <token>` (`MyAnimeListApi.kt:419`, `MyAnimeListInterceptor`).
- Simkl: `simkl-api-key: <CLIENT_ID>` (`simkl/SimklInterceptor.kt:24`).
- Bangumi and Shikimori: bearer header from their interceptors.

### 5.5 Secrets (locations only)
- Bangumi client secret: `bangumi/BangumiApi.kt:269`.
- Shikimori client secret: `shikimori/ShikimoriApi.kt:259`.
- Simkl client secret: `simkl/SimklApi.kt:238` (the `CLIENT_SECRET` constant).
- MAL: no client secret used (PKCE public client). `MyAnimeListApi.kt:53` sends only `client_id` in refresh.
- Rust: read from build-time config or a non-VCS file, not committed. Decision in rust design section 7.3.

## 6. Parameters constant table (OBSERVED, in one place)
| Name | Value | Source |
|---|---|---|
| AniList implicit token fake lifetime | 31 536 000 000 ms | `anilist/AnilistApi.kt:516` |
| AniList `expires_in` value | 31 536 000 000 (same) | `AnilistApi.kt:516` |
| PKCE verifier bytes | 50 (SecureRandom), Base64 URL, no padding | `util/PkceUtil.kt:9-13` (`app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt`) |
| MAL list pagination (part 1) | 250 | `myanimelist/MyAnimeListApi.kt:388` |
| Progress wheel fallback max | 10000 | behavior 6 (A:380) |
| Clipboard toast truncation | 50 chars, `...` | `ContextExtensions.kt`, behavior 3.7 |

## 7. Model classes for the in-scope items (`AnimeTrackItem`, `MangaTrackItem`)
- `data class AnimeTrackItem(val track: AnimeTrack?, val tracker: Tracker)` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/track/AnimeTrackItem.kt:6`).
- `data class MangaTrackItem(val track: MangaTrack?, val tracker: Tracker)` (`app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackItem.kt:6`; the file is 6 lines).
- Nullability: `track` is null when the entry has no row for this tracker ("Add tracking" row); `tracker` is never null.
- Equality: `data class` equality over `track` and `tracker` (identity for `Tracker` since it has no data class equality).

## 8. Interactions with DB writes
- `insertTrack.await(domainTrack)` after every successful remote setter (`AnimeTracker.kt:218-220`).
- `DeleteAnimeTrack.await(animeId, trackerId)` on remove (`DeleteAnimeTrack.kt:11-17`).
- `RefreshAnimeTracks` → `insertTrack.await` and `syncEpisodeProgressWithTrack` (`RefreshAnimeTracks.kt:33-35`).
- `AddAnimeTracks.bind` through `register` (part 1).

## 9. Data this area does not own
- Status and score constant lists per tracker (part 1, areas 28-33).
- Download and library data (areas 23-26).
- The `anime` and `animes` table definitions (area 04).
