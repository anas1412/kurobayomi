# Area 34: Images, covers and cache (overview)

Area id: `34-images-covers-cache`. Source root for every path below: `/home/blackbox/aniyomi-src` (repo-relative paths). Target: `/home/blackbox/kurobayomi` (Rust, gtk4-rs 0.11, GTK 4.16+, no libadwaita).

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

Aniyomi loads every picture it shows that is not a live video frame or a native UI icon: entry covers (manga and anime), anime backgrounds, user-chosen custom covers and backgrounds, reader page images, local-folder covers, and the images that a user saves or shares. Four subsystems do this work:

1. **Coil 3 image loading.** Custom fetchers, keyers and a decoder are registered with Coil's `ImageLoader` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/`). They turn a domain object (`Manga`, `Anime`, `MangaCover`, `AnimeCover`) or a raw URL into a decoded image. Covers are cached on disk as plain files, and non-library images go to Coil's disk cache.
2. **Cover and background file stores.** `MangaCoverCache`, `AnimeCoverCache` and `AnimeBackgroundCache` (`app/src/main/java/eu/kanade/tachiyomi/data/cache/`) write covers to `<data>/covers`, `<data>/animecovers`, `<data>/animebackgrounds` and their `custom` subfolders. Keys are MD5 hex of the thumbnail URL or the entry id (`p1-behavior.md` §18).
3. **Reader page cache.** `ChapterCache` (`app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt`) stores the page list as JSON and the page image bytes in a `DiskLruCache` capped at 100 MiB (`PARAMETER_CACHE_SIZE = 100L * 1024 * 1024`). Downloads reuse it (`p1-behavior.md` §20).
4. **Save and share.** `ImageSaver` (`app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt`) writes reader and cover images to `Pictures` through MediaStore on Android 10+ and to the file system on older versions. Sharing goes through a staging folder and the Android chooser (`p1-behavior.md` §21-§23).

Presentation helpers that format the text shown next to images are also in scope: the chapter and episode number formatter, the duration and relative-time helpers, the exception-to-message mapper, the `Navigator` and `ViewExtensions` helpers, and the Compose resource helper for the error placeholder (`app/src/main/java/eu/kanade/presentation/util/`, `app/src/main/java/eu/kanade/tachiyomi/util/view/`).

### 1.2 Why it matters for a usable Kurobayomi

- Without covers, the library, browse, history and updates screens are unusable. Every grid in areas 37, 40, 41 and 42 shows covers.
- Without the reader page cache, a page can be fetched on every view, and the offline reading and download-copy flows (area 23) do not work.
- Without save and share, the viewer's Save, Share and Copy actions (area 43 and 45) and the cover viewer's actions (areas 38 and 39) do nothing.
- Without the decoder stack, AVIF and JPEG XL covers from some sources do not appear at all.

### 1.3 Scope boundaries

- In scope: the 23 Kotlin files listed in `p1-inventory.md` (2111 LOC by the scope script), their behavior, their Rust replacements, and the Linux UI for covers, the full-size viewer, save, share and storage settings.
- Out of scope, owned elsewhere: the reader page loader and page viewers (area:43-reader-core, area:44-reader-viewers), the player's frame save (area:45-player-core), the source HTTP client (area:12-network-stack), the database schema (area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:05-data-repos-entries-items), and the GTK theme (area:49-theme-and-design-system).

---

## 2. Module maps

### 2.1 Kotlin module map (OBSERVED)

All paths are under `app/src/main/java/`. 23 files in total.

| Package (repo-relative) | Files | Responsibility |
|---|---|---|
| `eu/kanade/tachiyomi/data/coil/` | `AnimeCoverKeyer.kt`, `AnimeImageFetcher.kt`, `BufferedSourceFetcher.kt`, `MangaCoverFetcher.kt`, `MangaCoverKeyer.kt`, `TachiyomiImageDecoder.kt`, `Utils.kt` | Coil 3 fetchers (domain entry, cover projection, in-memory bytes), keyers (cache keys), the native decoder factory, request-option extension functions and the dimension-to-pixel mapping. |
| `eu/kanade/tachiyomi/data/cache/` | `AnimeBackgroundCache.kt`, `AnimeCoverCache.kt`, `ChapterCache.kt`, `MangaCoverCache.kt` | On-disk stores for covers, custom covers, backgrounds, page lists and page images. |
| `eu/kanade/tachiyomi/data/saver/` | `ImageSaver.kt` | Save to `Pictures` (MediaStore or direct file), stage a file for sharing, naming. |
| `eu/kanade/presentation/util/` | `ExceptionFormatter.kt`, `FastScrollAnimateItem.kt`, `ItemNumberFormatter.kt`, `Navigator.kt`, `Permissions.kt`, `Resources.kt`, `TimeUtils.kt`, `WindowSize.kt` | Text formatters (numbers, durations, relative time, exception text), navigation helpers, a permission state, a resource painter and a tablet check. |
| `eu/kanade/tachiyomi/util/view/` | `EditTextPreferenceExtensions.kt`, `ViewExtensions.kt`, `WindowExtensions.kt` | Android view helpers: Compose hosting, popup menus, visible-on-screen check, the `FLAG_SECURE` window flag, a preference listener accessor. |

Two files outside these packages are part of the image path and are cited by the design docs: `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt` (image-type sniffing, `ImageType` table) and `App.kt:189` (order of Coil factory registration, see `p1-topic-decode-and-sampling.md` §1).

The native decoder (`tachiyomi.decoder.ImageDecoder`, dependency `com.github.tachiyomiorg:image-decoder:41c059e540`) is not in the repository. Its behavior is known only from call sites (`p1-behavior.md` §14, open question O-3).

### 2.2 Proposed Rust module map (PROPOSAL)

Defined in `p1-rust-design.md` §1. Summary:

| Crate | Module | Replaces (Kotlin) | Responsibility |
|---|---|---|---|
| `kuro-images` (new) | `art.rs` | the four Coil fetchers and four keyers | `ArtRef`, `ArtKind`, `Owner`: one input type for manga, anime, cover and background |
| `kuro-images` | `keys.rs` | `*Keyer.kt` | `cache_key(&ArtRef, custom_exists)` |
| `kuro-images` | `options.rs`, `resource.rs`, `sniff.rs`, `sample.rs` | `Utils.kt`, `ImageUtil.kt` sniffing, `Resource` classification | `LoadOptions`, `classify`, `sniff`, `in_sample_size`, `target_dim` |
| `kuro-images` | `decode/{mod,raster,avif,jxl,heif,orient}.rs` | `TachiyomiImageDecoder.kt` | feature-gated decoders (`image`, `avif-decode`, `jxl-oxide`, `libheif-rs` off) |
| `kuro-images` | `fetch.rs`, `http.rs`, `cover_file.rs`, `disk_image_cache.rs`, `memory_cache.rs` | `MangaCoverFetcher.kt`, `AnimeImageFetcher.kt`, Coil memory and disk caches | `ImageService::load` pipeline |
| `kuro-images` | `cover_paths.rs`, `invalidate.rs` | `MangaCoverCache.kt`, `AnimeCoverCache.kt`, `AnimeBackgroundCache.kt` | file names, custom covers, backgrounds, `prep_update_cover` decision |
| `kuro-images` | `chapter_cache.rs` | `ChapterCache.kt` | page list JSON and page image LRU |
| `kuro-images` | `save.rs`, `share.rs` | `ImageSaver.kt` | write to Pictures, staging for share |
| `kuro-images` | `error.rs` | Coil error types | `ImageError` |
| `kuro-core` | `fsname.rs`, `errors.rs` | filename sanitisation (`ImageUtil.kt`), `ExceptionFormatter.kt` | `build_valid_filename`, `take_bytes`, `AppError`, `user_message` |
| `kuro-domain` | `format.rs` | `ItemNumberFormatter.kt`, `TimeUtils.kt` | `chapter_number_text`, `episode_number_text`, `duration_text`, `relative_time_text` |
| `kuro-platform` | `lib.rs` (traits) | Android file, notification, share and open APIs | `Paths`, `Notifier`, `Opener`, `Sharer` |
| mockup `src/ui.rs`, `src/images/` | `CoverImage`, `CoverViewer` | `ItemCover`, `full_cover` and `full_images` dialogs | GTK widgets (`p1-rust-design.md` §5, §14) |

Threads: GTK main thread for widgets and textures only. tokio multi-thread runtime for network, file I/O and decode. Decode permits `Semaphore(3)`, fetch permits `Semaphore(8)` (`p1-rust-design.md` §13).

---

## 3. Reading guide

All files are in this folder. Start with `p1-topic-*` for the concepts, then the reference documents.

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | One row per scope file: declarations, Kotlin purpose, port decision, target module and task IDs. Coverage check command. |
| [p1-behavior.md](p1-behavior.md) | The behavior contract, sections 1-33: pipeline, ImageLoader configuration, fetch orchestration, network, cover-file writes, keys, decoding, sampling, file caches, invalidation, chapter cache, saving, names, save and share UX, cover editing, cover display, viewer, formatting, navigation helpers, threading, cross-area links and open questions O-1 to O-9. |
| [p1-data-and-config.md](p1-data-and-config.md) | Data shapes: domain inputs, page model, pipeline types, on-disk layout, DB columns, serialized page-list format, request options, preferences, endpoints, UI tokens, Kotlin-to-Rust type map. |
| [p1-rust-design.md](p1-rust-design.md) | Rust design: crate and module layout, cargo dependencies, path mapping, decoding, widgets, caches, keys, names and saving, ICC (D-3), platform traits, formatting, errors, threading, mockup integration, Android-to-Linux mapping, pitfalls, tests. Deviations are labelled D-1 to D-18. |
| [p1-ui-parity.md](p1-ui-parity.md) | Linux UI for every surface (S1-S21): cover tiles, viewer, edit flow, save and share, storage settings, download menu, numbers, responsive layout, keyboard and mouse map, dropped Android UI, states matrix, accessibility, mockup gaps, acceptance checklist. |
| [p1-tasks.md](p1-tasks.md) | 71 implementation tasks `T-34a-001` to `T-34a-071` in groups A to H, each with priority, effort, dependencies and sources. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E-IMG-*, race conditions, invalidation and key tests, failure texts, dropped capabilities with the user loss (section 5), golden tables G-*, fixtures F-*, HTTP, concurrency, chapter cache, save and share, formatting tests, coverage matrix, manual desktop test script, open items. |
| [p1-topic-cover-cache-and-invalidation.md](p1-topic-cover-cache-and-invalidation.md) | Topic: the lifecycle of one cover (state machine, transitions, the last-modified timestamp, custom-cover precedence, migration copy, invalidation function). |
| [p1-topic-decode-and-sampling.md](p1-topic-decode-and-sampling.md) | Topic: which decoder runs for which format, the format matrix, sampling worked examples, memory budget for decoded images, decode error mapping. |
| [p1-topic-chapter-cache-and-saving.md](p1-topic-chapter-cache-and-saving.md) | Topic: page cache lifecycle, size control, save and share pipelines, folder and name rules, notification identity. |

Recommended order for a new engineer: this overview, `p1-topic-cover-cache-and-invalidation.md`, `p1-topic-decode-and-sampling.md`, `p1-rust-design.md`, then `p1-tasks.md` while reading `p1-behavior.md` for the exact rule each task cites.

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the plan

| # | Decision | Where | Effect for the user |
|---|---|---|---|
| D-1 | No low-RAM mode. `low_ram: false` always. No RGB565. | `p1-rust-design.md` §2, `p1-edge-cases-and-tests.md` §5 | None on desktop hardware. |
| D-2 | Image directories are created on first use, not at start. | `p1-rust-design.md` §3 | No empty folders for users who never save. |
| D-3 | ICC display profile is kept as a setting but does nothing until an optional `icc` feature (lcms2) exists. | `p1-rust-design.md` §9 | Custom colour profiles are not applied. |
| D-4 | EXIF orientation is applied before sampling. | `p1-rust-design.md` §4.5 | Rotated JPEG covers display upright. |
| D-5 | `content://` URIs are not supported. | `p1-rust-design.md` §4 (step 5) | None: Linux never produces them. |
| D-6 | A 304 response is reported as `DataSource::Network`. | `p1-rust-design.md` §5 | Debug label only. |
| D-7 | Viewer zoom animation is a 500 ms `glib::timeout` ease. | `p1-rust-design.md` §5.5 | Same timing, GTK-native implementation. |
| D-8 | Decoded-texture memory budget is fixed (64 MiB), not a fraction of the heap. | `p1-rust-design.md` §6.1 | Predictable memory use. |
| D-9 | Page image file name is `md5(url)` with no `.0` suffix. | `p1-rust-design.md` §6.3 | Internal only. |
| D-10 | A corrupt page-list JSON is deleted and treated as a miss. | `p1-rust-design.md` §6.3 | Internal only. |
| D-11 | `clear` also removes `.tmp` leftovers. | `p1-rust-design.md` §6.3 | Internal only. |
| D-12 | Saves are plain file writes. No media scan, no MediaStore. | `p1-rust-design.md` §8.1 | Files appear in `Pictures`, not in a gallery index. |
| D-13 | Names with a leading dot lose it; reserved names get a `_` prefix. | `p1-rust-design.md` §8.2 | Rare name changes. |
| D-14 | Cover shares also clear the staging folder first. | `p1-rust-design.md` §8.3 | No accumulated share files. |
| D-15 | Chapter and episode numbers use ASCII digits always. | `p1-rust-design.md` §11 | Arabic locales see Latin digits. |
| D-16 | `FLAG_SECURE` is dropped. | `p1-ui-parity.md` §12 | Incognito and always-secure screenshot blocking does not exist. |
| D-17 | Cover writes go to `<name>.part` and then `rename`. | `p1-rust-design.md` §16 | No partial-read window. |
| D-18 | BMP is accepted (magic `BM`, extension `bmp`) although Kotlin's `ImageType` table omits it. | `p1-rust-design.md` §4 | Same visible result as Kotlin, where Coil draws BMP. |

### 4.2 Open questions that need the user's call

Each has a default already in the plan. The user should confirm or change it.

| # | Question | Plan default | Recommendation |
|---|---|---|---|
| Q-1 | Keep the screenshot-block setting that cannot work on Linux? (D-16) | Keep the setting, show "Not available on Linux". | Keep it, so the settings screen matches the Android one. Do not spend time on a workaround. |
| Q-2 | Ship a colour-managed display profile (ICC) at all? (D-3, O-8) | Drop for P0 and P1. Optional feature `icc` at P2 (T-34a-031). | Drop. A solo user is unlikely to set a custom profile, and GTK 4.16 does not colour-manage images. |
| Q-3 | Ship HEIF/HEIC? (O-4) | Off by default. `libheif-rs` behind feature `heif` (T-34a-029, P2). | Leave off. It needs a system `libheif`, which adds a packaging burden for a rare format. |
| Q-4 | Ship AVIF and JPEG XL as P0? (T-34a-028) | Yes, P0, pure-Rust crates `avif-decode` and `jxl-oxide`. | Keep P0 only if the user's sources serve AVIF or JXL covers. Otherwise demote to P1. Spike `avif-decode` on real files first. |
| Q-5 | Border cropping (native `crop` option, O-2) without the native source | Implement border detection in Rust with the same threshold idea and validate against fixtures (T-34a-030, P1). | Ship it. Test it on the golden fixtures and accept a visible difference from Android where the thresholds differ. |
| Q-6 | EXIF orientation for JPEG covers (O-1) | Apply it (T-34a-032). | Apply it. It is the correct display. |
| Q-7 | Chapter cache size | 100 MiB, not user-configurable in P0. | Keep 100 MiB and expose it in storage settings at P1 (area:47-settings-screens-general). Linux users often have more disk space. |
| Q-8 | Memory budget for decoded images | 64 MiB fixed (D-8). | Keep it. Raise it only if the reader stutters on large pages. |
| Q-9 | Disk image cache clamp (2 % of free, 10-250 MiB) | As Kotlin. | Keep. |
| Q-10 | Webtoon reader path without memory and disk cache (O-9) | Keep, as a reader option. | Keep. It is owned by area:44-reader-viewers. |
| Q-11 | Relative time wording (O-7) | English phrases `N minutes ago`, `N hours ago`, `N days ago` up to 7 days, then `Mon D` or `Mon D, YYYY`. | Keep. The app's locale support is area:51-i18n's decision. |
| Q-12 | Share on Linux | Open the file in the default viewer and copy its URI. | Keep. Under Flatpak, use the portal (`ashpd`), not `xdg-open` directly (`p1-rust-design.md` §16 pitfall 10). |
| Q-13 | Digit set for numbers (O-5) | ASCII (D-15). | Keep. |
| Q-14 | Cover edit concurrency (E-IMG-8) | One edit at a time per entry (a `tokio::sync::Mutex` keyed by entry id). Kotlin has no such lock. | Keep. It removes a race that Kotlin leaves open. |

### 4.3 Data-source uncertainty (not user calls, but they limit what can be verified)

- O-2 and O-3: the native `tachiyomi.decoder` source is not in the repository. Crop and HEIF brand behavior cannot be copied exactly. Verification uses fixtures (T-34a-070).
- O-1: EXIF handling on Android is not verified on a device. The Rust result is the reference (`p1-edge-cases-and-tests.md` §15).

---

## 5. Cross-area dependencies

Counts are from `TASKS.csv`: 71 tasks in this area, with the `depends` tokens below.

### 5.1 What this area needs from others

| Area | Needed for | Tasks that depend on it |
|---|---|---|
| area:01-foundation-app-bootstrap | `AppContext`, `Paths` root, startup wiring | T-34a-001, T-34a-042, T-34a-047, T-34a-068 |
| area:02-preferences-system | Preference keys (display profile, chapter-cache clear, storage settings) | T-34a-001, T-34a-039 |
| area:03-db-manga-schema-migrations | `cover_last_modified` and page-list storage columns | T-34a-013, T-34a-037 |
| area:04-db-anime-schema-migrations | `background_last_modified` and anime columns | T-34a-013, T-34a-037 |
| area:05-data-repos-entries-items | Repository writes of the timestamps | T-34a-013 |
| area:08-domain-library-category-history-updates | `prepUpdateCover` caller, `Manga`/`Anime` shapes, `hasCustomCover` | T-34a-011 |
| area:12-network-stack | `reqwest` client, headers, status handling | T-34a-016, T-34a-021, T-34a-059 |
| area:13-core-util-storage-archive-torrent | Storage helpers (`DiskUtil`-style `.nomedia`, size reporting) | T-34a-020 |
| area:14-source-local-and-metadata | Local folder covers (`file://` and path). No task declares this dependency; T-34a-022 handles the files without it. | none declared |
| area:17-source-app-layer | Source `getImage` and `getImageUrl` calls, source error types | T-34a-059 |
| area:23-downloads-manga | Downloader copy from the page-image cache | T-34a-041 |
| area:36-ui-shell-navigation-security-webview | GTK navigation, GIO menus, shell-level clear on launch | T-34a-060, T-34a-061, T-34a-062, T-34a-064 |
| area:35-data-misc-updater-notification-export | Notification channel and `Notifier` | T-34a-048 |
| area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | Cover edit and delete actions, cover presentation | T-34a-050 |
| area:40-ui-browse-manga, area:41-ui-browse-anime | Migration copy of custom covers | T-34a-051 |
| area:44-reader-viewers | Visible-on-screen check for the viewer | T-34a-063 |
| area:49-theme-and-design-system | Colours, radius and spacing tokens for `CoverImage` | T-34a-052 |
| area:51-i18n | Message keys for save, share and cover results | T-34a-049 |
| area:53-platform-manifest-intents-workers | Save destination permissions and paths | T-34a-044 |

### 5.2 What others need from this area

Reverse dependencies on area:34-images-covers-cache tasks (computed from `depends` tokens in `TASKS.csv`; a count is the number of tasks in that area that depend on 34):

| Area | Tasks that depend on 34 | Uses |
|---|---|---|
| area:39-ui-entry-details-anime | 9 | `CoverImage`, `CoverViewer`, cover edit, save and share |
| area:38-ui-entry-details-manga | 5 | same as 39 |
| area:41-ui-browse-anime | 5 | cover tiles |
| area:42-ui-updates-history-stats-storage-upcoming | 5 | cover tiles, storage settings |
| area:40-ui-browse-manga | 4 | cover tiles |
| area:47-settings-screens-general | 4 | chapter-cache clear and size, storage settings |
| area:52-util-app | 3 | formatting helpers |
| area:37-ui-library | 2 | library grid covers |
| area:43-reader-core | 2 | page cache read-through (T-34a-040), save and share |
| area:10-app-domain-layer | 2 | domain-facing cover paths |
| area:01-foundation-app-bootstrap, area:15-source-api-manga, area:21-backup-format-and-create, area:23-downloads-manga, area:27-tracking-core, area:35-data-misc-updater-notification-export, area:36-ui-shell-navigation-security-webview, area:45-player-core, area:46-player-controls-ui, area:50-android-widgets, area:51-i18n | 1 each | see the table in `p1-inventory.md` "Cross-references" |

Concrete outputs this area provides:

- `ImageService::load(ArtRef, LoadOptions) -> LoadedImage` (`p1-rust-design.md` §2, §5).
- `cover_paths` functions for covers, custom covers and backgrounds (`p1-rust-design.md` §8).
- `invalidate::prep_update_cover` decision (`p1-topic-cover-cache-and-invalidation.md` §8).
- `ChapterCache` read-through API for the page loader (T-34a-040) and downloads (T-34a-041).
- `save_image`, `share_image` and `copy_image` actions (T-34a-044 to T-34a-047).
- `kuro-domain::format` text functions (T-34a-056 to T-34a-058).
- `AppError::user_message` mapping (T-34a-059).
- `CoverImage` and `CoverViewer` widgets (T-34a-052, T-34a-053).

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, filtered to rows whose `file` starts with `34-images-covers-cache/`. 71 rows in total (`T-34a-001` to `T-34a-071`).

### 6.1 Counts

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 21 | 13 | 2 | 0 | 36 |
| P1 | 17 | 5 | 1 | 0 | 23 |
| P2 | 4 | 2 | 0 | 0 | 6 |
| P3 | 6 | 0 | 0 | 0 | 6 |
| **Total** | **48** | **20** | **3** | **0** | **71** |

Effort ceiling: no task is XL. The three L tasks are `T-34a-027` (raster decoder), `T-34a-038` (page-image cache with size limit) and `T-34a-053` (full-screen cover and background viewer). Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint p1-tasks.md` prints `ok`.

### 6.2 P0 critical path, in order

Each step names the tasks that can run in parallel in brackets.

1. `T-34a-001` Art-store directory layout (needs area:01-foundation-app-bootstrap and area:02-preferences-system).
2. `T-34a-002` MD5 cache key hashing.
3. [`T-34a-003` image sniffing] and [`T-34a-005` manga cover path API] and [`T-34a-016` request options and extras, needs area:12-network-stack].
4. `T-34a-006` anime cover path API; `T-34a-017` resource classification (needs T-34a-016); `T-34a-024` in-memory byte source.
5. `T-34a-008` custom cover and background write; `T-34a-009` cover deletion with counts; `T-34a-014` keyers for domain entries; `T-34a-015` keyers for projections.
6. `T-34a-020` non-library disk cache (needs T-34a-002 and area:13-core-util-storage-archive-torrent); `T-34a-021` HTTP cover request (needs area:12-network-stack); `T-34a-022` local file loader.
7. `T-34a-019` library cover file cache (needs T-34a-005 and T-34a-020).
8. `T-34a-018` cover fetch orchestration (needs T-34a-008, T-34a-017, T-34a-019, T-34a-021, T-34a-022).
9. `T-34a-023` concurrency, cancellation and transition timing.
10. `T-34a-025` decoder selection rule (needs T-34a-003 and T-34a-016); `T-34a-026` in-sample size and target size.
11. `T-34a-027` raster decoder (L); `T-34a-028` AVIF and JPEG XL (M).
12. `T-34a-036` texture creation for GTK; `T-34a-071` decoded-image memory cache.
13. `T-34a-037` page-list cache; `T-34a-038` page-image cache with size limit (L); `T-34a-040` read-through API for the reader.
14. `T-34a-052` `CoverImage` widget (needs area:49-theme-and-design-system).
15. `T-34a-055` image loader wiring at startup (needs T-34a-014, T-34a-018, T-34a-020, T-34a-023).
16. `T-34a-056` chapter and episode number text; `T-34a-059` exception-to-message mapping; `T-34a-068` platform glue.
17. `T-34a-069` golden and unit test suite (P0). `T-34a-070` decoder fixtures (P1) gate the decoder formats before they are called done.

Longest chain, by effort: `T-34a-001` → `T-34a-005` → `T-34a-019` → `T-34a-018` → `T-34a-055` (wiring of the cover path). Decode chain: `T-34a-003` → `T-34a-025` → `T-34a-026` → `T-34a-027` → `T-34a-028` → `T-34a-036` → `T-34a-052`.

### 6.3 P1, P2 and P3 at a glance

- P1 (23): `T-34a-004` (`.nomedia`), `T-34a-007` (anime background path), `T-34a-010` (background deletion), `T-34a-030` (border crop), `T-34a-032` (EXIF orientation), `T-34a-039` (chapter-cache clear), `T-34a-041` (download copy), `T-34a-042`-`T-34a-049` (filenames, save, share, notifications, messages), `T-34a-050` (local cover edit), `T-34a-053` (full viewer), `T-34a-054` (crossfade and reduced motion), `T-34a-057`-`T-34a-058` (duration and relative time), `T-34a-060`-`T-34a-061` (navigation and context menus), `T-34a-070` (decoder fixtures).
- P2 (6): `T-34a-029` (HEIF), `T-34a-031` (ICC), `T-34a-033` (animated GIF and WebP frames), `T-34a-051` (migration copy), `T-34a-062` (tablet breakpoint), `T-34a-063` (visible-on-screen check).
- P3 (6, all drops with a stated replacement or reason): `T-34a-034` (hardware bitmaps), `T-34a-035` (RGB565), `T-34a-064` (`FLAG_SECURE`), `T-34a-065` (install-unknown-apps), `T-34a-066` (fast-scroll workaround), `T-34a-067` (Compose and preference plumbing).

### 6.4 Risks

| Risk | Impact | Mitigation in the plan |
|---|---|---|
| `avif-decode` and `jxl-oxide` may lack features or be slow on large files | Some covers fail to show | Spike on fixtures in T-34a-028; decode permits keep them from blocking fetches (`p1-rust-design.md` §16 pitfall 7); fallback is `libavif` C binding (`p1-rust-design.md` §1.1). |
| Native decoder source is not available (O-2, O-3) | Crop and HEIF rules differ from Android | Validate on fixtures (T-34a-070); document differences. |
| EXIF handling not verified on device (O-1) | Rotated covers could differ from Android | Rust is the reference (`p1-edge-cases-and-tests.md` §15). |
| Large originals decode to big RGBA buffers (4000 x 6000 is 96 MB, `p1-topic-decode-and-sampling.md` §6) | Memory spikes in the viewer | Only the viewer asks for Original; decode permits; pixel caps (design §4.6). |
| GTK texture stride mistakes panic (`p1-rust-design.md` §16 pitfall 9) | Crash | Stride checked in `texture.rs` (T-34a-036). |
| Cover writes in place are not atomic in Kotlin | Partial reads | Write `.part` then rename (D-17). |
| Dependency on area:12-network-stack and area:49-theme-and-design-system for P0 widget and HTTP tasks | Delays P0 | Mock HTTP in tests (`p1-edge-cases-and-tests.md` §8). |
| `T-34a-037` (page-list cache) lists area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations as dependencies, but the page list is a JSON file (`p1-rust-design.md` §6.3) | Unnecessary blocking of a P0 task on DB work | Flagged here, not changed in this pass. Confirm with the area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations leads and remove if not needed. |

---

## 7. Definition of done for this area in Kurobayomi

Each item is an observable behavior on Linux (GTK 4.16, KDE Plasma, Wayland or X11).

1. A library grid of 24 entries (`p1-rust-design.md` §14 mockup call `cover_grid(24, 130, ...)`) shows real cover images within the first paint, from cache when present, or a grey placeholder with the error icon when the URL fails (S1, S2 in `p1-ui-parity.md`).
2. The cover tile aspect ratio is 2:3 for grids and the detail header, 16:9 where Kotlin uses `ItemCover.Thumb` and 1:1 where it uses `ItemCover.Square` (`p1-rust-design.md` §14.2).
3. A custom cover chosen from a local file replaces the displayed cover immediately, survives restart, and is removed by the Delete action. The original cover is restored when the custom cover is removed (`p1-topic-cover-cache-and-invalidation.md` §5).
4. Opening the full-size viewer shows the image at original size, supports zoom with Ctrl+scroll, and closes with Escape (`p1-ui-parity.md` §4, §11).
5. Save writes the image to `Pictures/<relative path>` with the sanitised name, shows the notification "Picture saved" with open and share actions, and overwrites an existing file of the same name (`p1-topic-chapter-cache-and-saving.md` §4, §7, §8).
6. Share copies the image into the staging folder, opens it in the default viewer, and copies the URI. Staging is cleared on the next share (D-14).
7. A reader page that was viewed once is shown offline, without network, when the chapter has been read and the page list is cached (`p1-topic-chapter-cache-and-saving.md` §1).
8. The chapter cache never exceeds 100 MiB after a write. Clear from storage settings deletes files in `<cache>` only and reports the removed file count. Covers in `<data>` survive a clear (`p1-rust-design.md` §16 pitfall 5).
9. A corrupt page-list JSON is deleted and reported as a miss, and the page is fetched again (D-10).
10. Chapter numbers use the pattern `#.###` with HALF_EVEN rounding and ASCII digits: `12` shows `12`, `12.5` shows `12.5`, `12.3333` shows `12.333` (`p1-ui-parity.md` §9, D-15). Episode numbers follow the same rule.
11. Stats durations follow the `%dd %dh %dm %ds` rules (`1d 1h`, `5m 30s`, `1h`). Last-update text shows `Just now`, `Never`, or the relative phrase of Q-11 (`5 minutes ago`).
12. Every error shown to the user is one of the texts in `p1-edge-cases-and-tests.md` §4. No raw Rust `Debug` output is shown.
13. A cover that changed on the source (a new thumbnail URL) is refreshed on the next library update, and the old file is replaced without a blank frame (`p1-topic-cover-cache-and-invalidation.md` §8).
14. The golden and unit test suite (`T-34a-069`) passes on CI with no network. The decoder fixtures (`T-34a-070`) round-trip every enabled format.
15. The manual desktop test script in `p1-edge-cases-and-tests.md` §14 passes on KDE Plasma under Wayland and under X11.
16. Settings and features that cannot work on Linux (screenshot block) show the note "Not available on Linux" and do not fail silently (D-16).
