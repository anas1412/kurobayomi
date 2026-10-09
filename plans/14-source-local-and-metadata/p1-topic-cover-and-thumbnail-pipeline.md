# Topic: covers, backgrounds, episode thumbnails, and the ffmpeg frame pipeline

Scope: `source-local/src/androidMain/kotlin/tachiyomi/source/local/image/{manga,anime}/*.kt` (5 managers), `ANIME_SRC:332-362` (ffmpeg), `ANIME_SRC:272-313` (call sites), `MANGA_SRC:336-375` (archive/dir/epub cover fallback), `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:85-100` (`.nomedia`, media scan), `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt` (path quoting). Behavior: `p1-behavior.md` §9, §13, §14, §16. Edge cases: `p1-edge-cases-and-tests.md` §1.5, §1.7.

## 1. The five image slots
| Slot | Owner | Stem (case-insensitive) | Default file | Where written | Source URL field set |
|---|---|---|---|---|---|
| Manga cover | `LocalMangaCoverManager` | `cover` | `cover.jpg` | manga folder | `SManga.thumbnail_url` |
| Anime cover | `LocalAnimeCoverManager` | `cover` | `cover.jpg` | anime folder (or season folder) | `SAnime.thumbnail_url` |
| Anime background | `LocalAnimeBackgroundManager` | `background` | `background.jpg` | anime folder | `SAnime.background_url` |
| Episode thumbnail | `LocalEpisodeThumbnailManager` | exact `<episode.name>-thumbnail.jpg` (see §4) | same | anime folder | `SEpisode.preview_url` |
| Chapter-derived cover | (manga cover slot) | `cover` | `cover.jpg` | manga folder, from first chapter | `SManga.thumbnail_url` |

Common rules for the first three (OBSERVED in all managers):
- `find`: `getFilesInMangaDirectory(url) (manga) or getFilesInAnimeDirectory(url) (anime).filter { isFile && nameWithoutExtension.equals(stem, ignoreCase = true) }.firstOrNull { ImageUtil.isImage(name) { openInputStream() } }`.
- `update`: `directory = getMangaDirectory(url)` or `getAnimeDirectory(url)`; if null the stream is closed and null returned. Otherwise `target = find(url) ?: directory.createFile(default)!!`; the stream is copied into `target` (overwrite); `DiskUtil.createNoMediaFile(directory, context)`; the URL field is set to `target.uri.toString()`.
- No size or decode validation. Any byte stream is written.

## 2. Image detection (`ImageUtil.isImage`)
Two-step test (OBSERVED `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:39-47`):
1. Extension: `name.substringAfterLast('.')` must equal one of `ImageType.extension` values: `avif`, `gif`, `heif`, `jpg`, `jxl`, `png`, `webp`. Case-sensitive.
2. Sniff: `findImageType(openStream)` reads 32 bytes and asks `tachiyomi.decoder.ImageDecoder.findType` (external artifact `com.github.tachiyomiorg:image-decoder:41c059e540`, `gradle/libs.versions.toml:52`).

All local callers pass an `openStream` lambda, so a file with `.JPG` or `.jpeg` (fails step 1) is accepted when its bytes are an image.

Rust (`images::sniff`, design §4.4): magic-byte table over the first 32 bytes; identical acceptance for the formats above. HEIF brand list is a best-effort subset; the exact upstream brand list is an open item (behavior §32).

## 3. Chapter-derived cover (manga)
Runs only from `getChapterList` when `manga.thumbnail_url` is null or blank (behavior §9.3), and only for the lowest-numbered chapter (the list's last element after the descending sort).

| Chapter format | Candidate order | Candidate test | Copy source |
|---|---|---|---|
| Directory | `listFiles()` sorted by `compareToCaseInsensitiveNaturalOrder(name)` | `!isDirectory && isImage(name) { openInputStream() }` | `openInputStream()` |
| Archive | entries sorted by `compareToCaseInsensitiveNaturalOrder(name)` | `isFile && isImage(name) { getInputStream(name)!! }` | `getInputStream(name)!!` |
| EPUB | spine order from `getImagesFromPages()` | first element | `getInputStream(path)!!` |

Notes:
- Directory candidates include hidden files (`.cover.jpg`) — the chapter filter is not applied here (Kotlin parity; E-IMG-5 notes order).
- Errors: caught, logged (`"Error updating cover for ${manga.title}"`), result null.
- The result is written with the manga cover slot (`cover.jpg` or the existing `cover.*`).

## 4. Episode thumbnail naming (quirk and fix)
Kotlin:
```
fileName = "${episode.name}-thumbnail.jpg"               // LocalEpisodeThumbnailManager.kt:35
targetFile = find(anime.url, fileName) ?: directory.createFile(fileName)!!   // :36
```
`find(animeUrl, fileName)` matches `nameWithoutExtension.equals(fileName, ignoreCase = true)` (`:23`). `nameWithoutExtension` of a file named `EP 01-thumbnail.jpg` is `EP 01-thumbnail`, which never equals `EP 01-thumbnail.jpg`. So `find` never matches in `update`; every update goes to `createFile(fileName)`. The observable result is "write `<episode.name>-thumbnail.jpg`, overwriting". The Rust port looks up the exact file name instead; the observable result is identical and the code is honest (E-IMG-7).

Episode name sanitizing (Proposal, cross-platform addendum): `episode.name` is a file stem and can contain `: ? * " < > |` on Windows. The Rust name is `sanitize_file_stem(episode.name) + "-thumbnail.jpg"`. Linux output is identical when no forbidden character is present.

## 5. `.nomedia`
`DiskUtil.createNoMediaFile(dir, context)` (`DiskUtil.kt:87-95`): if `dir.exists()` and `.nomedia` is absent, create it; on Android also `MediaScannerConnection.scanFile` for the new file. Kotlin calls it after every slot write. Rust: `fs::create_nomedia(dir)` after each write; no scan (Linux has no MediaStore). Purpose on Linux: keep tracker-miner / Tracker indexers and any Android-synced copies from listing covers as media.

## 6. ffmpeg frame extraction
### 6.1 Commands (OBSERVED, `ANIME_SRC:349-357`)
Probe:
```
ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "<input>"
```
read `allLogsAsString.trim().toFloat()`.

Extract:
```
ffmpeg -ss <second> -i "<input>" -frames:v 1 -update true "<tmpOut>" -y
```
`<second> = duration.toInt() / 2` (integer seconds, half the duration truncated).

The output path has the suffix `<anime title>cover.jpg`, `<anime title>background.jpg` or `<anime title>thumbnail.jpg` (the episode thumbnail suffix also contains the episode name), so ffmpeg picks the JPEG muxer by extension. (The temp file name is `tmp_<random><title><suffix>`.)

### 6.2 Rust equivalent (`media.rs`, design §13)
- Argument vectors, no shell: `["-v","error","-show_entries","format=duration","-of","default=noprint_wrappers=1:nokey=1", input]` for ffprobe; `["-ss", second, "-i", input, "-frames:v", "1", "-update", "true", output, "-y"]` for ffmpeg.
- Tool lookup through `kuro-platform` (Linux: PATH; the Arch package `ffmpeg` provides both).
- Output: temp file via `tempfile` in `Paths::temp_dir()`; read and remove after use.
- Timeouts (Proposal): probe 20 s, extract 120 s. Kill child on timeout (`kill_on_drop`).
- Success test: output file exists and length > 0. Exit code not used (Kotlin parity).

### 6.3 Position rule
Only the midpoint is used. `duration` 0 gives `second = 0`. Negative durations are impossible from ffprobe; if they occur `second` is negative and ffmpeg fails, leaving an empty file (no image, no error).

### 6.4 Concurrency
Sequential in Kotlin. Rust: one `tokio::sync::Semaphore` with one permit shared by all local frame jobs, so two listings never run ffmpeg concurrently (behavior §24, design §16).

### 6.5 Error isolation
Every call site is wrapped (behavior §14); no error reaches the listing. A missing tool yields "no thumbnails" for the process lifetime with one warning (E-MEDIA-1).

## 7. When extraction runs (anime listing)
From `getOldEpisodeList` (behavior §11.5):
1. Episode thumbnails: for each episode whose `preview_url` is null. In Kotlin that is every episode on every listing (E-ANIME-6). Rust (Proposal): only when `thumbnail_path` does not already exist or is empty; a `force` flag (not exposed in UI) regenerates.
2. Cover: when `anime.thumbnail_url` is null or the cover file is missing; source = lowest episode.
3. Background: when `anime.background_url` is null or the background file is missing; source = lowest episode.

Cost model (Kotlin): N episodes -> N probes + N extractions per listing. Rust (Proposal): first listing N, later listings 0 (when files exist).

## 8. Writes from the user (set as art)
- Reader "set as cover" (manga): `coverManager.update(manga, stream)` -> §1 rules.
- Player "set as art": cover, background, or episode thumbnail through the same managers (`editCover`, `editBackground`, `editThumbnail`), see behavior §16.
- The result enum (`Success`, `AddToLibraryFirst`, `Error`) is UI state; not stored.

## 9. Acceptance checks (also in the test file)
- `t_14_images_overwrite_keeps_name`: writing into an existing `cover.png` keeps the name.
- `t_14_thumbnail_skip_existing` (Proposal): second listing does not invoke the fake ffmpeg.
- `t_14_thumbnail_temp_removed`: no `tmp_*` file left in the temp dir.
- `t_14_media_real_ffmpeg_frame` (skipped without ffmpeg): frame at 2 s of a 4 s clip is a JPEG.

## 10. Ponytail note
Five Kotlin managers become one `write_image`/`find_image` pair with two parameters (stem, default name) plus the thumbnail exact-name rule. Ceiling: none for behavior; thumbnail naming is the only special case.
