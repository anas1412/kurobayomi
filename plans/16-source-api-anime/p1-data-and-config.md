# 16-source-api-anime: data models, configuration, storage and network

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Types: `String`, `Int` (32-bit), `Long` (64-bit), `Float` (32-bit), `Double`, `Boolean`, `Json` = kotlinx JsonObject. "Nullable" means Kotlin `?`.

---

## 1. Anime models

### 1.1 `SAnime` (interface) and `SAnimeImpl` (default impl)
Source: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt`, `SAnimeImpl.kt`.

| Field | Kotlin type | Nullable | Default in `SAnimeImpl` | Notes |
|---|---|---|---|---|
| `url` | `String` | no (`lateinit`) | none; reading before set throws | Relative path (source sets it) |
| `title` | `String` | no (`lateinit`) | none | |
| `thumbnail_url` | `String?` | yes | `null` | Empty string means "no cover update" in the app (area:07) |
| `background_url` | `String?` | yes | `null` | |
| `artist` | `String?` | yes | `null` | |
| `author` | `String?` | yes | `null` | |
| `status` | `Int` | no | `0` (UNKNOWN) | Values in 1.4 |
| `description` | `String?` | yes | `null` | |
| `genre` | `String?` | yes | `null` | Comma-space separated; see behavior 4.10 |
| `update_strategy` | `AnimeUpdateStrategy` | no | `ALWAYS_UPDATE` | 1.3 |
| `fetch_type` | `FetchType` | no | `Episodes` | 1.3 |
| `season_number` | `Double` | no | `-1.0` | Season number parsed by the source; `-1.0` = unknown |
| `initialized` | `Boolean` | no | `false` | Set to `true` by the legacy detail path (`AnimeHttpSource.kt:260`) |
| `memo` | `JsonObject` | no | `{}` | Source-private metadata; see 6.3 |

Constants on the companion (status values, `SAnime.kt:72-79`):
| Name | Value |
|---|---|
| `UNKNOWN` | 0 |
| `ONGOING` | 1 |
| `COMPLETED` | 2 |
| `LICENSED` | 3 |
| `PUBLISHING_FINISHED` | 4 |
| `CANCELLED` | 5 |
| `ON_HIATUS` | 6 |
| `UPCOMING` | 7 |

Storage: `animes.status INTEGER` (`data/src/main/sqldelightanime/dataanime/animes.sq`, column `status`); the integer is stored as-is.

### 1.2 `SEpisode` (interface) and `SEpisodeImpl`
Source: `model/SEpisode.kt`, `model/SEpisodeImpl.kt`.

| Field | Kotlin type | Nullable | Default | Notes |
|---|---|---|---|---|
| `url` | `String` | no (`lateinit`) | none | Relative path; `getEpisodeUrl` returns it unchanged |
| `name` | `String` | no (`lateinit`) | none | |
| `episode_number` | `Float` | no | `-1f` | |
| `fillermark` | `Boolean` | no | `false` | DB: `fillermark INTEGER AS Boolean NOT NULL` (`episodes.sq:23`) |
| `scanlator` | `String?` | yes | `null` | DB: `scanlator TEXT` |
| `date_upload` | `Long` | no | `0` | Stored as `date_upload INTEGER NOT NULL` (`episodes.sq:17`); app treats as epoch milliseconds (area:07) |
| `summary` | `String?` | yes | `null` | DB: `summary TEXT` |
| `preview_url` | `String?` | yes | `null` | DB: `preview_url TEXT` |
| `memo` | `JsonObject` | no | `{}` | DB: `memo BLOB AS JsonObject NOT NULL DEFAULT '{}'` (`episodes.sq:24`), bytes are UTF-8 JSON text (`DatabaseAdapter.kt:50-57`) |

### 1.3 Enums stored in `animes`
| Enum | Values in declaration order (ordinal) | DB column | Storage | Unknown value fallback |
|---|---|---|---|---|
| `AnimeUpdateStrategy` (`model/AnimeUpdateStrategy.kt`) | `ALWAYS_UPDATE`=0, `ONLY_FETCH_ONCE`=1 | `animes.update_strategy INTEGER AS AnimeUpdateStrategy NOT NULL DEFAULT 0` (`animes.sq:27`) | ordinal (`DatabaseAdapter.kt:36-41`) | `ALWAYS_UPDATE` |
| `FetchType` (`model/FetchType.kt`) | `Seasons`=0, `Episodes`=1 | `animes.fetch_type INTEGER AS FetchType NOT NULL DEFAULT 1` (`animes.sq:33`) | ordinal (`DatabaseAdapter.kt:43-47`) | `Episodes` |

Note: `DEFAULT 1` in SQL matches `Episodes` (ordinal 1). Index `animes_fetch_type` exists (`animes.sq:46`).

### 1.4 `AnimeRelation`
`class AnimeRelation(name: String, animes: List<SAnime>)` (`model/AnimeRelation.kt:8-11`). No defaults. Persisted through `anime_relations.sq` (area:04/06).

### 1.5 `AnimesPage`
`class AnimesPage(animes: List<SAnime>, hasNextPage: Boolean)`; `hasNextPage` is non-null `Boolean`, no default. Deprecated `component1`, `component2`, `copy` are present for source-binary compatibility only.

### 1.6 `SAnimeEpisodeUpdate`, `SAnimeSeasonUpdate`
`class SAnimeEpisodeUpdate(val anime: SAnime, val episodes: List<SEpisode>)` and `class SAnimeSeasonUpdate(val anime: SAnime, val seasons: List<SAnime>)`. Both non-null, no defaults.

### 1.7 `Hoster`
Source: `model/Hoster.kt`. Declared `open class`, not `data class`.

| Field | Type | Nullable | Default | Notes |
|---|---|---|---|---|
| `hosterUrl` | `String` | no | `""` | Absolute URL used by `videoListRequest(hoster)` |
| `hosterName` | `String` | no | `""` | Display name; `"no_hoster_list"` for the synthetic hoster |
| `videoList` | `List<Video>?` | yes | `null` | `null` = videos must be fetched; non-null = already known |
| `internalData` | `String` | no | `""` | Opaque source payload |
| `lazy` | `Boolean` | no | `false` | `true` = do not fetch videos until the hoster is chosen |
| `memo` | `JsonObject` | no | `{}` | |
| `status` | `Hoster.State` (`@Transient @Volatile`) | no | `IDLE` | Runtime only |

`Hoster.State` enum: `IDLE`=0, `LOADING`=1, `READY`=2, `ERROR`=3 (declaration order, not persisted). `NO_HOSTER_LIST = "no_hoster_list"` (`Hoster.kt:57`).

Ext-lib-16 constructor (`Hoster.kt:30-43`) is the same as the primary one without `memo` (memo defaults to `{}`). Proposal: one constructor.

### 1.8 `Video`
Source: `model/Video.kt:30-191`. Declared `data class Video(...)` with a body (`status` is outside the constructor).

| Field | Type | Nullable | Default | Notes |
|---|---|---|---|---|
| `videoUrl` | `String` (var) | no | `""` | Playable URL. `"null"` literal = legacy "resolve later" marker (behavior 5.3) |
| `videoTitle` | `String` | no | `""` | Label shown in quality selection |
| `resolution` | `Int?` | yes | `null` | No unit defined in source |
| `bitrate` | `Int?` | yes | `null` | No unit defined in source |
| `headers` | `okhttp3.Headers?` | yes | `null` | Null = use `source.headers` at use time (`AnimeDownloader.kt:577`) |
| `preferred` | `Boolean` | no | `false` | Selection tier 1 (behavior 6.1) |
| `subtitleTracks` | `List<Track>` | no | `[]` | |
| `audioTracks` | `List<Track>` | no | `[]` | |
| `timestamps` | `List<TimeStamp>` | no | `[]` | Chapter markers |
| `mpvArgs` | `List<Pair<String, String>>` | no | `[]` | mpv option pairs |
| `ffmpegStreamArgs` | `List<Pair<String, String>>` | no | `[]` | Input-side ffmpeg options |
| `ffmpegVideoArgs` | `List<Pair<String, String>>` | no | `[]` | Output-side ffmpeg options |
| `internalData` | `String` | no | `""` | Opaque |
| `initialized` | `Boolean` | no | `false` | `true` once resolved (behavior 6.3) |
| `memo` | `JsonObject` | no | `{}` | |
| `status` | `Video.State` (`@Transient @Volatile`, body property) | no | `QUEUE` | Runtime only; not in equality |

Legacy members (`quality`, `url`, `videoPageUrl`, ext-lib-16 constructors, deprecated `copy`) are described in behavior 8.3 and are dropped in Rust.

`Video.State` enum: `QUEUE`=0, `LOAD_VIDEO`=1, `READY`=2, `ERROR`=3 (runtime only).

Constant: `MPV_ARGS_TAG = "ANIYOMI_MPV_ARGS"`.

Regex constant: `localUrl = Regex("""http:\/\/localhost:1(?!\d)""")`. Literal form in Rust: `r"http://localhost:1(?!\d)"` is NOT valid for the `regex` crate (no lookahead). Proposal: `http://localhost:1($|[^0-9])` (not anchored, because Kotlin `Regex.replace` rewrites every match anywhere in the string), replacing with `http://localhost:<port>` followed by the captured trailing character. This reproduces the lookahead: a digit after `1` is never consumed.

### 1.9 `Track`, `ChapterType`, `TimeStamp`
- `data class Track(val url: String, val lang: String)`. No defaults. `@Serializable`.
- `enum class ChapterType { Opening, Ending, Recap, MixedOp, Other }`. Ordinals 0..4. Serialized by name: `"Opening"`, `"Ending"`, `"Recap"`, `"MixedOp"`, `"Other"`.
- `data class TimeStamp(val start: Double, val end: Double, val name: String, val type: ChapterType = ChapterType.Other)`. `start` and `end` are seconds (the app's AniSkip and chapter code treat them as seconds; see area:39). `@Serializable`.

### 1.10 `ThumbnailInfo` and `TileInfo`
- `open class ThumbnailInfo(val tileInfo: List<TileInfo>, val imageTileUrls: List<String>)`. No defaults.
- `data class TileInfo(val imageIndex: Int, val timeMs: Long, val x: Int, val y: Int, val width: Int, val height: Int)`. No defaults. Pixel units for `x, y, width, height`.

### 1.11 `HttpServer`
`open class HttpServer : NanoHTTPD(0)`. Fields: `listeningPort` (from NanoHTTPD, 0 until started), `url` (computed), `@Volatile private var isRunning = false`. Constant `PLACEHOLDER_URL = "http://localhost:1"`.

### 1.12 `UriType` (anime)
`sealed interface UriType { data object Anime; data object Episode; data object Unknown }` (`online/ResolvableAnimeSource.kt:39-43`). Manga twin in `source/online/ResolvableSource.kt:39-43` with `Manga`, `Chapter`, `Unknown`.

---

## 2. Filter models

Source: `model/AnimeFilter.kt`, `model/AnimeFilterList.kt` (manga twin: `source/model/Filter.kt`, `source/model/FilterList.kt`).

| Class | Constructor | Stored state (`var state`) | Type of state |
|---|---|---|---|
| `AnimeFilter.Header` | `(name: String)` | `0` | `Any` (Int 0) |
| `AnimeFilter.Separator` | `(name: String = "")` | `0` | `Any` |
| `AnimeFilter.Select<V>` | `(name, values: Array<V>, state: Int = 0)` | index | `Int` |
| `AnimeFilter.Text` | `(name, state: String = "")` | text | `String` |
| `AnimeFilter.CheckBox` | `(name, state: Boolean = false)` | flag | `Boolean` |
| `AnimeFilter.TriState` | `(name, state: Int = STATE_IGNORE)` | `0`/`1`/`2` | `Int` |
| `AnimeFilter.Group<V>` | `(name, state: List<V>)` | children | `List<V>` |
| `AnimeFilter.Sort` | `(name, values: Array<String>, state: Selection? = null)` | selection | `Sort.Selection?` |
| `AnimeFilter.Sort.Selection` | `data class (index: Int, ascending: Boolean)` | | |

TriState constants: `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2` (`AnimeFilter.kt:21-23`).

`AnimeFilterList(list: List<AnimeFilter<*>>)` is `@Stable data class` delegating `List`. Secondary `constructor(vararg fs)`. `equals` always false. `hashCode = list.hashCode()`.

Filter bit layouts: none. Filters are ordinary values.

---

## 3. Manga-side models (shared shape, overlap with area:15)

- `SManga` (`source/model/SManga.kt`): `url`, `title` (lateinit), `artist?`, `author?`, `description?`, `genre?`, `status: Int = 0`, `thumbnail_url?`, `update_strategy = ALWAYS_UPDATE`, `initialized = false`. Constants `UNKNOWN=0, ONGOING=1, COMPLETED=2, LICENSED=3, PUBLISHING_FINISHED=4, CANCELLED=5, ON_HIATUS=6` (no UPCOMING).
- `SChapter` (`source/model/SChapter.kt`): `url`, `name` (lateinit), `date_upload: Long = 0`, `chapter_number: Float = -1f`, `scanlator?`.
- `Page` (`source/model/Page.kt`): `index: Int`, `url: String = ""`, `imageUrl: String? = null`, `uri: Uri?` (transient, deprecated). Runtime `status: State = QUEUE`, `progress: Int = 0`. `State` = `QUEUE`=0, `LOAD_PAGE`=1, `DOWNLOAD_IMAGE`=2, `READY`=3, `ERROR`=4. `number = index + 1`.
- `MangasPage(mangas: List<SManga>, hasNextPage: Boolean)` (`source/model/MangasPage.kt:3`).
- `UpdateStrategy` (`source/model/UpdateStrategy.kt`): `ALWAYS_UPDATE`=0, `ONLY_FETCH_ONCE`=1.

---

## 4. Torrent models

Source: `torrentutils/model/TorrentFile.kt`, `TorrentInfo.kt`. Remote model in `core/common/src/main/java/aniyomi/core/common/torrent/model/Torrent.kt`, `TorrentRequest.kt`.

| Type | Field | Type | Nullable | Default | JSON name (remote) |
|---|---|---|---|---|---|
| `TorrentFile` | `path` | `String` | no | none | — |
| | `indexFile` | `Int` | no | none | — |
| | `size` | `Long` | no | none | — |
| | `torrentHash` (private) | `String` | no | none | — |
| | `trackers` (private) | `List<String>` | no | `[]` | — |
| `TorrentInfo` | `title` | `String` | no | none | — |
| | `files` | `List<TorrentFile>` | no | none | — |
| | `hash` | `String` | no | none | — |
| | `size` | `Long` | no | none | — |
| | `trackers` | `List<String>` | no | `[]` | — |
| `Torrent` (remote) | `title` | `String` | no | — | `title` |
| | `hash` | `String?` | yes | `null` | `hash` |
| | `torrentSize` | `Long?` | yes | `null` | `torrent_size` |
| | `trackers` | `List<String>?` | yes | `null` | `trackers` |
| | `fileStats` | `List<FileStats>?` | yes | `null` | `file_stats` |
| `FileStats` (remote) | `id` | `Int?` | yes | `null` | `id` |
| | `path` | `String` | no | — | `path` |
| | `length` | `Long` | no | — | `length` |
| `TorrentRequest` (remote) | `action` | `String` | no | — | `action` (value `"add"` for magnets) |
| | `hash` | `String` | no | `""` | `hash` |
| | `link` | `String` | no | `""` | `link` |
| | `title` | `String` | no | `""` | `title` |
| | `poster` | `String` | no | `""` | `poster` |
| | `data` | `String` | no | `""` | `data` |
| | `saveToDb` | `Boolean` | no | `false` | `save_to_db` |

`DeadTorrentException : Exception()` and `DisabledTorrServerException` carry no fields.

---

## 5. Serialized wire formats (`model/Video.kt`, `model/Hoster.kt`)

These are the only JSON formats in scope. Used by the app to pass a hoster list through an Android intent (`PlayerActivity.kt:171`, `hostList` extra) and by `PlayerViewModel.kt:1297` to read it back. Proposal: keep the types in-process in Rust; the codec below is needed only when importing Aniyomi-written data (P2, T-16a-018).

### 5.1 `SerializableVideo`
Serialized with the plain kotlinx `Json` instance (`Json.encodeToString`, `Video.kt:213-234`), not `defaultJson`. Field order as declared:

| JSON key | Kotlin type | Nullable | Default | Encoded form |
|---|---|---|---|---|
| `videoUrl` | `String` | no | `""` | string |
| `videoTitle` | `String` | no | `""` | string |
| `resolution` | `Int?` | yes | `null` | number or null |
| `bitrate` | `Int?` | yes | `null` | number or null |
| `headers` | `List<Pair<String,String>>?` | yes | `null` | array of 2-element arrays, e.g. `[["Referer","https://a.example/"],["Cookie","x=1"]]`, or null |
| `preferred` | `Boolean` | no | `false` | bool |
| `subtitleTracks` | `List<Track>` | no | `[]` | `[{"url":..,"lang":..}]` |
| `audioTracks` | `List<Track>` | no | `[]` | same |
| `timestamps` | `List<TimeStamp>` | no | `[]` | `[{"start":..,"end":..,"name":..,"type":"Opening"}]` |
| `mpvArgs` | `List<Pair<String,String>>` | no | `[]` | `[["opt","val"]]` |
| `ffmpegStreamArgs` | `List<Pair<String,String>>` | no | `[]` | `[["key","val"]]` |
| `ffmpegVideoArgs` | `List<Pair<String,String>>` | no | `[]` | `[["key","val"]]` |
| `internalData` | `String` | no | `""` | string |
| `initialized` | `Boolean` | no | `false` | bool |
| `memo` | `JsonObject` | no | `{}` | object |

Notes:
- kotlinx serializes `Pair` as a two-element JSON array (behavior confirmed by the `Pair` serializer; treat as the wire contract).
- The default `Json` instance has `encodeDefaults = true` and `explicitNulls = true`, so every key is written, nulls included.
- Decoding uses the same default `Json` (`Video.kt:236-258`) with `ignoreUnknownKeys = false`. An unknown key makes decoding fail. Rust reader (proposal) ignores unknown keys (superset tolerance).
- `headers` conversion: `Headers.toList()` on write; `headersOf(*flatMap { it.toList() })` on read.

### 5.2 `SerializableHoster`
Serialized with the default `Json` (`Hoster.kt:82-108`):

| JSON key | Kotlin type | Nullable | Default | Notes |
|---|---|---|---|---|
| `hosterUrl` | `String` | no | `""` | |
| `hosterName` | `String` | no | `""` | |
| `videoList` | `String?` | yes | `null` | Nested JSON text of a `List<SerializableVideo>` (a string, not an array) |
| `internalData` | `String` | no | none (required) | Missing key fails to decode |
| `lazy` | `Boolean` | no | `false` | |
| `memo` | `JsonObject` | no | `{}` | |

Example (complete, one hoster with an empty video list): `[{"hosterUrl":"https://h.example/","hosterName":"Server A","videoList":"[]","internalData":"","lazy":false,"memo":{}}]`.

---

## 6. Preferences

### 6.1 Source-scoped preference store (`animesource/utils/Preferences.kt`, `ConfigurableAnimeSource.kt`)
- Key pattern: `"source_" + <source id as decimal Long>`. Examples: `source_4851548778462321757`.
- Backing store (Android): `SharedPreferences` file with that name, `Context.MODE_PRIVATE`.
- No keys are defined by this scope. Each extension defines its own keys inside its `setupPreferenceScreen` (behavior 11.2). Defaults are supplied by the extension at read time.
- Proposal: one JSON document per source id in the settings store (area:02): table row `(scope="source", key="source_<id>", value=JSON object)`. Inner keys are the extension's keys; values keep their type (bool, string, string set, int).

### 6.2 Keys read by this scope's code (outside the source API)
| Key | Type | Default | Location | Used by |
|---|---|---|---|---|
| `default_user_agent` | String | `"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"` | `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:19-22` | `NetworkHelper.defaultUserAgentProvider()` (trimmed) → `User-Agent` header of every source request |
| `pref_torrserver_enable` | Boolean | `false` | `core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt:9` | `PlayerActivity` torrent handoff |
| `pref_torrserver_port` | String | `"8090"` | `TorrentPreferences.kt:12` | `TorrentServerApi.setPort` → `hostUrl` |

### 6.3 Source-private metadata (`memo`)
`memo` is a `JsonObject` on `SAnime`, `SEpisode`, `Hoster`, `Video`. The doc contract (`SAnime.kt:36-47`, `SEpisode.kt:26-36`) says apps may define namespaced keys such as `"aniyomi.*"`. None are defined in this scope. Storage: `animes.memo` and `episodes.memo` BLOB (UTF-8 JSON text). Proposal: store as `serde_json::Map` and persist as TEXT; the BLOB type is an Android detail.

### 6.4 Bit-flag layouts
None in this scope. (`animes.season_flags`, `animes.episode_flags`, `animes.viewer` are DB-level fields owned by area:04 and area:07; this scope does not read their bits.)

---

## 7. Files, folders and formats

| Artifact | Path or name | Format | Writer | Notes |
|---|---|---|---|---|
| Per-source preference file (Android) | app private `shared_prefs/source_<id>.xml` | XML SharedPreferences | `sourcePreferences(key)` | Proposal replaces with settings store row (6.1) |
| Local HTTP server | no file | — | `HttpServer` | ephemeral 127.0.0.1 port |
| ffmpeg download temp | `<tmpDir>/<filename>.tmp` | Matroska container | `AnimeDownloader.kt` (area:24) | renamed to `<filename>.mkv` on success |
| Torrent handoff | TorrServer HTTP at `http://127.0.0.1:<port>` | JSON | `TorrentServerApi` | see 8.2 |
| Downloaded video metadata | container tag `ANIYOMI_MPV_ARGS` | string `opt=val;opt=val` | downloader (area:24) | read by `PlayerActivity.kt:1305-1330` |

Nothing in this scope creates a directory.

---

## 8. Network endpoints

### 8.1 Source endpoints
Defined by each extension, not by this scope. The base class only fixes the request shapes listed in behavior 2.2. Method is `GET` for all defaults. Request body types are the extension's choice (`POST` is possible via the `GET`-only helpers, so extension requests can use `OkHttpClient.post`).

Response parsing for default helpers: body text is parsed as HTML with Jsoup, base URI = final request URL (behavior 4.3). Video streams: whatever URL the source returns; the player/downloader performs the request with `Video.headers ?: source.headers` (behavior 16.2).

### 8.2 TorrServer (optional, `aniyomi/core/common/torrent/TorrentServerApi.kt`)
| Operation | Method | URL | Request body | Response |
|---|---|---|---|---|
| Add magnet (used by `getTorrentInfo`) | POST | `http://127.0.0.1:<port>/torrents` | JSON `{"action":"add","hash":"","link":"<magnet>","title":"<title>","poster":"","data":"","save_to_db":false}` (`application/json`) | `Torrent` JSON |
| Upload .torrent (not called from scope) | POST | `http://127.0.0.1:<port>/torrent/upload` | multipart: `file` (application/x-bittorrent), `save`, `title` | `Torrent` JSON |
| Liveness (not called from scope) | GET | `http://127.0.0.1:<port>/echo` | — | text |

`port` default `8090` (`pref_torrserver_port`). The app starts TorrServer before the call (`PlayerActivity.kt:1097-1100`).

Torrent JSON fields are listed in section 4.

### 8.3 Torrent file download (`TorrentUtils.getTorrentInfo`, non-magnet)
`GET <url>` through the app HTTP client, `awaitSuccess`, body parsed by `TorrentHelpers.parseTorrentDetailsFromTorrentFileContent` (bencode; `core/common/src/main/java/aniyomi/core/common/torrent/bencode`, area:13).

### 8.4 Local stream server (player and downloader)
`http://localhost:<port>` where `<port>` is the ephemeral port bound by `HttpServer`. Placeholder inbound prefix `http://localhost:1`. See behavior 10.

---

## 9. Database tables touched by this scope's models

| Table (file) | Columns touched | Written by | Read by |
|---|---|---|---|
| `animes` (`data/src/main/sqldelightanime/dataanime/animes.sq`) | `url, title, artist, author, description, genre, status, thumbnail_url, background_url, update_strategy (INTEGER, ordinal), fetch_type (INTEGER, ordinal, default 1), season_number (REAL), season_source_order, memo (BLOB), initialized` | `AnimeUpdate` path (area:04/05/07) | `SAnime` mapper (`data/src/main/java/tachiyomi/data/entries/anime/AnimeMapper.kt`) |
| `episodes` (`episodes.sq`) | `url, name, scanlator, episode_number (REAL), source_order, date_upload (INTEGER), summary, preview_url, fillermark (INTEGER boolean), memo (BLOB)` | episode sync (area:07) | episode repository (area:05) |
| `anime_relations` (`anime_relations.sq`) | relation rows for `AnimeRelation` | related-anime sync (`app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncRelatedAnimeWithSource.kt`) | `AnimeRelationRepositoryImpl` |
| `animesources` (`animesources.sq`) | source id registry | source manager (area:09) | source manager |

Column semantics for `episode_number` (REAL) and `season_number` (REAL) map to Kotlin `Float` and `Double` respectively; Rust stores `f64` for both to avoid rounding (proposal: store `episode_number` as `f64` and round-trip the `f32` value exactly, because Kotlin `Float` `-1f` default is `-1.0`).

---

## 10. Numeric and string formats

| Item | Format |
|---|---|
| Source id | decimal `Long` in logs and keys; sign bit always 0 (behavior 1.1) |
| `Video.headers` | ordered list of `name: value` pairs; duplicates allowed (`Headers` preserves order and duplicates) |
| ffmpeg header argument | `-headers 'Name: value\r\nName2: value2\r\n'` |
| mpv option string | `name="value",name2="value2"` |
| Filter `Selection` | `index` 0-based, `ascending` boolean |
| Timestamps | `start`, `end` in seconds (`Double`) |
| Thumbnail tile time | `timeMs` in milliseconds (`Long`) |
| Progress (manga pages) | integer percent 0..100, or -1 when unknown |

---

## 11. Defaults summary (single table)
| Entity | Field | Default |
|---|---|---|
| SAnimeImpl | status | 0 |
| SAnimeImpl | update_strategy | ALWAYS_UPDATE |
| SAnimeImpl | fetch_type | Episodes |
| SAnimeImpl | season_number | -1.0 |
| SAnimeImpl | initialized | false |
| SAnimeImpl | memo | {} |
| SEpisodeImpl | episode_number | -1f |
| SEpisodeImpl | fillermark | false |
| SEpisodeImpl | date_upload | 0 |
| SEpisodeImpl | memo | {} |
| SMangaImpl | status | 0 |
| SMangaImpl | update_strategy | ALWAYS_UPDATE |
| SChapterImpl | chapter_number | -1f |
| SChapterImpl | date_upload | 0 |
| Video | videoUrl, videoTitle | "" |
| Video | preferred, initialized | false |
| Video | lists (tracks, timestamps, mpv/ffmpeg args) | empty |
| Video | status | QUEUE |
| Hoster | hosterUrl, hosterName, internalData | "" |
| Hoster | lazy | false |
| Hoster | videoList | null |
| Hoster | status | IDLE |
| AnimeFilter.Select | state | 0 |
| AnimeFilter.TriState | state | 0 (IGNORE) |
| AnimeFilter.Sort | state | null |
| TorrentRequest | hash, link, title, poster, data | "" |
| TorrentRequest | saveToDb | false |
| Page (manga) | status | QUEUE |
| Page (manga) | progress | 0 |
| HttpServer | isRunning | false |
| Source (AnimeHttpSource) | versionId | 1 |
| Source (AnimeHttpSource) | lang | "" (AnimeSource default) |
| Source (AnimeHttpSource) | getFilterList | empty AnimeFilterList |
| Source (AnimeHttpSource) | supportsRelatedAnime | false |
