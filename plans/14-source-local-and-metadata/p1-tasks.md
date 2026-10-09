# Area 14 — Local source and metadata: implementation tasks

Task IDs: `T-14a-NNN`. Dependencies use `T-14a-NNN` (this file) or `area:<id>` (existing plan folders). Every task cites the Kotlin sources it implements (repo-relative to `/home/blackbox/aniyomi-src`) and the behavior section that defines it.

Coverage: every file in `p1-inventory.md` maps to at least one task. Priorities: P0 = needed to browse, read and watch local content daily; P1 = metadata, covers, thumbnails, library/update integration; P2 = legacy and long-tail; P3 = Android-only drops with the replacement stated.

---

### T-14a-001 — Local source identity and registration
- Priority: P0
- Effort: S
- Depends: area:06-data-repos-history-track-source-misc, area:09-domain-source-track-download-release-misc
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - `LocalMangaSource` reports id 0, name `Local manga source`, lang `other`, `supports_latest = true`, unmetered, not HTTP; `LocalAnimeSource` reports id 0, name `Local anime source`, lang `other`, `supports_latest = true`, `supports_related_anime = false` (behavior §1).
  - The local manga and anime sources are present in the source list regardless of enabled languages, and cannot be disabled (behavior §1, §17.3).
  - Related-anime listing returns an empty list.

### T-14a-002 — Local source id 0 and isLocal predicates
- Priority: P0
- Effort: S
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - `LOCAL_SOURCE_ID` is 0 and is the only value `is_local_source` treats as local for both media (behavior §1).
  - An entry whose `source` column is 0 is reported as local; no extension source uses id 0 (area:03-db-manga-schema-migrations id rule).

### T-14a-003 — Storage roots `local` and `localanime`
- Priority: P0
- Effort: M
- Depends: area:09-domain-source-track-download-release-misc, area:02-preferences-system, area:01-foundation-app-bootstrap
- Sources: domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt, domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/io/manga/LocalMangaSourceFileSystem.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/io/anime/LocalAnimeSourceFileSystem.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/io/manga/LocalMangaSourceFileSystem.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/io/anime/LocalAnimeSourceFileSystem.kt
- Acceptance:
  - Manga root is `<storage base>/local`, anime root is `<storage base>/localanime`; both are created on first access and never at startup alone (behavior §2, topic-layout §1).
  - Storage base from app-state key `__APP_STATE_storage_dir`; a missing base yields empty listings, not errors.

### T-14a-004 — Filesystem layer over std::fs
- Priority: P0
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent, T-14a-051
- Sources: source-local/src/commonMain/kotlin/tachiyomi/source/local/io/manga/LocalMangaSourceFileSystem.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/io/anime/LocalAnimeSourceFileSystem.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/io/manga/LocalMangaSourceFileSystem.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/io/anime/LocalAnimeSourceFileSystem.kt
- Acceptance:
  - `list_dir`, `find_child` (nested relative names), `metadata_of`, `modified_ms` reproduce `getFilesInBaseDirectory`, `getFilesInMangaDirectory`, `getFilesInAnimeDirectory`, `findFile`, `isDirectory`, `isFile`, `lastModified` for every layout in topic-layout §2-§4.
  - Listing is sorted by name in Rust (deterministic; Kotlin order is undefined) and is covered by the test `t_14_fs_safe_join_accepts_nested`.

### T-14a-005 — Name helpers (extension, stem)
- Priority: P0
- Effort: S
- Depends: T-14a-004
- Sources: core/common/src/main/java/tachiyomi/core/common/storage/UniFileExtensions.kt
- Acceptance:
  - `extension("cover") == "cover"`, `stem(".nomedia")` is `""`, `extension(".nomedia") == "nomedia"`, and `stem("a.b.cbz") == "a.b"` (behavior §4.1).
  - Extension is never lowercased by these helpers; callers apply their own case rule.

### T-14a-006 — Manga archive and EPUB format classification
- Priority: P0
- Effort: S
- Depends: T-14a-005
- Sources: source-local/src/commonMain/kotlin/tachiyomi/source/local/io/Archive.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/io/Format.kt
- Acceptance:
  - Manga archive set is exactly `zip cbz rar cbr 7z cb7 tar cbt`, matched case-insensitively; EPUB matched case-insensitively; order of checks is directory, epub, archive (behavior §4.2).
  - `Vol.CBZ`, `Vol.Cb7` are archives; `Vol.mkv` is not; a directory named `Book.epub` is a Directory format.

### T-14a-007 — Anime episode extension classification
- Priority: P0
- Effort: S
- Depends: T-14a-005
- Sources: source-local/src/commonMain/kotlin/tachiyomi/source/local/io/Archive.kt
- Acceptance:
  - Episode set is exactly `avi flv mkv mov mp4 webm wmv torrent` and is matched case-sensitively (`Ep.MKV` is not an episode) (behavior §4.2, E-CHAPTERS-9).
  - A `.torrent` file is listed as an episode (Kotlin parity) and fails at playback with no special message (documented).

### T-14a-008 — Image type detection
- Priority: P0
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/image/manga/LocalMangaCoverManager.kt
- Acceptance:
  - `is_image_named` first tests the case-sensitive extension list (`avif gif heif jpg jxl png webp`), and only sniffs the first 32 bytes when that fails (behavior §4.4).
  - Magic-byte table recognizes PNG, JPEG, GIF87a/89a, WEBP, AVIF, HEIF brands, JXL codestream and container; random bytes are not images (test `t_14_images_sniff_table`).

### T-14a-009 — Natural order comparator port
- Priority: P0
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `natural_cmp_ci` reproduces the upstream algorithm over UTF-16 units; all 13 cases of the test file §3.2 pass, including `z010 == z10` and `a == A` (topic-natural-sort §2-§3).
  - The upstream `InputOutputTest` fixtures sort identically.

### T-14a-010 — Chapter number recognition call contract
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items, T-14a-009
- Sources: domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterRecognition.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - Chapter numbers come from `parse_chapter_number(manga_title, entry_name, None)` with the entry name without extension for files and the directory name for directories; unknown results are -1 (behavior §5).
  - The manga title passed is the SManga title in effect at call time (the ComicInfo series title if details ran first).

### T-14a-011 — Episode number recognition call contract
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items, T-14a-009
- Sources: domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeRecognition.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - Episode numbers come from `parse_episode_number(anime_title, file_stem, None)`, and unknown values are -1 (behavior §5).
  - `Show - 01.mkv` style names parse through the shared algorithm (area:07-domain-entries-items tests).

### T-14a-012 — Manga browse listing (popular, latest, search)
- Priority: P0
- Effort: M
- Depends: T-14a-001, T-14a-004, T-14a-009, T-14a-013, T-14a-020
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - Browse lists non-dot directories of the manga root, de-duplicated by name, with `url = title = folder name` and cover thumbnail when `cover.*` is an image (behavior §6.1).
  - Latest keeps directories modified within 604 800 000 ms, sorts by date descending, ignores the query; blank query keeps all; non-blank query filters by case-insensitive contains (tests `t_14_browse_latest_date_desc_window`, `t_14_browse_query_contains_ignore_case`).
  - `page` is ignored and `has_next_page` is always false.

### T-14a-013 — Browse sort options (manga and anime)
- Priority: P0
- Effort: S
- Depends: T-14a-009
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/filter/manga/MangaOrderBy.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/filter/anime/AnimeOrderBy.kt
- Acceptance:
  - One Sort filter labelled `Order by` with options `Title` (index 0) and `Date` (index 1); default popular selection is Title ascending, latest selection is Date descending (behavior §6.2, data §2.6).
  - Title sort uses case-insensitive lexicographic order, not natural order (test `t_14_sort_title_is_not_natural`).

### T-14a-014 — Manga chapter list
- Priority: P0
- Effort: M
- Depends: T-14a-006, T-14a-010, T-14a-036, T-14a-041, T-14a-039, T-14a-043
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - Chapters are non-dot directories, manga archives and EPUB files; `url = "<manga>/<entry name>"`; sorted by chapter number descending then name descending natural case-insensitive (behavior §8).
  - Unrecognised numbers sort last; a broken EPUB does not hide other chapters (deviation E-MANGA-3, test `t_14_chapters_bad_epub_keeps_others`).

### T-14a-015 — Chapter-derived manga cover fallback
- Priority: P1
- Effort: S
- Depends: T-14a-014, T-14a-020
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/image/manga/LocalMangaCoverManager.kt
- Acceptance:
  - When the manga has no cover URL, the lowest-numbered chapter supplies the cover: first image in a directory, first image entry of an archive, or first spine image of an EPUB, each in natural order (behavior §9.3).
  - Failures are logged and swallowed; the cover is written as `cover.jpg` or the existing `cover.*` (tests `t_14_cover_fallback_*`).

### T-14a-016 — Manga reader format resolution
- Priority: P0
- Effort: S
- Depends: T-14a-006, T-14a-043
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - `format_of_chapter("<manga>/<entry>")` returns Directory, Archive or Epub; missing entry gives `Chapter not found`; wrong type gives `Invalid chapter format` (behavior §10.1, data §2.8).
  - A URL without `/` gives `Chapter not found` (deviation E-READ-1).

### T-14a-017 — Reader page loaders for local formats (consumer)
- Priority: P0
- Effort: M
- Depends: T-14a-016, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ChapterLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/EpubPageLoader.kt
- Acceptance:
  - A local chapter is loaded through the loader matching its Format: directory pages, archive pages, EPUB pages; downloaded-state precedence is kept (behavior §10.2).
  - Opening a local chapter with a missing file shows the `Chapter not found` message, not a crash.

### T-14a-018 — Anime browse listing
- Priority: P0
- Effort: M
- Depends: T-14a-001, T-14a-004, T-14a-013, T-14a-019, T-14a-020, T-14a-021
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - Browse lists non-dot anime folders with `url = title = folder name`, `fetch_type`, cover and background URLs (behavior §11.2).
  - Same query, latest window and sort rules as manga (tests `t_14_anime_browse_fetch_type_and_images`).

### T-14a-019 — Anime fetch type detection
- Priority: P0
- Effort: S
- Depends: T-14a-007, T-14a-004
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeFetchTypeManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeFetchTypeManager.kt
- Acceptance:
  - Any episode file gives Episodes; otherwise any directory (including hidden) gives Seasons; otherwise Episodes (behavior §11.1, E-ANIME-8, E-ANIME-10).
  - Persisted as 0 Seasons / 1 Episodes (data §1.7).

### T-14a-020 — Manga cover manager
- Priority: P0
- Effort: S
- Depends: T-14a-004, T-14a-008, T-14a-048
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/image/manga/LocalMangaCoverManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/manga/LocalMangaCoverManager.kt
- Acceptance:
  - `find` returns the first `cover.*` file (case-insensitive stem) that is an image; `update` overwrites the existing file (keeping its name) or creates `cover.jpg`, then creates `.nomedia` (behavior §9.1-9.2, E-IMG-1).
  - Set-as-cover on a local manga updates `thumbnail_url` and `cover_last_modified` (behavior §16).

### T-14a-021 — Anime cover and background managers
- Priority: P1
- Effort: S
- Depends: T-14a-020
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/image/anime/LocalAnimeCoverManager.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/image/anime/LocalAnimeBackgroundManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/anime/LocalAnimeCoverManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/anime/LocalAnimeBackgroundManager.kt
- Acceptance:
  - Cover (`cover.jpg`) and background (`background.jpg`) follow the same find/update rules as manga; background stem is case-insensitive (E-IMG-9).
  - `background_url` and `thumbnail_url` are set on update; `.nomedia` is created.

### T-14a-022 — Episode thumbnail manager
- Priority: P1
- Effort: S
- Depends: T-14a-021, T-14a-048
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/image/anime/LocalEpisodeThumbnailManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/anime/LocalEpisodeThumbnailManager.kt
- Acceptance:
  - Thumbnail file is `<episode name>-thumbnail.jpg` in the anime folder, overwritten on each update; `preview_url` is set to its `file://` URL (behavior §13.2, E-IMG-7).
  - Name is sanitised for Windows-invalid characters; Linux names are unchanged when no forbidden character is present (E-IMG-8).

### T-14a-023 — Anime episode list
- Priority: P0
- Effort: M
- Depends: T-14a-007, T-14a-011, T-14a-009, T-14a-040, T-14a-026, T-14a-027, T-14a-043
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - Episodes are non-dot files with an episode extension; `url = "<anime>/<file name>"`; `name` = file stem; `episode_number` from the recognizer; sorted by number descending then name descending (behavior §11.5).
  - Directories named like episodes are excluded (deviation E-ANIME-9, test `t_14_episodes_dir_named_mkv_excluded`).

### T-14a-024 — Anime seasons and season update flags
- Priority: P1
- Effort: M
- Depends: T-14a-018, T-14a-019, T-14a-023, T-14a-038
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - Season list = non-dot sub-folders with `url = "<anime>/<season>"`, title = season folder name, and per-season `fetch_type` (behavior §11.3, §12).
  - When a fetch flag is false the input list or object is returned unchanged; an error from one branch is returned after both finish (test `t_14_anime_season_update_error_from_one_side_still_returns_error`).

### T-14a-025 — Anime playback hoster for a local episode
- Priority: P0
- Effort: S
- Depends: T-14a-023, T-14a-051
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - A local episode resolves to one video whose URL is the file's `file://` URL and whose title is `Local source: <episode url>` (behavior §11.7, E-PLAY-3).
  - A missing or unsafe episode URL gives an empty hoster list without an error dialog; external-player launch passes a plain file path (E-PLAY-4).

### T-14a-026 — ffmpeg frame extraction (ffprobe, ffmpeg)
- Priority: P1
- Effort: M
- Depends: T-14a-022, T-14a-049, area:13-core-util-storage-archive-torrent
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt
- Acceptance:
  - Duration comes from `ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1`; the frame is `ffmpeg -ss <duration/2 truncated> -i <file> -frames:v 1 -update true <out> -y` run as an argument vector (behavior §14, topic-cover §6).
  - Missing tools skip thumbnails with one warning per process; probe and extraction have timeouts of 20 s and 120 s (E-MEDIA-1, E-MEDIA-5).

### T-14a-027 — Auto-generation policy in the episode listing
- Priority: P1
- Effort: S
- Depends: T-14a-026, T-14a-021, T-14a-022
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - Per-episode thumbnails are generated only when the thumbnail file is absent or empty (deviation E-ANIME-6, test `t_14_thumbnail_skip_existing`); cover and background fall back to the lowest episode when missing (behavior §11.5 steps 4-5).
  - Extraction failures never reach the listing result.

### T-14a-028 — Set-as-cover, background and thumbnail for local entries
- Priority: P1
- Effort: S
- Depends: T-14a-020, T-14a-021, T-14a-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - For local entries the image is written through the managers and the result is `Success`; for non-local favorites it goes to the custom cover cache; otherwise `AddToLibraryFirst` (behavior §16).
  - Errors map to `Error` without leaving a half-written file (atomic rename, T-14a-058).

### T-14a-029 — ComicInfo model
- Priority: P1
- Effort: M
- Depends: area:01-foundation-app-bootstrap
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt
- Acceptance:
  - `ComicInfo` has the 17 optional text fields in the Kotlin declaration order, with the element names and namespace prefixes of data §3.1 (`PublishingStatusTachiyomi`, `Categories` with `ty`, `SourceAniyomi` with `ay`).
  - `COMIC_INFO_FILE` is `ComicInfo.xml`.

### T-14a-030 — ComicInfo writer and golden test
- Priority: P1
- Effort: M
- Depends: T-14a-029
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - Writer emits the declaration `<?xml version="1.0" encoding="UTF-8"?>`, two-space indentation, root attributes `xmlns:xsd` and `xmlns:xsi`, the elements in data §5.3 order, and omits null fields (topic-comicinfo §2-§4).
  - The golden test `t_14_comicinfo_golden_upstream` is un-ignored and passes against a fixture generated from the upstream serializer (topic-comicinfo §8, open items O-1 to O-4 resolved).

### T-14a-031 — ComicInfo reader
- Priority: P1
- Effort: M
- Depends: T-14a-029
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - Reader requires root `ComicInfo`, ignores unknown children, accepts self-closing elements as empty values, unescapes entities, and accepts a UTF-8 BOM (behavior §19.4, E-CI-5, E-CI-9, E-CI-12).
  - Duplicate elements keep the last value; prefixes are ignored (deviations E-CI-4, E-CI-6); malformed or wrong-root input returns an error (E-CI-7, E-CI-8).

### T-14a-032 — ComicInfo publishing status mapping
- Priority: P1
- Effort: S
- Depends: T-14a-029
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt
- Acceptance:
  - Text <-> status mapping is exactly data §3.2: `Ongoing 1, Completed 2, Licensed 3, Publishing finished 4, Cancelled 5, On hiatus 6, Unknown 0`; unknown text reads as 0 and any other status (including 7) writes `Unknown`.
  - Lookup is case-sensitive (test `t_14_comicinfo_status_text_table`).

### T-14a-033 — ComicInfo to SManga mapping
- Priority: P1
- Effort: S
- Depends: T-14a-031, T-14a-032
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt
- Acceptance:
  - Series -> title, Writer -> author, Summary -> description; genre from Genre+Tags+Categories joined without splitting; artist from five credit fields split on `", "`; status always assigned (behavior §19.7, E-CI-1, E-CI-2, E-CI-3).
  - `Title`, `Number`, `Translator`, `Web`, `SourceAniyomi` are never read on import.

### T-14a-034 — SManga to ComicInfo bridge (legacy migration)
- Priority: P1
- Effort: S
- Depends: T-14a-030, T-14a-032
- Sources: core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt
- Acceptance:
  - The local-source builder writes Series, Summary, Writer, Penciller, Genre and PublishingStatusTachiyomi only (behavior §19.6).
  - The downloader's full builder (owned by area:10-app-domain-layer) produces the same element names for the fields it shares (test `t_14_manga_to_comic_info_fields`).

### T-14a-035 — Manga details orchestration
- Priority: P1
- Effort: M
- Depends: T-14a-033, T-14a-020, T-14a-030, T-14a-036, T-14a-037, T-14a-050, T-14a-057
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - Branch order: top-level ComicInfo.xml (removes `.noxml`); else legacy details.json (migrate, then delete); else, when no `.noxml`, copy ComicInfo from the first archive that has one, or create `.noxml` (behavior §7.1, E-MANGA-4, E-MANGA-5).
  - Never returns an error; failures are logged and the object keeps any fields already set (behavior §7.6).

### T-14a-036 — Archive ComicInfo copy
- Priority: P1
- Effort: S
- Depends: T-14a-035, area:13-core-util-storage-archive-torrent
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - Only archives whose entry is exactly `ComicInfo.xml` at the root are considered; the first one in name order is copied to the folder root (behavior §7.2, E-MANGA-7, E-MANGA-8).
  - Directories named like archives are skipped (E-MANGA-DETAILS-2).

### T-14a-037 — Legacy manga details.json migration
- Priority: P2
- Effort: S
- Depends: T-14a-034, T-14a-035
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/MangaDetails.kt
- Acceptance:
  - Non-null `details.json` fields (title, author, artist, description, genre joined `, `, status raw) are applied, ComicInfo.xml is written, and details.json is removed only after the write succeeds (E-MANGA-6).
  - Exact-name rule: `details.json` only (lowercase stem).

### T-14a-038 — Anime details.json
- Priority: P2
- Effort: S
- Depends: T-14a-024, area:04-db-anime-schema-migrations
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/AnimeDetails.kt
- Acceptance:
  - Same six fields as manga applied to the anime; decode failure propagates to the caller (E-ANIME-2, test `t_14_anime_details_bad_json_fails_update`).
  - No ComicInfo is read or written for anime.

### T-14a-039 — chapters.json overrides
- Priority: P2
- Effort: M
- Depends: T-14a-014, T-14a-050
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/ChapterDetails.kt
- Acceptance:
  - Whole file is ignored silently on any decode error; the first entry within `0.0001` of a chapter's number overrides name, date and scanlator; scanlator is assigned even when null (behavior §20.3, E-CHAPTERS-1..4).
  - A bad date string leaves the file date in place (deviation E-DATE-2).

### T-14a-040 — episodes.json overrides
- Priority: P2
- Effort: M
- Depends: T-14a-023, T-14a-050
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/EpisodeDetails.kt
- Acceptance:
  - Decode failure propagates (E-ANIME-3); `episode_number` is required (E-ANIME-4); override sets name, date, scanlator and summary (unconditional) and ignores `fillermark` and `preview_url` (E-ANIME-5).

### T-14a-041 — EPUB metadata fill
- Priority: P2
- Effort: M
- Depends: T-14a-042, T-14a-014
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/metadata/EpubReaderExtensions.kt
- Acceptance:
  - `dc:title` -> chapter name, `dc:creator` -> manga author and scanlator fallback, `dc:publisher` -> scanlator, `dc:description` -> manga description, `dc:date` (or `dcterms:modified`) -> chapter date when parseable (behavior §21).
  - Date formats `yyyy-MM-dd'T'HH:mm:ss` with `+hhmm`/`-hhmm`/`Z` accepted; unparseable dates leave the date unchanged (test `t_14_epub_date_formats`).

### T-14a-042 — EPUB reader contract (consumer of area:13-core-util-storage-archive-torrent)
- Priority: P1
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent, T-14a-006
- Sources: core/archive/src/main/kotlin/mihon/core/archive/EpubReader.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - The reader provides package href (container.xml, fallback `OEBPS/content.opf`), spine-ordered page images (`img/@src`, `image/@xlink:href`), and separator detection for backslash archives (catalog-03 EPUB entry; behavior §25).
  - Test fixtures `epub/minimal.epub` and `epub/backslash.epub` pass (`t_14_epub_separator_backslash`, `t_14_epub_first_image_spine_order`).

### T-14a-043 — Error model and user-visible messages
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - `Chapter not found` and `Invalid chapter format` are the exact user-visible texts; `getPageList` and `getVideoList` are absent in Rust (behavior §23.1-23.2).
  - Messages are resolved through the i18n key table with the base English values (test `t_14_messages_exact_text`).

### T-14a-044 — Local source in library, browse and download UX rules
- Priority: P1
- Effort: S
- Depends: area:08-domain-library-category-history-updates, area:09-domain-source-track-download-release-misc, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryScreenModel.kt, app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt, app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Local chapters and episodes count as downloaded; local download counts are directories plus manga archives (manga) and episode files (anime); EPUB is not counted (E-SYNC-3, E-SYNC-4).
  - `display_local_badge` (default true) controls the library badge; local entries are excluded from the Download action selection (behavior §17).

### T-14a-045 — Update rules for local rows
- Priority: P1
- Effort: S
- Depends: area:10-app-domain-layer, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt, app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt, app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt
- Acceptance:
  - For local rows a changed thumbnail bumps `cover_last_modified` to now without deleting any cover cache; for favorites the title is never overwritten by a refresh (behavior §17.4).
  - Anime background follows the same rules with `background_last_modified`.

### T-14a-046 — Sync exemption for empty local lists
- Priority: P1
- Effort: S
- Depends: area:10-app-domain-layer
- Sources: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt, app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SyncSeasonsWithSource.kt
- Acceptance:
  - An empty chapter, episode or season list is a valid result for local sources and raises no `NoChapters` / `NoEpisodes` / `NoSeasons` exception (behavior §18, E-SYNC-1).
  - Non-local sources keep their current exceptions.

### T-14a-047 — Help links for local sources
- Priority: P2
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - The help action opens a Kurobayomi-owned page (or is omitted); the Aniyomi URLs `https://aniyomi.org/help/guides/local-manga/` and `https://aniyomi.org/help/guides/local-anime/` are not opened (behavior §29, data §8).
  - The action is shown only in the local source browse toolbar.

### T-14a-048 — Cross-platform names and `.nomedia` writes
- Priority: P1
- Effort: M
- Depends: area:01-foundation-app-bootstrap, area:13-core-util-storage-archive-torrent
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/image/anime/LocalEpisodeThumbnailManager.kt
- Acceptance:
  - `create_nomedia` creates `.nomedia` only when absent; no media scan on Linux (behavior §27.2-27.3).
  - Generated names pass the NTFS-safe sanitizer (reserved device names, forbidden characters, trailing dot/space); Linux output is unchanged for ordinary names (topic-cover §4).

### T-14a-049 — Temp file lifecycle for frame extraction
- Priority: P2
- Effort: S
- Depends: T-14a-026, area:01-foundation-app-bootstrap
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - Temp outputs live in the app temp directory with a `tmp_` prefix and are removed after the image is written or on failure (E-ANIME-7, test `t_14_thumbnail_temp_removed`).
  - Dropping an in-flight extraction kills the child and removes the temp file (RC-7).

### T-14a-050 — Failure isolation and logging policy
- Priority: P1
- Effort: M
- Depends: T-14a-035, T-14a-039, T-14a-040
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt
- Acceptance:
  - Each failure point follows the table in behavior §31 and edge-case file §1: details and cover fallback swallowed, chapters.json silent, episodes.json and anime details propagated, EPUB failure isolated per chapter (deviation).
  - Every swallowed error is logged with the English text listed in behavior §23.3 and the path.

### T-14a-051 — Path safety for source URLs
- Priority: P0
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/io/manga/LocalMangaSourceFileSystem.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/io/anime/LocalAnimeSourceFileSystem.kt
- Acceptance:
  - `safe_join` rejects empty, `.`, `..`, absolute, NUL and backslash components and resolves nested season URLs (E-PATH-1, E-ANIME-11).
  - Rejected URLs behave as not found (empty hoster list, `Chapter not found`).

### T-14a-052 — Test suite and fixtures
- Priority: P0
- Effort: M
- Depends: T-14a-009, T-14a-014, T-14a-030, T-14a-042, T-14a-026
- Sources: domain/src/test/java/tachiyomi/domain/items/chapter/service/ChapterRecognitionTest.kt
- Acceptance:
  - All `t_14_*` tests in the edge-case file §5 exist; the ChapterRecognition cases (34) are ported or smoke-tested through the public function.
  - Fixtures from edge-case file §4 are committed; tests need no network and do not depend on filesystem order; tool-dependent tests skip with a printed reason when `ffmpeg` is absent.

### T-14a-053 — Drop Android-only plumbing
- Priority: P3
- Effort: S
- Depends: area:13-core-util-storage-archive-torrent
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt, core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt
- Acceptance:
  - Dropped with reason: `MediaScannerConnection` scan (no MediaStore on Linux; user loses nothing because indexers pick files up anyway), SAF `content://` URIs and `FFmpegKitConfig.getSafParameter` (Linux storage is a plain folder), `ParcelFileDescriptor` mmap reader (replaced by a path-based reader from area:13-core-util-storage-archive-torrent), FileProvider URIs for external players (plain paths instead).
  - Each drop is recorded in the inventory with its reason.

### T-14a-054 — Drop Gradle, ProGuard, consumer rules and manifests
- Priority: P3
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: core-metadata/build.gradle.kts, core-metadata/consumer-rules.pro, core-metadata/proguard-rules.pro, core-metadata/src/main/AndroidManifest.xml, source-local/build.gradle.kts, source-local/consumer-rules.pro, source-local/proguard-rules.pro, source-local/src/androidMain/AndroidManifest.xml, source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeFetchTypeManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/anime/LocalAnimeBackgroundManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/anime/LocalAnimeCoverManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/anime/LocalEpisodeThumbnailManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/image/manga/LocalMangaCoverManager.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/io/anime/LocalAnimeSourceFileSystem.kt, source-local/src/commonMain/kotlin/tachiyomi/source/local/io/manga/LocalMangaSourceFileSystem.kt
- Acceptance:
  - Dependencies are expressed in `kuro-core` and `kuro-source-local` Cargo manifests (serde, quick-xml, time, scraper, tokio, tempfile; no ffmpeg-kit, no xmlutil, no Jsoup); ProGuard and manifest files have no Rust counterpart and are dropped with that reason.
  - The nine `expect` declarations in `commonMain` (multiplatform interface layer) are dropped: each becomes the single Rust type of the same name in the module listed in the inventory; no trait is introduced for them (one implementation each, ponytail rule).
  - The Cargo feature for the local source is on by default; no runtime flag.

### T-14a-055 — Entry model defaults
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SMangaImpl.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapterImpl.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnimeImpl.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SEpisodeImpl.kt
- Acceptance:
  - Defaults match data §1: SChapter `chapter_number -1`, `date_upload 0`; SEpisode `episode_number -1`, `fillermark false`; SAnime `fetch_type Episodes`, `season_number -1.0`; SManga and SAnime `status 0`, `update_strategy ALWAYS_UPDATE` (test `t_14_entry_defaults`).
  - `url` and `name`/`title` are required on construction.

### T-14a-056 — Local time zone capture and date parsing
- Priority: P1
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/metadata/EpubReaderExtensions.kt
- Acceptance:
  - The local UTC offset is captured once at startup before any worker thread; if unavailable UTC is used (design §19).
  - `yyyy-MM-dd'T'HH:mm:ss` (chapters.json, episodes.json) and the EPUB date formats parse to epoch milliseconds in that offset; strict parsing with invalid values treated as absent (E-DATE-1, E-DATE-4).

### T-14a-057 — Per-manga serialization of details writes
- Priority: P1
- Effort: S
- Depends: T-14a-035
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt
- Acceptance:
  - Two details fetches for the same manga never write `ComicInfo.xml` or `.noxml` concurrently (RC-3, test `t_14_details_concurrent_same_manga_single_write`).
  - Different manga do not wait for each other.

### T-14a-058 — Atomic image writes
- Priority: P2
- Effort: S
- Depends: T-14a-020, T-14a-048
- Sources: source-local/src/androidMain/kotlin/tachiyomi/source/local/image/manga/LocalMangaCoverManager.kt, source-local/src/androidMain/kotlin/tachiyomi/source/local/image/anime/LocalAnimeCoverManager.kt
- Acceptance:
  - Writes go to a sibling temp file and are renamed over the target, so browse never sees a half-written cover (RC-1, test `t_14_images_write_atomic_rename`).
  - No temp file remains after success or failure.
