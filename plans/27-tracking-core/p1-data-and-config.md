# Area 27-tracking-core, part 1: data models, preferences, storage and endpoints

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Behaviour is in `p1-behavior.md`; the Rust types are in `p1-rust-design.md`. This document lists every field, type, default and stored value that part 1 reads or writes.

Contents: 1 tracker ids; 2 preference keys; 3 credential and token keys; 4 domain models; 5 legacy DB models; 6 database tables and SQL; 7 delayed queue storage; 8 search models; 9 enums and sentinel values; 10 OAuth endpoints and redirects; 11 bit layouts (none); 12 files on disk; 13 Kurobayomi mapping.

---

## 1. Tracker ids (OBSERVED)

Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:19-37`.

| Id (Long) | Constant name | Name shown in UI | Class | Kinds | Login model |
|---|---|---|---|---|---|
| 1 | (literal) | MyAnimeList | `MyAnimeList` | manga, anime | OAuth code grant with PKCE |
| 2 | `ANILIST` | AniList | `Anilist` | manga, anime | OAuth implicit grant (token in fragment) |
| 3 | `KITSU` | Kitsu | `Kitsu` | anime, manga | email + password (token exchange) |
| 4 | (literal) | Shikimori | `Shikimori` | manga, anime | OAuth code grant |
| 5 | (literal) | Bangumi | `Bangumi` | manga, anime | OAuth code grant |
| 6 | (literal) | Komga | `Komga` | manga (enhanced) | constant placeholder credentials |
| 7 | (literal) | MangaUpdates | `MangaUpdates` | manga (deletable) | username + password, session token |
| 8 | `KAVITA` | Kavita | `Kavita` | manga (enhanced) | constant placeholder credentials |
| 9 | (literal) | Suwayomi | `Suwayomi` | manga (enhanced) | constant placeholder credentials |
| 101 | `SIMKL` | Simkl | `Simkl` | anime | OAuth code grant |
| 102 | `JELLYFIN` | Jellyfin | `Jellyfin` | anime (enhanced) | constant placeholder credentials |

Kotlin type of the id: `Long`. Stored in the DB as `sync_id INTEGER`. Rust type: `TrackerId(i64)` (SQLite integer) or `u32` newtype with the same values. Negative ids do not occur.

Notes:
- The login model column is from `*.kt` sources in areas 28 to 33 (`saveCredentials` call sites listed in `p1-behavior.md` section 17.5).
- Enhanced trackers with ids 6, 8, 9 and 102 use constant credentials (`"user"` / `"pass"`), so `isLoggedIn` is true as soon as the login is recorded. Their login is `loginNoop()` in the settings screen (behaviour 12.4).

---

## 2. Preference keys (OBSERVED)

Source of keys and defaults: `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`. "Private" means the stored key is `Preference.privateKey(key)` = `"__PRIVATE_" + key` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:32-33, 48`).

| Key as written in source | Stored key | Type | Default | Private | Backed up | Accessor | Used by |
|---|---|---|---|---|---|---|---|
| `pref_mangasync_username_${tracker.id}` | `__PRIVATE_pref_mangasync_username_<id>` | String | `""` | yes | no | `trackUsername(tracker)` | BaseTracker login state, settings |
| `pref_mangasync_password_${tracker.id}` | `__PRIVATE_pref_mangasync_password_<id>` | String | `""` | yes | no | `trackPassword(tracker)` | BaseTracker login state; holds OAuth access token or Kitsu user id (section 3) |
| `pref_tracker_auth_expired_${tracker.id}` | `__PRIVATE_pref_tracker_auth_expired_<id>` | Boolean | `false` | yes | no | `trackAuthExpired(tracker)` | MyAnimeList only (`MyAnimeList.kt:269-273`) |
| `track_token_${tracker.id}` | `__PRIVATE_track_token_<id>` | String (JSON) | `""` | yes | no | `trackToken(tracker)` | AniList, Bangumi OAuth objects (`anilist/Anilist.kt:360-370`, `bangumi/Bangumi.kt:205-210`) |
| `anilist_score_type` | `anilist_score_type` | String | `Anilist.POINT_10` = `"POINT_10"` (`anilist/Anilist.kt:50`) | no | yes | `anilistScoreType()` | AniList score conversion (`anilist/AnilistUtils.kt:32`) |
| `pref_auto_update_manga_sync_key` | same | Boolean | `true` | no | yes | `autoUpdateTrack()` | reader and player and external player gating (section 9.3 of behaviour) |
| `track_on_adding_to_library` | same | Boolean | `true` | no | yes | `trackOnAddingToLibrary()` | entry screens, auto-open sheet (behaviour 11.4) |
| `show_next_episode_airing_time` | same | Boolean | `true` | no | yes | `showNextEpisodeAiringTime()` | anime entry screen (area 39) |
| `pref_auto_update_manga_on_mark_read` | same | enum by name, `AutoTrackState` | `AutoTrackState.ALWAYS` | no | yes | `autoUpdateTrackOnMarkRead()` | mark-read policy (behaviour 11) |

Notes:
- "Backed up" follows `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt:62-82`, which drops `__APP_STATE_` keys and private (`__PRIVATE_`) keys. Area 21 owns the full backup format.
- `getEnum` (`core/common/.../PreferenceStore.kt:27-45`) serialises with `it.name` and deserialises with `enumValueOf`, falling back to the default when the stored name is unknown.
- The key `pref_auto_update_manga_sync_key` is shared by anime and manga although its name says manga. Keep the key for migration compatibility.
- The anime `autoUpdateTrack` gating in the external player is `ExternalIntents.kt:525`.

Preference store location: the preference store is defined in area 02. Part 1 only calls `preferenceStore.getString`, `getBoolean` and `getEnum`.

---

## 3. Credential and token storage (OBSERVED, values per tracker)

For tracker id `N`, the stored strings are:

| Logical field | Preference key (before `__PRIVATE_` prefix) | Written by | Content |
|---|---|---|---|
| username | `pref_mangasync_username_N` | `saveCredentials` / `setCredentials` / `logout` | AniList: user name (`username.toString()`, `Anilist.kt:352`). Bangumi: user name (`Bangumi.kt:198`). MyAnimeList: `username` (`MyAnimeList.kt:256`). Simkl: `user.toString()` (`Simkl.kt:127`). Shikimori: `user.toString()` (`Shikimori.kt:228`). Kitsu: entered user name (`Kitsu.kt:230`). MangaUpdates: `authenticated.uid.toString()` (`MangaUpdates.kt:119`). Enhanced placeholders: `"user"`. |
| password | `pref_mangasync_password_N` | same | OAuth trackers: the access token (`oauth.accessToken`). Kitsu: the user id (`Kitsu.kt:230`). MangaUpdates: `authenticated.sessionToken`. Enhanced placeholders: `"pass"`. Credential login entered by the user: the entered password. |
| auth-expired | `pref_tracker_auth_expired_N` | MyAnimeList sets `true` (`MyAnimeList.kt:273`); `setCredentials` sets `false` | Boolean |
| token JSON | `track_token_N` | AniList `Anilist.kt:365`, Bangumi `Bangumi.kt:205` | JSON-serialised OAuth object |

`setCredentials(tracker, username, password)` (`TrackPreferences.kt:29-33`) writes username, password and `false` to auth-expired, in that order, without a transaction. `BaseTracker.logout()` calls it with `""`, `""`.

The secret-bearing fields are the password and the token JSON. Both must move to the OS keyring (Secret Service) on Linux. Design in `p1-rust-design.md` section 5.

---

## 4. Domain models (OBSERVED)

### 4.1 `tachiyomi.domain.track.anime.model.AnimeTrack`

Source: `domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20`. `data class`, `Serializable`.

| Field | Kotlin type | Nullable | Meaning | Default |
|---|---|---|---|---|
| `id` | `Long` | no | `_id` of the `anime_sync` row; `-1` before insert | none (required) |
| `animeId` | `Long` | no | `animes._id` | required |
| `trackerId` | `Long` | no | tracker id (section 1) | required |
| `remoteId` | `Long` | no | the tracker's own media id (`remote_id`) | required |
| `libraryId` | `Long?` | yes | tracker's library-entry id (`library_id`), nullable | required |
| `title` | `String` | no | tracker's title | required |
| `lastEpisodeSeen` | `Double` | no | progress | required |
| `totalEpisodes` | `Long` | no | total episodes known to the tracker; `0` = unknown | required |
| `status` | `Long` | no | tracker status code | required |
| `score` | `Double` | no | tracker score (in tracker's own scale, see 9.2) | required |
| `remoteUrl` | `String` | no | tracker's page URL (`tracking_url`) | required |
| `startDate` | `Long` | no | epoch millis (UTC wall-clock convention, behaviour 6.1); `0` = unset | required |
| `finishDate` | `Long` | no | epoch millis; `0` = unset | required |
| `private` | `Boolean` | no | tracked privately | required |

### 4.2 `tachiyomi.domain.track.manga.model.MangaTrack`

Source: `domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt:5-20`. Same fields with `mangaId`, `lastChapterRead`, `totalChapters`.

Rust mapping: `i64` for every `Long`, `f64` for every `Double`, `bool` for `Boolean`, `Option<i64>` for `Long?`, `String` for `String`. `id` is `i64` with `-1` as the "not inserted" sentinel, or `Option<i64>`. **Proposal:** `Option<i64>` (no magic `-1`), see `p1-rust-design.md` section 3.

### 4.3 Legacy DB-facing interface (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/data/database/models/anime/AnimeTrack.kt:7-35`: interface `AnimeTrack : Serializable` with mutable properties:

| Property | Type | Column it maps to |
|---|---|---|
| `id` | `Long?` | `_id` |
| `anime_id` | `Long` | `anime_id` |
| `tracker_id` | `Long` | `sync_id` |
| `remote_id` | `Long` | `remote_id` |
| `library_id` | `Long?` | `library_id` |
| `title` | `String` | `title` |
| `last_episode_seen` | `Double` | `last_episode_seen` |
| `total_episodes` | `Long` | `total_episodes` |
| `score` | `Double` | `score` |
| `status` | `Long` | `status` |
| `started_watching_date` | `Long` | `start_date` |
| `finished_watching_date` | `Long` | `finish_date` |
| `tracking_url` | `String` | `remote_url` |
| `private` | `Boolean` | `private` |

Manga interface `app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/MangaTrack.kt:7-35` is the same with `manga_id`, `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date`.

`create(serviceId)` returns `AnimeTrackImpl()` (or `MangaTrackImpl()`) with `tracker_id = serviceId`. `AnimeTrackImpl` is declared in area 06 (`data/database/models/anime/*`), out of part 1.

Rust: not needed. The Rust domain types replace the legacy interface. The tests in area 06 check the column mapping.

---

## 5. Search result models (OBSERVED)

`AnimeTrackSearch` and `MangaTrackSearch` (`app/src/main/java/eu/kanade/tachiyomi/data/track/model/*.kt`) are subclasses of the legacy interfaces with these defaults. Every field is in the table, in declaration order, with the default value.

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | `Long?` | `null` | |
| `anime_id` (manga: `manga_id`) | `Long` | `0` | set by `register` |
| `tracker_id` | `Long` | `0` | set by `create(serviceId)` |
| `remote_id` | `Long` | `0` | tracker's media id |
| `library_id` | `Long?` | `null` | |
| `title` | `String` | lateinit | throws if unset (D9) |
| `last_episode_seen` (manga: `last_chapter_read`) | `Double` | `0.0` | |
| `total_episodes` (manga: `total_chapters`) | `Long` | `0` | |
| `score` | `Double` | `-1.0` | `-1.0` = no score |
| `status` | `Long` | `0` | |
| `started_watching_date` (manga: `started_reading_date`) | `Long` | `0` | |
| `finished_watching_date` (manga: `finished_reading_date`) | `Long` | `0` | |
| `private` | `Boolean` | `false` | |
| `tracking_url` | `String` | lateinit | throws if unset |
| `authors` | `List<String>` | `emptyList()` | |
| `artists` | `List<String>` | `emptyList()` | |
| `cover_url` | `String` | `""` | |
| `summary` | `String` | `""` | |
| `publishing_status` | `String` | `""` | display text such as "Finished" |
| `publishing_type` | `String` | `""` | display text such as "Oneshot" (manga) or "OVA" (anime) |
| `start_date` | `String` | `""` | display string, format defined by tracker |

Equality (`equals`/`hashCode`): anime uses `anime_id`, `tracker_id`, `remote_id`. Manga uses `manga_id`, `tracker_id`, `remote_id`. Both `equals` first compare `javaClass`.

---

## 6. Database tables touched (OBSERVED)

Both tables are created in area 03 (manga) and 04 (anime). The definitions below are the authoritative SQLDelight files that part 1 relies on.

### 6.1 `anime_sync` (file `data/src/main/sqldelightanime/dataanime/anime_sync.sq`)

```sql
CREATE TABLE anime_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    anime_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_episode_seen REAL NOT NULL,
    total_episodes INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER AS Boolean DEFAULT 0 NOT NULL,
    UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(anime_id) REFERENCES animes (_id)
    ON DELETE CASCADE
);
```

Columns and SQLite types:

| Column | Type | Null | Default | Domain field |
|---|---|---|---|---|
| `_id` | INTEGER PRIMARY KEY | no | (rowid) | `id` |
| `anime_id` | INTEGER | no | | `animeId` |
| `sync_id` | INTEGER | no | | `trackerId` |
| `remote_id` | INTEGER | no | | `remoteId` |
| `library_id` | INTEGER | yes | | `libraryId` |
| `title` | TEXT | no | | `title` |
| `last_episode_seen` | REAL | no | | `lastEpisodeSeen` |
| `total_episodes` | INTEGER | no | | `totalEpisodes` |
| `status` | INTEGER | no | | `status` |
| `score` | REAL | no | | `score` |
| `remote_url` | TEXT | no | | `remoteUrl` |
| `start_date` | INTEGER | no | | `startDate` |
| `finish_date` | INTEGER | no | | `finishDate` |
| `private` | INTEGER (Boolean) | no | `0` | `private` |

Constraints: `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`; foreign key `anime_id` to `animes(_id)` with `ON DELETE CASCADE`.

Queries (exact SQL from the file):

```sql
delete:
DELETE FROM anime_sync
WHERE anime_id = :animeId AND sync_id = :syncId;

getAnimeTracks:
SELECT * FROM anime_sync;

getTrackByAnimeId:
SELECT * FROM anime_sync WHERE _id = :id;     -- note: parameter is the track _id

getTracksByAnimeId:
SELECT * FROM anime_sync WHERE anime_id = :animeId;

insert:
INSERT INTO anime_sync(anime_id,sync_id,remote_id,library_id,title,last_episode_seen,total_episodes,status,score,remote_url,start_date,finish_date,private)
VALUES (:animeId,:syncId,:remoteId,:libraryId,:title,:lastEpisodeSeen,:totalEpisodes,:status,:score,:remoteUrl,:startDate,:finishDate,:private);

update:
UPDATE anime_sync
SET
    anime_id = coalesce(:animeId, anime_id),
    sync_id = coalesce(:syncId, sync_id),
    remote_id = coalesce(:mediaId, remote_id),
    library_id = coalesce(:libraryId, library_id),
    title = coalesce(:title, title),
    last_episode_seen = coalesce(:lastEpisodeSeen, last_episode_seen),
    total_episodes = coalesce(:totalEpisodes, total_episodes),
    status = coalesce(:status, status),
    score = coalesce(:score, score),
    remote_url = coalesce(:trackingUrl, remote_url),
    start_date = coalesce(:startDate, start_date),
    finish_date = coalesce(:finishDate, finish_date),
    private = coalesce(:private, private)
WHERE _id = :id;
```

Which queries part 1 uses (through `AnimeTrackRepositoryImpl`, `data/src/main/java/tachiyomi/data/track/anime/AnimeTrackRepositoryImpl.kt`):
- `getTrackByAnimeId(id)` → `getTrackByAnimeId` (by `_id`).
- `getTracksByAnimeId(animeId)` → `getTracksByAnimeId`; flow variant `getTracksByAnimeIdAsFlow`.
- `getAnimeTracksAsFlow()` → `getAnimeTracks`.
- `delete(animeId, trackerId)` → `delete`.
- `insertAnime(track)` and `insertAllAnime(tracks)` → `insert` in one transaction (`handler.await(inTransaction = true)`).
- `update` is **not** used by part 1. It is used by the backup restorer (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt:433`).

Mapping: `AnimeTrackMapper.mapTrack(id, animeId, syncId, remoteId, libraryId, title, lastEpisodeSeen, totalEpisodes, status, score, remoteUrl, startDate, finishDate, private)` produces the domain `AnimeTrack` with `trackerId = syncId` (`data/src/main/java/tachiyomi/data/track/anime/AnimeTrackMapper.kt:6-36`).

Insert parameter mapping (`AnimeTrackRepositoryImpl.kt:54-68`): `animeId = track.animeId`, `syncId = track.trackerId`, `remoteId`, `libraryId`, `title`, `lastEpisodeSeen`, `totalEpisodes`, `status`, `score`, `remoteUrl`, `startDate`, `finishDate`, `private`. The track's own `id` is **not** written.

### 6.2 `manga_sync` (file `data/src/main/sqldelight/data/manga_sync.sq`)

Identical structure with these names:
- Table `manga_sync`, FK `manga_id` → `mangas(_id)`, `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`.
- `last_chapter_read REAL NOT NULL`, `total_chapters INTEGER NOT NULL`.
- Queries: `delete`, `getTracks`, `getTrackById` (`WHERE _id = :id`), `getTracksByMangaId`, `insert` (no `_id`), `update` (uses `:totalChapter`, which is a parameter name mismatch but a valid SQL statement).
- Repository: `data/src/main/java/tachiyomi/data/track/manga/MangaTrackRepositoryImpl.kt`. Method names and calls mirror the anime repository.

Note: `manga_sync.sq:54` uses `coalesce(:totalChapter, total_chapters)`, where the parameter name is singular. Keep the Rust column name `total_chapters`.

### 6.3 Behavioural consequence of the insert (OBSERVED)

Because the insert omits `_id`:
- An insert for a new `(entry, tracker)` pair assigns `_id = max(_id) + 1`.
- An insert for an existing pair deletes the old row (`ON CONFLICT REPLACE`) and assigns a new `_id` (SQLite computes the new rowid after the delete). The new `_id` equals the old one only when the old row was the largest `_id`.
- The domain `id` of a track is therefore not stable across writes. Defect D1 in the behaviour document.

**Proposal:** `INSERT ... ON CONFLICT(anime_id, sync_id) DO UPDATE SET ...` (an upsert that keeps `_id`), or an explicit `UPDATE` when `id` is known. Use `RETURNING _id` (SQLite 3.35+, bundled with `rusqlite` `bundled`) to get the id.

---

## 7. Delayed queue storage (OBSERVED, to be replaced)

### 7.1 Current storage

- Container: Android SharedPreferences file named `tracking_queue`, mode `MODE_PRIVATE` (`app/src/main/java/eu/kanade/domain/track/anime/store/DelayedAnimeTrackingStore.kt:13`, `.../manga/store/DelayedMangaTrackingStore.kt:13`). Both kinds share the file.
- Key: `trackId.toString()` (decimal `Long`), for example `"42"`.
- Value: `Float` (`putFloat`). Read back as `it.value.toString().toFloat()`.
- Examples of the on-disk XML: `<float name="42" value="12.0" />`. This is Android's format and is not used on Linux.

No other keys are written to this file. `getAnimeItems()` and `getMangaItems()` read the whole file (`preferences.all`).

### 7.2 Replacement (Proposal)

A table in the Kurobayomi database file (area 03/04/05 owns the file; this table is created by `kuro-track` through the shared migration mechanism):

```sql
CREATE TABLE track_retry_queue (
    kind        TEXT    NOT NULL CHECK (kind IN ('anime', 'manga')),
    entry_id    INTEGER NOT NULL,      -- animes._id or mangas._id of the kind
    tracker_id  INTEGER NOT NULL,      -- sync_id (section 1)
    progress    REAL    NOT NULL,      -- lastEpisodeSeen or lastChapterRead
    queued_at   INTEGER NOT NULL,      -- unix millis, first queueing time
    attempts    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (kind, entry_id, tracker_id)
);
```

- `progress` stores `f64`, fixing the `Float` precision loss (defect D7).
- The primary key includes `kind`, fixing defect D3. The key is `(entry, tracker)` rather than the track `_id`, which changes on every write (defect D1, `p1-rust-design.md` section 10).
- `attempts` and `queued_at` are new fields used by the retry scheduler (`p1-rust-design.md` section 6). They are not in the Kotlin design.
- Upsert rule kept from the source: `progress = MAX(progress, excluded.progress)` (the store keeps the highest pending value; behaviour 10.1). Implemented as one statement (`p1-rust-design.md` section 3.12).

---

## 8. Remote setter inputs and outputs (OBSERVED summary)

| Setter | Input | Field written in the model | Remote call | Local write |
|---|---|---|---|---|
| `setRemoteAnimeStatus` | `status: Long` | `status`, `last_episode_seen` (if completed and total known) | `update(track)` | `insertTrack` |
| `setRemoteLastEpisodeSeen` | `episodeNumber: Int` | `status` (rules), `last_episode_seen`, `status`, `finished_watching_date` | `update(track)` | `insertTrack` |
| `setRemoteScore` | `scoreString: String` | `score` (index in list or -1.0) | `update(track)` | `insertTrack` |
| `setRemoteStartDate` | `epochMillis: Long` (0 clears) | `started_watching_date` | `update(track)` | `insertTrack` |
| `setRemoteFinishDate` | `epochMillis: Long` | `finished_watching_date` | `update(track)` | `insertTrack` |
| `setRemotePrivate` | `private: Boolean` | `private` | `update(track)` | `insertTrack` |
| `TrackEpisode` | `episodeNumber: Double` | `lastEpisodeSeen = episodeNumber` on a refreshed copy | `refresh`, then `update(track, true)` | `insertTrack` |
| `TrackChapter` | `chapterNumber: Double` | `lastChapterRead = chapterNumber` on a refreshed copy | `refresh`, then `update(track, true)` | `insertTrack` |

Manga setters are the same with `last_chapter_read`, `total_chapters`, `started_reading_date`, `finished_reading_date`. `TrackChapter` calls `service.mangaService.update(updatedTrack.toDbTrack(), true)` (`TrackChapter.kt:43`), so manga also passes `didReadChapter = true`.

---

## 9. Enums and sentinels

### 9.1 `AutoTrackState` (OBSERVED)

| Constant | Stored name | Label key | English label |
|---|---|---|---|
| `ALWAYS` | `"ALWAYS"` | `MR.strings.lock_always` | Always |
| `ASK` | `"ASK"` | `MR.strings.default_category_summary` | Always ask |
| `NEVER` | `"NEVER"` | `MR.strings.lock_never` | Never |

Default `ALWAYS`. Declaration order is the order shown in the list (`SettingsTrackingScreen.kt:153-154`, `AutoTrackState.entries`).

### 9.2 Score sentinels and scales (OBSERVED)

- Score is `Double`. Its meaning is the tracker's scale, given by `getScoreList()` order: the stored value is `indexToScore(index)` = `index.toDouble()` (`AnimeTracker.kt:31-33`). A tracker with list `["0".."10"]` stores `0.0`..`10.0`.
- `-1.0`: "no score" in search results (default of `AnimeTrackSearch.score`, `AnimeTrackSearch.kt:25`), and the value `setRemoteScore` produces when the string is not in the list (`indexOf` returns `-1`).
- `0.0`: "no score" in the info sheet (`AnimeTrackInfoDialogHome.kt:101`, `takeIf { … score != 0.0 }`).
- Scale conversion: `get10PointScore(track)` returns `track.score` unchanged by default (`AnimeTracker.kt:43-45`). Trackers override it (area 28+).

### 9.3 Date sentinels (OBSERVED)

- `startDate` and `finishDate` use `0L` for "not set". Milliseconds since epoch otherwise.
- The sheet hides the value when `== 0L` (`AnimeTrackInfoDialogHome.kt:109, 113`).
- Remove flow calls the setter with `0` (behaviour 14.7).

### 9.4 Progress numbers (OBSERVED)

- `lastEpisodeSeen`/`lastChapterRead` are `Double`. Values are integral in normal use. Decimal values (for example `12.5` for a special chapter) are allowed by the setters. The remote setter receives `Int` and truncates.
- `totalEpisodes`/`totalChapters` is `Long`. `0` means unknown.
- Episode and chapter numbers in the domain are `Float` (`Episode.episodeNumber`, `Chapter.chapterNumber`). Negative means unrecognised (`isRecognizedNumber`).

### 9.5 Status codes

Tracker status codes are `Long` values defined by each tracker (for example AniList ints). They are not defined in part 1. The completion code is `getCompletionStatus()`. Labels come from `getStatusForAnime(code)` / `getStatusForManga(code)`, which return a `StringResource?`.

---

## 10. OAuth endpoints and redirects (OBSERVED, reference only)

These are used by the login rows (behaviour 12.3) and the redirect router (behaviour 17). Endpoint details are owned by areas 28 to 33. Constants are copied as they appear in source. None of them is a secret.

| Tracker | Authorise URL builder | Base or login URL constant | Redirect URI | Grant | Extra parameters |
|---|---|---|---|---|---|
| AniList | `AnilistApi.authUrl()` (`anilist/AnilistApi.kt:584-587`) | `BASE_URL = "https://anilist.co/api/v2/"` (`:572`), appends `oauth/authorize` | none in query (implicit flow; redirect configured on the AniList app) | `response_type=token` | `client_id` |
| MyAnimeList | `MyAnimeListApi.authUrl()` (`myanimelist/MyAnimeListApi.kt:392-396`) | `BASE_OAUTH_URL = "https://myanimelist.net/v1/oauth2"` (`:385`), appends `/authorize` | not in query | `response_type=code` | `client_id`, `code_challenge` (PKCE, `getPkceChallengeCode()`) |
| Shikimori | `ShikimoriApi.authUrl()` (`shikimori/ShikimoriApi.kt:261-265`) | `LOGIN_URL = "$BASE_URL/oauth/authorize"`, `BASE_URL = "https://shikimori.one"` (`:251-254`) | `REDIRECT_URL = "aniyomi://shikimori-auth"` (`:256`) | `response_type=code` | `client_id`, `redirect_uri` |
| Bangumi | `BangumiApi.authUrl()` (`bangumi/BangumiApi.kt:279-284`) | `LOGIN_URL = "https://bgm.tv/oauth/authorize"` (`:273`) | `REDIRECT_URL = "aniyomi://bangumi-auth"` (`:275`) | `response_type=code` | `client_id`, `redirect_uri` |
| Simkl | `SimklApi.authUrl()` (`simkl/SimklApi.kt:248-253`) | `LOGIN_URL = "$BASE_URL/oauth/authorize"`, `BASE_URL = "https://simkl.com"` (`:240-243`) | `REDIRECT_URL = "aniyomi://simkl-auth"` (`:246`) | `response_type=code` | `client_id`, `redirect_uri` |
| Kitsu | none (credential login) | `LOGIN_URL = "https://kitsu.app/api/oauth/token"` (`kitsu/KitsuApi.kt:434`) | none | password grant | n/a |
| MangaUpdates | none (credential login) | `BASE_URL = "https://api.mangaupdates.com"` (`mangaupdates/MangaUpdatesApi.kt:194`) | none | n/a | n/a |

The browser is opened with `openInBrowser(url, forceDefaultBrowser = true)` (`SettingsTrackingScreen.kt:164-167`). On Linux this is `xdg-open` (behaviour 17 and `p1-rust-design.md` section 8).

Redirect host names routed by `TrackLoginActivity` (behaviour 17.1): `anilist-auth`, `bangumi-auth`, `myanimelist-auth`, `shikimori-auth`, `simkl-auth`. The manifest scheme is `aniyomi://` (area 53).

Token exchange endpoints (`OAUTH_URL` for Shikimori `https://shikimori.one/oauth/token`, Bangumi `https://bgm.tv/oauth/access_token`, Simkl `API_URL/oauth/token`) are owned by areas 28 to 33.

---

## 11. Bit layouts

None in part 1. Tracking data uses integers, reals and booleans only. The tracker's own status and score encodings belong to areas 28 to 33.

---

## 12. Files on disk (OBSERVED on Android; Linux replacement noted)

| Item | Android location | Linux replacement |
|---|---|---|
| Queue file `tracking_queue` | app-private `shared_prefs/tracking_queue.xml` | rows in `track_retry_queue` (section 7.2) in the main database file |
| Preferences | app-private shared-preference file (area 02) | `kuro-prefs` store, TOML/JSON under `$XDG_CONFIG_HOME/kurobayomi/` (area 02) |
| Credentials and tokens | the same preference file as above (private keys) | Secret Service via `keyring` (`p1-rust-design.md` section 5) |
| Database rows `anime_sync`, `manga_sync` | app database (area 03/04) | same tables in the Kurobayomi SQLite database |
| OAuth redirect | `aniyomi://…` intents | `kurobayomi://…` custom scheme registered by a `.desktop` file (`p1-topic-tracker-oauth.md`), or a loopback listener on `127.0.0.1` |

---

## 13. Mapping to Kurobayomi's existing mockup

- `spec/entries.json` already has `tracking_sheet`, `status`, `chapters`, `episodes`, `score`, `start_date`, `finish_date`, `remove_date`, `remove_tracking`, `tracker_search` and `result_menu` dialogs. The `chapters` and `episodes` dialogs list the fixed options "0".."24". The real range is `0..total` or `0..10000` (behaviour 14.3).
- `spec/settings.json` has `settings_tracking` with `tracker_login` and `tracker_logout` dialogs. Their titles match `MR.strings.login_title` and `MR.strings.logout_title`.
- `src/entry.rs:496-545` (`tracking()`) builds a static sheet from hard-coded names ("AniList", "MyAnimeList", "Kitsu") and values ("Watching", "12 / 24", "8.0"). It must read the bindings from the database instead.
- `src/entry.rs:539-568` (`tracker_search()`) uses four hard-coded results. It must call the tracker's `search` and render `TrackSearchResult` (behaviour section 15).
- `src/settings.rs` builds settings rows from `spec/settings.json`. Tracking settings rows should be generated from the `TrackPreferences` key table in section 2, not from the spec file alone.
