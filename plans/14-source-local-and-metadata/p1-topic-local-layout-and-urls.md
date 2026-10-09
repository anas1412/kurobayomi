# Topic: on-disk layout of local content, entry URLs, and path resolution

Scope: `source-local/src/{androidMain,commonMain}/kotlin/tachiyomi/source/local/io/**` (4 files), the URL construction in `MANGA_SRC:245-272`, `ANIME_SRC:237-260`, the season URL in `ANIME_SRC:219-230`, playback lookup in `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt:124-146`, and storage roots in `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt`. Behavior: `p1-behavior.md` §2-§3, §11.3, §12. Data: `p1-data-and-config.md` §6.

## 1. Roots
| Medium | Folder under storage base | Constant | Accessor |
|---|---|---|---|
| Manga | `local/` | `LOCAL_SOURCE_PATH = "local"` (`StorageManager.kt:100`) | `getLocalMangaSourceDirectory()` (`:69-71`) |
| Anime | `localanime/` | `LOCAL_ANIMESOURCE_PATH = "localanime"` (`:101`) | `getLocalAnimeSourceDirectory()` (`:73-75`) |

The storage base is the preference `__APP_STATE_storage_dir` (`Preference.appStateKey("storage_dir")`). It is a content URI on Android and a path on Linux (Rust: a path). The folders are created by the accessor on first use (`createDirectory`), and by the storage-change listener for all default folders (`StorageManager.kt:31-52`).

Rust: `roots::manga_root(base) = base.join("local")`, `roots::anime_root(base) = base.join("localanime")`, `create_dir_all` on first access. Base missing -> `None` (empty listings).

## 2. Entry shapes
### 2.1 Manga
```
local/
  Title/                    <- manga; url = "Title"
    Chapter 01/             <- directory chapter; chapter url = "Title/Chapter 01"
    Chapter 02.cbz          <- archive chapter; url = "Title/Chapter 02.cbz"; name = "Chapter 02"
    Chapter 03.epub         <- epub chapter; url = "Title/Chapter 03.epub"; name from dc:title if present
```
- Manga `url` = folder name relative to `local/`. The manga folder cannot contain `/`.
- Chapter `url` = `"<manga url>/<entry name>"`; the entry name keeps its extension for files (`MANGA_SRC:260`).
- Chapter `name` = directory name, or file stem for files (`MANGA_SRC:261-265`).

### 2.2 Anime (episodes directly in the anime folder)
```
localanime/
  Show/                     <- anime; url = "Show"
    Ep 01.mkv               <- episode; url = "Show/Ep 01.mkv"; name = "Ep 01"
    Ep 01-thumbnail.jpg     <- thumbnail (not an episode: .jpg)
```

### 2.3 Anime with seasons
```
localanime/
  Show/                     <- fetch_type = Seasons (no episode files, has sub-folders)
    Season 1/               <- season; url = "Show/Season 1"; title = "Season 1"; fetch_type = Episodes
      Ep 01.mkv             <- episode; url = "Show/Season 1/Ep 01.mkv"
```
- Season `url` = `"<anime url>" + File.separator + "<season folder name>"` (`ANIME_SRC:222-228`), `File.separator` = `/`.
- Season title = last path segment of the season url (`getSAnime`, `ANIME_SRC:142`).
- Only one level of seasons is supported. A season's own sub-folders are not expanded (behavior §12).

## 3. URL-to-path resolution
Kotlin resolves a URL by splitting at the first `/` (`split('/', limit = 2)`):
- the first segment is looked up as a child of the root (manga: `getBaseDirectory().findFile(first)`; anime: `getBaseDirectory().findFile(first)` in playback, `getAnimeDirectory(url)` in details);
- the remainder is looked up as a child of that folder: `findFile(rest)`. For season episodes `rest` contains `/` (`"Season 1/Ep 01.mkv"`). Correct behavior depends on the raw backend resolving nested relative names (the UniFile RawFile backend). On SAF this would fail; Linux is raw-only.

Rust rule (design §3):
```
resolve(root, url):
  parts = url.split_once('/')           # first '/' only
  require every component non-empty, not "." or "..", no NUL, no '\\'
  root.join(part) for each '/'-separated part  # the whole url joined under root
  return Some(path) only if path exists (or is_file for media)
```
This is equivalent for every URL the app generates. Unsafe URLs resolve to `None` (E-PATH-1).

## 4. What is listed where (filters by level)
| Level | Listed when | Filter (Kotlin) | Exclusions |
|---|---|---|---|
| Root (browse) | directory, name not starting with `.` | `MANGA_SRC:90-93`, `ANIME_SRC:90-93` | files, dot-folders |
| Manga folder (chapters) | dir, manga archive (case-insensitive), epub (case-insensitive) | `MANGA_SRC:254-257` | dot entries, json, images, txt, markers |
| Anime folder (episodes) | `ArchiveAnime` extension (case-sensitive), not dot | `ANIME_SRC:245-248` | json, images, text; directories are NOT excluded, so a directory named `X.mkv` is listed (E-ANIME-9) |
| Anime folder (seasons) | dir, not dot | `ANIME_SRC:213-217` | files |
| Anime folder (fetch type) | any `ArchiveAnime` file -> Episodes; else any dir (dot dirs too) -> Seasons | `LocalAnimeFetchTypeManager.kt:13-19` | — |
| Folder metadata | exact names: `ComicInfo.xml`, `.noxml`, `details.json`, `chapters.json`, `episodes.json`, `cover.*`, `background.*` | `MANGA_SRC:156-161`, `ANIME_SRC:194-197`, `:238-241`, `image/*` | — |

## 5. Markers and generated files (exact names)
| Name | Level | Meaning | Created by | Removed by |
|---|---|---|---|---|
| `.nomedia` | any folder with generated images | hide from gallery apps | `DiskUtil.createNoMediaFile` | never (user) |
| `.noxml` | manga folder | no ComicInfo found, do not rescan | details branch (c) | details branch (a) when `ComicInfo.xml` appears |
| `ComicInfo.xml` | manga folder | manga metadata | archive copy (c), legacy migration (b) | user |
| `details.json` | manga or anime folder | legacy metadata | user | legacy migration (b) for manga, after writing ComicInfo |
| `chapters.json` / `episodes.json` | manga / anime folder | per-chapter and per-episode overrides | user | user |
| `cover.<ext>` | manga, anime, season folder | cover image | `update` (set as cover) or chapter-derived fallback | user |
| `background.<ext>` | anime folder | background image | `update` or fallback | user |
| `<episode>-thumbnail.jpg` | anime folder | episode preview | ffmpeg extraction or user | user |

Dot-files never appear as chapters or episodes (behavior §4.3).

## 6. Name rules that matter for URLs
- Names are compared exactly (case-sensitive) for `ComicInfo.xml`, `.noxml`, `details`, `chapters`, `episodes`. The stems `cover`, `background` are case-insensitive. Extensions are case-sensitive for images (`ImageType`), case-insensitive for manga archives and epub, case-sensitive for anime episodes (§4).
- A name containing `.` only as the extension: stem = everything before the last dot.
- Names with multiple dots: `Vol.1.cbz` -> stem `Vol.1`, extension `cbz`.
- Names without an extension cannot be archives, epubs, or episodes; for directory chapters the name is the whole name.

## 7. Windows and cross-platform notes
- Separator: Kotlin `File.separator` is `/` on Android; Rust uses `/` inside URLs and `Path` for filesystem access. Windows paths are only produced by `Path::join`.
- Case-insensitive file systems (Windows, macOS default): `ComicInfo.xml` may be stored as `comicinfo.xml`. The exact-name rule (§6) means such files are not recognized on any platform, identically. Documented.
- Reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`) and trailing dots/spaces cannot be created by this app except thumbnails (episode names). Covered by the sanitizer (`p1-topic-cover-and-thumbnail-pipeline.md` §4).
- Length: Windows MAX_PATH; not handled here (area:01-foundation-app-bootstrap addendum).

## 8. Threats and guards
- URL traversal (`..`) from stored or imported data: rejected by the resolver (E-PATH-1).
- Symlinks inside the root: followed (Kotlin parity, E-PATH-2).
- Hard links: indistinguishable from files; no special handling.
- Huge folders: listing is O(n) per call; no pagination (Kotlin parity).

## 9. Tests
Listed in `p1-edge-cases-and-tests.md` §5.1 (names/formats), §5.4 (browse), §5.13 (paths). Fixture trees: `manga_tree/`, `anime_tree/` (§4 of the test file).
