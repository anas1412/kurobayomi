# 24-downloads-anime: data models, preferences, disk layout, endpoints, tables

Paths are repo-relative to `/home/blackbox/aniyomi-src`. "Nullable" means the Kotlin type ends in `?`. Defaults are the Kotlin defaults.

---

## 1. In-memory models

### 1.1 `AnimeDownload` (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownload.kt:16-22`)

| Field | Kotlin type | Nullable | Default | Persisted | Notes |
|---|---|---|---|---|---|
| `source` | `AnimeHttpSource` | no | none | as `anime.source` only | Resolved on restore via `AnimeSourceManager.get(anime.source) as? AnimeHttpSource`. |
| `anime` | `tachiyomi.domain.entries.anime.model.Anime` | no | none | as `animeId` | Only `id`, `title`, `url`, `source` are used here. |
| `episode` | `tachiyomi.domain.items.episode.model.Episode` | no | none | as `episodeId` | Fields used: `id: Long`, `animeId: Long`, `seen: Boolean`, `bookmark: Boolean`, `fillermark: Boolean`, `sourceOrder: Long`, `url: String`, `name: String`, `scanlator: String?`. Source of truth: `domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt:7-27`. |
| `changeDownloader` | `Boolean` | no | `false` | no | Selects the alternative (external) downloader path when it differs from the preference (§5.1 of behaviour). |
| `video` | `Video?` (`var`) | yes | `null` | no | Resolved hoster video. Mutable. Participates in data-class `equals`/`hashCode` (defect, T-24a-051). |
| `_statusFlow` | `MutableStateFlow<State>` | no | `NOT_DOWNLOADED` | no | `@Transient`. Exposed as `statusFlow`. |
| `progressStateFlow` | `MutableStateFlow<Int>` | no | `0` | no | `@Transient`. Exposed as `progressFlow`. Values: `0..100`, or `-1` when content length is unknown (only from `ProgressListener.update`, which this area does not call for ffmpeg). |

Equality: data class over the five primary-constructor properties (`source`, `anime`, `episode`, `changeDownloader`, `video`).

### 1.2 `AnimeDownload.State` (`model/AnimeDownload.kt:62-68`)

| Name | `value: Int` | Meaning |
|---|---|---|
| `NOT_DOWNLOADED` | 0 | Removed or cleared; not in queue anymore (in-memory marker). |
| `QUEUE` | 1 | Waiting for a slot. |
| `DOWNLOADING` | 2 | In progress. |
| `DOWNLOADED` | 3 | Finalised; about to be removed from queue. |
| `ERROR` | 4 | Failed; remains in queue and store until retried or cancelled. |

"Active" = `value <= 2`. "Finished" = not active.

### 1.3 `AnimeDownloadPart` (`model/AnimeDownloadPart.kt:94-144`) — dead code, not persisted

| Field | Type | Default | Notes |
|---|---|---|---|
| `placingDir` | `UniFile` (private) | ctor arg | Directory for the part file. |
| `_file` | `UniFile?` (private) | `null` | Lazily created as `"${range.first}.part.tmp"`. |
| `range` | `Pair<Long, Long>` | ctor arg | Byte range `[first, second]` of the part. |
| `completed` | `Boolean` (`@Volatile`) | `false` | |
| `request` | `okhttp3.Request?` | `null` | |
| `listener` | `ProgressListener?` | `null` | |

Never instantiated. Dropped (T-24a-003).

### 1.4 `Video` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:30-46`) — fields this area reads or writes

| Field | Type | Default | Used |
|---|---|---|---|
| `videoUrl` | `String` (`var`) | `""` | Input to ffmpeg and ffprobe; overwritten with `file.uri.path` after download (`AnimeDownloader.kt:478`) and with torrent play URL (`:563`). |
| `videoTitle` | `String` | `""` | Torrent `addTorrent` title; `buildVideo` prefixes `"download: "`. |
| `headers` | `okhttp3.Headers?` | `null` | Per-video request headers; fallback to `source.headers`. |
| `subtitleTracks` | `List<Track>` | `emptyList()` | ffmpeg subtitle inputs and maps. |
| `audioTracks` | `List<Track>` | `emptyList()` | ffmpeg audio inputs and maps. |
| `ffmpegStreamArgs` | `List<Pair<String, String>>` | `emptyList()` | Inserted before the video `-i` as `-k "v"`. |
| `ffmpegVideoArgs` | `List<Pair<String, String>>` | `emptyList()` | Inserted before output as `-k "v"`. |
| `initialized` | `Boolean` | `false` | Set `true` by `buildVideo`. |
| `status` | `Video.State` (`@Transient @Volatile`, `var`) | `QUEUE` | Set to `LOAD_VIDEO`, `READY`, `ERROR`. |
| `preferred` | `Boolean` | `false` | Used by the hoster loader (outside this area). |

`Video.State` (`Video.kt:180-185`): `QUEUE`, `LOAD_VIDEO`, `READY`, `ERROR`.

`Track` (`Video.kt:11`): `data class Track(val url: String, val lang: String)`.

`Video.usesHttpServer()` (`Video.kt:151-165`): true if any of `videoUrl`, audio track urls or subtitle track urls matches `Regex("""http:\/\/localhost:1(?!\d)""")`.

`Video.copyHttpServer(port)` (`Video.kt:167-180`): replaces that regex match with `"http://localhost:$port"` in the video url and each track url.

`Video.MPV_ARGS_TAG = "ANIYOMI_MPV_ARGS"` (`Video.kt:188`) is not used in this area.

### 1.5 Persisted queue element `AnimeDownloadObject` (`DL/AnimeDownloadStore.kt:293`)

```kotlin
@Serializable
private data class AnimeDownloadObject(val animeId: Long, val episodeId: Long, val order: Int)
```

JSON example (the Kotlin serializer produces this shape):

```json
{"animeId":1234,"episodeId":98765,"order":3}
```

Decoded with `Json { ignoreUnknownKeys = true; explicitNulls = false }` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`). Undecodable values are skipped silently (`AnimeDownloadStore.kt:276-282`).

### 1.6 Persisted pending-delete entry (`DL/AnimeDownloadPendingDeleter.kt:292-318`)

```kotlin
@Serializable data class Entry(val episodes: List<EpisodeEntry>, val anime: AnimeEntry)
@Serializable data class EpisodeEntry(val id: Long, val url: String, val name: String, val scanlator: String? = null)
@Serializable data class AnimeEntry(val id: Long, val url: String, val title: String, val source: Long)
```

Example stored value under key `"1234"` in preference file `episodes_to_delete`:

```json
{"episodes":[{"id":98765,"url":"/ep/98765","name":"Episode 12","scanlator":null}],"anime":{"id":1234,"url":"/anime/1234","title":"Example","source":4242}}
```

With `explicitNulls = false`, a null `scanlator` is omitted entirely. Decoding relies on the default of `null`.

### 1.7 Cache snapshot (`DL/AnimeDownloadCache.kt:447-472`), `kotlinx.serialization.protobuf`

| Class | Field | Type | Serializer |
|---|---|---|---|
| `RootDirectory` | `dir` | `UniFile?` | `UniFileAsStringSerializer` (URI string or null) |
| `RootDirectory` | `sourceDirs` | `Map<Long, SourceDirectory>` (`var`, default `mapOf()`) | default |
| `SourceDirectory` | `dir` | `UniFile?` | URI string or null |
| `SourceDirectory` | `animeDirs` | `Map<String, AnimeDirectory>` (`var`, default `mapOf()`) | default |
| `AnimeDirectory` | `dir` | `UniFile?` | URI string or null |
| `AnimeDirectory` | `episodeDirs` | `MutableSet<String>` (`var`, default `mutableSetOf()`) | default |

Key of `sourceDirs` is the installed source id (`Long`). Keys of `animeDirs` are anime directory names (`DiskUtil.buildValidFilename(title)`). `episodeDirs` holds directory names or name-without-extension for `.mp4`/`.mkv` files.

Disk file: `context.cacheDir / "dl_anime_index_cache_v3"`. Version suffix `v3`. Increment the suffix in the port when the record shape changes.

---

## 2. Enum and integer value tables

| Enum or int | Values | Stored as | Location |
|---|---|---|---|
| `AnimeDownload.State` | 0 NOT_DOWNLOADED, 1 QUEUE, 2 DOWNLOADING, 3 DOWNLOADED, 4 ERROR | in memory only (never persisted; queue store does not store status) | `model/AnimeDownload.kt:62-68` |
| `Video.State` | QUEUE, LOAD_VIDEO, READY, ERROR (ordinal) | in memory only | `Video.kt:180-185` |
| `ChapterType` (timestamps, not used here) | Opening, Ending, Recap, MixedOp, Other | n/a | `Video.kt:14-20` |
| ffmpeg log level threshold | `Level.AV_LOG_WARNING` (forward when `log.level <= AV_LOG_WARNING`) | n/a | `AnimeDownloader.kt:595` |
| Notification IDs | -203 progress, -204 error | system | `Notifications.kt:41,44` |
| Retry attempt counter | 0-based in `retryWhen`; retry while `attempt < 3` | in memory | `AnimeDownloader.kt:521` |
| Pending-delete JSON key | `anime.id.toString()` | SharedPreferences key | `AnimeDownloadPendingDeleter.kt:188,209` |
| Queue store key | `episode.id.toString()` | SharedPreferences key | `AnimeDownloadStore.kt:231` |

Bit-flag layouts: none in this area.

---

## 3. Preference keys (all read here)

Type and default from `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt` unless noted. Keys are exactly as in the Kotlin.

| Key | Accessor | Type | Default | Used in behaviour section |
|---|---|---|---|---|
| `pref_download_only_over_wifi_key` | `downloadOnlyOverWifi()` | Boolean | `true` | §4.2 |
| `use_external_downloader` | `useExternalDownloader()` | Boolean | `false` | §5.1 |
| `external_downloader_selection` | `externalDownloaderSelection()` | String | `""` | §5.4 |
| `pref_download_fillermarked` | `downloadFillermarkedItems()` | Boolean | `false` | §3.1 |
| `pref_remove_bookmarked` | `removeBookmarkedChapters()` | Boolean | `false` | §3.2 |
| `remove_exclude_anime_categories` | `removeExcludeAnimeCategories()` | StringSet (category ids as strings) | `emptySet()` | §3.2 |
| `download_slots` | `numberOfDownloads()` | Int | `1` | Not used by anime engine (§11 behaviour). |
| `download_speed_limit` | `downloadSpeedLimit()` | Int | `0` | Not used by anime engine. |
| `save_chapter_as_cbz` | `saveChaptersAsCBZ()` | Boolean | `true` | Manga only. Not used here. |
| `split_tall_images` | `splitTallImages()` | Boolean | `true` | Manga only. |
| `auto_download_while_watching` | `autoDownloadWhileWatching()` | Int | `0` | Read by the player, not this area. |
| `remove_after_read_slots` | `removeAfterReadSlots()` | Int | `-1` | Read by the remove-after-read logic (other area). |
| `pref_remove_after_marked_as_read_key` | `removeAfterMarkedAsRead()` | Boolean | `false` | Other area. |
| `download_new_episode` | `downloadNewEpisodes()` | Boolean | `false` | Library update (other area). |
| `download_new_anime_categories` | `downloadNewEpisodeCategories()` | StringSet | `emptySet()` | Library update. |
| `download_new_anime_categories_exclude` | `downloadNewEpisodeCategoriesExclude()` | StringSet | `emptySet()` | Library update. |
| `download_new_unread_episodes_only` | `downloadNewUnseenEpisodesOnly()` | Boolean | `false` | Library update. |
| `hide_notification_content` | `SecurityPreferences.hideNotificationContent()` (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:19`) | Boolean | `false` | §10.2 |
| Torrent server enable | `TorrentPreferences.torrServerEnable()` | Boolean | owned by area:13-core-util-storage-archive-torrent | §5.5 |

Keys read in the tables above but owned by other areas are listed for the port's preference loader. The port must read them with the same key names so that an imported Aniyomi preference file keeps working (T-24a-054).

Preference store: `tachiyomi.core.common.preference.PreferenceStore` (area:02-preferences-system). The Rust port reads these keys through its own preference store and must use the same key strings and defaults, so that a migrated preference set keeps its meaning.

---

## 4. Disk layout and file formats

### 4.1 Root and tree

Root = the user's configured downloads directory (`StorageManager.getDownloadsDirectory()`). Anime tree:

```
<root>/
  <Source name> (<LANG>)/                  e.g. "Example (EN)"
    <anime title, FAT-safe>/               e.g. "Some Show"
      <scanlator>_<episode name>/          finished episode dir (directory form)
         <episode name>.mkv                finished video (renamed from .tmp by the ffmpeg path)
         .nomedia                          created by DiskUtil.createNoMediaFile (may fail silently, behaviour §5.9)
      <scanlator>_<episode name>_tmp/      staging dir while downloading (suffix "_tmp")
         <episode name>.tmp                ffmpeg output while remuxing
         <episode name>.mkv                after success, before finalise
         <anime title> - <episode>_tmp.mkv external-downloader placeholder
```

Filename rules: `DiskUtil.buildValidFilename` (see behaviour §9.2).

Note the layout is a directory per episode even for a single video. `DiskUtil.createNoMediaFile` writes `.nomedia` in the staging dir before rename (behaviour §5.9).

### 4.2 Other on-disk files

| File | Location | Format | Owner | Notes |
|---|---|---|---|---|
| Queue store | Android SharedPreferences `active_downloads` (`AnimeDownloadStore.kt:176`) | XML prefs; values are JSON strings | this area | Rust: SQLite table (§5.1). |
| Pending deletes | SharedPreferences `episodes_to_delete` (`AnimeDownloadPendingDeleter.kt:158`) | XML prefs; values JSON | this area | Rust: SQLite table (§5.2). |
| Index cache | `cacheDir/dl_anime_index_cache_v3` | kotlinx ProtoBuf | this area | Rust: bincode snapshot in `$XDG_CACHE_HOME/kurobayomi/dl_anime_index_v3.bin` (T-24a-035). |
| Downloads root marker | none | n/a | | |
| `.nomedia` | inside each finished episode dir | empty file | | Required for gallery hiding (Linux: harmless, keep for Aniyomi interop). |

### 4.3 Media formats

- Finished container: Matroska (`-f matroska`), with codecs copied: video, audio, subtitle (`-c:v copy -c:a copy -c:s copy`). Attachments copied via `-map 0:t?`.
- Accepted finished-file extensions when counting cache entries: `.mkv`, `.mp4` (`AnimeDownloadCache.kt:378-380`); folders are counted as entries regardless of content.
- Playable offline: any file whose MIME type contains `video` (`AnimeDownloadManager.kt:413`). On Linux, use extension `mkv`, `mp4`, `webm`, `m4v`, `mov`, `avi`, `ts`. This is a change; see T-24a-039.

---

## 5. Database tables and SQL

This area has no SQL of its own. It reads through domain interactors: `GetAnime.await(id)`, `GetEpisode.await(id)`, `GetAnimeCategories.await(animeId)`. Their tables are owned by area:04-db-anime-schema-migrations and area:05-data-repos-entries-items.

Because the Kotlin code persists queue and pending-delete state in SharedPreferences, the Rust port needs the following tables. These are proposals, but their columns are fixed here so that other areas can rely on them.

### 5.1 `anime_download_queue` (replaces `active_downloads`)

```sql
CREATE TABLE IF NOT EXISTS anime_download_queue (
    episode_id   INTEGER NOT NULL PRIMARY KEY,   -- one row per episode, like the Kotlin key
    anime_id     INTEGER NOT NULL,
    ord          INTEGER NOT NULL,                -- queue order; counter per process, see behaviour §18
    changed_at   INTEGER NOT NULL DEFAULT (strftime('%s','now'))
);
CREATE INDEX IF NOT EXISTS idx_anime_download_queue_ord ON anime_download_queue(ord);
```

Operations (match `AnimeDownloadStore`):
- `addAll(downloads)`: `INSERT OR REPLACE INTO anime_download_queue(episode_id, anime_id, ord) VALUES (?,?,?)` for each, with `ord` = `counter++`.
- `remove(d)`: `DELETE FROM anime_download_queue WHERE episode_id = ?`.
- `removeAll(list)`: same, in one transaction.
- `clear()`: `DELETE FROM anime_download_queue`.
- `restore()`: `SELECT anime_id, episode_id FROM anime_download_queue ORDER BY ord ASC`, then resolve entities; drop unresolved rows, then `clear()` and re-`addAll`.

Note: `ord` is a process counter starting at 0 in the Kotlin code. Because restore re-adds with a fresh counter, `ord` values are always contiguous from 0 after a restart. Use `INSERT ... ORDER BY` semantics to avoid collisions.

### 5.2 `anime_download_pending_delete`

```sql
CREATE TABLE IF NOT EXISTS anime_download_pending_delete (
    episode_id   INTEGER NOT NULL PRIMARY KEY,
    anime_id     INTEGER NOT NULL,
    episode_url  TEXT    NOT NULL,
    episode_name TEXT    NOT NULL,
    scanlator    TEXT,                            -- nullable (Kotlin: String?)
    anime_url    TEXT    NOT NULL,
    anime_title  TEXT    NOT NULL,
    anime_source INTEGER NOT NULL,
    seq          INTEGER NOT NULL                  -- insertion order within anime
);
CREATE INDEX IF NOT EXISTS idx_anime_download_pending_anime ON anime_download_pending_delete(anime_id, seq);
```

Operations:
- `addEpisodes(episodes, anime)`: insert each episode not already present for that `anime_id` (dedupe by `episode_id`, both against existing rows and within the batch — this fixes the observed defect in behaviour §7). Skip the call entirely if nothing new.
- `getPendingEpisodes()`: `SELECT * FROM ... ORDER BY anime_id, seq`; group by `anime_id`; then `DELETE FROM anime_download_pending_delete` in the same transaction as the read. Proposal (fixes the lost-deletes defect): delete only rows whose source resolved, keep the rest.

### 5.3 Index cache

Not in SQL. In-memory `HashMap<SourceId, SourceDir>`, persisted as a bincode snapshot (§4.2). The snapshot never contains the user's files, only names.

---

## 6. Network endpoints

This area issues no HTTP requests itself. Network use happens through:

| Caller | Request | Method | Headers | Auth | Response handling |
|---|---|---|---|---|---|
| ffmpeg (`ffmpegDownload`) | the video URL from `Video.videoUrl`, plus each subtitle and audio track URL that starts with `http` | GET (ffmpeg chooses) | `-headers '<Name: value\r\n>...'` from `video.headers ?: source.headers` | whatever the source sets in headers (cookies, referer, user agent) | Matroska output; success = exit code success |
| ffprobe (`getDuration`) | same video URL | GET (ffprobe chooses) | same `-headers` prefix (always present) | same | duration in seconds as text; `toFloatOrNull` |
| torrent server (`torrentDownload`, area:13-core-util-storage-archive-torrent) | `torrentServerApi.hostUrl` + `addTorrent(url, title, "", "", false)` and `getTorrentPlayLink(torrent, index)` | per area:13-core-util-storage-archive-torrent | none in this area | none in this area | returns play URL used as ffmpeg input |
| Local HTTP server (`createHttpServer`, `HttpServerService`) | none (server side) | n/a | n/a | n/a | port from `listeningPort` or `HttpServerService.port`; URLs rewritten to `http://localhost:<port>` |
| External download apps | Android intent only, no HTTP | n/a | n/a | n/a | dropped on Linux (T-24a-024) |

Headers encoding detail: each header pair becomes `Name: value` followed by real CR LF (`\r\n`), all inside one `-headers '...'` argument. Rust port: pass as a single argv element `"Name: value\r\nName2: value2\r\n"`, no shell quoting.

Torrent path detail: the torrent server URL pattern parsed from the source-provided URL: `...link=<hash>&index=<n>...`, converted to `magnet:?xt=urn:btih:<hash>&index=<n>` before `addTorrent` (`AnimeDownloader.kt:546-551`). `isTorrent` treats `magnet...`, `*.torrent`, or `<torrentServerApi.hostUrl>...` as torrent (`:533-536`).

---

## 7. Files on disk: naming summary

| Name pattern | Created by | Removed by |
|---|---|---|
| `<scanlator>_<name>_tmp` (dir) | `downloadEpisode` `createDirectory(episodeDirname + "_tmp")` | renamed to `<scanlator>_<name>` on success; deleted in ADM path |
| `<name>.tmp` (file) | `downloadVideo` `createFile("$filename.tmp")` | renamed to `<name>.mkv` on ffmpeg success; deleted on failure or cancel |
| `<name>.mkv` (file) | ffmpeg rename | left in finished dir; renamed-into place by dir rename |
| `<anime> - <name>_tmp.mkv` (file) | `downloadVideoExternal` | deleted before finalise (defect, §5.4) |
| `.nomedia` | `DiskUtil.createNoMediaFile` | never removed by this area |
| `dl_anime_index_cache_v3` | `updateDiskCache` | `invalidateCache` |

---

## 8. Mapping table: Kotlin persisted field to Rust column

| Kotlin field | Kotlin location | Rust destination |
|---|---|---|
| `AnimeDownloadObject.animeId` | `AnimeDownloadStore.kt:293` | `anime_download_queue.anime_id` |
| `AnimeDownloadObject.episodeId` | same | `anime_download_queue.episode_id` |
| `AnimeDownloadObject.order` | same | `anime_download_queue.ord` |
| `Entry.episodes[].id` | `AnimeDownloadPendingDeleter.kt:302-307` | `anime_download_pending_delete.episode_id` |
| `Entry.episodes[].url` | same | `episode_url` |
| `Entry.episodes[].name` | same | `episode_name` |
| `Entry.episodes[].scanlator` | same | `scanlator` (nullable) |
| `Entry.anime.id` | `:313-318` | `anime_id` |
| `Entry.anime.url` | same | `anime_url` |
| `Entry.anime.title` | same | `anime_title` |
| `Entry.anime.source` | same | `anime_source` |
| cache `SourceDirectory.dir`, `AnimeDirectory.dir` | `AnimeDownloadCache.kt:458-472` | path string in bincode snapshot |
| cache `episodeDirs` | same | `HashSet<String>` in snapshot |

---

## 9. Deprecated or unused data in scope

- `AnimeDownloadPart` (behaviour §1.3): dead class.
- `ensureSuccessfulAnimeDownload` (`AnimeDownloader.kt:829-851`): dead function.
- `ProgressListener` implementation on `AnimeDownload` (`AnimeDownload.kt:22,53-60`): `update()` is never invoked from this area. Keep `progress` as a plain field.
- `Video` constructors marked `@Deprecated(DeprecationLevel.HIDDEN)` (ext lib 16 compatibility): not relevant to Rust.
