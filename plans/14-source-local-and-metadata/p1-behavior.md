# Area 14 — Local source (manga + anime) and metadata: behavior spec

Scope: the 36 files listed in `p1-inventory.md`. Every path below is repo-relative to `/home/blackbox/aniyomi-src`.

Labels: **OBSERVED** = what the Kotlin does (cited path:line). **Proposal:** = what the Rust port should do. Anything not labelled is a fact from the cited source.

Short names used below:
- `MANGA_SRC` = `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt`
- `ANIME_SRC` = `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt`
- `COMIC` = `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt`
- `ARCH` = `source-local/src/commonMain/kotlin/tachiyomi/source/local/io/Archive.kt`
- `FMT` = `source-local/src/commonMain/kotlin/tachiyomi/source/local/io/Format.kt`

---

## 0. Feature index

| # | Feature | Kotlin entry point | Priority |
|---|---|---|---|
| 1 | Source identity and registration | `MANGA_SRC:53-76`, `ANIME_SRC:47-72` | P0 |
| 2 | Storage roots (`local`, `localanime`) | `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:69-74,100-101` | P0 |
| 3 | Filesystem access layer | `source-local/src/androidMain/kotlin/tachiyomi/source/local/io/{manga,anime}/Local*SourceFileSystem.kt` | P0 |
| 4 | Format and archive classification | `FMT`, `ARCH` | P0 |
| 5 | Image type detection | `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:39-95` | P0 |
| 6 | Natural sort comparator | `core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:37-40` | P0 |
| 7 | Chapter / episode number recognition (call contract) | `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterRecognition.kt`, `domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeRecognition.kt` | P0 |
| 8 | Manga browse (popular / latest / search) | `MANGA_SRC:79-143` | P0 |
| 9 | Anime browse | `ANIME_SRC:75-156` | P0 |
| 10 | Manga chapter list | `MANGA_SRC:244-303` | P0 |
| 11 | Manga reader format resolution | `MANGA_SRC:321-334`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ChapterLoader.kt:98-104` | P0 |
| 12 | Anime episode list | `ANIME_SRC:237-316` | P0 |
| 13 | Anime playback resolution | `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt:124-146` | P0 |
| 14 | Anime seasons | `ANIME_SRC:158-234` | P1 |
| 15 | Manga cover discovery and fallback | `MANGA_SRC:134-136,296-300,336-375` | P0 / P1 |
| 16 | Anime cover, background, episode thumbnail | `ANIME_SRC:146-154,289-313,332-362`, `image/anime/*` | P1 |
| 17 | ffmpeg frame extraction | `ANIME_SRC:332-362` | P1 |
| 18 | Manga details orchestration + ComicInfo read | `MANGA_SRC:146-241` | P1 |
| 19 | ComicInfo model, read, write, mapping | `COMIC` | P1 |
| 20 | Legacy `details.json` (manga) | `MANGA_SRC:172-191` | P2 |
| 21 | Anime `details.json` | `ANIME_SRC:185-210` | P2 |
| 22 | `chapters.json` overrides | `MANGA_SRC:247-251,281-287` | P2 |
| 23 | `episodes.json` overrides | `ANIME_SRC:237-243,262-270` | P2 |
| 24 | EPUB metadata fill | `source-local/src/androidMain/kotlin/tachiyomi/source/local/metadata/EpubReaderExtensions.kt` | P2 |
| 25 | EPUB page / cover extraction (consumer of area:13-core-util-storage-archive-torrent reader) | `MANGA_SRC:275-278,363-369` | P1 |
| 26 | Set-as-cover / background / thumbnail | `app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt:50-65`, `AnimeExtensions.kt:36-79` | P1 |
| 27 | Local entries in downloads / library / updates | `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:224-248`, `ChapterFilter.kt`, `UpdateManga.kt:49` | P1 |
| 28 | Sync exemption for empty source lists | `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt:56`, `SyncEpisodesWithSource.kt:54`, `SyncSeasonsWithSource.kt:31` | P1 |
| 29 | Help links | `MANGA_SRC:379`, `ANIME_SRC:366` | P2 |

---

## 1. Source identity and registration

OBSERVED (`MANGA_SRC:53-76`):
- `actual class LocalMangaSource(context, fileSystem: LocalMangaSourceFileSystem, coverManager: LocalMangaCoverManager) : CatalogueSource, UnmeteredSource`.
- `name = context.stringResource(AYMR.strings.local_manga_source)` -> English value `Local manga source` (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:495`).
- `id = ID` where `const val ID = 0L` (`MANGA_SRC:378`).
- `lang = "other"`. `toString() = name`. `supportsLatest = true`.
- `PopularFilters = FilterList(MangaOrderBy.Popular(context))`, `LatestFilters = FilterList(MangaOrderBy.Latest(context))` (`MANGA_SRC:62-66`).
- `getFilterList() = FilterList(MangaOrderBy.Popular(context))` (`MANGA_SRC:314`), a fresh instance each call.

OBSERVED (`ANIME_SRC:47-72`):
- `actual class LocalAnimeSource(context, fileSystem, coverManager, backgroundManager, thumbnailManager, fetchTypeManager) : AnimeSource, UnmeteredSource`.
- `name` = `AYMR.strings.local_anime_source` = `Local anime source` (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:496`).
- `id = ID = 0L`; `lang = "other"`; `supportsLatest = true`; `supportsRelatedAnime = false` (`ANIME_SRC:169`); `getRelatedAnimeList` returns empty list (`:171`).
- Filters: `AnimeOrderBy.Popular` / `AnimeOrderBy.Latest` (`ANIME_SRC:59-62`); `getFilterList()` at `:327`.

OBSERVED registration: `app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt:53-57` puts `LocalMangaSource.ID to LocalMangaSource(context, Injekt.get(), Injekt.get())` into the source map before any extension source. `app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:53-60` does the same with `context` first and five `Injekt.get()` (file system, cover, background, thumbnail, fetch-type). The map is rebuilt inside `collectLatest` on every change of `installedExtensionsFlow`, so the local source is re-created whenever extensions change (singletons from Injekt are reused).

OBSERVED availability rules:
- `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt:32` keeps the local source even when its language is not enabled: `it.lang in enabledLanguages || it.id == LocalMangaSource.ID`. Anime equivalent at `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:27`.
- `GetMangaSourcesWithFavoriteCount.kt:25` and the anime equivalent `GetAnimeSourcesWithFavoriteCount.kt:25` drop the local source (`filterNot { it.first.id == ID }`).
- `isLocal()` predicates: `MANGA_SRC:385-387` (`Manga.isLocal() = source == LocalMangaSource.ID`, `MangaSource.isLocal() = id == LocalMangaSource.ID`); `ANIME_SRC:375-377` the same for Anime/AnimeSource.

Proposal: `kuro-source-local` exposes `pub const LOCAL_SOURCE_ID: i64 = 0;` for both media. Because manga and anime use separate tables (`mangas`, `animes`), both can be 0 without collision. A row with `source = 0` is local. No extension may ever use id 0 (extension ids are hashes; area:03-db-manga-schema-migrations owns the id width).

---

## 2. Storage roots

OBSERVED (`StorageManager.kt`):
- `getLocalMangaSourceDirectory(): UniFile? = baseDir?.createDirectory("local")` (`:69-71`).
- `getLocalAnimeSourceDirectory(): UniFile? = baseDir?.createDirectory("localanime")` (`:73-75`).
- Constants: `LOCAL_SOURCE_PATH = "local"`, `LOCAL_ANIMESOURCE_PATH = "localanime"` (`:100-101`).
- `baseDir` is initialised from preference `storagePreferences.baseStorageDirectory()` (`:25`). The preference key is `Preference.appStateKey("storage_dir")` = `__APP_STATE_storage_dir` (`domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt:12`, `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`), default `folderProvider.path()` (a content URI on Android from `core/common/src/main/java/tachiyomi/core/common/storage/AndroidStorageFolderProvider.kt:22-24`).
- `getBaseDir(uri) = UniFile.fromUri(context, uri.toUri()).takeIf { it?.exists() == true }` (`:37-40`). If the base is missing, every accessor returns null.
- When the storage pref changes (`drop(1)`, `distinctUntilChanged`) the manager creates `autobackup`, `local`, `localanime`, `downloads` (+`.nomedia`), `mpv-config` (`:31-52`). On a fresh start the two local folders are created lazily by the accessor (the `createDirectory` call creates the folder when missing).

OBSERVED file-system helpers (`LocalMangaSourceFileSystem.kt:10-26`, `LocalAnimeSourceFileSystem.kt:10-29`):
- `getBaseDirectory()` = base dir (or null).
- `getFilesInBaseDirectory()` = `getBaseDirectory()?.listFiles().orEmpty().toList()`.
- `getMangaDirectory(name)` = `getBaseDirectory()?.findFile(name)?.takeIf { it.isDirectory }`; `getFilesInMangaDirectory(name)` = `getMangaDirectory(name)?.listFiles().orEmpty().toList()`.
- Anime: `getAnimeDirectory(name)` identical to manga; `getFilesInAnimeDirectory(name)` = `base?.findFile(name)?.takeIf { it.isDirectory }?.listFiles().orEmpty().toList()`.
- `findFile(name)` is called with names that can contain `/` (anime season URLs are `"<show>/<season>"`, see §11.3). That only works because UniFile's raw-file backend resolves the nested path; this is an OBSERVED dependency on the library and is replaced by an explicit path join in Rust (§4.3 and the design file).

Proposal (Rust): `LocalRoots { manga: PathBuf, anime: PathBuf }` computed from `kuro-platform::Paths` and the storage-dir preference owned by area:09-domain-source-track-download-release-misc; created with `create_dir_all` on first access (the same lazy rule). A missing root yields empty listings, never an error (matching `orEmpty()`).

---

## 3. Entry model on disk and in URLs

### 3.1 Manga
- Manga folder: `<manga-root>/<title-folder>/`. The folder name is both the title (at browse time) and the `url`.
- Chapter entries inside a manga folder, when they pass the chapter filter (§8.2): a sub-directory (a "directory chapter"), or a file whose extension is a manga archive (`zip cbz rar cbr 7z cb7 tar cbt`, case-insensitive), or a `.epub` (case-insensitive).
- Chapter `url` = `"<manga-folder-name>/<chapter-entry-name>"` where the entry name includes the file extension for files (OBSERVED `MANGA_SRC:260` `url = "${manga.url}/${chapterFile.name}"`).

### 3.2 Anime
- Anime folder: `<anime-root>/<anime-folder>/`. Anime folders may contain episode files (video archives), or season sub-folders, never both by intent; `LocalAnimeFetchTypeManager` decides (§11.1).
- Episode file: a file in the anime folder (or in a season folder) whose extension is in `ArchiveAnime` (§4.2).
- Episode `url` = `"<anime-url>/<episode-file-name-with-extension>"` (OBSERVED `ANIME_SRC:251`).
- Season `url` = `"<anime-url>" + File.separator + "<season-folder-name>"` (OBSERVED `ANIME_SRC:222-228`, `File.separator` is `/` on Android and Linux). A season's episodes therefore have URLs such as `"show/season1/ep01.mkv"`.

### 3.3 URL resolution rules
OBSERVED: every local URL is split with `split('/', limit = 2)`: the first segment is the top folder name, the remainder is the file path below the base (`MANGA_SRC:323`, `ANIME_SRC:344`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt:128`). For manga, the remainder is a single name. For a season episode the remainder is `"season1/ep01.mkv"` and is resolved with `findFile` on the raw backend.

Proposal (behavior-preserving, safer): a URL is resolved as `base.join(url)`. Each `/`-separated segment must be a normal component: reject empty, `.`, `..`, absolute prefixes and any segment containing a NUL byte. A rejected URL behaves like "not found" (the same user-visible error as a missing file). Kotlin does not validate; see the edge-case file for the security note.

---

## 4. Filesystem access, names and formats

### 4.1 Name helpers (OBSERVED `core/common/src/main/java/tachiyomi/core/common/storage/UniFileExtensions.kt:5-9`)
- `UniFile.extension` = `name?.substringAfterLast('.')`. For a name with no dot the whole name is the "extension" (`"cover"` -> `"cover"`). For `".nomedia"` the extension is `"nomedia"`.
- `UniFile.nameWithoutExtension` = `name?.substringBeforeLast('.')`. For `"a.b.cbz"` it is `"a.b"`. For a name with no dot it is the whole name.
- Both are `String?`; every caller applies `orEmpty()` or null-checks.
- Case: `extension` is NOT lowercased. Comparisons differ per caller (§4.2).

### 4.2 Archive and format classification

`ARCH` (OBSERVED, full file):
- `object ArchiveAnime`: `SUPPORTED_ARCHIVE_TYPES = listOf("avi", "flv", "mkv", "mov", "mp4", "webm", "wmv", "torrent")`. `isSupported(file) = file.extension in SUPPORTED_ARCHIVE_TYPES`. **Case-sensitive**: `EP01.MKV` is NOT an episode.
- `object ArchiveManga`: `SUPPORTED_ARCHIVE_TYPES = listOf("zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt")`. `isSupported(file) = file.extension?.lowercase() in SUPPORTED_ARCHIVE_TYPES` (**case-insensitive**).

`FMT` (OBSERVED, full file):
```
sealed interface Format {
    data class Directory(val file: UniFile)
    data class Archive(val file: UniFile)
    data class Epub(val file: UniFile)
    class UnknownFormatException : Exception()
    companion object {
        fun valueOf(file: UniFile) = when {
            file.isDirectory -> Directory(file)
            file.extension.equals("epub", true) -> Epub(file)
            isArchiveSupported(file) -> Archive(file)          // ArchiveManga.isSupported
            else -> throw UnknownFormatException()
        }
    }
}
```
The check order matters: directory first, then `epub` (case-insensitive), then manga archive list. A directory named `x.epub` is a `Directory`.

Observed inconsistency: chapter listing accepts `it.isDirectory || ArchiveManga.isSupported(it) || it.extension.equals("epub", true)` (`MANGA_SRC:257`), equal to `Format.valueOf` acceptance. Anime uses only `ArchiveAnime`, so `.MKV` files are invisible to anime but a `.CBZ` is visible to manga.

Supported formats table (what the reader / player accept):

| Kind | Entries | Read by |
|---|---|---|
| Manga directory chapter | any directory not starting with `.` | `DirectoryPageLoader` (the reader-core area (43, no plan folder in this set) owns the loader) |
| Manga archive | `zip cbz rar cbr 7z cb7 tar cbt` (case-insensitive) | `ArchivePageLoader` via `mihon.core.archive.archiveReader` (area:13-core-util-storage-archive-torrent) |
| Manga EPUB | `epub` (case-insensitive) | `EpubPageLoader` via `epubReader` (area:13-core-util-storage-archive-torrent) |
| Anime episode | `avi flv mkv mov mp4 webm wmv torrent` (case-sensitive) | the player, as a plain file URI (§11.5) |

### 4.3 Hidden files and folders
- Browse (manga and anime): base-folder entries are kept only if `isDirectory && !name.startsWith('.')` (`MANGA_SRC:92`, `ANIME_SRC:92`).
- Chapter and episode listing: `filterNot { it.name.startsWith('.') }` (`MANGA_SRC:256`, `ANIME_SRC:247`). Dot files are ignored everywhere in listings. The markers `.nomedia` and `.noxml` are dot files and are therefore never chapters.
- Seasons: `filter { it.isDirectory && !it.name.startsWith('.') }` (`ANIME_SRC:216`).
- Fetch type: `files.any { it.isDirectory }` does NOT exclude hidden directories (`LocalAnimeFetchTypeManager.kt:17`). A hidden directory alone turns an anime into a Seasons anime. Preserved in Proposal as-is, flagged as an edge case.
- Cover / background / thumbnail search: not filtered by hidden status, only by name and `isFile`.

### 4.4 Image type detection (ImageUtil)
OBSERVED `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:39-47`:
```
fun isImage(name: String?, openStream: (() -> InputStream)? = null): Boolean {
    if (name == null) return false
    val extension = name.substringAfterLast('.')
    return ImageType.entries.any { it.extension == extension } || openStream?.let { findImageType(it) } != null
}
```
- `ImageType` (`ImageUtil.kt`, `enum class ImageType(val mime: String, val extension: String)`): `AVIF("image/avif","avif")`, `GIF("image/gif","gif")`, `HEIF("image/heif","heif")`, `JPEG("image/jpeg","jpg")`, `JXL("image/jxl","jxl")`, `PNG("image/png","png")`, `WEBP("image/webp","webp")`.
- The extension test is **case-sensitive** and uses the `jpg` spelling only (so `.jpeg` falls through to sniffing). Every local caller passes an `openStream` lambda, so sniffing always happens for names that fail the extension test.
- `findImageType(stream)`: reads up to 32 bytes (`getImageType`, `ImageUtil.kt`, function `getImageType`), `mark/reset` if supported, then `tachiyomi.decoder.ImageDecoder.findType(bytes)`; maps `Format.Avif/Gif/Heif/Jpeg/Jxl/Png/Webp` to `ImageType`, anything else to null; exceptions give null.
- `tachiyomi.decoder` is an external artifact `com.github.tachiyomiorg:image-decoder:41c059e540` (`gradle/libs.versions.toml:52`), not part of this clone. Its brand table is not visible here.

Proposal (Rust, see design file §6.4): detect by magic bytes over the first 32 bytes (or fewer at EOF). Table: PNG `89 50 4E 47 0D 0A 1A 0A`; JPEG `FF D8 FF`; GIF `GIF87a` / `GIF89a`; WEBP `RIFF....WEBP`; AVIF `....ftypavif` (bytes 4..12 is `ftypavif`); HEIF `....ftypheic|ftypheix|ftyphevc|ftypheim|ftypheis|ftyphevx|ftypmif1|ftypmsf1` (brand list is a best-effort subset; port the exact upstream list when the crate is checked out); JXL codestream `FF 0A`, JXL container `00 00 00 0C 4A 58 4C 20 0D 0A 87 0A`. The extension rule is kept verbatim (case-sensitive `jpg`, `png`, etc.) so behavior is identical for well-named files.

### 4.5 Natural order comparator (OBSERVED, exact)
`StringExtensions.kt:37-40`: `compareToCaseInsensitiveNaturalOrder(other) = CaseInsensitiveSimpleNaturalComparator.getInstance().compare(this, other)` from `net.greypanther.natsort` (artifact `net.grey-panther:natural-comparator:1.1`, source at `https://github.com/gpanther/java-nat-sort`). Algorithm (`AbstractSimpleNaturalComparator.compare`, read from that repository):

```
len1, len2; idx1 = idx2 = 0
while idx1 < len1 && idx2 < len2:
    c1 = s1[idx1++]; c2 = s2[idx2++]          // Java char = UTF-16 code unit
    d1 = isDigit(c1); d2 = isDigit(c2)        // '0' <= c <= '9'
    if d1 && !d2: return -1
    if !d1 && d2: return 1
    if !d1 && !d2:
        c = compareChars(c1, c2)              // CI: Character.toLowerCase(c1) - Character.toLowerCase(c2)
        if c != 0: return c
    else:                                     // both digits
        num1 = c1 - '0'
        while idx1 < len1:                    // greedy digit run, u64 wrapping
            ch = s1[idx1++]
            if isDigit(ch): num1 = num1*10 + (ch-'0')
            else: idx1--; break
        num2 = same for s2
        if num1 != num2: return compareUnsigned(num1, num2)   // -1 / 1
after loop:
    if idx1 < len1: return 1
    if idx2 < len2: return -1
    return 0
```
Consequences (all OBSERVED from the algorithm): `"z010.doc" == "z10.doc"` (returns 0; leading zeros do not break ties); `"z10" < "z10a"`; any digit run sorts before any non-digit character at the same position; numbers compare as unsigned 64-bit values with wrap-around beyond 19 digits; case-insensitivity uses per-code-unit `Character.toLowerCase`, so `"a" == "A"`.

The comparator is used for: chapter/episode name tie-breaks (`MANGA_SRC:292`, `ANIME_SRC:286`), cover/first-page selection (`MANGA_SRC:342,356`), and never for browse sorting (browse uses `String.CASE_INSENSITIVE_ORDER`, §6.3).

### 4.6 Date parsing helpers
- `parseDate(isoDate)` in both sources: `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss", Locale.getDefault()).parse(isoDate)?.time ?: 0L` (`MANGA_SRC:305-307`, `ANIME_SRC:318-320`). Uses the device default time zone. Java `SimpleDateFormat` is lenient by default (`2024-02-31T10:00:00` rolls over to March 2). Trailing text after the pattern is ignored. **No try/catch**: a non-matching string throws `ParseException` out of the caller.
- `fillMetadata` EPUB date: `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ssZ", Locale.getDefault())` inside `try { ... } catch (e: ParseException) { }` (`EpubReaderExtensions.kt:37-47`). Whether a literal `Z` suffix matches the `Z` pattern letter is not established by this source; a failure is swallowed and the date is left untouched.
- Float equality for chapter/episode numbers: `abs(this - other) < 0.0001` (`MANGA_SRC:309-311`, `ANIME_SRC:322-324`).

Proposal (Rust): use the `time` crate per the cross-cutting convention (area:01-foundation-app-bootstrap). Lenient rollover is not reproduced: invalid dates are treated as "no date" for chapters.json / episodes.json and as "unchanged" for EPUB, which is a documented deviation (§edge cases, E-DATE-1). Local time zone is captured once at process start (see design §8.4).

---

## 5. Chapter and episode number recognition (call contract)

Owner of the algorithm: area:07-domain-entries-items (`p1-topic-item-number-recognition.md`). This area only calls it.

Call sites (OBSERVED):
- Manga: `ChapterRecognition.parseChapterNumber(manga.title, this.name, this.chapter_number.toDouble()).toFloat()` (`MANGA_SRC:268-271`). At this point `chapter_number` is the SChapter default `-1f` (`SChapterImpl.kt:13`), so the known-number short-circuit is not taken: the number is always recognised from the entry name.
- Anime: `EpisodeRecognition.parseEpisodeNumber(anime.title, this.name, this.episode_number.toDouble()).toFloat()` (`ANIME_SRC:255-259`), `episode_number` default `-1f` (`SEpisodeImpl.kt:14`).

Inputs that matter for local content:
- The **title argument is `manga.title` / `anime.title` at call time**. For manga this is the SManga title that is in effect: the folder name from browse, or the ComicInfo `Series` value if `getMangaDetails` ran before (§7.4). Changing the series title therefore changes parsed numbers. For anime this is the anime title (season title for seasons).
- The name argument is the entry name **without** the extension for files (`nameWithoutExtension`) and the directory name for directory chapters (`MANGA_SRC:263-265`). Anime uses `nameWithoutExtension` always (`ANIME_SRC:252`).
- Name is set before the number is computed, so for EPUB the name is still the file stem at this point; the EPUB title (§21) replaces the name afterwards but does NOT recompute the number.

Result: `Double`, converted to `Float`. `-1.0` means "unrecognised" (ChapterRecognition returns `chapterNumber ?: -1.0` when no digits are found, `ChapterRecognition.kt:53`). Several unrecognised entries share `-1` and are ordered by name (§8.4).

Reference samples taken from the upstream test suite (`domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt`, 34 cases) are reproduced in the test file for area 14. Examples: `("Bleach", "Bleach 567 Down With Snowwhite") -> 567.0`, `("Solanin", "Solanin 028.a Vol. 2") -> 28.1`, `("random", "Foo") -> -1.0`.

---

## 6. Manga browse (search / popular / latest)

Entry points (OBSERVED):
- `getPopularManga(page) = getSearchManga(page, "", PopularFilters)` (`MANGA_SRC:79`).
- `getLatestUpdates(page) = getSearchManga(page, "", LatestFilters)` (`:81`).
- `getSearchManga(page, query, filters)` (`:83-143`).

### 6.1 Algorithm (`getSearchManga`)
1. `withIOContext { ... }` (Dispatchers.IO).
2. `lastModifiedLimit = if (filters === LatestFilters) System.currentTimeMillis() - LATEST_THRESHOLD else 0L`. The test is **object identity** on the `LatestFilters` instance. `LATEST_THRESHOLD = TimeUnit.MILLISECONDS.convert(7, TimeUnit.DAYS)` = `604800000` ms (`MANGA_SRC:381`).
3. `mangaDirs = fileSystem.getFilesInBaseDirectory()`.
4. Keep `it.isDirectory && !it.name.orEmpty().startsWith('.')`.
5. `distinctBy { it.name }` (first occurrence wins).
6. Filter:
   - if `lastModifiedLimit == 0 && query.isBlank()`: keep all;
   - else if `lastModifiedLimit == 0`: keep where `name.contains(query, ignoreCase = true)`;
   - else (Latest): keep where `lastModified() >= lastModifiedLimit`. The **query is ignored** for latest.
7. For each filter in `filters` (in order): if `MangaOrderBy.Popular` -> ascending `compareBy(String.CASE_INSENSITIVE_ORDER) { name }` or descending `compareByDescending(...)`; if `MangaOrderBy.Latest` -> ascending `sortedBy(UniFile::lastModified)` or descending `sortedByDescending(UniFile::lastModified)`; other filters: no-op. `filter.state!!` is dereferenced, so a Sort filter with null state throws NPE (never happens for these classes because they are constructed with a Selection).
8. Map each directory to `SManga.create()` with `title = name`, `url = name`, and `thumbnail_url = coverManager.find(name)?.uri.toString()` when a cover exists.
9. `MangasPage(mangas, false)`. `page` is never used. There is never a next page.

Empty results: a missing base folder gives `MangasPage([], false)`.

### 6.2 Defaults
- Popular = `Selection(index 0, ascending true)` = **Title ascending**. (The name "Popular" is misleading; no popularity exists.)
- Latest = `Selection(index 1, ascending false)` = **Date descending** plus the 7-day window (`MANGA_SRC:66`, `MangaOrderBy.kt:14`).
- Filter UI: one Sort filter named `Order by` (`MR.strings.local_filter_order_by` = `Order by`, `i18n/src/commonMain/moko-resources/base/strings.xml:701`), values `[Title, Date]` (`MR.strings.title` = `Title` at `:788`, `MR.strings.date` = `Date` at `:702`).

### 6.3 `CASE_INSENSITIVE_ORDER` semantics
`String.CASE_INSENSITIVE_ORDER` compares char by char after `toUpperCase` and then `toLowerCase` (Java `String.CASE_INSENSITIVE_ORDER`). It is NOT natural order: `"10"` sorts before `"9"`. Proposal: keep this exact (it is what users see today); compare by per-char lowercase of Unicode scalar values.

### 6.4 Concurrency and cost
Each directory's cover lookup is a `listFiles` on that directory, launched with `async` and joined with `awaitAll()` (`:126-140`), i.e. all directories are listed concurrently on Dispatchers.IO. Proposal: one `spawn_blocking` task, sequential `read_dir` per folder; ceiling noted in the design file.

---

## 7. Manga details (`getMangaDetails`)

Entry point `MANGA_SRC:146-214`. Input: `manga: SManga` with `url` set. Output: the same object, mutated. Never throws (see 7.6).

### 7.1 Steps
1. `coverManager.find(manga.url)?.let { manga.thumbnail_url = it.uri.toString() }` (outside the try).
2. `try {`
3. `mangaDir = fileSystem.getMangaDirectory(manga.url) ?: error("${manga.url} is not a valid directory")`.
4. `mangaDirFiles = mangaDir.listFiles().orEmpty()` (not sorted, not filtered).
5. Find markers (all exact, case-sensitive):
   - `comicInfoFile` = first entry whose `name == "ComicInfo.xml"`;
   - `noXmlFile` = first entry whose `name == ".noxml"`;
   - `legacyJsonDetailsFile` = first entry whose `extension == "json" && nameWithoutExtension == "details"`.
6. `when` (first matching branch only):
   - **(a)** `comicInfoFile != null`: `noXmlFile?.delete()`, then `setMangaDetailsFromComicInfoFile(comicInfoFile.openInputStream(), manga)` (§19.5).
   - **(b)** `legacyJsonDetailsFile != null`: decode `MangaDetails` from the JSON (§20), apply each non-null field, then build `manga.getComicInfo()` (§19.6), `mangaDir.createFile("ComicInfo.xml")?.openOutputStream()?.use { write(xml); legacyJsonDetailsFile.delete() }`. The JSON is deleted only inside the `use` block after the XML bytes are written; a failed write leaves the JSON.
   - **(c)** `noXmlFile == null` (no ComicInfo, no legacy JSON, no marker): `chapterArchives = mangaDirFiles.filter(ArchiveManga::isSupported)` (not sorted); `copiedFile = copyComicInfoFileFromArchive(chapterArchives, mangaDir)`. If it returned a file, parse it into the manga (§19.5). Otherwise `mangaDir.createFile(".noxml")` (creates an empty marker so later calls skip the scan).
   - **(d)** `noXmlFile != null` and no ComicInfo and no legacy JSON: nothing happens.
7. `} catch (e: Throwable) { logcat(LogPriority.ERROR, e) { "Error setting manga details from local metadata for ${manga.title}" } }`.
8. Return `manga`.

### 7.2 `copyComicInfoFileFromArchive` (OBSERVED `MANGA_SRC:216-225`)
For each archive in order: open `archiveReader(context)`; `getInputStream("ComicInfo.xml")` (exact entry name at archive root); if non-null, return `copyComicInfoFile(stream, folder)` immediately (the first match wins, the rest are not inspected). Returns null when no archive has it. `copyComicInfoFile` creates `ComicInfo.xml` in the folder and copies the stream into it (`:227-233`). An existing `ComicInfo.xml` cannot exist in this branch (branch (a) would have been taken).

### 7.3 Ordering facts
- Archive order = `listFiles()` order, which is filesystem-dependent on Android (undefined). Proposal: sort by natural CI name (§4.5) to make it deterministic.
- The `.noxml` marker is created only when no archive contained ComicInfo; after a successful copy, the next call takes branch (a) and does not rescan.
- A manga with only directory chapters has no archives, so `.noxml` is created immediately on first details fetch.
- Adding a top-level `ComicInfo.xml` by hand later deletes `.noxml` on the next details fetch (branch (a)).

### 7.4 Effect on title and chapter parsing
Branch (a)/(b)/(c) can set `manga.title` (from `Series`) before the chapter list is built (§5). This is the reason the chapter parser sees the series title.

### 7.5 ComicInfo-driven status override
`copyFromComicInfo` always assigns `status` (§19.7), even when the file has no publishing status, so a file without the element yields `status = 0` (UNKNOWN) and overwrites the previous value.

### 7.6 Failure policy
Every failure inside steps 2-7 is logged at ERROR with the manga title and swallowed. The manga is returned partially updated (for example the cover is set but the ComicInfo fields are not). The malformed ComicInfo.xml stays on disk and is re-parsed (and fails again) on every details fetch, because branch (a) keeps being taken. Proposal: keep behavior, add a `tracing::warn` with the file path (no UI change).

---

## 8. Manga chapter list (`getChapterList`)

Entry point `MANGA_SRC:244-303`. Input `manga: SManga` (`url`, `title`, `thumbnail_url`). Output `List<SChapter>` sorted newest-first. Errors propagate (no try around the whole method).

### 8.1 Steps
1. `chaptersData` = from `fileSystem.getFilesInMangaDirectory(manga.url).firstOrNull { extension == "json" && nameWithoutExtension == "chapters" }?.let { runCatching { json.decodeFromStream<List<ChapterDetails>>(...) }.getOrNull() }`. Any read or decode error gives null, silently (§20.3).
2. `entries = getFilesInMangaDirectory(manga.url)` filtered by `!name.startsWith('.')` and `(isDirectory || ArchiveManga.isSupported || extension.equals("epub", true))`.
3. For each entry, build `SChapter`:
   - `url = "${manga.url}/${name}"`;
   - `name = if (isDirectory) name else nameWithoutExtension` (`orEmpty()`);
   - `date_upload = lastModified()` (ms);
   - `chapterNumber = ChapterRecognition.parseChapterNumber(manga.title, this.name, -1.0).toFloat()` (§5), `chapter_number = chapterNumber`;
   - `format = Format.valueOf(chapterFile)`; if `Epub`, `format.file.epubReader(context).use { epub.fillMetadata(manga, this) }` (§21). Errors here are **not** caught: a broken EPUB fails the whole chapter list (E-MANGA-3);
   - if `chaptersData != null`: the first element `dataList.firstOrNull { it.chapter_number.equalsTo(chapterNumber) }` (tolerance 0.0001) overrides: `name` if non-null; `date_upload = parseDate(it)` if non-null (throws on bad format, E-DATE-2); `scanlator = data.scanlator` **unconditionally** (null clears an EPUB scanlator).
4. Sort: `sortedWith { c1, c2 -> val c = c2.chapter_number.compareTo(c1.chapter_number); if (c == 0) c2.name.compareToCaseInsensitiveNaturalOrder(c1.name) else c }` = descending by number, then descending natural name.
5. If `manga.thumbnail_url.isNullOrBlank()`: `chapters.lastOrNull()?.let { updateCover(it, manga) }`. `lastOrNull` after the descending sort is the **lowest-numbered** chapter (the first chapter).
6. Return `chapters` (a `List<SChapter>`, one per entry, no dedupe).

### 8.2 The chapter filter (exact)
`filterNot { startsWith('.') }` then `filter { isDirectory || ArchiveManga.isSupported(it) || it.extension.equals("epub", true) }`. Note `ArchiveManga.isSupported` already lowercases, the EPUB test lowercases too, so `.EPUB`, `.CBZ` are accepted. Files such as `page001.jpg` directly inside the manga folder are ignored (they are not chapters).

### 8.3 Chapter ordering summary
Descending by `chapter_number` (Float), ties broken by descending natural case-insensitive name. Unrecognised numbers (-1) therefore sort below all recognised numbers and among themselves by name descending.

### 8.4 Date
`date_upload` is the entry's modification time in milliseconds. For directories this is the directory's mtime (changes when files are added to it). chapters.json can replace it (§20.3).

---

## 9. Manga cover discovery and fallback

### 9.1 Discovery (`LocalMangaCoverManager.find`, OBSERVED `image/manga/LocalMangaCoverManager.kt:19-25`)
`getFilesInMangaDirectory(url).filter { isFile && nameWithoutExtension.equals("cover", ignoreCase = true) }.firstOrNull { ImageUtil.isImage(name) { openInputStream() } }`. So `cover.jpg`, `Cover.PNG`, `COVER.webp` match; `cover.txt` does not (unless its bytes are an image, which sniffing accepts). The first match in filesystem order wins (undefined on Android). Proposal: sort by name for determinism.

### 9.2 Write (`update`, OBSERVED `:27-49`)
1. `directory = getMangaDirectory(manga.url)`; if null: close stream, return null.
2. `targetFile = find(manga.url) ?: directory.createFile("cover.jpg")!!` (an existing cover of any extension is reused; its name is kept).
3. Copy the whole stream into `targetFile` (overwrite). No image validation.
4. `DiskUtil.createNoMediaFile(directory, context)` (§27.3).
5. `manga.thumbnail_url = targetFile.uri.toString()`; return the file.

### 9.3 Fallback from the first chapter (OBSERVED `updateCover`, `MANGA_SRC:336-375`)
Runs only from `getChapterList` when `thumbnail_url` is blank, and only for the lowest chapter. Errors are logged (`"Error updating cover for ${manga.title}"`) and swallowed, returning null.

- `Format.Directory`: `listFiles()` sorted by `compareToCaseInsensitiveNaturalOrder(name)`, then `find { !isDirectory && isImage(name) { openInputStream() } }`. Hidden files are NOT skipped (a `.cover.jpg` in a chapter folder can be chosen). The found image is copied to the cover.
- `Format.Archive`: `useEntries { entries.sortedWith { name compare natural CI }.find { isFile && isImage(name) { reader.getInputStream(name)!! } } }`; the entry is re-opened with `getInputStream(name)` and copied. Entries whose names start with `.` are not excluded. Directory entries in the archive are excluded by `isFile`.
- `Format.Epub`: `epub.getImagesFromPages().firstOrNull()` (first image in spine order) and `epub.getInputStream(it)!!` copied to the cover.

### 9.4 Browse thumbnails vs. details
Browse calls `find` (not `update`), so a manga with a cover file shows it in the browse grid without `getMangaDetails` running.

---

## 10. Manga reader format and page sources

### 10.1 `getFormat(chapter)` (OBSERVED `MANGA_SRC:321-334`)
```
try {
    val (mangaDirName, chapterName) = chapter.url.split('/', limit = 2)
    return fileSystem.getBaseDirectory()?.findFile(mangaDirName)?.findFile(chapterName)?.let(Format.Companion::valueOf)
        ?: throw Exception(context.stringResource(MR.strings.chapter_not_found))
} catch (e: Format.UnknownFormatException) {
    throw Exception(context.stringResource(MR.strings.local_invalid_format))
} catch (e: Exception) { throw e }
```
- Missing base, folder, or entry -> `Exception("Chapter not found")` (`MR.strings.chapter_not_found`, base value `Chapter not found`, `i18n/src/commonMain/moko-resources/base/strings.xml:699`).
- Existing entry that is neither directory, epub, nor manga archive -> `Exception("Invalid chapter format")` (`MR.strings.local_invalid_format`, `:700`).
- A `chapter.url` without `/` makes `split` return one element; destructuring `val (a, b)` then calls `component2()`, which throws `IndexOutOfBoundsException`. The generic `catch (e: Exception) { throw e }` rethrows it unchanged, so the reader shows a raw index error. Proposal: map to `Chapter not found`.

### 10.2 Reader dispatch (OBSERVED `app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ChapterLoader.kt:95-104`)
`isDownloaded` is checked first. Then `source is LocalMangaSource -> source.getFormat(chapter.chapter).let { when(format) { Directory -> DirectoryPageLoader(format.file); Archive -> ArchivePageLoader(format.file.archiveReader(context)); Epub -> EpubPageLoader(format.file.epubReader(context)) } }`. Downloaded chapters of a local manga would therefore be read through `DownloadPageLoader`; a local manga never has downloads in its own folder.

### 10.3 EPUB metadata for the reader
EPUB pages are the spine documents; images come from `img/@src` and `image/@xlink:href` (area:13-core-util-storage-archive-torrent reader contract, catalog `catalogs/android-api-mapping-03.md` describes the algorithm).

---

## 11. Anime

### 11.1 Fetch type (OBSERVED `LocalAnimeFetchTypeManager.kt:12-19`, `expect` at `source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeFetchTypeManager.kt:5-7`)
```
files = getFilesInAnimeDirectory(animeUrl)
when {
    files.any { ArchiveAnime.isSupported(it) } -> FetchType.Episodes
    files.any { it.isDirectory } -> FetchType.Seasons
    else -> FetchType.Episodes
}
```
- `FetchType` enum (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:10-20`): `Seasons` ("SAnime will only call getSeasonList") and `Episodes` ("will only call getEpisodeList"). Default in `SAnimeImpl` is `Episodes` (`SAnimeImpl.kt:32`).
- Evaluation order: any episode file wins over any directory. The `isSupported` check is case-sensitive. A directory whose name ends with `.mkv` counts as an episode (`ArchiveAnime.isSupported` does not check `isDirectory`).
- Hidden directories count (§4.3).

### 11.2 Browse (OBSERVED `ANIME_SRC:79-156`)
Same algorithm as manga (§6.1) except:
- Base list filter and dedupe identical; Latest and query rules identical; sort uses `AnimeOrderBy.Popular` and `AnimeOrderBy.Latest` (`filter/anime/AnimeOrderBy.kt:13-14`, same labels and Selection values as manga).
- Each directory is turned into `getSAnime(name)`: `title = name.substringAfterLast(File.separator)` (equals the name for top-level folders), `url = name`, `fetch_type = fetchTypeManager.find(name)`, `thumbnail_url = cover file uri or unchanged`, `background_url = background file uri or unchanged` (`ANIME_SRC:140-156`).
- `AnimesPage(animes, false)`.
- Cost: per folder three directory listings (fetch type, cover, background). Each listing is a `listFiles` of the same folder.

### 11.3 Seasons (OBSERVED `ANIME_SRC:158-234`)
`getAnimeSeasonUpdate(anime, seasons, fetchDetails, fetchSeasons): SAnimeSeasonUpdate` runs both parts in `supervisorScope`:
- `asyncAnime = if (fetchDetails) async { getOldAnimeDetails(anime) } else null`;
- `asyncSeasons = if (fetchSeasons) async { getOldSeasonList(anime) } else null`;
- result `SAnimeSeasonUpdate(asyncAnime?.await() ?: anime, asyncSeasons?.await() ?: seasons)`. When a flag is false the input is returned unchanged (`seasons` is the caller's list, possibly stale).

`getOldSeasonList(anime)`:
1. `animeDirs = getFilesInAnimeDirectory(anime.url).filter { isDirectory && !name.startsWith('.') }.distinctBy { name }`.
2. For each directory (concurrently via `async`): `url = anime.url + "/" + season`; `getSAnime(url)`. So a season `url` is `"show/season1"` and its `title` is `"season1"`.
3. Season's `fetch_type` = `find("show/season1")`: Episodes if it has video files, Seasons if it has sub-directories.
4. Sub-seasons are not recursed: `getOldSeasonList` is only called for the anime itself.

`getAnimeEpisodeUpdate(anime, episodes, fetchDetails, fetchEpisodes): SAnimeEpisodeUpdate` is symmetric (`ANIME_SRC:173-182`) with `getOldAnimeDetails` and `getOldEpisodeList`.

`override val supportsRelatedAnime = false` and `getRelatedAnimeList = emptyList()` (`:169-171`).

### 11.4 Anime details (`getOldAnimeDetails`, OBSERVED `ANIME_SRC:185-210`)
1. `coverManager.find(anime.url)?.let { anime.thumbnail_url = it.uri.toString() }`.
2. `backgroundManager.find(anime.url)?.let { anime.background_url = ... }`.
3. `animeDirFiles = getFilesInAnimeDirectory(anime.url)`; `firstOrNull { extension == "json" && nameWithoutExtension == "details" }` -> decode `AnimeDetails` (§20.2), apply each non-null field: `title`, `author`, `artist`, `description`, `genre` (joined `", "`), `status` (raw int). **No try/catch**: a bad JSON propagates to the caller (E-ANIME-2).
4. Return `anime`.

No ComicInfo is read for anime (ComicInfo only exists in `core-metadata` for manga-style content).

### 11.5 Episodes (`getOldEpisodeList`, OBSERVED `ANIME_SRC:237-316`)
Steps in order:
1. `episodesData = getFilesInAnimeDirectory(anime.url).firstOrNull { extension=="json" && nameWithoutExtension=="episodes" }?.let { json.decodeFromStream<List<EpisodeDetails>>(...) }`. **No runCatching**: decode failure propagates (E-ANIME-3).
2. `episodes = getFilesInAnimeDirectory(anime.url).filterNot { startsWith('.') }.filter { ArchiveAnime.isSupported(it) }.map { ... }`:
   - `url = "${anime.url}/${episodeFile.name}"`;
   - `name = episodeFile.nameWithoutExtension.orEmpty()`;
   - `date_upload = episodeFile.lastModified()`;
   - `episode_number = EpisodeRecognition.parseEpisodeNumber(anime.title, this.name, -1.0).toFloat()` (§5);
   - `episodesData?.also { list -> list.firstOrNull { it.episode_number.equalsTo(episodeNumber) }?.also { data -> data.name?.also { name = it }; data.date_upload?.also { date_upload = parseDate(it) }; scanlator = data.scanlator; summary = data.summary } }` (unconditional `scanlator` and `summary`; `fillermark` and `preview_url` are NOT read, §20.4);
   - thumbnail: `if (this.preview_url == null) { try { tempFileSuffix = anime.title + this.name + "thumbnail.jpg"; updateImageFromVideo(this, anime, tempFileSuffix) { thumbnailManager.update(anime, this, it) } } catch (e: Exception) { logcat ERROR "Couldn't extract thumbnail from video: $e" } }` (§14). Runs for **every** episode on every call where `preview_url` is null, which is every call (the field is always null on a fresh SEpisode). See E-ANIME-6.
3. `.sortedWith { e1, e2 -> val e = e2.episode_number.compareTo(e1.episode_number); if (e == 0) e2.name.compareToCaseInsensitiveNaturalOrder(e1.name) else e }` = descending number, then descending natural name.
4. Cover fallback: `if (anime.thumbnail_url == null || coverManager.find(anime.url) == null) { try { episodes.lastOrNull()?.let { updateImageFromVideo(it, anime, anime.title + "cover.jpg") { coverManager.update(anime, it) } } } catch ... log "Couldn't extract cover from video: $e" }`. Uses the **lowest** episode.
5. Background fallback: same condition shape with `backgroundManager`, suffix `anime.title + "background.jpg"`, last episode (lowest), log "Couldn't extract background from video: $e".
6. Return `episodes`.

Note the Kotlin constant names: `DEFAULT_COVER_NAME = "cover.jpg"`, `DEFAULT_BACKGROUND_NAME = "background.jpg"`, `DEFAULT_THUMBNAIL_NAME = "thumbnail.jpg"` (`ANIME_SRC:368-370`).

### 11.6 Anime cover/background/thumbnail writes
Identical pattern to manga cover (§9.2) with `background` and `thumbnail` specifics in §13.

### 11.7 Playback resolution (OBSERVED `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt:33-45,124-146`)
- `getHosters(episode, anime, source)`: `isDownloaded = isDownload(episode, anime)` first (uses AnimeDownloadManager.isEpisodeDownloaded with `skipCache = true`); if downloaded -> the downloaded path; else `source is AnimeHttpSource` -> online; else `source is LocalAnimeSource -> getHostersOnLocal(episode)`; else `error("source not supported")`.
- `getHostersOnLocal(episode)`: `try { val (animeDirName, episodeName) = episode.url.split('/', limit = 2); val fileSystem: LocalAnimeSourceFileSystem = Injekt.get(); val videoFile = fileSystem.getBaseDirectory()?.findFile(animeDirName)?.findFile(episodeName); val videoUri = videoFile!!.uri; val video = Video(videoUri.toString(), "Local source: ${episode.url}"); listOf(video).toHosterList() } catch (e: Exception) { emptyList() }`. So a missing file yields **no hosters** silently (the UI shows nothing playable).
- `getVideos(source, hoster)` for a local hoster: `hoster.videoList != null -> hoster.videoList!!` (no sorting since not AnimeHttpSource).
- The Video URL is the file URI (`file://...` or `content://...`). The player plays it directly.
- External player (`app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt:145-160`): for local anime `isOnDevice = true`; if the URI scheme is not `content`, the app passes a FileProvider URI. Proposal: pass the plain file path to the external program (no FileProvider on Linux).

---

## 12. Anime seasons and season-scoped episodes

Covered in §11.3 and §11.5. Additional rules:
- A season folder is treated as an anime (`fetch_type` computed on it). Its episodes are its own video files only (no recursion into season sub-folders). A season that contains only folders has `fetch_type = Seasons` and therefore its episode list is never requested by the app.
- Season `url` contains `/`. Season episode URLs contain two `/`. Parsing of the episode URL with `split('/', limit = 2)` yields `["show", "season1/ep01.mkv"]` and resolution relies on the nested-path resolution (§3.3).

---

## 13. Thumbnails, covers and backgrounds for anime: managers

### 13.1 `LocalAnimeCoverManager` / `LocalAnimeBackgroundManager` (OBSERVED)
Same as manga cover (§9) with names `cover` and `background` (`image/anime/*.kt:12` constants `cover.jpg`, `background.jpg`). Differences:
- `update` sets `anime.thumbnail_url` (cover) or `anime.background_url` (background).
- The directory is `getAnimeDirectory(anime.url)`. For a season, `anime.url` is `"show/season1"`, so the season folder is used.

### 13.2 `LocalEpisodeThumbnailManager` (OBSERVED `image/anime/LocalEpisodeThumbnailManager.kt:20-48`)
- `find(animeUrl, fileName)`: files in the anime folder with `isFile`, `nameWithoutExtension.equals(fileName, ignoreCase = true)`, first that `isImage`.
- `update(anime, episode, inputStream)`: `fileName = "${episode.name}-thumbnail.jpg"`; `targetFile = find(anime.url, fileName) ?: directory.createFile(fileName)!!`.
- **Quirk**: `find` compares the *stem* (`nameWithoutExtension`) with the full `fileName` (which still ends in `.jpg`). For `fileName = "EP 01-thumbnail.jpg"` the stem of a file called `EP 01-thumbnail.jpg` is `"EP 01-thumbnail"`, not equal. So `find` never matches in `update`, the code always takes the `createFile(fileName)!!` branch, and relies on the library returning the existing file (or creating a new one). Behavior is OBSERVED as-is; the file is always written to `<episode.name>-thumbnail.jpg`.
- Writes via `inputStream.use { output.copy }`, then `DiskUtil.createNoMediaFile(directory, context)`, then `episode.preview_url = targetFile.uri.toString()`.
- `episode.name` can contain characters that are invalid on Windows (`: ? * " < > |`). Proposal: the name passes through the NTFS-safe sanitizer of the cross-platform addendum (§27.4).
- `SEpisode.editThumbnail(anime, manager, stream)` (`app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt:66-74`) calls `update` only when `anime.isLocal()`.

---

## 14. ffmpeg frame extraction (OBSERVED `ANIME_SRC:332-362`)

`updateImageFromVideo(episode, anime, tempFileSuffix, updateImage: (InputStream) -> Unit)`:
1. `tempFile = File.createTempFile("tmp_", tempFileSuffix)`; `outFile = tempFile.path`. Created in `java.io.tmpdir`, never deleted by this code.
2. `episodeName = episode.url.split('/', limit = 2).last()` (for a season episode this is `"season1/ep01.mkv"`).
3. `animeDir = fileSystem.getAnimeDirectory(anime.url)!!`; `episodeFile = animeDir.findFile(episodeName)!!` (NPE when missing -> caught by caller's `catch (e: Exception)`).
4. `episodeFilename = { episodeFile.toFFmpegString(context) }` (raw path on Linux; SAF `content://` gets `FFmpegKitConfig.getSafParameter`; the result has `"` escaped as `\"`, `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt:21-26`).
5. Probe: `FFprobeKit.execute("-v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 \"${path}\"")`; `duration = ffProbe.allLogsAsString.trim().toFloat()`. Non-numeric output throws `NumberFormatException` (caught by the caller).
6. `second = duration.toInt() / 2` (truncate seconds, then integer division by 2).
7. Extract: `FFmpegKit.execute("-ss $second -i \"${path}\" -frames:v 1 -update true \"$outFile\" -y")`. The return code is ignored.
8. `if (tempFile.length() > 0L) updateImage(tempFile.inputStream())`. A zero-length output means no image and no error.

Callers and their suffixes:
- Episode thumbnail: `anime.title + this.name + "thumbnail.jpg"` (`ANIME_SRC:275`).
- Anime cover: `anime.title + "cover.jpg"` (`:293`).
- Anime background: `anime.title + "background.jpg"` (`:306`).

Error isolation: every call site wraps `updateImageFromVideo` in try/catch(Exception) and logs; nothing propagates. `FFmpegKit` never throws for a bad input (it returns a session), so errors surface as an empty temp file.

Proposal: see design §9 (ffmpeg subprocess, timeout, sequencing, no re-extraction when the target exists — a deviation from E-ANIME-6, flagged).

---

## 15. Manga details branch (b) and (c) (JSON and archive detail)

Covered in §7 and §20. The legacy `details.json` path is listed at P2.

---

## 16. Set-as-art for local entries (OBSERVED)

- Manga cover: `Manga.editCover(coverManager, stream, updateManga, coverCache)` (`app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt:50-65`): if `isLocal()` -> `coverManager.update(toSManga(), stream)` then `awaitUpdateCoverLastModified(id)`; else if `favorite` -> custom cover cache.
- Anime cover / background: `Anime.editCover` and `Anime.editBackground` (`app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt:36-64`): same shape with `LocalAnimeCoverManager` / `LocalAnimeBackgroundManager`, and `awaitUpdateBackgroundLastModified` for background.
- Episode thumbnail: `SEpisode.editThumbnail` (§13.2).
- Reader "set as cover" (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:887-900`): result `Success` when `manga.isLocal() || manga.favorite`, else `AddToLibraryFirst`. Exceptions -> `Error`.
- Player "set as art" (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1921-1937`): result `Success` when `anime.isLocal() || anime.favorite`, else `AddToLibraryFirst`; exceptions -> `Error`.
- Overwrite rule: an existing cover/background/thumbnail file (any image extension, matched by stem) is overwritten in place, keeping its name and extension. Bytes are not converted.

---

## 17. Local entries in downloads, library, filters, stats (OBSERVED)

### 17.1 Downloaded state
- Chapter list filter `downloaded`: `downloadManager.isChapterDownloaded(...) || isLocalManga` (`app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt:17-30`); the sorted-list variant at `:43-52` does `it.isDownloaded || isLocalManga`. So every chapter of a local manga counts as downloaded.
- Episode filter: `EpisodeFilter.kt:17,45` same shape with `isLocalAnime`.
- Library "downloaded" filter: `MangaLibraryScreenModel.kt:197` `it.libraryManga.manga.isLocal() || it.downloadCount > 0 || downloadManager.getDownloadCount(manga) > 0`; anime `AnimeLibraryScreenModel.kt:200` the same.
- Counts: `MangaDownloadManager.getDownloadCount(manga)` for local = `getFilesInMangaDirectory(url).count { it.isDirectory || ArchiveManga.isSupported(it) }` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:224-227`). Note: **EPUB is not counted** here, although it is a chapter. Anime `AnimeDownloadManager.getDownloadCount(anime)` for local = `count { ArchiveAnime.isSupported(it) }` on the anime folder (`AnimeDownloadManager.kt:229-233`).
- Sizes: `getDownloadSize(manga)` for local = `getMangaDirectory(url)?.size() ?: 0L` (`MangaDownloadManager.kt:245-249`); anime = `getAnimeDirectory(url)?.size() ?: 0L` (`AnimeDownloadManager.kt:250-255`). `size()` is the UniFile recursive size. Proposal: recursive byte sum.
- Local deletion of downloads: `deleteChapters` for local entries is not in this scope.

### 17.2 Library badge
- Preference `display_local_badge`, default `true` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:85`). When true, the library item carries `isLocal = manga.isLocal()` (`MangaLibraryScreenModel.kt:382`, `AnimeLibraryScreenModel.kt:396`).
- Stats screens count local entries with `isLocal()` (`MangaStatsScreenModel.kt:64`, `AnimeStatsScreenModel.kt:64`).

### 17.3 Local-only UI flags (summary, UI owned elsewhere)
- Download indicator hidden for local: `MangaScreen.kt:811` and `AnimeScreen.kt:1108` (`!manga.isLocal()`).
- Library selection: the **Download** action is passed only when every selected item is non-local (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt:179-180`: `.takeIf { state.selection.fastAll { !it.manga.isLocal() } }`); anime equivalent `AnimeLibraryTab.kt:172`.
- Browse toolbar: `BrowseMangaSourceScreen.kt:82` `actions = if (source is LocalMangaSource)` shows the help action (`onLocalSourceHelpClick`); anime `BrowseAnimeSourceScreen.kt:82`.
- Source list: the "Disable" action is not shown for the local ID (`app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt:192`: `if (source.id != LocalMangaSource.ID)`; anime `AnimeSourcesScreen.kt:188`). The local source cannot be disabled.

### 17.4 Update rules for local rows (OBSERVED `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt:40-60`, anime analogue in `UpdateAnimeFromRemote.kt`)
- `coverLastModified` for local: when `remoteManga.thumbnail_url` is non-empty and changed (or manual fetch), `localManga.isLocal() -> Instant.now().toEpochMilli()` (no cover cache deletion).
- Title: `title = null` (unchanged) when the row is a favorite (`localManga.favorite`) or the remote title is empty; else the remote title overwrites.

---

## 18. Sync rules for empty source lists (OBSERVED)
- Chapters: `SyncChaptersWithSource.await`: `if (rawSourceChapters.isEmpty() && !source.isLocal()) throw NoChaptersException()` (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt:56-58`). A local manga folder with no chapters is therefore a valid empty result.
- Episodes: `SyncEpisodesWithSource.kt:54` same with `NoEpisodesException`.
- Seasons: `SyncSeasonsWithSource.kt:31` same with `NoSeasonsException`.

---

## 19. ComicInfo.xml: model, read, write, mapping

### 19.1 Constants
- `const val COMIC_INFO_FILE = "ComicInfo.xml"` (`COMIC:9`).
- Schema reference comment: `https://anansi-project.github.io/docs/comicinfo/schemas/v2.0` (`COMIC:64`). Namespaces written by the app: `xmlns:xsd="http://www.w3.org/2001/XMLSchema"` and `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"` (`COMIC:88-93`).

### 19.2 Root and properties
`@XmlSerialName("ComicInfo", "", "")` root. Properties in declaration order (this is the element order on write):
`title, series, number, summary, writer, penciller, inker, colorist, letterer, coverArtist, translator, genre, tags, web, publishingStatus, categories, source` then the two attribute-like declarations `xmlSchema` and `xmlSchemaInstance` (`COMIC:68-93`).

Serial element names (from nested classes, each `@XmlValue(true) val value: String = ""`):

| Property | Class | Element name | Namespace prefix |
|---|---|---|---|
| title | Title | `Title` | none |
| series | Series | `Series` | none |
| number | Number | `Number` | none |
| summary | Summary | `Summary` | none |
| writer | Writer | `Writer` | none |
| penciller | Penciller | `Penciller` | none |
| inker | Inker | `Inker` | none |
| colorist | Colorist | `Colorist` | none |
| letterer | Letterer | `Letterer` | none |
| coverArtist | CoverArtist | `CoverArtist` | none |
| translator | Translator | `Translator` | none |
| genre | Genre | `Genre` | none |
| tags | Tags | `Tags` | none |
| web | Web | `Web` | none |
| publishingStatus | PublishingStatusTachiyomi | `PublishingStatusTachiyomi` | `ty` = `http://www.w3.org/2001/XMLSchema` (`COMIC:153`) |
| categories | CategoriesTachiyomi | `Categories` | `ty` = `http://www.w3.org/2001/XMLSchema` (`:157`) |
| source | SourceAniyomi | `SourceAniyomi` | `ay` = `http://www.w3.org/2001/XMLSchema` (`:161`) |

Note the nested class names are used as element names; the class name `Categories` is not the element name (`CategoriesTachiyomi` is the class). The `ty` and `ay` prefixes are both bound to the XML Schema URI; this is what the source declares, and it is kept as-is.

Null properties are omitted on write (every non-null field has exactly one child). Empty string values produce an empty element.

### 19.3 Write (OBSERVED `MangaDownloader.kt:650-680`, `xml.encodeToString(ComicInfo.serializer(), comicInfo)`)
The XML instance is the app's `XML { defaultPolicy { ignoreUnknownChildren() }; autoPolymorphic = true; xmlDeclMode = XmlDeclMode.Charset; indent = 2; xmlVersion = XmlVersion.XML10 }` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:179-189`). Consequences:
- XML declaration with encoding: `<?xml version="1.0" encoding="UTF-8"?>` (Charset mode writes an encoding declaration).
- Two-space indentation.
- Root element `<ComicInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">`.
- Child elements in the table order (nulls omitted).

Expected output for a chapter `Chapter 12` of series `Foo`, status Ongoing, genre `Action, Drama`, scanlator `Team`, URL `https://x`, category `Read`, source `Source A` (derived from the annotations; the golden file must be produced by running the upstream serializer, see the test file):

```
<?xml version="1.0" encoding="UTF-8"?>
<ComicInfo xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Title>Chapter 12</Title>
  <Series>Foo</Series>
  <Number>12</Number>
  <Translator>Team</Translator>
  <Genre>Action, Drama</Genre>
  <Web>https://x</Web>
  <ty:PublishingStatusTachiyomi xmlns:ty="http://www.w3.org/2001/XMLSchema">Ongoing</ty:PublishingStatusTachiyomi>
  <ty:Categories xmlns:ty="http://www.w3.org/2001/XMLSchema">Read</ty:Categories>
  <ay:SourceAniyomi xmlns:ay="http://www.w3.org/2001/XMLSchema">Source A</ay:SourceAniyomi>
</ComicInfo>
```

Two points are NOT fully determined by the source and must be confirmed with the golden run: (1) whether the `xmlns:ty`/`xmlns:ay` declarations are emitted on the element (shown) or on the root; (2) whether text escaping covers quotes (`&`, `<`, `>` are certain). Also `Number` for `12.5` is `12.5` (not `12.50`) per `Manga.kt:105-109`.

### 19.4 Read (OBSERVED `MANGA_SRC:235-241`)
`AndroidXmlReader(stream, "UTF-8").use { xml.decodeFromReader<ComicInfo>(it) }` then `manga.copyFromComicInfo(comicInfo)`. With `ignoreUnknownChildren()`, elements that are not in the model are skipped. Missing elements decode to null. Element text is `@XmlValue` (text of the element). An element present but empty decodes to the default `""` value (since the wrapper class default is `""`). The root element name must be `ComicInfo`.

Namespace matching: elements declared with a namespace (`ty:`, `ay:`) match only when the document binds the prefix to the same URI. A plain `<PublishingStatusTachiyomi>` without namespace does not match and is treated as absent (E-CI-6).

Fields read: `Series, Writer, Summary, Genre, Tags, Categories, Penciller, Inker, Colorist, Letterer, CoverArtist, PublishingStatusTachiyomi`. Fields never read on import: `Title, Number, Translator, Web, SourceAniyomi`.

### 19.5 Read entry points (manga)
- From top-level `ComicInfo.xml` (branch a) and from the copy made from an archive (branch c): both call `setMangaDetailsFromComicInfoFile` (`MANGA_SRC:235-241`).

### 19.6 Mapping SManga -> ComicInfo (OBSERVED `COMIC:11-31`, the local-source extension; the downloader builder is `app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:95-125`, owned by area:10-app-domain-layer)
```
fun SManga.getComicInfo() = ComicInfo(
    series = ComicInfo.Series(title),
    summary = description?.let { Summary(it) },
    writer = author?.let { Writer(it) },
    penciller = artist?.let { Penciller(it) },
    genre = genre?.let { Genre(it) },
    publishingStatus = PublishingStatusTachiyomi(ComicInfoPublishingStatus.toComicInfoValue(status.toLong())),
    title = null, number = null, web = null, translator = null, inker = null, colorist = null,
    letterer = null, coverArtist = null, tags = null, categories = null, source = null)
```
Only these 6 elements are written by the local-source builder (plus the implicit namespaces). Used only for branch (b).

### 19.7 Mapping ComicInfo -> SManga (OBSERVED `COMIC:33-62`)
Applied in this order:
1. `series?.let { title = it.value }` (an empty `Series` sets title to `""`).
2. `writer?.let { author = it.value }` (verbatim, no splitting).
3. `summary?.let { description = it.value }`.
4. Genre: `listOfNotNull(genre?.value, tags?.value, categories?.value).distinct().joinToString(", ") { it.trim() }`; if the result is non-empty (`takeIf { isNotEmpty() }`) set `genre`. The three strings are NOT split on commas; `distinct` compares whole strings; each string is trimmed when joined. An all-empty result leaves `genre` unchanged.
5. Artist: `listOfNotNull(penciller?.value, inker?.value, colorist?.value, letterer?.value, coverArtist?.value).flatMap { it.split(", ") }.distinct().joinToString(", ") { it.trim() }`; if non-empty set `artist`. Here the strings ARE split on `", "` (exact separator, two chars) before `distinct`. `translator` is not used.
6. `status = ComicInfoPublishingStatus.toSMangaValue(publishingStatus?.value)`: always assigned. Absent element -> `toSMangaValue(null)` -> UNKNOWN `0`.

Example: `genre = "Action, Drama"`, `tags = "Drama"` -> joined `"Action, Drama, Drama"` (no dedupe across commas). This is observed behavior; Proposal keeps it (byte-compatible with the Kotlin output). Note: the genre string is therefore not normalised.

### 19.8 Publishing status mapping (OBSERVED `COMIC:165-189`)
| `ComicInfoPublishingStatus` | `comicInfoValue` | `sMangaModelValue` (SManga) |
|---|---|---|
| ONGOING | `Ongoing` | 1 |
| COMPLETED | `Completed` | 2 |
| LICENSED | `Licensed` | 3 |
| PUBLISHING_FINISHED | `Publishing finished` | 4 |
| CANCELLED | `Cancelled` | 5 |
| ON_HIATUS | `On hiatus` | 6 |
| UNKNOWN | `Unknown` | 0 |

- `toComicInfoValue(value: Long)`: first entry with `sMangaModelValue == value.toInt()`, else `"Unknown"`. So SManga `0` and any out-of-range value (including SAnime `UPCOMING = 7`) write `Unknown`.
- `toSMangaValue(value: String?)`: first entry with `comicInfoValue == value` (exact, case-sensitive), else `0`. So `"ongoing"` (lowercase) reads as `0`.

### 19.9 `details.json` and the manga legacy migration
See §20.1. Legacy values are copied straight into SManga (`status` raw, not mapped through ComicInfo) and then `getComicInfo()` maps the status to a name for the written file.

---

## 20. JSON side files

### 20.1 Manga `details.json` (branch b, OBSERVED `MANGA_SRC:172-191`)
Decoded as `MangaDetails` (`core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/MangaDetails.kt`), all fields nullable. Apply, each only when non-null: `title`, `author`, `artist`, `description`, `genre` (joined with `", "` via `joinToString()`), `status` (raw). Then ComicInfo is written and the JSON deleted. Decoding uses the injected `Json` (`ignoreUnknownKeys = true`, `explicitNulls = false`, `AppModule.kt:173-178`). A decode failure is caught by the method-wide try (logged), the JSON remains.

### 20.2 Anime `details.json` (OBSERVED `ANIME_SRC:196-207`)
Decoded as `AnimeDetails`; the same six fields. **Not** caught locally: failures propagate to the episode / season update call (E-ANIME-2).

### 20.3 `chapters.json` (OBSERVED `MANGA_SRC:247-251`, `281-287`)
Decoded as `List<ChapterDetails>`; **the whole decode is inside `runCatching`**, so any failure (including a single element missing `chapter_number`) silently disables the file. Per-chapter overrides in §8.1 step 3.

### 20.4 `episodes.json` (OBSERVED `ANIME_SRC:237-243`, `262-270`)
Decoded as `List<EpisodeDetails>` with **no** runCatching. A failure aborts `getOldEpisodeList` (and therefore the episode update). Per-episode overrides: `name`, `date_upload`, `scanlator`, `summary`.

### 20.5 Matching rule for JSON entries
`dataList.firstOrNull { it.chapter_number.equalsTo(chapterNumber) }` — the first element in file order whose number is within `0.0001`. Duplicated numbers: the first wins. Entries that match no entry keep their defaults.

---

## 21. EPUB metadata fill (`fillMetadata`, OBSERVED `EpubReaderExtensions.kt:13-47`)

Called per EPUB chapter during the chapter list (§8.1 step 3) with `(manga, chapter)`:
1. `ref = getPackageHref()` (§25); `doc = getPackageDocument(ref)` (Jsoup parse of the OPF).
2. `title = doc.getElementsByTag("dc:title").first()`; `publisher` = `dc:publisher`; `creator` = `dc:creator`; `description` = `dc:description`; `date = dc:date` or, if absent, `doc.select("meta[property=dcterms:modified]").first()`.
3. `creator?.text()?.let { manga.author = it }`; `description?.text()?.let { manga.description = it }` — these mutate the **manga** on every EPUB chapter (last one wins).
4. `title?.text()?.let { chapter.name = it }`.
5. Scanlator: `publisher != null -> chapter.scanlator = publisher.text()`; else `creator != null -> chapter.scanlator = creator.text()`; else unchanged.
6. Date: if `date != null`: `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ssZ", Locale.getDefault()).parse(date.text())`; on success `chapter.date_upload = time`; on `ParseException` nothing.

`getElementsByTag` matches the tag name `dc:title` literally (Jsoup lowercases tag names in HTML mode). Namespace prefixes are not resolved.

Proposal: keep steps exactly; implement the parse with `time` and the format `[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory][offset_minute]`, which accepts `+0100` and `-0800`; a bare `Z` is accepted as UTC (Kotlin behavior for bare `Z` is not established, see E-DATE-3).

---

## 22. Anime and manga entry defaults (for the SManga / SAnime / SChapter / SEpisode structs)

Values from `source-api`:
- `SChapterImpl`: `url` and `name` lateinit (must be set), `date_upload = 0`, `chapter_number = -1f`, `scanlator = null`. (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapterImpl.kt:5-15`)
- `SEpisodeImpl`: `url`, `name` lateinit; `episode_number = -1f`; `fillermark = false`; `scanlator = null`; `date_upload = 0`; `summary = null`; `preview_url = null`; `memo = {}`. (`animesource/model/SEpisodeImpl.kt:8-27`)
- `SAnimeImpl`: `url`, `title` lateinit; `thumbnail_url = null`; `background_url = null`; `artist`, `author` null; `status = 0`; `description`, `genre` null; `initialized = false`; `update_strategy = ALWAYS_UPDATE`; `fetch_type = Episodes`; `season_number = -1.0`; `memo = {}`. (`animesource/model/SAnimeImpl.kt:8-36`)
- `SMangaImpl`: `url`, `title` lateinit; `artist`, `author`, `description`, `genre`, `thumbnail_url` null; `status = 0`; `update_strategy = ALWAYS_UPDATE`; `initialized = false`. (`source/model/SMangaImpl.kt:5-25`)

Serializable `status` constants: `UNKNOWN = 0, ONGOING = 1, COMPLETED = 2, LICENSED = 3, PUBLISHING_FINISHED = 4, CANCELLED = 5, ON_HIATUS = 6` (SManga), plus `UPCOMING = 7` (SAnime only) (`SManga.kt:48-54`, `SAnime.kt:72-79`).

`getGenres()` splits `genre` on `", "`, trims, drops blanks, distinct (`SManga.kt:29-32`) — the local source never sets `genre` through split, so the domain mapper does the split.

---

## 23. Errors, messages, and logging

### 23.1 User-visible strings used by this area
| Key (MR / AYMR) | Base English | Where raised |
|---|---|---|
| `MR.strings.chapter_not_found` | `Chapter not found` | `getFormat` (§10.1) |
| `MR.strings.local_invalid_format` | `Invalid chapter format` | `getFormat` (§10.1) |
| `MR.strings.local_filter_order_by` | `Order by` | sort filter label, both media |
| `MR.strings.title` | `Title` | sort option 0 |
| `MR.strings.date` | `Date` | sort option 1 |
| `AYMR.strings.local_manga_source` | `Local manga source` | source name |
| `AYMR.strings.local_anime_source` | `Local anime source` | source name |

Strings are resolved with `context.stringResource(...)` (`core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt`). The Rust port must use the same English text via `kuro-i18n` keys (the i18n area (51, no plan folder in this set) is not present in this plan set; the keys above are the contract).

### 23.2 Unimplemented methods (OBSERVED)
- `MANGA_SRC:316-319` `getPageList(chapter)` throws `UnsupportedOperationException("Unused")`. Pages are never requested through the source for local manga; the reader uses the loaders in §10.2.
- `ANIME_SRC:330` `getVideoList(episode)` throws `UnsupportedOperationException("Unused")`. Local playback uses §11.7.
Proposal: these are not public API in Rust; the methods are absent.

### 23.3 Logging
All logs use `logcat(LogPriority.ERROR)`:
- `"Error setting manga details from local metadata for ${manga.title}"` (with exception) — `MANGA_SRC:207-210`.
- `"Error updating cover for ${manga.title}"` — `MANGA_SRC:372`.
- `"Couldn't extract thumbnail from video: $e"`, `"Couldn't extract cover from video: $e"`, `"Couldn't extract background from video: $e"` — `ANIME_SRC:279,298,311`.
Proposal: `tracing::error!` with the same texts (in English), plus the path.

---

## 24. Concurrency and dispatchers (OBSERVED)
- Every public suspend method of both sources runs inside `withIOContext` (Dispatchers.IO): `getSearchManga`, `getMangaDetails`, `getChapterList`, `getSearchAnime`, `getOldAnimeDetails`, `getOldSeasonList`, `getOldEpisodeList`.
- Browse: concurrent `async` per directory with `awaitAll()` (§6.4, §11.2).
- Seasons: `supervisorScope` with `async` for details and seasons (§11.3); failure of one does not cancel the other, but an exception in an awaited child is rethrown from the scope.
- Chapter list cover and episode thumbnail generation: sequential.
- `updateImageFromVideo` runs ffmpeg on the calling thread (blocking) inside IO.
- The local source keeps no mutable state of its own. Covers and thumbnails are file-system state only.
- No debounce, no timeouts on ffmpeg/ffprobe (none in source).

---

## 25. EPUB reader contract consumed by this area (area:13-core-util-storage-archive-torrent owns the reader)

`EpubReader` (`core/archive/src/main/kotlin/mihon/core/archive/EpubReader.kt`) is not in this scope. The local source needs:
- `getPackageHref()`: `META-INF/container.xml` -> `rootfile[full-path]`, fallback `OEBPS/content.opf` (`EpubReader.kt:39-49`). Path separator is detected by the existence of the entry `META-INF\container.xml` (`:95-103`): if present the epub uses `\`.
- `getPackageDocument(ref)`: `getInputStream(ref)!!` parsed with Jsoup (`:54-56`). Missing entry -> NPE.
- `getImagesFromPages()`: spine order over `manifest > item` with `media-type == "application/xhtml+xml"` (`:61-68`), each page parsed, `img[src]` and `image[xlink:href]` resolved against the page directory (`:73-90`).
- `resolveZipPath(base, rel)` (`:108-122`): absolute if `rel` starts with the separator; otherwise `File(base, rel).canonicalPath`-style normalisation with the separator converted back.
- `getParentDirectory(path)` (`:127-134`).
Only `getPackageHref`, `getPackageDocument`, `getInputStream`, `getImagesFromPages` are used by this area (cover: `MANGA_SRC:364-367`; metadata: `EpubReaderExtensions`).

---

## 26. Preference and configuration keys touched

Full table in `p1-data-and-config.md` §4. Behaviorally relevant:
- `display_local_badge` (default true) — library badge (§17.2).
- `__APP_STATE_storage_dir` (`Preference.appStateKey("storage_dir")`) — base folder (§2).
- No preference controls the local source's sorting, page size, or thumbnail generation.

---

## 27. Cross-platform and Linux specifics

### 27.1 Paths
- `File.separator` is `/` on both Android and Linux; on Windows the Rust port must use `Path` joins and store URLs with `/` (§3.3).

### 27.2 Media scanning
`DiskUtil.scanMedia` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:97-100`, `MediaScannerConnection.scanFile`) is called from `createNoMediaFile` only when `.nomedia` is newly created and a context is given. Drop on Linux (no MediaStore). Behavior kept: create `.nomedia`.

### 27.3 `.nomedia` (OBSERVED `DiskUtil.kt:85-95`, `NOMEDIA_FILE = ".nomedia"` at `:140`)
`createNoMediaFile(dir, context)`: if `dir` exists and has no `.nomedia` child, create it (and scan on Android). Called after every cover, background, and thumbnail write (§9.2, §13). Never created for chapter or episode content folders by this area (the downloader creates `.nomedia` in downloads only: `StorageManager.kt:44-47`).

### 27.4 Name sanitising
Generated names in this area: `cover.jpg`, `background.jpg`, `thumbnail.jpg`, `ComicInfo.xml`, `.noxml`, `<episode-name>-thumbnail.jpg`, `tmp_*` temp files. Only the thumbnail name contains user data (episode name). Proposal: sanitise with the NTFS-safe function from the cross-platform addendum (forbidden characters `<>:"/\|?*`, reserved device names, trailing dot/space), keeping the stem unchanged on Linux when no forbidden character appears (so Linux output equals Kotlin output for ordinary names). Note: the `find` quirk in §13.2 means the sanitised name must be used for both find and create.

---

## 28. Interactions with other areas (contracts)

| Area | What this area provides | What this area needs |
|---|---|---|
| area:03-db-manga-schema-migrations / area:04-db-anime-schema-migrations | Local source id `0` stored in `mangas.source` / `animes.source` | nothing |
| area:05-data-repos-entries-items / area:06-data-repos-history-track-source-misc | Entry defaults (§22), `Manga/Anime` update rules | repository writes of returned values |
| area:07-domain-entries-items | Calls `parse_chapter_number` / `parse_episode_number` | the algorithms and the -1 convention |
| area:08-domain-library-category-history-updates | Library filters and badges (§17) | filter predicates honoring `isLocal` |
| area:09-domain-source-track-download-release-misc | Storage roots (§2), source list (§1) | `StorageManager` accessors and `Paths` |
| area:10-app-domain-layer | `getComicInfo` (downloader) builder shape (§19.6) and ComicInfo golden test | ComicInfo model and writer |
| area:13-core-util-storage-archive-torrent | Archive reader (`archiveReader`), EPUB reader (§25), `ImageUtil`, `DiskUtil`, `FFmpegUtils`, natural comparator home | these APIs |
| area:01-foundation-app-bootstrap | Cross-platform rules, DI names (`local_manga_fs` etc.), crate names (`kuro-source-local`, `kuro-core::metadata`) | conventions |
| area:02-preferences-system | `display_local_badge` and `__APP_STATE_storage_dir` preferences | preference store |
| the reader-core area (43, no plan folder in this set) (not present in this plan set) | Page loaders for Directory/Archive/Epub | `Format` values from §4.2 |

---

## 29. Help links (OBSERVED)
- `MANGA_SRC:379`: `HELP_URL = "https://aniyomi.org/help/guides/local-manga/"`.
- `ANIME_SRC:366`: `HELP_URL = "https://aniyomi.org/help/guides/local-anime/"`.
- Opened by the browse toolbar help action (§17.3). The URLs point at the Aniyomi project site; Proposal: replace with a Kurobayomi help page or open the local docs file. The help screen content is not part of this scope.

---

## 30. Constants (complete list, OBSERVED)

| Constant | Value | Location |
|---|---|---|
| `LocalMangaSource.ID` | `0L` | `MANGA_SRC:378` |
| `LocalAnimeSource.ID` | `0L` | `ANIME_SRC:365` |
| `LATEST_THRESHOLD` (both) | `604800000` ms (7 days) | `MANGA_SRC:381`, `ANIME_SRC:371` |
| `lang` (both) | `"other"` | `MANGA_SRC:72`, `ANIME_SRC:68` |
| `supportsLatest` (both) | `true` | `MANGA_SRC:76`, `ANIME_SRC:72` |
| `supportsRelatedAnime` (anime) | `false` | `ANIME_SRC:169` |
| Cover file name | `cover.jpg` | `image/manga/LocalMangaCoverManager.kt:12`, `image/anime/LocalAnimeCoverManager.kt:12`, `ANIME_SRC:368` |
| Background file name | `background.jpg` | `image/anime/LocalAnimeBackgroundManager.kt:12`, `ANIME_SRC:369` |
| Thumbnail suffix | `thumbnail.jpg` (file = `<episode.name>-thumbnail.jpg`) | `image/anime/LocalEpisodeThumbnailManager.kt:13,35`, `ANIME_SRC:370` |
| Metadata file | `ComicInfo.xml` | `COMIC:9` |
| No-XML marker | `.noxml` | `MANGA_SRC:159,202` |
| Legacy details | `details.json` | `MANGA_SRC:161`, `ANIME_SRC:197` |
| Chapter data | `chapters.json` | `MANGA_SRC:247` |
| Episode data | `episodes.json` | `ANIME_SRC:240` |
| No-media marker | `.nomedia` | `DiskUtil.kt:140` |
| Local folder names | `local`, `localanime` | `StorageManager.kt:100-101` |
| Number-equality tolerance | `0.0001` | `MANGA_SRC:310`, `ANIME_SRC:323` |
| Frame position | `duration.toInt() / 2` seconds | `ANIME_SRC:353` |
| ffprobe args | `-v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "<path>"` | `ANIME_SRC:350` |
| ffmpeg args | `-ss <sec> -i "<path>" -frames:v 1 -update true "<out>" -y` | `ANIME_SRC:356` |
| Date pattern (chapters/episodes json, manga & anime) | `yyyy-MM-dd'T'HH:mm:ss` | `MANGA_SRC:306`, `ANIME_SRC:319` |
| Date pattern (EPUB) | `yyyy-MM-dd'T'HH:mm:ssZ` | `EpubReaderExtensions.kt:38` |
| Manga archives | `zip cbz rar cbr 7z cb7 tar cbt` | `ARCH:17` |
| Anime episode files | `avi flv mkv mov mp4 webm wmv torrent` | `ARCH:8` |
| Epub ext | `epub` (case-insensitive) | `FMT:18`, `MANGA_SRC:257` |
| Default sort (popular) | Title, ascending | `MangaOrderBy.kt:13`, `AnimeOrderBy.kt:13` |
| Default sort (latest) | Date, descending, 7-day window | `MangaOrderBy.kt:14`, `AnimeOrderBy.kt:14` |
| Browse page size | none (single page, `hasNextPage = false`) | `MANGA_SRC:142`, `ANIME_SRC:137` |
| ComicInfo status values | 0..6 (+7 anime) | `SManga.kt:48-54`, `SAnime.kt:72-79` |
| EpisodeRecognition sub-number specials | `extra` 0.99, `omake` 0.98, `special` 0.97; alpha `a..i` -> 0.1..0.9 | owned by area:07-domain-entries-items |

---

## 31. Behavioral summary of Kotlin quirks that the Rust port must reproduce (or consciously change)

Reproduce exactly (observable in user data):
1. Chapter and episode order (§8.3, §11.5).
2. Browse order and the 7-day Latest window with query ignored (§6).
3. Genre merge without comma split and artist merge with `", "` split (§19.7).
4. Status always overwritten by ComicInfo (§19.7 step 6).
5. Unconditional `scanlator`/`summary` override from JSON (§20.3-20.4).
6. Silent chapters.json failure versus propagating episodes.json failure (§20.3-20.4).
7. The `.noxml` marker semantics (§7.3).
8. Extension case rules (§4.1-4.2).
9. Format check order directory -> epub -> archive (§4.2).

Consciously change (documented deviations, each with a test):
- Deterministic (sorted) directory and archive iteration instead of filesystem order.
- Path safety on URLs (§3.3).
- Frame extraction skipped when the target already exists unless forced (deviates from E-ANIME-6).
- Temp files deleted after use (no leak in cache).
- Natural order and case-insensitive ordering kept byte-exact (no change).
- `ChapterDetails`/`EpisodeDetails` date errors: treated as absent (no abort) — deviates from E-DATE-2 abort, flagged.

---

## 32. Open verification items (not resolvable from this clone)
- Exact `xmlutil` output for namespace declaration placement and text escaping (§19.3). Verify by running the upstream serializer once and committing the golden file.
- `tachiyomi.decoder.ImageDecoder` brand table (§4.4).
- `UniFile.createFile` behavior when the name already exists (§13.2) — the Rust port overwrites; Kotlin behavior is library-defined.
- Whether a literal `Z` suffix parses with `SimpleDateFormat` pattern `Z` (§21 step 6).
- `net.greypanther.natsort` behavior for empty strings: `compare("", "")` returns 0 (loop does not run, both indices 0) — established from the algorithm, listed for test coverage.
