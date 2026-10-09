# Area 15 — Manga source API: data models, persistence, preferences, files, endpoints

Every field is listed with Kotlin type, nullability, default, and the Rust type proposed for Kurobayomi. "Rust" columns are PROPOSAL.

## 1. Data models

### 1.1 `SManga` (interface; concrete `SMangaImpl`) — source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt, SMangaImpl.kt

| Kotlin field | Kotlin type | Nullable | Default (SMangaImpl) | Meaning | Rust (PROPOSAL) |
|---|---|---|---|---|---|
| `url` | `String` | no (`lateinit`) | none; must be set | Path relative to `baseUrl`, starts with `/` by convention (see behaviour section 6) | `url: String` |
| `title` | `String` | no (`lateinit`) | none; must be set | Display title | `title: String` |
| `artist` | `String?` | yes | `null` | Artist name | `artist: Option<String>` |
| `author` | `String?` | yes | `null` | Author name | `author: Option<String>` |
| `description` | `String?` | yes | `null` | Synopsis | `description: Option<String>` |
| `genre` | `String?` | yes | `null` | Genres joined by `", "` (parsed by `getGenres`) | `genre: Option<String>` |
| `status` | `Int` | no | `0` (UNKNOWN) | Publication status code (table 1.1.1) | `status: i32` (kept raw; see `MangaStatus`) |
| `thumbnail_url` | `String?` | yes | `null` | Cover URL | `thumbnail_url: Option<String>` |
| `update_strategy` | `UpdateStrategy` | no | `ALWAYS_UPDATE` | Library update rule (table 1.4) | `update_strategy: UpdateStrategy` |
| `initialized` | `Boolean` | no | `false` | True after details were fetched through the HTTP source path (`HttpSource.fetchMangaDetails` sets it, HttpSource.kt:223) | `initialized: bool` |

Field names with underscores are kept exactly (`thumbnail_url`, `update_strategy`, `date_upload`, `chapter_number`) so that serde and the DB adapter names match Kotlin.

#### 1.1.1 Status codes (`SManga.companion`, SManga.kt:48-54)

| Name | Value |
|---|---|
| `UNKNOWN` | 0 |
| `ONGOING` | 1 |
| `COMPLETED` | 2 |
| `LICENSED` | 3 |
| `PUBLISHING_FINISHED` | 4 |
| `CANCELLED` | 5 |
| `ON_HIATUS` | 6 |

Any other integer is legal in the DB and in source output; the UI shows the `unknown` text for it (behaviour section 12).

#### 1.1.2 Methods
- `getGenres(): List<String>?` — see behaviour section 10.
- `copy(): SManga` — copies every field except none (url, title, artist, author, description, genre, status, thumbnail_url, update_strategy, initialized); returns a new `SMangaImpl`.
- `create(): SManga` — `SMangaImpl()`.

### 1.2 `SChapter` (interface; concrete `SChapterImpl`) — SChapter.kt, SChapterImpl.kt

| Kotlin field | Kotlin type | Nullable | Default (SChapterImpl) | Meaning | Rust (PROPOSAL) |
|---|---|---|---|---|---|
| `url` | `String` | no (`lateinit`) | none | Path relative to `baseUrl` | `url: String` |
| `name` | `String` | no (`lateinit`) | none | Chapter title as shown by the source (cleaned later by the app's `ChapterSanitizer`) | `name: String` |
| `date_upload` | `Long` | no | `0` (SChapterImpl.kt:11) | Upload time, epoch milliseconds; `0` means unknown | `date_upload: i64` |
| `chapter_number` | `Float` | no | `-1f` (SChapterImpl.kt:13) | Number as given by the source; `-1` means unknown | `chapter_number: f32` |
| `scanlator` | `String?` | yes | `null` | Group name | `scanlator: Option<String>` |

`copyFrom(other)` (SChapter.kt:19-25) copies `name`, `url`, `date_upload`, `chapter_number`, `scanlator` (not identity fields). Rust: `fn copy_from(&mut self, other: &SChapter)`.

`create(): SChapter` returns `SChapterImpl()`.

### 1.3 `Page` (open class) — model/Page.kt

| Field | Kotlin type | Nullable | Default | Persisted? | Rust (PROPOSAL) |
|---|---|---|---|---|---|
| `index` | `Int` | no | constructor arg (no default) | yes (cache JSON) | `index: i32` |
| `url` | `String` | no | `""` | yes | `url: String` |
| `imageUrl` | `String?` (`var`) | yes | `null` | yes (cache JSON) | `image_url: Option<String>` |
| `uri` | `android.net.Uri?` (`@Transient var`) | yes | `null` | no | replaced by `local_path: Option<PathBuf>` (not persisted) |
| `number` | `Int` (computed) | no | `index + 1` | no | `fn number(&self) -> i32` |
| `status` | `State` (backed by `MutableStateFlow`, `@Transient`) | no | `State.QUEUE` | no | `watch::Sender<PageState>` |
| `progress` | `Int` (backed by `MutableStateFlow`, `@Transient`) | no | `0` | no | `watch::Sender<i32>` |

`Page.update(bytesRead, contentLength, done)`: `progress = if (contentLength > 0) (100 * bytesRead / contentLength).toInt() else -1`.

`Page.State` enum (ordinals in declaration order, in-memory only, never persisted): `QUEUE = 0`, `LOAD_PAGE = 1`, `DOWNLOAD_IMAGE = 2`, `READY = 3`, `ERROR = 4`.

Serialization: `@Serializable` (kotlinx). Only `index`, `url`, `imageUrl` are serialized (others are transient). Encoded by the page list cache with the app `Json` (table 4.3). Rust: `#[derive(Serialize, Deserialize)] struct PageCacheEntry { index: i32, url: String, #[serde(skip_serializing_if = "Option::is_none")] image_url: Option<String> }` with `#[serde(rename = "imageUrl")]` on `image_url`.

### 1.4 `UpdateStrategy` (enum) — model/UpdateStrategy.kt

| Name | Ordinal (stored value) | Meaning |
|---|---|---|
| `ALWAYS_UPDATE` | 0 | Included in library updates (default) |
| `ONLY_FETCH_ONCE` | 1 | Skipped during library updates |

Stored as the ordinal (`MangaUpdateStrategyColumnAdapter.encode = value.ordinal.toLong()`, data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:29-34). Decoding an unknown value yields `ALWAYS_UPDATE`.

Rust: `#[repr(i64)] pub enum UpdateStrategy { AlwaysUpdate = 0, OnlyFetchOnce = 1 }` with `TryFrom<i64>` falling back to `AlwaysUpdate`.

### 1.5 `MangasPage` — model/MangasPage.kt

`data class MangasPage(val mangas: List<SManga>, val hasNextPage: Boolean)`. Rust: `pub struct MangasPage { pub mangas: Vec<SManga>, pub has_next_page: bool }`.

### 1.6 `Filter` (sealed) — model/Filter.kt

Full variant table (Kotlin constructor, default state, Rust variant):

| Kotlin class | Constructor | Default state | Stored state type | Notes | Rust variant (PROPOSAL) |
|---|---|---|---|---|---|
| `Filter.Header` | `(name: String)` | `0` (boxed `Any`) | `Any` (value unused) | Section heading, no value | `Filter::Header { name }` |
| `Filter.Separator` | `(name: String = "")` | `0` | `Any` | Divider, no value | `Filter::Separator { name }` |
| `Filter.Select<V>` | `(name, values: Array<V>, state: Int = 0)` | index `0` | `Int` (index into `values`) | No bounds check (see behaviour topic file) | `Filter::Select { name, values: Vec<String>, state: i32 }` |
| `Filter.Text` | `(name, state: String = "")` | `""` | `String` | | `Filter::Text { name, state: String }` |
| `Filter.CheckBox` | `(name, state: Boolean = false)` | `false` | `Boolean` | | `Filter::CheckBox { name, state: bool }` |
| `Filter.TriState` | `(name, state: Int = STATE_IGNORE)` | `0` | `Int` (0, 1, 2) | constants table 1.6.1 | `Filter::TriState { name, state: TriStateState }` |
| `Filter.Group<V>` | `(name, state: List<V>)` | none (must be given) | `List<V>` (nested filters) | Only nested `Filter` items are rendered | `Filter::Group { name, state: Vec<Filter> }` |
| `Filter.Sort` | `(name, values: Array<String>, state: Selection? = null)` | `null` | `Selection?` | `Selection(index: Int, ascending: Boolean)` | `Filter::Sort { name, values: Vec<String>, state: Option<SortSelection> }` |

Equality (Filter.kt:31-42): `name == other.name && state == other.state`; `hashCode = 31 * name.hashCode() + (state?.hashCode() ?: 0)`.

#### 1.6.1 TriState constants (Filter.kt:17-21)
| Name | Value | Helper |
|---|---|---|
| `STATE_IGNORE` | 0 | `isIgnored()` |
| `STATE_INCLUDE` | 1 | `isIncluded()` |
| `STATE_EXCLUDE` | 2 | `isExcluded()` |

The UI maps these to the app's `TriState` enum: `0 → DISABLED`, `1 → ENABLED_IS`, `2 → ENABLED_NOT`; any other value throws `IllegalStateException("Unknown TriState state: $this")` (SourceFilterMangaDialog.kt:162-180).

#### 1.6.2 Sort selection
`SortSelection { index: i32, ascending: bool }` (Kotlin data class `Filter.Sort.Selection`). `null` = no selection.

### 1.7 `FilterList` — model/FilterList.kt

`data class FilterList(val list: List<Filter<*>>) : List<Filter<*>> by list`; `FilterList(vararg fs)` builds from an array (empty array → `emptyList()`); `equals` always returns `false`; `hashCode = list.hashCode()`. Rust: `pub struct FilterList(pub Vec<Filter>)`.

### 1.8 `ResolvableSource` types — online/ResolvableSource.kt
`sealed interface UriType { Manga, Chapter, Unknown }` (data objects, no fields). Rust: `enum UriType { Manga, Chapter, Unknown }`.

### 1.9 `Video` / `VideoUrl` — util/VideoInfo.kt
`sealed class Video`; `data class VideoUrl(val url: String) : Video()`. Unused in manga code. Dropped (T-15a-045).

### 1.10 `ConfigurableSource` — no data model; see section 4.

## 2. Mapping to the domain model and the database

These mappings live outside the scope (app and domain). They are listed here because the field-level rules of this scope depend on them. Full logic in `p1-topic-models-and-sync.md`.

### 2.1 `SManga` ↔ domain `Manga` (app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt)

| SManga field | Domain/DB field | DB column (data/src/main/sqldelight/data/mangas.sq) | DB type | Null handling |
|---|---|---|---|---|
| `url` | `url` | `url` | `TEXT NOT NULL` | required |
| `title` | `title` | `title` | `TEXT NOT NULL` | required; detail merge writes it only for non-favourites with non-empty remote title |
| `artist` | `artist` | `artist` | `TEXT` | `null` in MangaUpdate means keep (coalesce) |
| `author` | `author` | `author` | `TEXT` | same |
| `description` | `description` | `description` | `TEXT` | same |
| `genre` (parsed) | `genre` | `genre` | `TEXT AS List<String>` | stored as `", "`-joined string; `null` keeps |
| `status` | `status` | `status` | `INTEGER NOT NULL` | written on every detail merge (`status = remote.status`) |
| `thumbnail_url` | `thumbnailUrl` | `thumbnail_url` | `TEXT` | `takeIf { notEmpty }` in the merge |
| `update_strategy` | `updateStrategy` | `update_strategy` | `INTEGER AS UpdateStrategy NOT NULL DEFAULT 0` | ordinal |
| `initialized` | `initialized` | `initialized` | `INTEGER AS Boolean NOT NULL` | `remote.initialized && local.initialized` in `Manga.copyFrom`; `true` in UpdateManga |
| (source id) | `source` | `source` | `INTEGER NOT NULL` | = `MangaSource.id` |

`mangas.sq` also defines `cover_last_modified INTEGER NOT NULL`, `last_modified_at INTEGER NOT NULL DEFAULT 0`, `version INTEGER NOT NULL DEFAULT 0`, which the detail merge updates (cover and version); those belong to area:03 and area:34.

### 2.2 `SChapter` ↔ domain `Chapter` ↔ `chapters` table (app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt, data/src/main/sqldelight/data/chapters.sq)

| SChapter field | Domain field | DB column | DB type | Rule |
|---|---|---|---|---|
| `url` | `url` | `url` | `TEXT NOT NULL` | identity for sync |
| `name` | `name` | `name` | `TEXT NOT NULL` | sanitized by `ChapterSanitizer` (area:07) |
| `scanlator` | `scanlator` | `scanlator` | `TEXT` | blank → null, trimmed (`copyFromSChapter`) |
| `date_upload` | `dateUpload` | `date_upload` | `INTEGER NOT NULL` | `0` is replaced during sync (`p1-topic-models-and-sync.md` section 5.3) |
| `chapter_number` | `chapterNumber` | `chapter_number` | `REAL NOT NULL` | `chapter_number.toDouble()`; then re-parsed by `ChapterRecognition` |
| (list index) | `sourceOrder` | `source_order` | `INTEGER NOT NULL` | `i` after `distinctBy { url }` |
| (app) | `mangaId` | `manga_id` | `INTEGER NOT NULL` | owner |

Domain defaults for a new chapter: `Chapter.create()` gives `url = ""`, `name = ""`, `dateUpload = -1`, `chapterNumber = -1.0`, `sourceOrder = 0` (domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:33-45).

### 2.3 `sources` table (data/src/main/sqldelight/data/sources.sq)

```
CREATE TABLE sources(
    _id INTEGER NOT NULL PRIMARY KEY,
    lang TEXT NOT NULL,
    name TEXT NOT NULL
);
```
Queries: `findAll`, `findOne WHERE _id = :id`, `upsert` (`INSERT ... ON CONFLICT(_id) DO UPDATE`). Written by `MangaStubSourceRepositoryImpl.upsertStubMangaSource(id, lang, name)` (data/src/main/java/tachiyomi/data/source/manga/MangaStubSourceRepositoryImpl.kt:26). Purpose: remember `name` and `lang` of sources that are no longer installed (stub sources) so their library entries still show a name. The `id` is the `MangaSource.id` from section 1 of the behaviour spec.

### 2.4 Source id as foreign key
`mangas.source` holds the source id. Changing a source's `name`, `lang` or `versionId` changes its id (behaviour 1.1), so all its rows are orphaned. The `versionId` migration path is the documented way out.

## 3. Enumerations and stored integer values (complete)

| Enum / constant set | Values | Stored where | Notes |
|---|---|---|---|
| `SManga` status | 0..6 (table 1.1.1) | `mangas.status` | raw integer, unknown values kept |
| `UpdateStrategy` | ALWAYS_UPDATE=0, ONLY_FETCH_ONCE=1 | `mangas.update_strategy` | ordinal |
| `Filter.TriState` | 0 ignore, 1 include, 2 exclude | not persisted (in-memory filter state; backup does not store browse filters) | |
| `Filter.Sort.Selection` | index (Int), ascending (Boolean) | not persisted | |
| `Page.State` | 0..4 | not persisted | |
| `UriType` | Manga, Chapter, Unknown | not persisted | |
| `genre` list | `", "`-joined | `mangas.genre` | see behaviour section 10 |

No bit-flag layout is defined inside this scope. The manga `viewer`/`chapter_flags` integers are defined by area:05/area:07 and are not produced by source code.

## 4. Preferences touched by this scope or its direct callers

### 4.1 Source-owned preference files (ConfigurableSource)
- Name: `source_<id>` where `<id>` is the decimal `Long` source id (`ConfigurableSource.preferenceKey()`, ConfigurableSource.kt:22).
- Storage: Android `SharedPreferences` (`MODE_PRIVATE`), one XML file per source, key/value pairs defined by the source. Types supported by the backup (`toBackupPreferences`) are the Android preference types, (app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt:60-77: `Int`, `Long`, `Float`, `String`, `Boolean`, and `Set<String>` values are exported; other value types are skipped).
- Keys and defaults are defined by each source; the app does not know them.
- Proposal: `source_prefs` table keyed by `(source_id INTEGER, key TEXT)` with `value` as JSON text, or one JSON file `source_<id>.json` per source under the config directory. Chosen in `p1-rust-design.md` section 7.

### 4.2 App-level preferences used by the scope (keys and defaults verbatim)

| Key | Kotlin accessor (file:line) | Type | Default | Used by |
|---|---|---|---|---|
| `default_user_agent` | `NetworkPreferences.defaultUserAgent()` (core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:19-24) | String | `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0` | `HttpSource.headersBuilder`, `UserAgentInterceptor` (trimmed) |
| `source_languages` | `SourcePreferences.enabledLanguages()` (app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:23-26) | StringSet | `setOf("all", "en", Locale.getDefault().language)` (LocaleHelper.kt:72-74) | `getNameForMangaInfo`, source filtering (area:17) |
| `show_nsfw_source` | `SourcePreferences.showNsfwSource()` (:28) | Boolean | `true` | Extension loader NSFW gate (area:18) |
| `data_saver` | `SourcePreferences.dataSaver()` (:97) | enum `DataSaver` | `NONE` | image URL transform (topic file) |
| `ignore_jpeg` | `dataSaverIgnoreJpeg()` (:99) | Boolean | `false` | data saver |
| `ignore_gif` | `dataSaverIgnoreGif()` (:101) | Boolean | `true` | data saver |
| `data_saver_image_quality` | `dataSaverImageQuality()` (:103) | Int | `80` | data saver |
| `data_saver_image_format_jpeg` | `dataSaverImageFormatJpeg()` (:105-107) | Boolean | `false` | data saver |
| `data_saver_server` | `dataSaverServer()` (:110) | String | `""` | Bandwidth Hero base URL |
| `data_saver_color_bw` | `dataSaverColorBW()` (:112) | Boolean | `false` | Bandwidth Hero `bw` parameter |
| `data_saver_excluded` | `dataSaverExcludedSources()` (:114) | StringSet | empty | sources excluded from data saver (source id strings) |
| `data_saver_downloader` | `dataSaverDownloader()` (:116) | Boolean | `true` | apply data saver to downloads |

`DataSaver` enum (SourcePreferences.kt:118-123): `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` (stored by name through `getEnum`).

### 4.3 Json instance (app DI)
`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`: `Json { ignoreUnknownKeys = true; explicitNulls = false }`. Used by `ChapterCache` (page list cache) and `defaultJson` (source-api util). Rust: `serde_json` with `#[serde(default)]` and unknown fields ignored (default serde behaviour); `Option::None` fields omitted on write.

## 5. Files, folders and formats on disk

| Item | Location (Android) | Format | Owner | Key | Notes |
|---|---|---|---|---|---|
| Chapter disk cache | `context.cacheDir/chapter_disk_cache` (ChapterCache.kt:35-40) | `DiskLruCache` (app version 1, 1 value per entry, max 100 MiB) | `ChapterCache` (app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt) | see below | PARAMETER_APP_VERSION = 1 (ChapterCache.kt:203), PARAMETER_VALUE_COUNT = 1 (:206), PARAMETER_CACHE_SIZE = 100 * 1024 * 1024 bytes (:209) |
| Page list cache entry | same directory | JSON text of `List<Page>` | ChapterCache | `md5("${mangaId}${chapterUrl}")` (ChapterCache.kt:197-199, key function `getKey`; `DiskUtil.hashKeyForDisk` = lowercase hex MD5, Hash.kt:24-50) | written on reader `recycle()` for online chapters (HttpPageLoader.kt:~250) |
| Image cache entry | same directory | raw image bytes in file `<md5(imageUrl)>.0` | ChapterCache.getImageFile (:130-134) | `md5(imageUrl)` | `isImageInCache` uses `diskCache.get(md5)` |
| Source preferences | Android `shared_prefs/source_<id>.xml` | XML | Android SharedPreferences | file name | see 4.1 |
| Download temporary files | chapter directory of the download (area:23) | `*.tmp` | MangaDownloader | `"$filename.tmp"` | deleted at start of each download (MangaDownloader.kt:~386-389) |

Proposal (Linux): `$XDG_CACHE_HOME/kurobayomi/chapter-cache/` (default `~/.cache/kurobayomi/chapter-cache/`) for page-list JSON and images, keyed by the same MD5 scheme so that cache keys stay stable across the migration; `$XDG_CONFIG_HOME/kurobayomi/` for everything else. The page-list cache size limit (100 MiB) is kept.

## 6. Network endpoints and request shapes

### 6.1 Defined by this scope (generic)
- Every request is `GET`. No request body, no authentication, no cookies are added by the scope itself (cookies and Cloudflare are area:12).
- URL = `baseUrl + manga.url` (details, chapter list), `baseUrl + chapter.url` (page list), `page.url` (image URL), `page.imageUrl` (image bytes). Source subclasses may override each builder.
- Headers: `User-Agent` (see 4.2), plus whatever the subclass adds in `headersBuilder()`.
- Responses: any 2xx body is parsed; any other status → `HttpException(code)`.
- Popular/search/latest endpoints, details/chapter/page HTML or JSON shape: defined by each source. Not part of Kotlin code in scope.

### 6.2 Data saver endpoints (defined in app/src/main/java/aniyomi/util/DataSaver.kt, used by `HttpSource.getImage`)

| Mode | Request sent (replaces the image URL) | Parameters | Rules |
|---|---|---|---|
| `BANDWIDTH_HERO` | `"$dataSavedServer/?jpg=$format&l=$quality&bw=$colorBW&url=$escapedUrl"` | `format` = `"1"` if `data_saver_image_format_jpeg` else `"0"`; `quality` = `data_saver_image_quality`; `colorBW` = `"1"`/`"0"` from `data_saver_color_bw`; `escapedUrl` = `URLEncoder.encode(imageUrl, "utf-8")` | `dataSavedServer` = `data_saver_server` with trailing `/` removed. If blank, or if the image URL already contains that server, the URL is unchanged. `.jpeg`/`.jpg` (case-insensitive) → unchanged when `ignore_jpeg`; `.gif` → unchanged when `ignore_gif`. |
| `WSRV_NL` | `"https://wsrv.nl/?url=$imageUrl" + suffix` | suffix for `.webp` or `.gif` in URL: if format flag false `&q=$quality&n=-1`, else `&output=jpg&q=$quality&n=-1`; otherwise: if format flag true `&output=jpg&q=$quality`, else `&output=webp&q=$quality` | `imageUrl` is NOT URL-encoded (observed). `.jpeg/.jpg` → unchanged when `ignore_jpeg`; `.gif` → unchanged when `ignore_gif`. |
| `RESMUSH_IT` | `GET http://api.resmush.it/ws.php?img=$imageUrl&qlty=$quality` (performed synchronously with `client.newCall(...).execute()`), then the image URL is taken from the response text between `"dest":"` and `",` | `img` not encoded (observed); plain `http` scheme (observed) | Same `ignore_jpeg`/`ignore_gif` rules. Response parsing is `substringAfter("\"dest\":\"").substringBefore("\",")`; a non-matching response yields a garbage URL (observed). |

The data saver is applied by temporarily replacing `page.imageUrl` with the compressed URL for one request, then restoring it in `finally` (DataSaver.kt:30-41).

## 7. Database tables touched by this scope (summary)

| Table | Written by | Read by | Columns used |
|---|---|---|---|
| `mangas` | detail merge (`UpdateManga`, `Manga.copyFrom`) | library, browse, detail | `url, title, artist, author, description, genre, status, thumbnail_url, update_strategy, initialized, source` (table 2.1) |
| `chapters` | chapter sync (`SyncChaptersWithSource`) | reader, downloader, library | `url, name, scanlator, date_upload, chapter_number, source_order, manga_id` (table 2.2) |
| `sources` | `MangaStubSourceRepositoryImpl.upsertStubMangaSource` | stub sources | `_id, lang, name` (table 2.3) |

Schema, indexes and migrations: area:03 (`mangas.sq`, `chapters.sq`, `sources.sq`); repositories: area:05 and area:06.

## 8. Serialization formats of the public API

- `SManga`, `SChapter`, `Page`, `MangasPage`, `Filter*`, `FilterList`: in-process objects; only `Page` is serialized (JSON, table 1.3).
- `SManga`/`SChapter` implement `java.io.Serializable` (declared by the interface) but no code in the scope serializes them with Java serialization.
- Backup: manga and chapter backup formats are separate types (`BackupManga`, `BackupChapter`; area:21). Backup stores `updateStrategy` as `@ProtoNumber(105)` (app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupManga.kt:38) and source preferences as `BackupSourcePreferences(sourceKey, prefs)` (PreferenceBackupCreator.kt:37-56).

## 9. Constants used by data code in this scope

| Constant | Value | Location |
|---|---|---|
| `LIST_OF_STRINGS_SEPARATOR` | `", "` | data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:16 |
| `DEFAULT_CACHE_CONTROL` | max-age 600 s | core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:12 |
| `DEFAULT_HEADERS` | empty Headers | Requests.kt:13 |
| `DEFAULT_BODY` (POST only) | empty FormBody | Requests.kt:14 |
| hex digits | `0123456789abcdef` (lowercase) | core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt:7-10 |
| `PARAMETER_CACHE_SIZE` | 104857600 | ChapterCache.kt:209 |
| `HttpSource.versionId` | 1 | HttpSource.kt:45 |
