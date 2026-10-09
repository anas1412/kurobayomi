# Area 23 — Manga downloads: data models, configuration, disk and network

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Proposal items are marked **Proposal:**. This area has no bit-flag layouts: its only numeric enum is the download state, and the notification ids are plain constants.

---

## 1. In-memory models

### 1.1 `MangaDownload` (app/src/main/java/eu/kanade/tachiyomi/data/download/manga/model/MangaDownload.kt)

| Field | Kotlin type | Nullability | Default | Persisted | Notes |
|---|---|---|---|---|---|
| source | `HttpSource` | non-null | from `sourceManager.get(manga.source)` | no (rebuilt from `manga.source`) | must be an `HttpSource` |
| manga | `Manga` (`tachiyomi.domain.entries.manga.model.Manga`) | non-null | from DB | by `manga.id` only | data-class equality compares all fields |
| chapter | `Chapter` (`tachiyomi.domain.items.chapter.model.Chapter`) | non-null | from DB | by `chapter.id` only | data-class equality compares all fields |
| pages | `List<Page>?` | nullable | null until page list fetched | no | not part of equality |
| status | `State` (var, backed by `MutableStateFlow`) | non-null | NOT_DOWNLOADED | no | see state values in 1.4 |
| statusFlow | `StateFlow<State>` | non-null | derived | no | |
| progressFlow | `Flow<Int>` | non-null | derived | no | average of page progress, debounce 50 ms |
| totalProgress | `Int` (computed) | non-null | `pages?.sumOf(Page::progress) ?: 0` | no | SUM of page progress |
| progress | `Int` (computed) | non-null | average of page progress, 0 if no pages | no | AVERAGE; not used by the queue UI |
| downloadedImages | `Int` (computed) | non-null | count of pages with status READY | no | |

### 1.2 `Page` (source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt)

| Field | Type | Nullability | Default | Serialised | Notes |
|---|---|---|---|---|---|
| index | `Int` | non-null | constructor | yes | 0-based. Downloader re-indexes (Dl.kt:362-369) |
| url | `String` | non-null | `""` | yes | page URL from the source |
| imageUrl | `String?` (var) | nullable | null | yes | image URL, filled by `getImageUrl` when empty |
| uri | `Uri?` (var, transient) | nullable | null | no | deprecated; set to the image file URI after download |
| number (computed) | `Int` | non-null | `index + 1` | no | |
| status (var) | `Page.State` | non-null | `QUEUE` | no | |
| progress (var) | `Int` | non-null | 0 | no | 0..100, or -1 when the content length is unknown (Page.kt `update`) |
| progressFlow | `StateFlow<Int>` | non-null | 0 | no | |

`Page.State` ordinal names: `QUEUE`, `LOAD_PAGE`, `DOWNLOAD_IMAGE`, `READY`, `ERROR` (Page.kt:51-57). Ordinals are not stored anywhere in this area.

### 1.3 `Manga` and `Chapter` fields this area reads

| Entity | Field | Used for |
|---|---|---|
| Manga | id (Long), url, title, source (Long), description, author, artist, genre (List<String>?), status (Long) | folder name, ComicInfo, source lookup |
| Chapter | id (Long), mangaId, url, name, scanlator (String?), read, bookmark, sourceOrder (Long), chapterNumber (Double), dateUpload | folder name, queue order, filters, ComicInfo |

`Chapter.sourceOrder` is set from the index in the source's list (`SyncChaptersWithSource.kt:69`, `copy(mangaId = manga.id, sourceOrder = i.toLong())`).

### 1.4 `MangaDownload.State` (model file)

| Name | Stored/compared value | Used for |
|---|---|---|
| NOT_DOWNLOADED | 0 | "not queued" (transient) |
| QUEUE | 1 | queued or paused |
| DOWNLOADING | 2 | in progress |
| DOWNLOADED | 3 | finished |
| ERROR | 4 | failed, kept in queue |

"Active" comparisons use `value <= 2`. Proposal: keep the same numeric values in the Rust enum (`#[repr(u8)]`) because the queue table stores the status column for crash recovery (section 3.2).

### 1.5 `MangaDownloadHeaderItem` (ui file)

Fields: `id: Long` (source id), `name: String` (source display name), `size: Int` (count of rows). Equality: `id`, `name`, `size`, `subItemsCount`, and identity of `subItems`.

### 1.6 `MangaDownloadItem` (ui file)

Field: `download: MangaDownload`, `header: MangaDownloadHeaderItem`. Equality: `chapter.id` equal. `hashCode`: `chapter.id.toInt()` (truncates; Proposal: use the full `i64`).

---

## 2. Persisted records (what the Kotlin writes today)

### 2.1 SharedPreferences `active_downloads` (MangaDownloadStore.kt)

- Key: `chapter.id.toString()` (decimal).
- Value: JSON string of `DownloadObject`:

```json
{"mangaId": <Long>, "chapterId": <Long>, "order": <Int>}
```

- `order` is a per-process counter starting at 0, incremented on every write (`counter++`). Restore sorts by `order`.
- Serialiser: `kotlinx.serialization.json.Json` injected from DI. Unknown keys: Kotlin default (fails); undecodable values are skipped.

### 2.2 SharedPreferences `chapters_to_delete` (MangaDownloadPendingDeleter.kt)

- Key: `manga.id.toString()`.
- Value: JSON string of `Entry`:

```json
{
  "chapters": [ {"id": <Long>, "url": "<string>", "name": "<string>", "scanlator": "<string or null>"} ],
  "manga":    {"id": <Long>, "url": "<string>", "title": "<string>", "source": <Long>}
}
```

- `scanlator` is nullable with default null (`ChapterEntry`, Del.kt:170-175), so it may be omitted on decode.

### 2.3 Disk index `dl_index_cache_v3` (MangaDownloadCache.kt)

- Location: `context.cacheDir/dl_index_cache_v3` (`diskCacheFile`, Cache.kt:97-98).
- Format: kotlinx ProtoBuf of `RootDirectory`.
- Tree:
  - `RootDirectory { dir: UniFile? (as URI string), sourceDirs: Map<Long, SourceDirectory> }`
  - `SourceDirectory { dir: UniFile? (URI string), mangaDirs: Map<String, MangaDirectory> }`
  - `MangaDirectory { dir: UniFile? (URI string), chapterDirs: MutableSet<String> }`
- Map keys: `sourceDirs` by source id; `mangaDirs` by the manga folder name (`getMangaDirName`); `chapterDirs` by chapter folder name **without** `.cbz` for CBZ files.
- Written 1 second after the last change (debounced). Deleted on invalidation. Deleted if it fails to decode.

Proposal: replace with an SQLite table in the app database (section 3.1), or a versioned bincode file under the cache directory. Either way, bump the file name version and never read `dl_index_cache_v3`.

### 2.4 ComicInfo.xml (written inside each chapter folder before archiving)

File name: `ComicInfo.xml` (`COMIC_INFO_FILE`, core-metadata/.../ComicInfo.kt:9). Namespace declarations `xmlns:xsd="http://www.w3.org/2001/XMLSchema"` and `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"`. Root element `<ComicInfo>`.

| Element (order) | Source | Notes |
|---|---|---|
| `<Title>` | `chapter.name` | |
| `<Series>` | `manga.title` | |
| `<Number>` | chapter number (see behaviour 10.10) | omitted if negative |
| `<Summary>` | `manga.description` | omitted if null |
| `<Writer>` | `manga.author` | omitted if null |
| `<Penciller>` | `manga.artist` | omitted if null |
| `<Inker>`, `<Colorist>`, `<Letterer>`, `<CoverArtist>` | null | never written |
| `<Translator>` | `chapter.scanlator` | omitted if null |
| `<Genre>` | `manga.genre` joined with `", "` | omitted if null |
| `<Tags>` | null | never written |
| `<Web>` | `urls` joined with `" "` | always written, may be empty |
| `<ty:PublishingStatusTachiyomi>` (ns `http://www.w3.org/2001/XMLSchema`, prefix `ty`) | status text | "Ongoing", "Completed", "Licensed", "Publishing finished", "Cancelled", "On hiatus", "Unknown" |
| `<ty:Categories>` | category names joined with `", "` | omitted if no categories |
| `<ay:SourceAniyomi>` (prefix `ay`, same namespace URI) | `source.name` | Aniyomi extension |

Element text is the `@XmlValue` of each wrapper; the encoder is xmlutil (`nl.adaptivity.xmlutil`), so text is XML-escaped. Proposal: use `quick-xml` with the same element names and order.

### 2.5 CBZ archive (`<chapter>.cbz`)

- ZIP, method STORE (no compression), UTF-8 names, flat (no directories), entries = every file present in the chapter's `_tmp` folder at archive time (page images and `ComicInfo.xml`), entry order = directory listing order.
- Written to `<chapter>.cbz_tmp`, renamed to `<chapter>.cbz` on success, then the `_tmp` folder is deleted.

---

## 3. Proposal: Rust storage

### 3.1 SQLite tables (in the manga database owned by area:03; created by a migration step that area:03 numbers)

The chapter key follows the Kotlin table `chapters(_id)` (data/src/main/sqldelight/data/chapters.sq:3).

```sql
CREATE TABLE manga_download_queue (
    chapter_id   INTEGER NOT NULL PRIMARY KEY REFERENCES chapters(_id) ON DELETE CASCADE,
    manga_id     INTEGER NOT NULL REFERENCES mangas(_id) ON DELETE CASCADE,
    source_id    INTEGER NOT NULL,
    position     INTEGER NOT NULL,           -- queue order, 0 = first
    status       INTEGER NOT NULL DEFAULT 1, -- MangaDownload.State value at last write (1 QUEUE, 4 ERROR)
    enqueued_at  INTEGER NOT NULL            -- unix millis
);
CREATE INDEX manga_download_queue_position ON manga_download_queue(position);
CREATE INDEX manga_download_queue_source ON manga_download_queue(source_id, position);

CREATE TABLE manga_pending_delete (
    chapter_id   INTEGER NOT NULL PRIMARY KEY,   -- no FK: the chapter may already be gone
    manga_id     INTEGER NOT NULL,
    manga_title  TEXT    NOT NULL,
    manga_url    TEXT    NOT NULL,
    source_id    INTEGER NOT NULL,
    chapter_name TEXT    NOT NULL,
    chapter_url  TEXT    NOT NULL,
    scanlator    TEXT,
    enqueued_at  INTEGER NOT NULL
);
CREATE INDEX manga_pending_delete_manga ON manga_pending_delete(manga_id);
```

Notes:

- `position` replaces the Kotlin `order` counter. Keep it dense (0..n-1) after every reorder, in one transaction.
- The status column is written when the queue changes, not on every status flip. On restart, rows are reset to QUEUE (section 3.3), so the stored value is informational.
- `manga_pending_delete` has no foreign key to `chapters`, because the reader may enqueue a chapter whose manga is later removed. Rows for a missing manga are dropped by the flush.
- Both tables replace SharedPreferences. Migration from Aniyomi backups is not needed (section 6).

### 3.2 Queue store contract (Rust)

- `load_queue(conn) -> Vec<QueueRow>` ordered by `position`.
- `replace_queue(conn, rows)` in one transaction (delete all, insert all) for `updateQueue`.
- `upsert_one`, `delete_one`, `delete_many_by_chapter`, `delete_by_manga` for incremental operations.
- Writes happen on the downloader thread through a single writer connection (rusqlite, `bundled`, WAL mode from area:03).

### 3.3 Restore rules (Rust)

1. Load `manga_download_queue` ordered by `position`.
2. For each row: resolve manga, chapter and source. If the **source is not yet registered**, keep the row and retry when the source gate opens (behaviour 2.1 defect).
3. If manga or chapter is gone, delete the row and log it at info level.
4. Set status QUEUE. Do not restore DOWNLOADING.

### 3.4 Disk index (Rust)

```rust
pub struct DownloadIndex {
    pub root: PathBuf,
    pub sources: HashMap<i64, SourceDir>,   // keyed by source id
    pub built_at: Option<SystemTime>,
}
pub struct SourceDir {
    pub dir_name: String,                   // "Name (LANG)" after sanitising
    pub manga: HashMap<String, MangaDir>,   // keyed by sanitised manga folder name
}
pub struct MangaDir {
    pub dir_name: String,
    pub chapters: BTreeSet<String>,         // bare names: folder name, or cbz name without ".cbz"
}
```

Serialised as bincode (version header `KIDX\x01`), file name `download-index-v1.bin` under the cache dir. Read failures delete the file.

---

## 4. Files and folders on disk

### 4.1 Root

- Root = the downloads directory from `StorageManager.getDownloadsDirectory()` (area:09 / area:13). Linux default: `Paths::default_downloads_dir()` from `kuro-platform` (`$XDG_DOWNLOAD_DIR/Kurobayomi` or `~/Downloads/Kurobayomi`; decision in area:01).
- Invalid root: the directory cannot be created, and the error text is `Invalid location: <displayable path>` (key `invalid_location`).

### 4.2 Hierarchy

```
<root>/
  <source dir>/                     "<source name> (<LANG>)" sanitised, e.g. "MangaDex (EN)"
    <manga dir>/                    sanitised manga title
      <chapter>/                    folder mode: "<scanlator>_<name>" or "<name>", sanitised
        001.jpg                     page files, zero-padded to max(3, digits of page count)
        002.png                     (example: a 4-page chapter also has 003.webp and 004.jpg)
        003.webp
        004.jpg
        001__001.jpg                split parts for tall page 001 (see below)
        001__002.jpg
        ComicInfo.xml
      <chapter>.cbz                 CBZ mode: same name plus ".cbz"
      <chapter>_tmp/                in-progress folder (suffix "_tmp", TMP_DIR_SUFFIX)
        NNN.tmp                     in-progress page file
      <chapter>.cbz_tmp             in-progress archive
```

- Page file name: `"%0{digits}d".format(number)` with extension from the response type (table in behaviour 10.3). Split parts: `"%03d__%03d.jpg"` (prefix from `"%03d".format(number)`, part index 1-based).
- `.nomedia` is named by `DiskUtil.NOMEDIA_FILE` but not written on the download path (behaviour 8.4).
- Hidden or temp names never start with `.` except `.nomedia`.

### 4.3 Name sanitising

Function `buildValidFilename` (behaviour 8.3). Characters rejected: control characters `0x00..0x1F`, `0x7F`, and `"*/:<>?\|`. Leading and trailing `.` and space removed. Empty becomes `(invalid)`. Maximum 240 UTF-16 units.

Windows-readable Proposal (addendum rule 5): additionally map reserved names `CON PRN AUX NUL COM1..COM9 LPT1..LPT9` (case-insensitive, with or without an extension) by appending `_`, and strip trailing dots and spaces after truncation. The Linux output is otherwise the same as the Kotlin output for every title that does not contain a reserved name.

### 4.4 Other on-disk state

| Path | Created by | Removed by |
|---|---|---|
| `<cache>/dl_index_cache_v3` (Kotlin) | Cache.updateDiskCache | Cache.invalidateCache, decode failure |
| SharedPreferences `active_downloads.xml` (Android only) | Store | Store.clear |
| SharedPreferences `chapters_to_delete.xml` (Android only) | Del | Del.getPendingChapters |
| Kurobayomi tables (Proposal, section 3.1) | migration | flush or delete |

---

## 5. Preferences consumed by this area

Defaults are the exact Kotlin defaults. Preference classes are owned by area:02 and area:25; this table is the contract this area relies on.

| Key | Type | Default | Owner file | Read by |
|---|---|---|---|---|
| `pref_download_only_over_wifi_key` | Boolean | `true` | domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:9-12 | Job |
| `save_chapter_as_cbz` | Boolean | `true` | DownloadPreferences.kt:21 | Dl |
| `split_tall_images` | Boolean | `true` | DownloadPreferences.kt:23 | Dl |
| `download_speed_limit` | Int (KiB/s) | `0` | DownloadPreferences.kt:70 | Dl |
| `download_slots` | Int | `1` | DownloadPreferences.kt:69 (**not read by manga downloader**) | none in scope |
| `remove_exclude_categories` | StringSet of category ids | empty | DownloadPreferences.kt:39-42, key constant at :76 | Mgr |
| `pref_remove_bookmarked` | Boolean | `false` | DownloadPreferences.kt:35 | Mgr |
| `remove_after_read_slots` | Int | `-1` | DownloadPreferences.kt:28 | Reader (area:43), not here |
| `pref_remove_after_marked_as_read_key` | Boolean | `false` | DownloadPreferences.kt:30-33 | Reader and notification receiver, not here |
| `download_new` | Boolean | `false` | DownloadPreferences.kt:48 | library updater (area:26), not here |
| `download_new_categories` | StringSet | empty | DownloadPreferences.kt:51-54 | library updater, not here |
| `download_new_categories_exclude` | StringSet | empty | DownloadPreferences.kt:60-63 | library updater, not here |
| `download_new_unread_chapters_only` | Boolean | `false` | DownloadPreferences.kt:72 | library updater, not here |
| `auto_download_while_reading` | Int | `0` | DownloadPreferences.kt:25 | Reader, not here |
| `data_saver_downloader` | Boolean | `true` | app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:116 | Dl |
| `data_saver` | enum `DataSaver` | `NONE` | SourcePreferences.kt:97 | aniyomi/util/DataSaver.kt |
| `data_saver_excluded` | StringSet of source ids | empty | SourcePreferences.kt:114 | aniyomi/util/DataSaver.kt |
| `ignore_gif` | Boolean | `true` | SourcePreferences.kt:101 | DataSaver |
| `ignore_jpeg` | Boolean | `false` | SourcePreferences.kt:99 | DataSaver |
| `data_saver_image_quality` | Int | `80` | SourcePreferences.kt:103 | DataSaver |
| `data_saver_server` | String | `""` | SourcePreferences.kt:110 | DataSaver |
| `data_saver_color_bw` | Boolean | `false` | SourcePreferences.kt:112 | DataSaver |
| `hide_notification_content` | Boolean | `false` | core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:19 | Ntf |

Ignore-list details for DataSaver are in the owning area (26). The source-level exclusion check is in `aniyomi/util/DataSaver.kt` (`if (dataSaver != NONE && source.id.toString() in dataSaverExcludedSources()) NoOp`).

### 5.1 Proposal keys for the Rust settings (area:02 owns the schema)

| Key | Type | Default | Notes |
|---|---|---|---|
| `downloads.only_over_wifi` | bool | `true` | "Wi-Fi" means "not metered" on Linux |
| `downloads.save_as_cbz` | bool | `true` | |
| `downloads.split_tall_images` | bool | `true` | |
| `downloads.speed_limit_kib_s` | u32 | `0` | 0 = unlimited |
| `downloads.max_parallel_chapters` | u8 | `5` | Proposal: sources limit; replaces the dead `download_slots` |
| `downloads.pages_per_chapter` | u8 | `2` | Proposal: fixed at 2 for parity |
| `downloads.remove_exclude_categories` | list of category ids | `[]` | |
| `downloads.remove_bookmarked` | bool | `false` | |
| `downloads.remove_after_read_slots` | i32 | `-1` | owned by area:43 |
| `downloads.hide_notification_content` | bool | `false` | owned by area:02 |

---

## 6. Android-only state that is not migrated

- SharedPreferences `active_downloads` and `chapters_to_delete`, and the file `dl_index_cache_v3`, belong to an Android install. A Linux install starts empty. Task T-23a-039 records this as a drop.
- The download folder itself can be reused: the Kurobayomi index scans the same `<root>/<source>/<manga>/<chapter>` layout, so a copied Aniyomi download folder is recognised once the source names match (the same `"Name (LANG)"` format is produced).

---

## 7. Network endpoints touched by downloads

This area issues no requests of its own. Every request goes through the source (`HttpSource`) or through the data-saver proxy.

| Call | Method | URL | Headers | Response used | Source of truth |
|---|---|---|---|---|---|
| `source.getPageList(chapter)` | per source | source-defined | source headers, `HttpSource` client | `List<Page>` | area:15 |
| `source.getImageUrl(page)` | per source | source-defined | source headers | String URL | area:15 |
| `source.getImage(page)` | GET | `page.imageUrl` | source headers | body bytes, `Content-Type` | area:15 |
| `source.getChapterUrl(chapter)` | none (string build) | `HttpSource.getChapterUrl` | none | String (for ComicInfo `Web`) | area:15 |
| Data saver: Bandwidth Hero | GET | `{server}/?jpg={0\|1}&l={quality}&bw={0\|1}&url={urlencoded}` | source headers not added | body bytes | aniyomi/util/DataSaver.kt:83-87 |
| Data saver: wsrv.nl | GET | `https://wsrv.nl/?url={raw url}` + `&output=jpg\|webp` + `&q={quality}` (+ `&n=-1` for animated) | none | body bytes | aniyomi/util/DataSaver.kt:113-136 |
| Data saver: resmush.it | GET | `http://api.resmush.it/ws.php?img={url}&qlty={quality}` | none | JSON body; the URL is the text between `"dest":"` and `",` | aniyomi/util/DataSaver.kt:165-169 |
| Help link on large bulk warning | (opens browser) | `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads` | none | none | MangaLibraryUpdateNotifier.kt:392-393 |

Proposal notes:

- The resmush.it data saver uses plain HTTP and parses JSON with string search. Proposal: use `serde_json` and HTTPS, or drop the option (area:26 decides).
- Bandwidth Hero builds its URL with `URLEncoder.encode(imageUrl, "utf-8")`. Proposal: `url::form_urlencoded` with the same character set.
- Image requests respect the per-source client (cookies, user agent, rate limit) from area:12. The downloader does not set headers itself.

---

## 8. Constants and literals (full list)

| Literal | Value | Location |
|---|---|---|
| `TMP_DIR_SUFFIX` | `"_tmp"` | Dl companion |
| `WARNING_NOTIF_TIMEOUT_MS` | `30_000L` | Dl companion |
| `CHAPTERS_PER_SOURCE_QUEUE_WARNING_THRESHOLD` | `15` | Dl companion |
| `DOWNLOADS_QUEUED_WARNING_THRESHOLD` | `30` | Dl companion (private) |
| `MIN_DISK_SPACE` | `200L * 1024 * 1024` | Dl.kt:773-774 (private top-level) |
| Index renew interval | `1.hours.inWholeMilliseconds` | Cache.kt:84 |
| Index source wait | `30.seconds` | Cache.kt:346 |
| Index indicator debounce | `1000L` | Cache.kt:94 |
| Disk write debounce | `delay(1000)` | Cache.kt:426 |
| Disk cache file name | `"dl_index_cache_v3"` | Cache.kt:98 |
| Restore store name | `"active_downloads"` | Store.kt (SharedPreferences name) |
| Pending store name | `"chapters_to_delete"` | Del.kt:26-29 |
| Job tag | `"MangaDownloader"` | Job companion |
| Notification ids | -201 progress, -202 error | Notifications.kt:40, 43 |
| Notification channel ids | `downloader_progress_channel`, `downloader_error_channel` | Notifications.kt:39, 42 |
| Notification group | `group_downloader` | Notifications.kt:38 |
| Title caps | 15 and 30 characters | Ntf.kt:104, 110 |
| Chop replacement | `"…"` (U+2026) | StringExtensions.kt:12 |
| Name cap | `take(240)` | DiskUtil.kt (buildValidFilename) |
| Reserved suffix for NOMEDIA | `".nomedia"` | DiskUtil.kt:140 |
| Image type table | see behaviour 10.3 | ImageUtil.kt:104-113 |
| Split JPEG quality | `100` | ImageUtil.kt (splitTallImage) |
| Tall rule | `> 3` with Int division | ImageUtil.kt isTallImage |
| Retry attempts | 3 retries (4 attempts) | Dl.downloadImage |
| Retry delays | 2 s, 4 s, 8 s | Dl.downloadImage |
| Flat-map concurrency | 2 | Dl.downloadChapter |
| Source concurrency | 5 | Dl.launchDownloaderJob |
| Poll and debounce | 50 ms | MangaDownload.progressFlow, QVM |
| `stateIn` timeout | 5000 ms | QVM.isDownloaderRunning |
| Page status and state ordinals | see 1.2 and 1.4 | |

---

## 9. Data invariants (for tests and for the Rust port)

1. A chapter appears in the queue at most once (enforced by the PK in 3.1; Kotlin enforces it only by convention).
2. A chapter is in the queue iff its row exists in `manga_download_queue`.
3. `position` values are dense and unique after every queue write.
4. A chapter folder name is unique within a manga folder; collisions are treated as "already downloaded".
5. For every finished chapter there is exactly one of: a folder `<name>` or an archive `<name>.cbz` (never both; the archiver deletes the folder after writing the archive).
6. A finished folder contains exactly `pages` page keys (`NNN` or `NNN__001`) plus `ComicInfo.xml`, and no `.tmp` files.
7. The index never lists a chapter folder that does not exist on disk for longer than one renew interval (1 hour). Proposal: also refresh the index after every write from this area.
8. Pending deletes are removed in the same transaction that deletes their folders (Proposal).
