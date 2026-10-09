# Area 34: Images, covers and cache — implementation tasks

Task IDs: `T-34a-NNN`. Dependencies use `T-34a-NNN` (this file) or `area:<id>` (other plan folders). Behavior references (`behavior §N`) point to `p1-behavior.md`; data references (`data §N`) point to `p1-data-and-config.md`; design references (`design §N`) point to `p1-rust-design.md`. Every Kotlin source path is repo-relative to `/home/blackbox/aniyomi-src`.

Coverage: every file in `p1-inventory.md` maps to at least one task. Platform rules come from `addenda/requirements-cross-platform.md`: OS access only through `kuro-platform`, paths only through the `Paths` trait, file names only through the shared sanitiser.

Abbreviations used in Sources: `CF` = `app/src/main/java/eu/kanade/tachiyomi/data/coil/`, `CC` = `app/src/main/java/eu/kanade/tachiyomi/data/cache/`, `PU` = `app/src/main/java/eu/kanade/presentation/util/`, `TV` = `app/src/main/java/eu/kanade/tachiyomi/util/view/`. (Sources lines below always spell out full paths.)

---

## Group A — Storage roots, hashing, image detection

### T-34a-001 — Art-store directory layout
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeBackgroundCache.kt
- Acceptance:
  - Manga covers live under `<data>/covers/` and custom manga covers under `<data>/covers/custom/`; anime covers under `<data>/animecovers/` and `<data>/animecovers/custom/`; anime backgrounds under `<data>/animebackgrounds/` and `<data>/animebackgrounds/custom/` (behavior §18, data §4).
  - All roots are obtained from `Paths::data_dir()`; the directories are created on first access (behavior §18.1).
  - No string literal path outside the `Paths` implementation (addendum rule 4).

### T-34a-002 — Cache key hashing (md5 hex)
- Priority: P0
- Effort: S
- Depends: T-34a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt
- Acceptance:
  - `hash_key(s)` equals md5 of the UTF-8 bytes of `s`, lower-case hex, 32 characters (behavior §18.2).
  - Golden vectors in T-34a-069 pass (`md5("")` = `d41d8cd98f00b204e9800998ecf8427e`).
  - Custom-cover file name is `hash_key(mangaId.to_string())` (decimal id, no padding).

### T-34a-003 — Image sniffing and ImageType table
- Priority: P0
- Effort: M
- Depends: T-34a-002
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt
- Acceptance:
  - `sniff(bytes)` reads at most 32 bytes and returns one of AVIF, GIF, HEIF, JPEG, JXL, PNG, WEBP or none (behavior §17.2).
  - `is_image(name, bytes)` returns true when the extension (case-sensitive, after the last `.`) is one of `avif gif heif jpg jxl png webp`, or when sniffing succeeds (behavior §17.3).
  - Mime and extension table matches data §6.1 exactly (`image/jpeg` ↔ `jpg`).

### T-34a-004 — .nomedia marker in image folders
- Priority: P1
- Effort: S
- Depends: T-34a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `create_nomedia(dir)` creates an empty `.nomedia` file only if the directory exists and the file is absent (behavior §21.6).
  - Calling it twice does not change the file.

---

## Group B — Cover, background and custom-cover cache

### T-34a-005 — Manga cover cache path API
- Priority: P0
- Effort: S
- Depends: T-34a-001, T-34a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt
- Acceptance:
  - `cover_file(thumbnail_url: Option<&str>) -> Option<PathBuf>` returns `None` for a missing URL and `covers/<md5(url)>` otherwise, including for empty strings (behavior §18.3).
  - `custom_cover_file(manga_id) -> PathBuf` always returns a path (behavior §18.3).

### T-34a-006 — Anime cover cache path API
- Priority: P0
- Effort: S
- Depends: T-34a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt
- Acceptance:
  - Same contract as T-34a-005 under `animecovers/` and `animecovers/custom/` (behavior §18.4).

### T-34a-007 — Anime background cache path API
- Priority: P1
- Effort: S
- Depends: T-34a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeBackgroundCache.kt
- Acceptance:
  - `background_file(url)` → `animebackgrounds/<md5(url)>`; `custom_background_file(anime_id)` → `animebackgrounds/custom/<md5(id)>` (behavior §18.5).

### T-34a-008 — Custom cover and background write
- Priority: P0
- Effort: S
- Depends: T-34a-005, T-34a-006, T-34a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeBackgroundCache.kt
- Acceptance:
  - `set_custom_cover(id, reader)` copies the whole stream into the custom file, truncating an existing file; any I/O error is returned to the caller and nothing else changes (behavior §18.6).
  - No validation of the bytes happens here: any stream is accepted (behavior §24.3).

### T-34a-009 — Cover deletion with deletion counts
- Priority: P0
- Effort: S
- Depends: T-34a-005, T-34a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt
- Acceptance:
  - `delete_from_cache(entry, delete_custom)` deletes the thumbnail-keyed file and, when requested, the custom file, and returns the number of files actually deleted (behavior §18.7).
  - `delete_custom_cover(id)` returns true only when the file existed and was removed.

### T-34a-010 — Background deletion with deletion counts
- Priority: P1
- Effort: S
- Depends: T-34a-007, T-34a-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeBackgroundCache.kt
- Acceptance:
  - Same counting contract as T-34a-009 for `animebackgrounds` (behavior §18.7).

### T-34a-011 — Cover invalidation when the source changes the thumbnail URL
- Priority: P0
- Effort: M
- Depends: T-34a-009, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt
- Acceptance:
  - `prep_update_cover(entry, remote)` implements the decision table in behavior §19.1 exactly (null or empty remote URL returns the entry unchanged; same URL returns unchanged unless `refresh_same_url`; local entries only bump the timestamp; custom-cover entries delete the thumbnail cache file without bumping the timestamp; others delete and bump).
  - Test cases C1 to C7 in `p1-edge-cases-and-tests.md` §3 pass.

### T-34a-012 — Cover removal when an entry leaves the library
- Priority: P0
- Effort: S
- Depends: T-34a-009, T-34a-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt
- Acceptance:
  - `remove_covers` deletes the thumbnail and custom cover, and bumps `cover_last_modified` only when at least one file was deleted; local entries are never touched (behavior §19.2).
  - Removing an entry from the library (`toggle_favorite` false branch) calls the same path; the bump is written only when the returned entry differs from the input (behavior §19.4).

### T-34a-013 — Cover and background last-modified persistence
- Priority: P0
- Effort: S
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt, data/src/main/sqldelight/data/mangas.sq, data/src/main/sqldelightanime/dataanime/animes.sq
- Acceptance:
  - `await_update_cover_last_modified(id)` writes `now_ms` (epoch milliseconds) to `cover_last_modified` only (behavior §19.5).
  - `await_update_background_last_modified(id)` writes `background_last_modified` only (anime).
  - Columns are `INTEGER NOT NULL` (data §5).

### T-34a-014 — Keyers for domain entries (memory and disk cache keys)
- Priority: P0
- Effort: M
- Depends: T-34a-005, T-34a-006, T-34a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeCoverKeyer.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt
- Acceptance:
  - `MangaKeyer` yields `manga;<id>;<cover_last_modified>` when a custom cover file exists and `manga;<thumbnail_url>;<cover_last_modified>` otherwise (behavior §10.1).
  - `AnimeKeyer` implements the four-branch table in behavior §10.2 (background, custom background, custom cover, thumbnail), with the background branches selected by the `useBackground` request option.
  - A key never contains a path; changing the cover timestamp changes the key (test C12).

### T-34a-015 — Keyers for cover projections (MangaCover / AnimeCover)
- Priority: P0
- Effort: S
- Depends: T-34a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeCoverKeyer.kt
- Acceptance:
  - `MangaCoverKeyer` yields `manga;<mangaId>;<lastModified>` when the custom file exists, otherwise `manga;<url>;<lastModified>` where a null URL is rendered as the four letters `null` (behavior §10.3).
  - `AnimeCoverKeyer` yields the `anime;` equivalents (behavior §10.4).

---

## Group C — Fetch pipeline (Coil replacement)

### T-34a-016 — Request options and extras
- Priority: P0
- Effort: S
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt
- Acceptance:
  - `LoadOptions` has fields `use_custom_cover` (default true), `use_background` (false), `crop_borders` (false), `custom_decoder` (false), `disk_cache` and `network_cache` (both `Enabled`), `memory_cache` (Enabled), `size` (Original) and `scale` (Fit) (data §7.1).
  - `use_custom_cover` is the only option whose default is true (behavior §3.1).
  - Requests can disable the memory cache per call (the full-cover viewer does; behavior §26).
  - Webtoon reader requests disable the memory and disk caches and the crossfade (behavior §3.2; reader choice recorded as open question O-9).

### T-34a-017 — Resource classification and error taxonomy
- Priority: P0
- Effort: S
- Depends: T-34a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt
- Acceptance:
  - `classify(s)` follows behavior §11.1 in order: empty or null → invalid; starts with `http` (case-insensitive) or `Custom-` (case-insensitive) → HTTP; starts with `/` or `file://` → file; starts with `content` → URI; otherwise invalid.
  - Invalid input produces the error `Invalid image`; null URL with no custom cover produces `No cover specified` (behavior §11.3).

### T-34a-018 — Cover fetch orchestration
- Priority: P0
- Effort: M
- Depends: T-34a-008, T-34a-017, T-34a-019, T-34a-021, T-34a-022
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt
- Acceptance:
  - Order of operations is exactly behavior §5.1: custom cover first (only when `use_custom_cover` and the file exists), then the resource dispatch.
  - Background requests (`use_background`) use the background URL and background caches; cover-only factories ignore `use_background` (behavior §4.2).

### T-34a-019 — Library cover file cache (write-through and promotion)
- Priority: P0
- Effort: M
- Depends: T-34a-005, T-34a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt
- Acceptance:
  - For favourite entries the HTTP loader reads `covers/<md5(url)>` first when the read policy allows it (behavior §6.2 step 1).
  - A fresh network response is written to the cover file through `write_source_to_cover_file` (mkdirs, delete, stream copy, delete on failure) when the write policy allows it (behavior §8.1).
  - A disk-cache snapshot is promoted into the cover file and the disk-cache entry is removed (behavior §6.3).
  - Failures while writing are logged with the exact messages in behavior §8.3 and fall back to the next source in the chain.

### T-34a-020 — Non-library disk cache (Coil-equivalent)
- Priority: P0
- Effort: M
- Depends: T-34a-002, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - Entries are addressed by the keyer string (T-34a-014/015), stored under `<cache>/image_cache`, and written as open-editor, write, commit (behavior §9.1).
  - Size limit follows Coil 3.1.0 defaults: 2 percent of free space, clamped to [10 MiB, 250 MiB] (behavior §9.2).
  - A failed write aborts the editor and re-raises the error (behavior §9.1 step 6).

### T-34a-021 — HTTP cover request
- Priority: P0
- Effort: M
- Depends: area:12-network-stack, T-34a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt
- Acceptance:
  - The client is the source's HTTP client when the entry's source is an HTTP source, otherwise the shared network client (behavior §7.1).
  - Source headers are copied verbatim; `Cache-Control: no-store` is sent when the network read policy is enabled, otherwise `no-cache, only-if-cached` (behavior §7.2).
  - HTTP 304 is accepted; any other non-2xx produces an error whose message is the HTTP reason phrase (behavior §7.3).

### T-34a-022 — Local file loader (`file://` and absolute paths)
- Priority: P0
- Effort: S
- Depends: T-34a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/AnimeImageFetcher.kt
- Acceptance:
  - `file://` prefix is stripped with `substringAfter("file://")`; absolute paths are used as-is (behavior §11.2).
  - The file is streamed, not loaded whole, and tagged with a disk data-source (behavior §11.2).
  - Android `content://` loading is dropped; a `content` URI returns the error `Invalid image` on Linux (data §9.2).

### T-34a-023 — Concurrency, cancellation and transition timing
- Priority: P0
- Effort: S
- Depends: T-34a-018
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - At most 8 fetches run concurrently and at most 3 decodes run concurrently (behavior §2.3).
  - Cancelling a request cancels its network call and drops its partial cache write (behavior §31).
  - Crossfade duration is `300 * animator_scale` milliseconds, truncated to an integer, and 0 when animations are disabled (behavior §2.4, task T-34a-054).

### T-34a-024 — In-memory byte source (page data already in memory)
- Priority: P0
- Effort: S
- Depends: T-34a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/BufferedSourceFetcher.kt
- Acceptance:
  - Bytes passed in by the reader are decoded without touching disk; the data source is `Memory` and the MIME type is unset (behavior §4.3).
  - Ownership: the buffer is consumed exactly once; a second decode of the same request is an error (behavior §4.3, edge E-IMG-12).

---

## Group D — Decoding and presentation

### T-34a-025 — Decoder selection rule
- Priority: P0
- Effort: M
- Depends: T-34a-003, T-34a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - The custom decoder handles a request when `custom_decoder` is true, or when the sniffed type is AVIF, JXL or HEIF (behavior §13.1).
  - Every other image type goes to the platform decoder path (T-34a-027).

### T-34a-026 — In-sample size and target-size mapping
- Priority: P0
- Effort: S
- Depends: T-34a-025
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt
- Acceptance:
  - `in_sample_size` implements behavior §15.1 with integer division, `takeHighestOneBit` semantics, FILL/FIT min/max and a floor of 1; a zero or sentinel destination dimension yields 1 (no division by zero).
  - `Size::Original` maps both dimensions to the source; `Pixel(n)` maps to `n`; `Undefined` maps to `i32::MIN` for FILL and `i32::MAX` for FIT (behavior §15.2).
  - Golden table G-SAMPLE in T-34a-069 passes.

### T-34a-027 — Raster decoder (JPEG, PNG, GIF, WebP, BMP) to RGBA
- Priority: P0
- Effort: L
- Depends: T-34a-025, T-34a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - Header dimensions are read before full decode; zero width or height is an error with message `Failed to initialize decoder` (behavior §14.2).
  - A decode returning no pixels is an error with message `Failed to decode image` (behavior §14.4).
  - Output is RGBA8 with straight alpha, row-major, and `is_sampled` is true when the in-sample size is greater than 1 (behavior §14.5).

### T-34a-028 — AVIF and JPEG XL still-image decoders
- Priority: P0
- Effort: M
- Depends: T-34a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - AVIF and JXL stills decode to the same RGBA output contract as T-34a-027.
  - A corrupt file produces the same two error messages; no partial texture is shown.
  - Decoders are feature-gated (`avif`, `jxl`) so a build without them still starts (design §4.3).

### T-34a-029 — HEIF/HEIC decoder
- Priority: P2
- Effort: M
- Depends: T-34a-028
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt
- Acceptance:
  - HEIF stills decode through `libheif` when the `heif` feature is enabled; without the feature the file reports `Failed to initialize decoder` (behavior §13.3).
  - Animated HEIF is not required (behavior §13.4).

### T-34a-030 — Border cropping option
- Priority: P1
- Effort: S
- Depends: T-34a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - When `crop_borders` is true the decoder crops uniform-colour borders before sampling (behavior §14.3). The border-detection rule is owned by area:44-reader-viewers; this task only passes the flag through.
  - When false the decode returns the full frame.

### T-34a-031 — Display ICC profile option
- Priority: P2
- Effort: S
- Depends: T-34a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - A user-chosen ICC file (preference `pref_display_profile_key`) is loaded once when the setting changes and applied to decodes (behavior §14.6).
  - If the profile cannot be applied, the image still displays without colour management (no error shown).
  - Linux-only deviation documented in design §9.

### T-34a-032 — EXIF orientation
- Priority: P1
- Effort: S
- Depends: T-34a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt
- Acceptance:
  - JPEG images with an EXIF orientation tag are rotated before the texture is made (design §4.5 proposal).
  - Parity with Android is recorded as unverified in behavior §14.7 until a device comparison is done.

### T-34a-033 — Animated GIF and WebP frames for covers
- Priority: P2
- Effort: M
- Depends: T-34a-027, T-34a-052
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt
- Acceptance:
  - Animated GIF and animated WebP play as frame sequences in the full-cover viewer (behavior §13.5 supported set).
  - Covers in grids show the first frame only.

### T-34a-034 — Hardware-bitmap threshold and device denylist (drop)
- Priority: P3
- Effort: S
- Depends: T-34a-027
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - No Linux code reads `pref_hardware_bitmap_threshold`; the key is not migrated (data §8.1).
  - Decision recorded: Android hardware bitmaps have no equivalent; GTK uploads RGBA textures and clamps by the GPU itself (behavior §16.3).

### T-34a-035 — Low-RAM RGB565 option (drop)
- Priority: P3
- Effort: S
- Depends: T-34a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - `allowRgb565` has no Linux equivalent; the decoder always produces RGBA8 (behavior §2.2).
  - Users lose a memory reduction on sub-3 GiB devices, which do not run this app on Linux.

### T-34a-036 — Texture creation for GTK
- Priority: P0
- Effort: S
- Depends: T-34a-027
- Sources: app/src/main/java/eu/kanade/presentation/util/Resources.kt, app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt
- Acceptance:
  - Decoded RGBA becomes a `gdk::MemoryTexture` with format `R8g8b8a8` and the decoded stride, created on the GTK main thread only (design §5.3).
  - The conversion never blocks the GTK thread for more than the copy time of the image (no decode on the main thread).

---

## Group E — Chapter (page) cache

### T-34a-037 — Page-list cache
- Priority: P0
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt, app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt
- Acceptance:
  - Key is `md5(chapter.manga_id concatenated with chapter.url)` (behavior §20.2).
  - Value is a JSON array of `{index, url, imageUrl}` with unknown keys ignored, null values omitted and default values omitted (data §6.4).
  - A missing or corrupt entry is reported as a miss, never a crash (behavior §20.3).

### T-34a-038 — Page-image cache with size limit
- Priority: P0
- Effort: L
- Depends: T-34a-002, T-34a-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt
- Acceptance:
  - Stored under `<cache>/chapter_disk_cache/` with the file name `<md5(url)>.0` (behavior §20.4).
  - Writes go to a temp file and are renamed on success, so a crash never leaves a partial entry visible (design §6.3).
  - Total size is capped at 100 MiB (104857600 bytes) with least-recently-used eviction (behavior §20.5).
  - Parameters `APP_VERSION = 1`, `VALUE_COUNT = 1`, `CACHE_SIZE = 100 MiB` appear as constants (data §8.3).

### T-34a-039 — Chapter-cache clearing and usage reporting
- Priority: P1
- Effort: S
- Depends: T-34a-038, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - `clear()` deletes every entry except the journal and returns the number of entries removed (behavior §20.6).
  - Settings shows `Used: <size>` from the recursive directory size, and a toast `Cache cleared, %1$d files deleted` on success or `Error occurred while clearing` on failure.
  - Auto-clear on launch uses preference `auto_clear_chapter_cache` (default false) (data §8.2).

### T-34a-040 — Read-through API for the reader page loader
- Priority: P0
- Effort: S
- Depends: T-34a-037, T-34a-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt
- Acceptance:
  - Exposes `get_page_list`, `put_page_list`, `is_image_cached`, `image_file`, `put_image` with the semantics in behavior §20.7.
  - A `put_page_list` failure is logged and ignored (behavior §20.3).
  - `image_file` is only called after `is_image_cached` returned true.

### T-34a-041 — Downloader copy from the page-image cache
- Priority: P1
- Effort: S
- Depends: T-34a-040, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt
- Acceptance:
  - A download reads a cached page image instead of refetching it when `is_image_cached(url)` is true (app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:481-482).
  - The copied file keeps its image type extension, detected by sniffing (area:23-downloads-manga owns the extension rule).

---

## Group F — Saving, naming and sharing

### T-34a-042 — Shared filename sanitiser
- Priority: P1
- Effort: M
- Depends: area:01-foundation-app-bootstrap, T-34a-069
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - `build_valid_filename` implements behavior §22.1 exactly (trim `.` and space, replace FAT-forbidden characters and controls with `_`, empty becomes `(invalid)`, cut to 240 characters).
  - Additional NTFS rules from the addendum (reserved names, trailing dot or space, 255 UTF-16 units) are applied afterwards; the Android-compatible result is kept whenever it is already valid (design §8.2).

### T-34a-043 — Byte-limited titles for reader and player file names
- Priority: P1
- Effort: S
- Depends: T-34a-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt
- Acceptance:
  - `take_bytes(s, n)` keeps the first `n` UTF-8 bytes and removes replacement characters produced by a cut mid-sequence (behavior §22.2).
  - Reader names are `<title> - <chapter name>` truncated to `250 - byte_size(" - <page>")` bytes, then `+ " - <page number>"` (behavior §22.3).
  - Player names use `" - <HH:MM:SS>"` as the suffix (behavior §22.4).

### T-34a-044 — Save destination and write semantics
- Priority: P1
- Effort: M
- Depends: T-34a-042, T-34a-043, area:53-platform-manifest-intents-workers
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt
- Acceptance:
  - Saved files go to `<XDG Pictures>/Kurobayomi/<relative path>/<name>.<ext>`, with the relative path being the folder-per-manga title when that preference is on (behavior §21.3).
  - Existing files with the same name are overwritten (behavior §21.4).
  - The Android MediaStore branch is replaced by a plain file write; the media-scan call is dropped (behavior §21.5).

### T-34a-045 — Share staging directory
- Priority: P1
- Effort: S
- Depends: T-34a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - Share writes into `<cache>/shared_image/` after deleting that directory recursively; only the most recent share is kept (behavior §21.7).
  - The staged file is removed by the next share, not on exit.

### T-34a-046 — Bitmap re-encode for covers
- Priority: P1
- Effort: S
- Depends: T-34a-027, T-34a-044
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt
- Acceptance:
  - Cover and background saves decode at original size and re-encode as JPEG quality 100 (behavior §21.2).
  - Names are the manga title for manga covers, `cover` or `background` for anime images, and the file extension is `jpg` (behavior §21.8).

### T-34a-047 — Share and copy actions
- Priority: P1
- Effort: M
- Depends: T-34a-045, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `Share` opens the staged file through the `Opener`/`Sharer` trait of `kuro-platform` (design §10).
  - `Copy image` (reader and player copy events) places the decoded texture on the clipboard.
  - When no sharing service is available the file is revealed in the file manager instead, and no error is shown.

### T-34a-048 — Save notifications
- Priority: P1
- Effort: M
- Depends: T-34a-044, area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt
- Acceptance:
  - Success shows the title `Picture saved` with a thumbnail (720 x 1280 bound) and two actions, open and share (behavior §23.2).
  - Failure shows title `Error` with the error text or `Unknown error`; the id is shared across saves so a new save replaces the old notification (behavior §23.2).
  - Notification is sent through `Notifier` in `kuro-platform` (design §10).

### T-34a-049 — Result messages for save, share and cover actions
- Priority: P1
- Effort: S
- Depends: T-34a-044, area:51-i18n
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt
- Acceptance:
  - Each action shows exactly the message key in behavior §23.1 (`Cover saved`, `Error saving cover`, `Background saved`, `Error saving background`, `Error sharing cover`, `Error sharing background`, `Cover updated`, `Background updated`, `Failed to update cover`, `Failed to update background`).
  - Messages are snackbars with a dismiss action on the dialog's snackbar host (behavior §23.1).

### T-34a-050 — Edit and delete custom cover or background from a local file
- Priority: P1
- Effort: M
- Depends: T-34a-008, T-34a-013, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaCoverScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt
- Acceptance:
  - Picking a file (filter `image/*`) copies it into the custom cover (library entries only) and bumps the timestamp (behavior §24.1-24.2).
  - Local-source entries write into the entry's own folder via the local cover managers instead (area:14-source-local-and-metadata owns that write).
  - Delete custom removes the custom file, bumps the timestamp and shows `Cover updated` (behavior §24.4).

### T-34a-051 — Migration copy of custom cover and background
- Priority: P2
- Effort: S
- Depends: T-34a-008, area:40-ui-browse-manga, area:41-ui-browse-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaDialog.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt
- Acceptance:
  - Migrating an entry copies its custom cover (and background for anime) to the new entry id (behavior §24.5).
  - A failure in the copy does not abort the migration.

---

## Group G — Cover and image UI

### T-34a-052 — CoverImage widget with three aspect ratios
- Priority: P0
- Effort: M
- Depends: T-34a-036, area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt, app/src/main/java/eu/kanade/presentation/util/Resources.kt
- Acceptance:
  - Ratios are `Square` 1:1, `Book` 2:3, `Thumb` 16:9 (behavior §25.1).
  - Crop-fills the box (`ContentScale.Crop` equivalent); corner radius is the theme's extra-small shape (data §10).
  - While loading shows a placeholder of colour `#1F888888`; on error shows the bundled `cover_error` image (behavior §25.2-25.3).
  - Replaces the grey frame placeholder in `src/ui.rs` `cover()`.

### T-34a-053 — Full-screen cover and background viewer
- Priority: P1
- Effort: L
- Depends: T-34a-052, T-34a-047, T-34a-050
- Sources: app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt, app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt
- Acceptance:
  - Two-page pager for anime (page 0 cover, page 1 background); manga has one page (behavior §26.1).
  - Top-right pill holds share, save, and edit (edit opens the picker, or a menu with Edit and Delete when a custom image exists) (behavior §26.3).
  - Tapping the image background dismisses the viewer; zoom animates 500 ms (behavior §26.2).

### T-34a-054 — Crossfade and reduced motion
- Priority: P1
- Effort: S
- Depends: T-34a-052
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - New images fade in over 300 ms when GTK animations are enabled and are shown immediately when they are not (behavior §2.4).
  - Reads `gtk-enable-animations` from `GtkSettings`.

### T-34a-055 — Image loader wiring at startup
- Priority: P0
- Effort: S
- Depends: T-34a-014, T-34a-018, T-34a-020, T-34a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - One shared image service is created at start and registered for the cover, background and reader paths (design §2).
  - Verbose logging (`verbose_logging` preference) turns on debug-level image logs (behavior §2.5).

---

## Group H — Formatting, messages, navigation helpers and platform glue

### T-34a-056 — Chapter and episode number text
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt
- Acceptance:
  - Output equals Java `DecimalFormat("#.###")` with decimal separator `.`, rounding HALF_EVEN, ASCII digits (behavior §27.1).
  - Golden values in T-34a-069 (G-NUM): 1.0 → `1`, 1.5 → `1.5`, 1.3333 → `1.333`, 1.0005 → `1`, 1.0015 → `1.002`, 0.0005 → `0`, -0.5 → `-0.5`, NaN → `NaN`, +Inf → `∞`.
  - Locale digits are not used (deviation recorded in design §11).

### T-34a-057 — Duration text
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt
- Acceptance:
  - Produces the units `%dd`, `%dh`, `%dm`, `%ds` with the rules in behavior §27.2 (minutes only when days are zero or hours are zero; seconds only when days and hours are zero).
  - Empty result returns the caller's fallback string.

### T-34a-058 — Relative update time text
- Priority: P1
- Effort: S
- Depends: T-34a-057
- Sources: app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt
- Acceptance:
  - Non-positive timestamps show `Never`; less than 60 seconds ago (including future timestamps) shows `Just now` (behavior §27.3).
  - Older values show a relative phrase with minute resolution up to one week and an absolute date after that (approximation recorded as open item O-7 in behavior §33).

### T-34a-059 — Exception to user message mapping
- Priority: P0
- Effort: M
- Depends: area:12-network-stack, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt
- Acceptance:
  - The branch order in behavior §28.1 is kept: torrent-disabled, HTTP, unknown host (offline versus `Couldn't reach %s`), no-results, source-not-installed, otherwise the class name rule.
  - HTTP status text is `HTTP %d, check website in WebView` with the code.
  - Golden cases G-EXC in T-34a-069 pass.

### T-34a-060 — Navigation helpers mapped to GTK and tokio
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/util/Navigator.kt
- Acceptance:
  - Screen transitions become a 300 ms `GtkStack` crossfade (forward and back use the same fade; the shared-axis slide is dropped, see p1-ui-parity.md §2) (behavior §29.2).
  - Back press is Escape or Alt+Left at the window level (behavior §29.1).
  - Per-screen background work runs on a tokio task group cancelled when the screen closes (behavior §29.3).

### T-34a-061 — Context menus through GIO menus
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt
- Acceptance:
  - `popup_menu(anchor, items, init)` becomes a `gtk::PopoverMenu` with the same item order; items marked hidden by `init` are not shown (behavior §30.3).
  - Opened by right-click and the Menu key (no long-press on desktop).

### T-34a-062 — Tablet layout breakpoint
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/util/WindowSize.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt
- Acceptance:
  - `is_tablet_ui()` returns true when `min(window_width, window_height)` in logical pixels is at least 720 (Android uses smallest screen width in dp, behavior §30.4).
  - Changes when the window is resized across the threshold.

### T-34a-063 — Visible-on-screen check
- Priority: P2
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - A widget counts as visible when it is mapped and its bounds intersect the root window bounds (behavior §30.5).
  - Used once when a page image finishes loading, to apply landscape zoom.

### T-34a-064 — Secure-screen flag (drop)
- Priority: P3
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - `FLAG_SECURE` has no Linux equivalent; the setting `secure_screen_v2` is kept in the schema but has no effect, and the settings screen states that screenshot blocking is unavailable on Linux (behavior §30.1, data §8.4).
  - User loses: screenshot and screen-recording blocking in incognito mode. Recorded in `p1-edge-cases-and-tests.md` §5.

### T-34a-065 — Install-unknown-apps permission state (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/util/Permissions.kt
- Acceptance:
  - No Linux code reads a package-install permission; extension installation is governed by area:18-extensions-manga and area:19-extensions-anime.
  - The helper is not ported.

### T-34a-066 — Fast-scroll item animation workaround (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/util/FastScrollAnimateItem.kt
- Acceptance:
  - The Compose work-around (issue 352584409) is not ported; list views use GTK defaults.
  - No user-visible loss.

### T-34a-067 — Compose and preference-view plumbing (drop)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/view/EditTextPreferenceExtensions.kt
- Acceptance:
  - `setComposeContent` and the `EditTextPreference` accessor are not ported; their replacements are the GTK widget tree and the preference model of area:02-preferences-system.
  - Nothing else in the codebase depends on them after the port.

### T-34a-068 — Platform glue for images
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: addenda/requirements-cross-platform.md
- Acceptance:
  - Every image path uses `Paths` (data, cache, pictures) and no `cfg(target_os)` appears in `kuro-images` (addendum rules 1 and 4).
  - Notifications, opening files and sharing go through `Notifier`, `Opener` and the share trait (design §10).

### T-34a-069 — Golden and unit test suite for this area
- Priority: P0
- Effort: M
- Depends: T-34a-002, T-34a-003, T-34a-026, T-34a-042, T-34a-056, T-34a-059
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt, app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - Golden vectors: md5 of the empty string and of the cover URL in `p1-edge-cases-and-tests.md` §6; sampling table G-SAMPLE; number table G-NUM; exception table G-EXC; filename table G-NAME.
  - Each non-trivial branch in T-34a-011 and T-34a-018 has one test (edge cases C1 to C12).
  - `cargo test -p kuro-images` passes with no network and no display.

### T-34a-071 — Decoded-image memory cache
- Priority: P0
- Effort: S
- Depends: T-34a-027, T-34a-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaCoverDialog.kt
- Acceptance:
  - A decoded image is cached by its cache key with a byte budget (proposal 64 MiB, design §6.1); the least-recently-used entry is evicted first (behavior §9.3 for the Coil default that is replaced).
  - Requests with the memory cache disabled (full-cover viewer, behavior §26.2) neither read nor write the cache.
  - Entries are never GTK objects; the cache holds shared RGBA buffers only (design §16 item 11).

### T-34a-070 — Decoder fixtures and format round-trip
- Priority: P1
- Effort: S
- Depends: T-34a-027, T-34a-028, T-34a-069
- Sources: core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - Fixture set in `p1-edge-cases-and-tests.md` §7: one file per supported type plus truncated and zero-byte files.
  - Each fixture decodes or fails with the messages in behavior §14.
