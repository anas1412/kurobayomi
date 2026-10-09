# Area 14: Local source (manga and anime) and metadata

Area id: `14-source-local-and-metadata`. Task prefix: `T-14a-NNN`. Repo root for all paths: `/home/blackbox/aniyomi-src` (repo-relative paths below).

## 1. Purpose and scope

Aniyomi's "Local source" is a built-in source that treats folders and files on disk as a catalogue. It has two halves:

- **Manga**: a `local/` folder under the storage base. Each subfolder is a manga. Chapters are subfolders, `.cbz`/`.zip`/`.rar`/`.cbr`/`.7z`/`.epub` archives, or EPUB files. Covers come from `cover.*` images. Metadata comes from `ComicInfo.xml` (and legacy `details.json` / `chapters.json`).
- **Anime**: a `localanime/` folder. Each subfolder is an anime. Either it holds episode files directly (`.mkv`, `.mp4`, etc.) or it holds season subfolders. Covers, backgrounds and per-episode thumbnails are image files in the folder. Thumbnails can be generated with `ffmpeg` frame extraction.

Why it matters for a usable Kurobayomi: the local source is the only source that works with zero network access and zero extensions installed. It is the first thing a user can read or watch, it is the test bed for the reader and player, and it is the only place where ComicInfo.xml interop with other comic managers happens. Its file-format rules (what counts as a chapter, an episode, a cover, or a thumbnail) are reused by downloads, backups and the reader.

In scope: source identity, storage roots, filesystem access, format and image classification, natural sort, browse, details, chapter and episode lists, covers, backgrounds, thumbnails, ffmpeg frame extraction, seasons, playback hoster, ComicInfo read and write, legacy JSON side files, EPUB metadata fill, set-as-art, sync and update exemptions, error model, and the test suite for all of this.

Out of scope (owned elsewhere): the chapter and episode number recognition algorithm (area:07-domain-entries-items), the archive and EPUB reader (area:13-core-util-storage-archive-torrent), the source API types (area:15-source-api-manga, area:16-source-api-anime), and the reader and player UI (area:43-reader-core, area:45-player-core).

Scope inventory: 36 files under `core-metadata/` and `source-local/` (28 Kotlin sources, 8 build, ProGuard and manifest files). Full per-file table with port decisions: [p1-inventory.md](p1-inventory.md).

## 2. Module map

### 2.1 Kotlin (observed)

| Package or path | Responsibility |
|---|---|
| `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt` | ComicInfo v2.0 model, XML read and write, publishing status table, SManga mapping both ways |
| `core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/` | JSON side-file models: `MangaDetails`, `AnimeDetails`, `ChapterDetails`, `EpisodeDetails` |
| `source-local/src/commonMain/.../local/entries/{manga,anime}/` | `expect` declarations of the two sources (no logic) |
| `source-local/src/androidMain/.../local/entries/manga/LocalMangaSource.kt` | Manga source: browse, details, chapter list, cover fallback, reader format, cover update |
| `source-local/src/androidMain/.../local/entries/anime/LocalAnimeSource.kt` | Anime source: browse, seasons, episodes, thumbnails, covers, backgrounds, details.json |
| `source-local/src/androidMain/.../local/entries/anime/LocalAnimeFetchTypeManager.kt` | Episodes vs Seasons decision for an anime folder |
| `source-local/src/androidMain/.../local/filter/{manga,anime}/*OrderBy.kt` | Sort filter with options Title and Date |
| `source-local/src/androidMain/.../local/image/{manga,anime}/*Manager.kt` | Cover, background and episode thumbnail slots (find and write) |
| `source-local/src/androidMain/.../local/io/{manga,anime}/Local*SourceFileSystem.kt` | Access to the `local` and `localanime` roots (UniFile, SAF-capable) |
| `source-local/src/commonMain/.../local/io/Format.kt`, `Archive.kt` | Chapter format (`Directory`, `Archive`, `Epub`) and extension sets |
| `source-local/src/androidMain/.../local/metadata/EpubReaderExtensions.kt` | EPUB package metadata fill for manga and chapter |
| `core/common/.../util/lang/StringExtensions.kt:37-40` | Case-insensitive natural order comparator wrapper (`net.greypanther.natsort`) |
| `core/common/.../util/storage/DiskUtil.kt` | `.nomedia` writing and MediaStore scan (dropped on Linux) |
| `core/common/.../tachiyomi/core/common/util/system/ImageUtil.kt:39-95` | Image magic-byte detection |
| `domain/.../storage/service/StorageManager.kt` | Storage roots `local` and `localanime` |
| `domain/.../items/chapter/service/ChapterRecognition.kt`, `domain/.../items/episode/service/EpisodeRecognition.kt` | Number recognition (called, not owned) |
| `app/.../ui/reader/loader/ChapterLoader.kt:98-104`, `app/.../ui/player/loader/EpisodeLoader.kt:124-146` | Callers that resolve format and playback for local entries |
| `app/.../util/MangaExtensions.kt`, `app/.../util/AnimeExtensions.kt` | Set-as-cover, set-as-background, set-as-thumbnail |
| `app/.../domain/items/chapter/interactor/SyncChaptersWithSource.kt:56`, `SyncEpisodesWithSource.kt:54`, `SyncSeasonsWithSource.kt:31` | Sync exemption for empty lists |

### 2.2 Rust (proposal, from [p1-rust-design.md](p1-rust-design.md) section 1)

| Crate | Module | Replaces |
|---|---|---|
| `kuro-core` | `src/metadata/comicinfo.rs` | `ComicInfo.kt`, quick-xml writer and reader |
| `kuro-core` | `src/metadata/details.rs` | the four JSON models, serde structs |
| `kuro-source-local` | `src/fs.rs`, `src/roots.rs`, `src/names.rs` | UniFile, `StorageManager` accessors |
| `kuro-source-local` | `src/format.rs`, `src/images.rs`, `src/natsort.rs` | `Format.kt`, `Archive.kt`, `ImageUtil.kt`, natsort |
| `kuro-source-local` | `src/sort.rs`, `src/recognition.rs`, `src/ids.rs` | filter sort objects, recognition wrappers, `id = 0` |
| `kuro-source-local` | `src/manga/{mod,browse,chapters,details,reader}.rs` | `LocalMangaSource.kt` |
| `kuro-source-local` | `src/anime/{mod,browse,fetch_type,seasons,episodes,playback}.rs` | `LocalAnimeSource.kt`, `LocalAnimeFetchTypeManager.kt` |
| `kuro-source-local` | `src/media.rs` | FFmpeg-kit, replaced by `ffprobe` and `ffmpeg` via `tokio::process` |
| `kuro-source-local` | `src/epub.rs` | `EpubReaderExtensions.kt`, `scraper` for OPF |

Rules that apply to every module: no `cfg(target_os)` in this area, paths only through `Path` and `PathBuf`, generated file names pass the NTFS-safe sanitizer, external tools are located through `kuro-platform` (area:01-foundation-app-bootstrap).

## 3. Reading guide

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Every in-scope file with LOC, declarations, port decision, target Rust module, priority and task IDs. Table of P3 drops. |
| [p1-behavior.md](p1-behavior.md) | The behavior spec, sections 0-32: identity, roots, paths, formats, browse, details, chapters, covers, reader, anime, seasons, thumbnails, ffmpeg, ComicInfo, JSON side files, EPUB, errors, concurrency, constants, Kotlin quirks to reproduce, open verification items. |
| [p1-data-and-config.md](p1-data-and-config.md) | Data models (shared source model, local models, side-file models), JSON shapes, ComicInfo on disk, on-disk artifacts, configuration keys, network endpoints, database rows, bit flags, edge data shapes. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases and failure modes (E-* ids), races, upstream tests to port, fixtures, new behavioral tests, test infrastructure, behavior-to-test coverage map, acceptance thresholds. |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, public API signatures, filesystem layer, names and formats, ComicInfo, browse, details, chapters, images, episodes, seasons, fetch type, playback, media, EPUB, threading, logging, Android-to-Rust mapping, time zone pitfall, mockup integration, test seams, non-goals, ponytail notes. |
| [p1-tasks.md](p1-tasks.md) | 58 implementation tasks `T-14a-001` to `T-14a-058` with priority, effort, dependencies, sources and acceptance criteria. |
| [p1-topic-local-layout-and-urls.md](p1-topic-local-layout-and-urls.md) | On-disk layout of local content, storage roots, entry URLs, path resolution, playback lookup. |
| [p1-topic-cover-and-thumbnail-pipeline.md](p1-topic-cover-and-thumbnail-pipeline.md) | The five image slots, stems, default file names, write rules, ffmpeg frame pipeline and its path quoting. |
| [p1-topic-comicinfo-xml.md](p1-topic-comicinfo-xml.md) | ComicInfo.xml byte-level contract: skeleton, element order, namespaces, golden-test open items. |
| [p1-topic-natural-sort.md](p1-topic-natural-sort.md) | Natural sort use sites (tie-breaks, cover candidates) versus plain case-insensitive sort (browse "Popular"). |

## 4. Key design decisions and open questions

Each item states the recommendation. Items marked **User call** need the user's answer before implementation of the affected task starts.

1. **ComicInfo.xml writes: keep or read-only?** Recommendation: keep both read and write. Other comic managers read the file, and Aniyomi-written files must stay readable. (User call, affects T-14a-030, T-14a-034.)
2. **Legacy `details.json` and `chapters.json` / `episodes.json`:** Recommendation: read-only parity (T-14a-037, T-14a-038, T-14a-039, T-14a-040), no writes. These are P2 and can slip without blocking P0. (User call only if the user has real legacy folders.)
3. **ffmpeg for thumbnails and frame extraction:** Recommendation: optional. If `ffmpeg` or `ffprobe` is missing from `PATH` (found through `kuro-platform`), the episode thumbnail feature is disabled with a visible message and everything else works. Do not bundle FFmpeg. (User call on whether the user installs `ffmpeg`.)
4. **Local root location:** Recommendation: the `storage_dir` preference from area:02-preferences-system defaults to `$XDG_DATA_HOME/kurobayomi`, and the local roots are `local/` and `localanime/` under it, matching Aniyomi's folder names so existing folders can be copied across unchanged. (User call on the base path.)
5. **Entry count parity (E-SYNC-4):** Recommendation: keep parity. The downloaded count excludes EPUB files, as in Aniyomi (`MangaDownloadManager.getDownloadCount`, lines 224-227). Documented deviation only if the user asks.
6. **Dropped unused hooks (`getPageList`, `getVideoList`):** Recommendation: drop. Both throw `Unused` and are never called, so the user loses nothing.
7. **Thumbnail find behavior:** Recommendation: exact-name lookup (`<episode name>-thumbnail.jpg`). This gives the same observable result as Kotlin's `find` without its never-matching branch (see the inventory row for `LocalEpisodeThumbnailManager.kt`).
8. **Priority inversions in the task graph (needs a decision, see section 6.3):** several P0 tasks depend on P1 or P2 tasks. Recommendation: split the P0 path so the optional side-file and overlay inputs are not prerequisites.

## 5. Cross-area dependencies

### 5.1 What this area needs

| From | What |
|---|---|
| area:01-foundation-app-bootstrap | App paths, `kuro-platform` tool lookup, `tracing` logging, cross-platform name rules, time zone capture |
| area:02-preferences-system | `storage_dir` preference, error message keys (T-14a-043 depends on it) |
| area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations | Source id `0` rule and `isLocal` predicates |
| area:05-data-repos-entries-items | Entry model defaults (`SManga`, `SAnime`, `SChapter`, `SEpisode`), `T-14a-055` |
| area:06-data-repos-history-track-source-misc | Source registration |
| area:07-domain-entries-items | Chapter and episode number recognition (`ChapterRecognition`, `EpisodeRecognition`) |
| area:08-domain-library-category-history-updates, area:09-domain-source-track-download-release-misc | Library and download UX for local rows, storage root accessors |
| area:10-app-domain-layer | Sync exemption and update rules for local rows |
| area:12-network-stack | Not required for local content (listed for completeness: the local source is `UnmeteredSource`) |
| area:13-core-util-storage-archive-torrent | Archive and EPUB reader, image decode, path-based reader, file-safe joins, natural order comparator (T-14a-009), path safety (T-14a-051 is in this area) |
| area:15-source-api-manga, area:16-source-api-anime | `SManga`, `SChapter`, `SAnime`, `SEpisode`, `Video` and source traits |

### 5.2 What others need from this area

| Consumer | What it uses |
|---|---|
| area:17-source-app-layer | Local source registration and source id `0` |
| area:23-downloads-manga, area:24-downloads-anime | Local entries in download counts, `.nomedia`, `format_of_chapter` |
| area:26-library-update-jobs | Empty-list sync exemption, update rules for local rows |
| area:37-ui-library | Local entries in library, filters and stats |
| area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | Details orchestration, set-as-cover, set-as-background, set-as-thumbnail |
| area:43-reader-core | `format_of_chapter` and the page loaders for directory, archive and EPUB chapters |
| area:45-player-core | `hosters_for(root, episode_url)` playback resolution and `Video` titled `Local source: <episode url>` |
| area:22-backup-restore | ComicInfo read and write for entries in backups |
| area:34-images-covers-cache | Cover image writes and atomic file rule (T-14a-058) |

## 6. Task summary

### 6.1 Counts (from `TASKS.csv`, rows whose file starts with `14-source-local-and-metadata/`)

Total 58 tasks.

| Priority | Effort S | Effort M | Effort L | Effort XL | Total |
|---|---:|---:|---:|---:|---:|
| P0 | 17 | 8 | 0 | 0 | 25 |
| P1 | 15 | 8 | 0 | 0 | 23 |
| P2 | 5 | 3 | 0 | 0 | 8 |
| P3 | 2 | 0 | 0 | 0 | 2 |
| Total | 39 | 19 | 0 | 0 | 58 |

Check: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint` on [p1-tasks.md](p1-tasks.md) prints `ok`.

### 6.2 P0 critical path (in order)

1. T-14a-051 Path safety for source URLs (depends on area:13).
2. T-14a-004 Filesystem layer (depends on area:13 and T-14a-051).
3. T-14a-005 Name helpers, then T-14a-006 archive and EPUB format classification and T-14a-007 episode extension classification.
4. T-14a-008 image type detection and T-14a-009 natural order comparator (both depend on area:13).
5. T-14a-001 and T-14a-002 source identity and id `0` predicates (depend on area:03, area:04, area:06, area:09).
6. T-14a-003 storage roots.
7. T-14a-010 and T-14a-011 recognition call contracts (depend on area:07 and T-14a-009).
8. T-14a-013 browse sort options and T-14a-019 anime fetch type detection.
9. T-14a-020 manga cover manager (depends on T-14a-048, which is P1).
10. T-14a-012 manga browse and T-14a-018 anime browse.
11. T-14a-016 manga reader format resolution and T-14a-017 reader page loaders.
12. T-14a-043 error model and user-visible messages.
13. T-14a-014 manga chapter list and T-14a-023 anime episode list (both depend on P1 and P2 tasks, see 6.3).
14. T-14a-025 anime playback hoster for a local episode.
15. T-14a-055 entry model defaults.
16. T-14a-052 test suite and fixtures, last, depends on P1 T-14a-026 (ffmpeg).

### 6.3 Risks

- **Priority inversions.** P0 tasks depend on non-P0 tasks: T-14a-014 depends on T-14a-036 (P1), T-14a-039 (P2), T-14a-041 (P2); T-14a-023 depends on T-14a-026 (P1), T-14a-027 (P1), T-14a-040 (P2); T-14a-020 depends on T-14a-048 (P1); T-14a-052 depends on T-14a-026 (P1). Left as written because task IDs and dependencies must not change in this pass. Fix: either raise the optional inputs to P0 or make the chapters.json, episodes.json and EPUB fill inputs optional at call time.
- **Recognition coupling.** The chapter and episode number is computed from the series title in effect at call time, so a ComicInfo title change alters parsed numbers (see [p1-data-and-config.md](p1-data-and-config.md)). Risk of confusing results if area:07 changes its algorithm.
- **Filesystem enumeration order.** Browse results must not depend on the order `read_dir` returns. Tests must sort or compare as sets (acceptance in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md)).
- **Missing ffmpeg.** Thumbnails and frame extraction silently degrade without `ffmpeg`. Must show a message, not fail browse.
- **ComicInfo golden test.** The exact `xmlutil` output (namespace placement, escaping) is unverified (behavior section 32). The golden file must be produced from the upstream serializer once before T-14a-030 can be accepted.
- **Ponytail ceilings.** No directory cache (every browse re-lists); fetch type re-lists anime folders unless the listing is passed down; sequential frame extraction. All recorded in [p1-rust-design.md](p1-rust-design.md) section 24.

### 6.4 Other area findings

- `_tools/tasks.py export` reports 4 unresolved tokens in `40-ui-browse-manga/p1-tasks.md` (lines 37, 83, 158, 537): `area:41-ui-browse-manga-part-2`, which is not a plan folder. Not in area 14; reported to the lead, not changed here. The export exit status is 1 because of these; the CSV is still written.

## 7. Definition of done for this area

Observable behaviors in Kurobayomi, with no extensions installed and no network:

1. A folder under `~/.local/share/kurobayomi/local/` (or the configured storage root) with subfolders of images shows up as a manga in Browse under the Local source with source id `0`.
2. Browse Popular sorts by title case-insensitively; Browse Latest uses the `Date` sort option (exact key in behavior section 6); Search matches the folder name. Sort option labels are `Order by`, `Title` and `Date`.
3. Opening a manga with a `ComicInfo.xml` shows its title, author, description, genres and publishing status. Changing those in the app and saving writes back a `ComicInfo.xml` that another comic manager reads.
4. Chapters appear for each subfolder, `.cbz`, `.zip`, `.rar`, `.cbr`, `.7z` and `.epub` file. Chapter numbers come from area:07 recognition. Ties are sorted by natural order, descending. Unrecognised chapters have number `-1`.
5. Opening a chapter in the reader works for directory chapters, archive chapters and EPUB chapters. Unknown formats show `Invalid chapter format` (T-14a-043) and do not crash.
6. An anime folder with `.mkv` or `.mp4` files lists episodes; an anime folder with only subfolders lists seasons, and each season lists its episodes. Opening an episode plays the file in the player with title `Local source: <episode url>`.
7. A missing file, an unsafe path in a source URL (for example one containing `..`), or a missing `ffmpeg` never crashes the app. Each shows an error message or disables the feature.
8. `cover.jpg` (any case, any supported image type) is used as the cover. Without it, the first image of the first chapter is used. Set as cover writes `cover.jpg` atomically.
9. Local entries are exempt from "removed from source" sync: an empty local folder does not delete existing library chapters.
10. Local entries appear in the library, download counts (excluding EPUB) and update lists under the same rules as Aniyomi.
11. `cargo test -p kuro-source-local -p kuro-core` passes on Linux with no optional tools. Tests that need `ffmpeg` print a reason and skip. The natural sort cases in T-14a-009 and the ComicInfo golden test both pass.
12. Every task `T-14a-001` to `T-14a-058` is either done, or dropped with the reason stated in its acceptance criteria (T-14a-053 and T-14a-054 are the P3 drop tasks).
