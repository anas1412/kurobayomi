# Area 14 — Local source and metadata: data models, on-disk formats, configuration

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Rust names follow the crate layout in `p1-rust-design.md`: `kuro-core::metadata` (ComicInfo and JSON side-file models) and `kuro-source-local` (the local source).

Field notation: Kotlin type, nullability (`?` = nullable), default. Rust type in the right-hand column.

---

## 1. Shared source model (owned by area:15-source-api-manga and area:16-source-api-anime, used here)

Kotlin definitions are in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/`. The Rust mirror is `kuro-source` (area:15-source-api-manga and area:16-source-api-anime). Only the fields this area reads or writes are listed; everything else is inherited from the owning area.

### 1.1 `SManga` (`source/model/SManga.kt:7-27`, impl `source/model/SMangaImpl.kt:5-25`)

| Field | Kotlin | Default | Rust |
|---|---|---|---|
| `url` | `String` (lateinit) | must be set | `String` |
| `title` | `String` (lateinit) | must be set | `String` |
| `artist` | `String?` | `null` | `Option<String>` |
| `author` | `String?` | `null` | `Option<String>` |
| `description` | `String?` | `null` | `Option<String>` |
| `genre` | `String?` (comma-space joined, see §1.5) | `null` | `Option<String>` |
| `status` | `Int` | `0` (UNKNOWN) | `i32` |
| `thumbnail_url` | `String?` | `null` | `Option<String>` |
| `update_strategy` | `UpdateStrategy` | `ALWAYS_UPDATE` | `UpdateStrategy` |
| `initialized` | `Boolean` | `false` | `bool` |

### 1.2 `SAnime` (`animesource/model/SAnime.kt:8-47`, impl `animesource/model/SAnimeImpl.kt:8-36`)

| Field | Kotlin | Default | Rust |
|---|---|---|---|
| `url` | `String` (lateinit) | must be set | `String` |
| `title` | `String` (lateinit) | must be set | `String` |
| `thumbnail_url` | `String?` | `null` | `Option<String>` |
| `background_url` | `String?` | `null` | `Option<String>` |
| `artist` | `String?` | `null` | `Option<String>` |
| `author` | `String?` | `null` | `Option<String>` |
| `status` | `Int` | `0` | `i32` |
| `description` | `String?` | `null` | `Option<String>` |
| `genre` | `String?` | `null` | `Option<String>` |
| `update_strategy` | `AnimeUpdateStrategy` | `ALWAYS_UPDATE` | `AnimeUpdateStrategy` |
| `fetch_type` | `FetchType` | `Episodes` | `FetchType` |
| `season_number` | `Double` | `-1.0` | `f64` |
| `initialized` | `Boolean` | `false` | `bool` |
| `memo` | `JsonObject` | `{}` (`JsonObject.EMPTY`) | `serde_json::Map<String, Value>` |

### 1.3 `SChapter` (`source/model/SChapter.kt:7-25`, impl `source/model/SChapterImpl.kt:5-15`)

| Field | Kotlin | Default | Rust |
|---|---|---|---|
| `url` | `String` (lateinit) | must be set | `String` |
| `name` | `String` (lateinit) | must be set | `String` |
| `date_upload` | `Long` (epoch ms) | `0` | `i64` |
| `chapter_number` | `Float` | `-1f` | `f32` |
| `scanlator` | `String?` | `null` | `Option<String>` |

### 1.4 `SEpisode` (`animesource/model/SEpisode.kt:8-37`, impl `animesource/model/SEpisodeImpl.kt:8-27`)

| Field | Kotlin | Default | Rust |
|---|---|---|---|
| `url` | `String` (lateinit) | must be set | `String` |
| `name` | `String` (lateinit) | must be set | `String` |
| `episode_number` | `Float` | `-1f` | `f32` |
| `fillermark` | `Boolean` | `false` | `bool` |
| `scanlator` | `String?` | `null` | `Option<String>` |
| `date_upload` | `Long` (epoch ms) | `0` | `i64` |
| `summary` | `String?` | `null` | `Option<String>` |
| `preview_url` | `String?` | `null` | `Option<String>` |
| `memo` | `JsonObject` | `{}` | `serde_json::Map<String, Value>` |

`SEpisode.copyFrom` copies every field including `memo` (`SEpisode.kt:39-49`). `SAnime.copy()` copies every field including `fetch_type`, `season_number` and `memo` (`SAnime.kt:54-69`).

### 1.5 `genre` encoding
`SManga.getGenres()` splits on `", "`, trims, drops blank, `distinct()` (`source/model/SManga.kt:29-32`; `SAnime.kt:49-52`). Local sources write `genre` as the raw joined text (`", "` separator) — `copyFromComicInfo` (§5.4) and the JSON details (§4.1, §4.2) use `joinToString()` (`", "`). The domain layer splits on `", "` when storing (area:05-data-repos-entries-items, `DatabaseAdapter.kt`).

### 1.6 Status constants
| Name | Value | Where |
|---|---|---|
| `UNKNOWN` | 0 | `SManga.kt:48`, `SAnime.kt:72` |
| `ONGOING` | 1 | `SManga.kt:49`, `SAnime.kt:73` |
| `COMPLETED` | 2 | `SManga.kt:50`, `SAnime.kt:74` |
| `LICENSED` | 3 | `SManga.kt:51`, `SAnime.kt:75` |
| `PUBLISHING_FINISHED` | 4 | `SManga.kt:52`, `SAnime.kt:76` |
| `CANCELLED` | 5 | `SManga.kt:53`, `SAnime.kt:77` |
| `ON_HIATUS` | 6 | `SManga.kt:54`, `SAnime.kt:78` |
| `UPCOMING` | 7 | `SAnime.kt:79` only |

Rust: `pub const UNKNOWN: i32 = 0; pub const ONGOING: i32 = 1; pub const COMPLETED: i32 = 2; pub const LICENSED: i32 = 3; pub const PUBLISHING_FINISHED: i32 = 4; pub const CANCELLED: i32 = 5; pub const ON_HIATUS: i32 = 6; pub const UPCOMING: i32 = 7;` in `kuro-source::status` (area:15-source-api-manga). The local source only writes manga statuses 0..6 from ComicInfo or details.json.

### 1.7 `FetchType` (`animesource/model/FetchType.kt:10-20`)
| Variant | Persisted (`animes.fetch_type`, area:05-data-repos-entries-items) | Meaning |
|---|---|---|
| `Seasons` | `0` | `SAnime` will only call `getSeasonList` |
| `Episodes` | `1` | `SAnime` will only call `getEpisodeList` |

Decode of an unknown integer: `Episodes` (`data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:44-47`). Rust: `#[repr(i64)] enum FetchType { Seasons = 0, Episodes = 1 }`, `from_db(i64) -> Episodes` fallback.

### 1.8 `UpdateStrategy` / `AnimeUpdateStrategy`
Values and persistence are area:05-data-repos-entries-items territory. Local sources always return `ALWAYS_UPDATE` (defaults in §1.1/§1.2 are not overwritten by this area).

---

## 2. Local-source models (Rust `kuro-source-local`)

These types do not exist in Kotlin; they name the intermediate values the Kotlin code computes inline.

### 2.1 Roots
```
pub struct LocalRoots { pub manga: Option<PathBuf>, pub anime: Option<PathBuf> }
```
`manga` = `<storage_dir>/local`, `anime` = `<storage_dir>/localanime` (Kotlin `StorageManager.kt:69-75`, folders created on first access). `None` when the storage base is missing: Kotlin returns null and every listing is empty; Rust returns an empty page for the same case. Each source holds its own `Option<PathBuf>` (design §2.2 and §2.3).

### 2.2 `FsEntry`
```
pub struct FsEntry {
    pub name: String,          // file name, no path (Kotlin UniFile.name)
    pub path: PathBuf,         // absolute path under the root
    pub is_dir: bool,          // Kotlin isDirectory
    pub is_file: bool,         // Kotlin isFile (false for dirs and dangling links)
    pub modified_ms: i64,      // Kotlin lastModified(); 0 when unknown
}
```

### 2.3 `Format`
```
pub enum Format {
    Directory(PathBuf),   // FMT Format.Directory
    Archive(PathBuf),     // FMT Format.Archive (manga archives only)
    Epub(PathBuf),        // FMT Format.Epub
}
```
`FormatError::Unknown` corresponds to `Format.UnknownFormatException` (`FMT:12`).

### 2.4 Extension lists (exact, as data)
```
pub const ANIME_EPISODE_EXTENSIONS: &[&str] = &["avi", "flv", "mkv", "mov", "mp4", "webm", "wmv", "torrent"]; // case-sensitive
pub const MANGA_ARCHIVE_EXTENSIONS: &[&str] = &["zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt"];       // compared lowercased
pub const EPUB_EXTENSION: &str = "epub";                                                                     // compared case-insensitively
```
Source: `source-local/src/commonMain/kotlin/tachiyomi/source/local/io/Archive.kt:8,17`.

### 2.5 `ImageType`
```
pub enum ImageType { Avif, Gif, Heif, Jpeg, Jxl, Png, Webp }
impl ImageType {
    pub fn mime(self) -> &'static str;      // "image/avif","image/gif","image/heif","image/jpeg","image/jxl","image/png","image/webp"
    pub fn extension(self) -> &'static str; // "avif","gif","heif","jpg","jxl","png","webp"
}
```
Source: `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt` (`enum class ImageType(val mime: String, val extension: String)`). Note JPEG's extension is `jpg`.

### 2.6 `SortSelection` (browse sort)
```
pub struct SortSelection { pub index: u8, pub ascending: bool }  // index 0 = Title, 1 = Date
pub const POPULAR_SORT: SortSelection = SortSelection { index: 0, ascending: true };
pub const LATEST_SORT:  SortSelection = SortSelection { index: 1, ascending: false };
```
Source: `filter/manga/MangaOrderBy.kt:13-14`, `filter/anime/AnimeOrderBy.kt:13-14`. Labels `Title`/`Date` (`i18n/src/commonMain/moko-resources/base/strings.xml:788,702`). Filter name `Order by` (`:701`).

### 2.7 `ChapterCandidate` / `EpisodeCandidate` (intermediate, before number recognition)
```
pub struct ChapterCandidate { pub url: String, pub name: String, pub is_dir: bool, pub path: PathBuf, pub date_upload: i64, pub format: Format }
pub struct EpisodeCandidate { pub url: String, pub name: String, pub path: PathBuf, pub date_upload: i64 }
```
Source: `MANGA_SRC:258-272`, `ANIME_SRC:249-260`.

### 2.8 Error type
```
#[derive(thiserror::Error, Debug)]
pub enum LocalSourceError {
    #[error("{}", kuro_i18n::t("chapter_not_found"))]  ChapterNotFound,        // MR.strings.chapter_not_found, "Chapter not found"
    #[error("{}", kuro_i18n::t("local_invalid_format"))] InvalidFormat,        // MR.strings.local_invalid_format, "Invalid chapter format"
    #[error("unsupported: {0}")] Unsupported(&'static str),                    // getPageList / getVideoList "Unused"
    #[error(transparent)] Io(#[from] std::io::Error),
    #[error("archive: {0}")] Archive(String),                                   // area:13-core-util-storage-archive-torrent error, converted
    #[error("xml: {0}")] Xml(String),                                           // ComicInfo decode failure
    #[error("json: {0}")] Json(#[from] serde_json::Error),
    #[error("epub: {0}")] Epub(String),
    #[error("date: {0}")] Date(String),                                         // invalid chapters/episodes json date
}
```
Kotlin has no typed errors; this mapping is a Proposal that keeps the two user-visible messages exact.

---

## 3. Side-file data models (`kuro-core::metadata`)

### 3.1 `ComicInfo` (Kotlin `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt:68-163`)

Every element value is a `String` wrapped in a class with `@XmlValue(true) val value: String = ""`. The Rust model uses `Option<String>` per element (absent = `None`, present-but-empty = `Some("")`).

| Property (Kotlin) | Element name | Namespace | Kotlin type | Rust | Read by local source | Written by local source (§5.6 bridge) |
|---|---|---|---|---|---|---|
| `title` | `Title` | none | `Title?` | `title: Option<String>` | no | no (always null) |
| `series` | `Series` | none | `Series?` | `series` | yes -> `title` | yes (`SManga.title`) |
| `number` | `Number` | none | `Number?` | `number` | no | no |
| `summary` | `Summary` | none | `Summary?` | `summary` | yes -> `description` | yes |
| `writer` | `Writer` | none | `Writer?` | `writer` | yes -> `author` | yes |
| `penciller` | `Penciller` | none | `Penciller?` | `penciller` | yes -> `artist` | yes |
| `inker` | `Inker` | none | `Inker?` | `inker` | yes -> `artist` | no |
| `colorist` | `Colorist` | none | `Colorist?` | `colorist` | yes -> `artist` | no |
| `letterer` | `Letterer` | none | `Letterer?` | `letterer` | yes -> `artist` | no |
| `coverArtist` | `CoverArtist` | none | `CoverArtist?` | `cover_artist` | yes -> `artist` | no |
| `translator` | `Translator` | none | `Translator?` | `translator` | no | no |
| `genre` | `Genre` | none | `Genre?` | `genre` | yes -> `genre` | yes |
| `tags` | `Tags` | none | `Tags?` | `tags` | yes -> `genre` | no |
| `web` | `Web` | none | `Web?` | `web` | no | no |
| `publishingStatus` | `PublishingStatusTachiyomi` | `ty` = `http://www.w3.org/2001/XMLSchema` | `PublishingStatusTachiyomi?` | `publishing_status` | yes -> `status` | yes |
| `categories` | `Categories` | `ty` = `http://www.w3.org/2001/XMLSchema` | `CategoriesTachiyomi?` | `categories` | yes -> `genre` | no (local) |
| `source` | `SourceAniyomi` | `ay` = `http://www.w3.org/2001/XMLSchema` | `SourceAniyomi?` | `source` | no | no |

Root element: `ComicInfo`, no namespace (`COMIC:67`). Declarations `xmlSchema` (`xmlns:xsd` = `http://www.w3.org/2001/XMLSchema`) and `xmlSchemaInstance` (`xmlns:xsi` = `http://www.w3.org/2001/XMLSchema-instance`) are constants, always written (`COMIC:87-93`). Rust: written as literal attributes; read: ignored.

Rust struct (exact):
```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComicInfo {
    pub title: Option<String>,
    pub series: Option<String>,
    pub number: Option<String>,
    pub summary: Option<String>,
    pub writer: Option<String>,
    pub penciller: Option<String>,
    pub inker: Option<String>,
    pub colorist: Option<String>,
    pub letterer: Option<String>,
    pub cover_artist: Option<String>,
    pub translator: Option<String>,
    pub genre: Option<String>,
    pub tags: Option<String>,
    pub web: Option<String>,
    pub publishing_status: Option<String>,   // element PublishingStatusTachiyomi
    pub categories: Option<String>,           // element Categories
    pub source: Option<String>,               // element SourceAniyomi
}
```
Field order above is the write order (§5.3).

### 3.2 `ComicInfoPublishingStatus` (`COMIC:165-189`)
Rust: `pub enum ComicInfoPublishingStatus { Ongoing, Completed, Licensed, PublishingFinished, Cancelled, OnHiatus, Unknown }` with `comic_info_value(self) -> &'static str` and `s_manga_value(self) -> i32`.

| Variant | Text (`comicInfoValue`) | `sMangaModelValue` |
|---|---|---|
| `Ongoing` | `Ongoing` | 1 |
| `Completed` | `Completed` | 2 |
| `Licensed` | `Licensed` | 3 |
| `PublishingFinished` | `Publishing finished` | 4 |
| `Cancelled` | `Cancelled` | 5 |
| `OnHiatus` | `On hiatus` | 6 |
| `Unknown` | `Unknown` | 0 |

Kotlin declaration order is `ONGOING, COMPLETED, LICENSED, PUBLISHING_FINISHED, CANCELLED, ON_HIATUS, UNKNOWN` and lookups use `entries.firstOrNull` (first match wins; no duplicates exist).

### 3.3 `MangaDetails`, `AnimeDetails` (`core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/MangaDetails.kt:6-13`, `AnimeDetails.kt:6-13`)
Identical field sets, all nullable with default `null`:

| JSON key | Kotlin | Rust (`serde`) |
|---|---|---|
| `title` | `String?` | `Option<String>` |
| `author` | `String?` | `Option<String>` |
| `artist` | `String?` | `Option<String>` |
| `description` | `String?` | `Option<String>` |
| `genre` | `List<String>?` | `Option<Vec<String>>` |
| `status` | `Int?` | `Option<i32>` |

Rust: `#[derive(Deserialize, Serialize, Default)] #[serde(default)]` on both; no `deny_unknown_fields`; `#[serde(skip_serializing_if = "Option::is_none")]` on every field (convention in area:01-foundation-app-bootstrap §6).

### 3.4 `ChapterDetails` (`core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/ChapterDetails.kt:6-11`)
| JSON key | Kotlin | Required? | Rust |
|---|---|---|---|
| `chapter_number` | `Float` | **required** (no default) | `f32` (no `#[serde(default)]`) |
| `name` | `String?` | optional | `Option<String>` |
| `date_upload` | `String?` (`yyyy-MM-dd'T'HH:mm:ss`) | optional | `Option<String>` |
| `scanlator` | `String?` | optional | `Option<String>` |

### 3.5 `EpisodeDetails` (`core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/EpisodeDetails.kt:6-14`)
| JSON key | Kotlin | Required? | Used by code? | Rust |
|---|---|---|---|---|
| `episode_number` | `Float` | **required** | yes (matching) | `f32` |
| `name` | `String?` | optional | yes | `Option<String>` |
| `date_upload` | `String?` | optional | yes (parsed) | `Option<String>` |
| `fillermark` | `Boolean = false` | optional | **no** (not copied to `SEpisode`) | `#[serde(default)] bool` (kept, unused) |
| `scanlator` | `String?` | optional | yes (unconditional) | `Option<String>` |
| `summary` | `String?` | optional | yes (unconditional) | `Option<String>` |
| `preview_url` | `String?` | optional | **no** | `Option<String>` (kept, unused) |

### 3.6 Decoder settings for all four JSON models
`Json { ignoreUnknownKeys = true; explicitNulls = false }` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:173-178`). Unknown keys are ignored; the mapping rule for absent nullable keys is "null". Missing non-null required key -> decode exception (`MissingFieldException`). Rust: `serde_json::from_slice` with the struct attributes above gives the same acceptance set, except that `serde_json` also accepts integers for `f32` (Kotlin does too) and rejects `NaN`/`Infinity` (Kotlin `Json` default also rejects them unless `allowSpecialFloatingPointValues`).

### 3.7 `ImageDecoder` result (external, `tachiyomi.decoder`)
Used only through `ImageUtil.findImageType`. Kotlin fields consulted: `format` (enum with the variants `Avif`, `Gif`, `Heif`, `Jpeg`, `Jxl`, `Png`, `Webp` as used by `ImageUtil.kt`), `isAnimated` (reader only, not this area). Rust replacement: §6.4 of `p1-behavior.md` (magic bytes).

---

## 4. JSON file shapes (on disk, examples)

### 4.1 `details.json` (manga legacy; keys above)
```json
{"title":"Foo","author":"A","artist":"B","description":"Desc","genre":["Action","Drama"],"status":1}
```
Written by nobody any more (legacy). Read-only support (§7 in behavior).

### 4.2 `details.json` (anime)
Same shape as §4.1; anime `status` uses the SAnime integer values (`UPCOMING = 7` possible).

### 4.3 `chapters.json` (manga, placed in the manga folder)
```json
[
  {"chapter_number": 1, "name": "Prologue", "date_upload": "2024-03-01T12:00:00", "scanlator": "Team A"},
  {"chapter_number": 1.5, "name": "Side story"}
]
```
Keys: `chapter_number` (required, number), `name`, `date_upload` (string `yyyy-MM-dd'T'HH:mm:ss` in local time), `scanlator`. Unknown keys ignored.

### 4.4 `episodes.json` (anime, placed in the anime folder)
```json
[
  {"episode_number": 1, "name": "Episode 1", "date_upload": "2024-03-01T12:00:00", "fillermark": false, "scanlator": "Sub", "summary": "Opening", "preview_url": "file:///..."}
]
```
Required: `episode_number`. `fillermark` and `preview_url` are accepted and discarded.

---

## 5. ComicInfo.xml on disk

### 5.1 Location
`<manga-root>/<manga-folder>/ComicInfo.xml` (top-level of the manga folder, `COMIC_INFO_FILE`). Chapter-level `ComicInfo.xml` inside downloaded chapter folders is written by the downloader (the downloads area (23, no plan folder in this set)) and is NOT read by the local source (the local source only reads archives at their root, §7.2 in behavior).

### 5.2 Declaration and encoding
Kotlin writes `<?xml version="1.0" encoding="UTF-8"?>` (`XmlDeclMode.Charset`, `XmlVersion.XML10`, `AppModule.kt:179-189`), two-space indent, UTF-8 bytes (`comicInfoString.toByteArray()`, default charset is UTF-8 on Android). Read side: `AndroidXmlReader(stream, "UTF-8")`.

### 5.3 Element order on write
`Title, Series, Number, Summary, Writer, Penciller, Inker, Colorist, Letterer, CoverArtist, Translator, Genre, Tags, Web, PublishingStatusTachiyomi, Categories, SourceAniyomi`. Absent (null) elements are not written.

### 5.4 Full example (chapter-level, written by the downloads area (23, no plan folder in this set) — shown because the schema is shared)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<ComicInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Title>Chapter 12</Title>
  <Series>Foo</Series>
  <Number>12</Number>
  <Summary>A story.</Summary>
  <Writer>A</Writer>
  <Penciller>B</Penciller>
  <Translator>Team</Translator>
  <Genre>Action, Drama</Genre>
  <Web>https://a.example/1 https://b.example/2</Web>
  <ty:PublishingStatusTachiyomi xmlns:ty="http://www.w3.org/2001/XMLSchema">Ongoing</ty:PublishingStatusTachiyomi>
  <ty:Categories xmlns:ty="http://www.w3.org/2001/XMLSchema">Read, Favorites</ty:Categories>
  <ay:SourceAniyomi xmlns:ay="http://www.w3.org/2001/XMLSchema">Source A</ay:SourceAniyomi>
</ComicInfo>
```
Verification note: namespace-declaration placement and escaping are not fully determined by the annotations (behavior §19.3). The golden file is authoritative once generated.

### 5.5 Namespace-prefix conventions
- `ty:` binds `http://www.w3.org/2001/XMLSchema` (this is the XML Schema URI, not a Tachiyomi URI). Used for `PublishingStatusTachiyomi` and `Categories`.
- `ay:` binds `http://www.w3.org/2001/XMLSchema` too (`COMIC:161`). Used for `SourceAniyomi`.
- Readers must bind by URI; the Rust reader matches by local name (Proposal, behavior E-CI-6).

### 5.6 Mapping tables (exact)

**SManga -> ComicInfo (local, `COMIC:11-31`)**

| ComicInfo | Value |
|---|---|
| `series` | `SManga.title` (always) |
| `summary` | `description` if non-null |
| `writer` | `author` if non-null |
| `penciller` | `artist` if non-null |
| `genre` | `genre` if non-null |
| `publishing_status` | `s_manga_to_comic_info(status as i64)` (always) |
| all others | `None` |

**ComicInfo -> SManga (`COMIC:33-62`)**

| SManga | Source | Rule |
|---|---|---|
| `title` | `series` | set when `series` present (even `""`) |
| `author` | `writer` | set when present |
| `description` | `summary` | set when present |
| `genre` | join(`genre`, `tags`, `categories`) | `distinct` on whole strings, each `trim`med, joined `", "`; set only when result non-empty |
| `artist` | join(split(penciller, inker, colorist, letterer, cover_artist, ", ")) | split on `", "`, `distinct`, `trim`, joined `", "`; set only when non-empty |
| `status` | `publishing_status` | **always** set: `to_s_manga(text)` (absent -> 0) |

### 5.7 Publishing status text mapping
See §3.2. Lookup is exact-match, case-sensitive. Unknown text -> 0.

---

## 6. Other on-disk artifacts (local folders)

### 6.1 Tree (manga)
```
<storage_dir>/local/
  <Manga Title>/                      (manga folder; name = SManga.url = SManga.title at browse time)
    ComicInfo.xml                     (optional, branch a)
    .noxml                            (optional empty marker, branch c)
    details.json                      (optional, legacy, branch b; deleted after migration)
    chapters.json                     (optional, §4.3)
    cover.jpg                         (optional; any of cover.{jpg,jpeg,png,webp,gif,avif,heif,jxl}; reused by name)
    .nomedia                          (created when a cover is written)
    Chapter 01/                       (directory chapter)
      001.jpg
      002.jpg
    Chapter 02.cbz                    (archive chapter)
    Chapter 03.epub                   (EPUB chapter)
    notes.txt                         (ignored: not dir / archive / epub)
    .hidden-folder/                   (ignored: starts with '.')
```

### 6.2 Tree (anime, episodes)
```
<storage_dir>/localanime/
  <Anime Title>/
    background.jpg                    (optional)
    cover.jpg                         (optional)
    episodes.json                     (optional, §4.4)
    details.json                      (optional, §4.2)
    .nomedia                          (created when a cover/background/thumbnail is written)
    Episode 01.mkv
    Episode 01-thumbnail.jpg          (optional, from ffmpeg or manual; see §6.4)
    Episode 02.mp4
```

### 6.3 Tree (anime, seasons)
```
<storage_dir>/localanime/
  <Show>/                             (fetch_type = Seasons when it has sub-folders and no episode files)
    Season 1/                         (season url = "Show/Season 1", fetch_type = Episodes)
      Ep 01.mkv
      Ep 01-thumbnail.jpg
      cover.jpg                       (season-level cover)
    Season 2/
      Ep 01.mkv
```

### 6.4 Generated names
| Name | Created by | Content | Notes |
|---|---|---|---|
| `cover.jpg` | `LocalMangaCoverManager.update` / `LocalAnimeCoverManager.update` (when no cover exists) | raw bytes copied from input | Overwrite keeps the existing file name when one exists (any extension) |
| `background.jpg` | `LocalAnimeBackgroundManager.update` | raw bytes | same rule |
| `<episode.name>-thumbnail.jpg` | `LocalEpisodeThumbnailManager.update` | raw bytes (ffmpeg output JPEG, or user input) | see behavior §13.2 quirk |
| `.nomedia` | `DiskUtil.createNoMediaFile` | empty | only if absent |
| `.noxml` | `MangaSource.getMangaDetails` branch (c) | empty | marker |
| `ComicInfo.xml` | `copyComicInfoFile` (archive copy) and branch (b) | ComicInfo document | overwritten via `createFile` (existing file reused) |
| `tmp_<random><suffix>` | `File.createTempFile("tmp_", suffix)` in `java.io.tmpdir` | ffmpeg output | never deleted in Kotlin (E-ANIME-7) |

Suffixes for temp files: `anime.title + "cover.jpg"`, `anime.title + "background.jpg"`, `anime.title + episode.name + "thumbnail.jpg"` (behavior §14).

### 6.5 Thumbnail placement and URL form
`thumbnail_url`, `background_url`, `preview_url` hold `uri.toString()` of the target file: `file:///<absolute path>` on Linux (Kotlin `UniFile.uri` for raw files). Rust: `url::Url::from_file_path(path)` then `.to_string()`; percent-encoded (`space -> %20`). Reading back: `Url::parse` then `to_file_path()`; a non-`file` scheme (Kotlin could store `content://`) is not produced on Linux and is treated as "no local file".

---

## 7. Configuration

### 7.1 Preferences read by this area

| Key (raw) | Accessor | Default | Type | Source | Used for |
|---|---|---|---|---|---|
| `display_local_badge` | `LibraryPreferences.localBadge()` | `true` | Boolean | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:85` | library "Local" badge (behavior §17.2) |
| `__APP_STATE_storage_dir` | `StoragePreferences.baseStorageDirectory()` via `Preference.appStateKey("storage_dir")` | `folderProvider.path()` | String (URI or path) | `domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt:12`, `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47` (prefix `__APP_STATE_`) | base folder that contains `local/` and `localanime/` |

Rust: `display_local_badge` is a `Pref<bool>` in area:02-preferences-system; `storage_dir` is an app-state preference in area:02-preferences-system and area:09-domain-source-track-download-release-misc with key `__APP_STATE_storage_dir` kept for migration parity. On Linux the default folder is `XDG_DATA_HOME/kurobayomi` (`Paths::data`, area:01-foundation-app-bootstrap).

### 7.2 Preferences that look related but are not read here
- `download` settings (`saveChaptersAsCBZ`, etc.) belong to the downloads area (23, no plan folder in this set) and to `DownloadPreferences`.
- No preference for ffmpeg, thumbnail generation, natural sort, or latest window.

### 7.3 Constants that are configuration in Rust (Proposal)
| Name | Value | Rust location |
|---|---|---|
| Latest window | 7 days = 604 800 000 ms | `kuro-source-local::browse::LATEST_WINDOW_MS` |
| ffprobe timeout | none in Kotlin; Proposal 20 s | `kuro-source-local::media::PROBE_TIMEOUT` |
| ffmpeg timeout | none in Kotlin; Proposal 120 s | `kuro-source-local::media::EXTRACT_TIMEOUT` |
| Concurrency for listing | 1 (sequential) | `kuro-source-local::browse` |
| Frame position | `(duration as i64) / 2` seconds (Kotlin `duration.toInt() / 2`) | `kuro-source-local::media::frame_position` |
| Number tolerance | `1e-4` | `kuro-source-local::json::NUM_EPS` |

### 7.4 Json / XML instances (singletons in Kotlin)
- `Json { ignoreUnknownKeys = true; explicitNulls = false }` (`AppModule.kt:173-178`).
- `XML { defaultPolicy { ignoreUnknownChildren() }; autoPolymorphic = true; xmlDeclMode = XmlDeclMode.Charset; indent = 2; xmlVersion = XmlVersion.XML10 }` (`AppModule.kt:179-189`).
Rust: no singletons (area:01-foundation-app-bootstrap §6); the conventions are carried by the serde attributes (§3.6) and by `ComicInfo::to_xml` (design §5).

---

## 8. Network endpoints

This area performs no network I/O. Literal URLs present in scope:

| URL | Where | Purpose | Kurobayomi handling |
|---|---|---|---|
| `https://aniyomi.org/help/guides/local-manga/` | `MANGA_SRC:379` | help link of the local manga browse toolbar | replace with the Kurobayomi help location or omit (the UI area) |
| `https://aniyomi.org/help/guides/local-anime/` | `ANIME_SRC:366` | help link of the local anime browse toolbar | same |
| `https://anansi-project.github.io/docs/comicinfo/schemas/v2.0` | `COMIC:64` (comment) | ComicInfo v2.0 schema reference | documentation only |
| `http://www.w3.org/2001/XMLSchema` | `COMIC:153,157,161` and `COMIC:88` | XML Schema namespace (written into files) | keep verbatim (file format) |
| `http://www.w3.org/2001/XMLSchema-instance` | `COMIC:93` | XSI namespace (written) | keep verbatim |

---

## 9. Database

This area reads and writes no table directly. Values returned here are persisted by callers. The columns they land in (area:03-db-manga-schema-migrations baseline schema `p1-topic-baseline-schema.md`, area:04-db-anime-schema-migrations for anime):

| Returned value | Manga column (`mangas`) | Anime column (`animes`) | Chapter/episode column |
|---|---|---|---|
| `url` | `url` TEXT NOT NULL | `url` TEXT NOT NULL | `chapters.url` / `episodes.url` |
| `title` | `title` TEXT NOT NULL | `title` | `chapters.name` / `episodes.name` |
| `author` | `author` | `author` | — |
| `artist` | `artist` | `artist` | — |
| `description` | `description` | `description` | — |
| `summary` (SEpisode) | — | — | `episodes.summary` |
| `genre` | `genre` (adapter `StringListColumnAdapter`) | `genre` | — |
| `status` | `status` INTEGER NOT NULL | `status` | — |
| `thumbnail_url` | `thumbnail_url` | `thumbnail_url` | `episodes.preview_url` |
| `background_url` | — | `background_url` (plus `background_last_modified`) | — |
| `fetch_type` | — | `fetch_type` INTEGER (0 Seasons, 1 Episodes) | — |
| `chapter_number` | — | — | `chapters.chapter_number` REAL NOT NULL |
| `episode_number` | — | — | `episodes.episode_number` |
| `date_upload` | — | — | `chapters.date_upload` INTEGER NOT NULL / `episodes.date_upload` |
| `scanlator` | — | — | `chapters.scanlator` / `episodes.scanlator` |
| `fillermark` | — | — | `episodes.fillermark` (set by callers only; never by this area) |
| `memo` | — | `animes.memo` | `episodes.memo` (never set by local source) |
| `source` (id 0) | `mangas.source` INTEGER NOT NULL | `animes.source` | — |

Local-specific write paths outside this area that touch these columns: `cover_last_modified` (`mangas`) on cover update (`UpdateManga.kt:50`), `background_last_modified` on background update (`UpdateAnime.awaitUpdateBackgroundLastModified`, area:10-app-domain-layer).

---

## 10. Concurrency primitives and state

- No persistent state inside this area. Every value is recomputed from the file system on each call.
- No in-memory cache in Kotlin. Rust may add one; Proposal: none (ponytail; the file system is the cache). If a cache is added later, it must invalidate on `modified_ms` change.
- Locks: none. The only write operations are file creates/overwrites, which the caller (download or user action) serialises per entry.

---

## 11. Bit-flag layouts

There are no bit-flag fields in this area. The `chapter_flags`, `viewer`, and `episode_flags` columns (area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations) are set by callers, not here. `UpdateStrategy` is an ordinal integer, not a bit field.

---

## 12. Edge data-shape rules (summary; details in the edge-case file)
- Chapter/episode numbers are `f32` (`Float`) on the model and `f64` during recognition. Rust must cast the same way (`as f32`, round to nearest).
- Dates are `i64` epoch milliseconds. Directory mtime may be 0 on unusual filesystems.
- Empty strings are valid values for Series, Writer and every other ComicInfo element; they are not treated as absent.
- Genre strings are never split on read (except the artist path, which splits on `", "`).
