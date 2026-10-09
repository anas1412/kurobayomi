# 52-util-app — implementation tasks (part 1 of 1)

Scope: `eu.kanade.core.*` and `eu.kanade.tachiyomi.util.*` (40 files). Every file maps to at least one task (see the task column in `p1-inventory.md`). Algorithms and constants: `p1-behavior.md`. Data and flags: `p1-data-and-config.md`. Rust design and signatures: `p1-rust-design.md`. UI: `p1-ui-parity.md`. Test vectors and IDs `TC-nn-x`: `p1-edge-cases-and-tests.md`. Deep dives: `p1-topic-airing-and-simkl.md`, `p1-topic-selection-and-sort.md`, `p1-topic-time-and-locale.md`.

Crates: `kuro-core` (no GTK), `kuro-platform` (gio, portals, D-Bus, PAM), `kuro-i18n`, `kurobayomi` (GTK binary).

### T-52a-001 — Preference bridge Pref<T> to GTK state (PreferenceMutableState)
- Priority: P0
- Effort: S
- Depends: T-52a-066, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/core/preference/PreferenceMutableState.kt
- Acceptance:
  - `Pref<T>::get()` seeds the widget value; a `set` from another window updates the bound widget within one main-loop iteration (test with two bound widgets).
  - A UI write goes through `set()` and the widget changes only after the watch echo (TC-01-a).
  - Dropping the widget removes its subscription; a later preference change does not touch the destroyed widget (TC-01-a).
  - `set` with an equal value does not emit (EC-02, TC-01-b).

### T-52a-002 — Tri-state checkbox mapping (CheckboxState.asToggleableState)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/core/preference/CheckboxState.kt
- Acceptance:
  - `None` -> inconsistent=false, active=false; `Include` -> inconsistent=false, active=true; `Exclude` -> inconsistent=true, active=false (TC-02-a).
  - The `tristate` item in `spec/settings.json` renders as `gtk::CheckButton` using this mapping.
  - Space toggles the check button (keyboard test).

### T-52a-003 — insert_separators and insert_separators_reversed (CollectionUtils)
- Priority: P0
- Effort: S
- Depends: T-52a-066
- Sources: app/src/main/java/eu/kanade/core/util/CollectionUtils.kt
- Acceptance:
  - Empty input returns empty output and makes zero generator calls (TC-03-a).
  - `[a, b]` produces `[Sep(_,a), a, Sep(a,b), b, Sep(b,_)]` with exactly three generator calls (TC-03-b).
  - Reversed variant calls the generator in reverse index order and returns the same output for pure generators (TC-03-d).

### T-52a-004 — Collection helpers addOrRemove, fastFilterNot, fastPartition, fastCountNot
- Priority: P2
- Effort: S
- Depends: T-52a-066
- Sources: app/src/main/java/eu/kanade/core/util/CollectionUtils.kt
- Acceptance:
  - `add_or_remove(set, v, false)` on an absent value is a no-op (TC-04-a).
  - Rust uses `Iterator::filter` and `partition`; no extra public functions are exported (checked by `cargo doc` item list).
  - Partition keeps input order in both halves (unit test).

### T-52a-005 — Source-readiness gate (ifMangaSourcesLoaded, ifAnimeSourcesLoaded)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/core/util/MangaSourceUtil.kt, app/src/main/java/eu/kanade/core/util/AnimeSourceUtil.kt
- Acceptance:
  - Browse and Extensions pages show a spinner until the manga and anime source managers report ready (watch receiver).
  - When the readiness value never arrives, the empty state appears after 30 s (TC-05-a, EC-10) using a paused tokio clock.

### T-52a-006 — RxJava to stream adapter (RxJavaExtensions) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/core/util/RxJavaExtensions.kt
- Acceptance:
  - No `rx`-style dependency in the Cargo tree (`cargo tree | grep -i rx` prints nothing).
  - Download managers (tasks in areas 23-25) use `tokio::sync::watch` / `mpsc`; no user-visible behaviour changes.
  - Loss: none (internal plumbing).

### T-52a-007 — Next-unread chapter selection (ChapterGetNextUnread)
- Priority: P0
- Effort: M
- Depends: T-52a-066, T-52a-003, area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt, app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt
- Acceptance:
  - Ascending view returns the first unread after filters and sort; descending returns the last unread (TC-07-c, fixture S1 -> chapter 2).
  - Empty list and all-read return None (TC-07-a, TC-07-b).
  - "Show read only" unread filter with nothing unread returns None, as upstream (TC-07-d).
  - Downloaded-only override ignores non-downloaded chapters (TC-07-e).

### T-52a-008 — Next-unseen episode selection (EpsiodeGetNextUnseen)
- Priority: P0
- Effort: S
- Depends: T-52a-066, T-52a-007, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt, app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt, domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt
- Acceptance:
  - Same rules as T-52a-007 using `seen` (TC-07 table applies to episodes).
  - Player "resume next" selects the same episode the library list shows as next (integration test with a fixture anime).

### T-52a-009 — Chapter dedupe by chapter number (ChapterRemoveDuplicates)
- Priority: P0
- Effort: S
- Depends: T-52a-066
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - One chapter per number; current id wins, then same scanlator, then first (TC-09-a, TC-09-b).
  - `null` scanlator matches `null` (TC-09-c); `1.0` and `1.00` are one group; `-0.0` and `0.0` are one group (TC-09-d).
  - Output order equals first-appearance order of the groups.

### T-52a-010 — Downloaded-only filter for chapters and episodes (ChapterFilterDownloaded, EpisodeFilterDownloaded)
- Priority: P0
- Effort: M
- Depends: T-52a-066, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt, app/src/main/java/eu/kanade/tachiyomi/util/episode/EpisodeFilterDownloaded.kt
- Acceptance:
  - Local source (`source == 0`) returns the input unchanged (TC-10-a).
  - Other sources keep only units found by the download cache lookup with name, scanlator, title and source id (test with a fake cache).
  - Reader and player call sites use the filter only when downloaded-only is on.

### T-52a-011 — Cover invalidation when the remote thumbnail changes (prepUpdateCover)
- Priority: P1
- Effort: M
- Depends: T-52a-066, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt
- Acceptance:
  - `None` or empty remote URL: no change (EC-23, EC-24).
  - Same URL with `refresh_same_url = false`: no change (EC-25).
  - Local source: `coverLastModified` set to now in ms, no file deleted (TC-11-d).
  - Custom cover exists: old thumbnail file deleted, timestamp not bumped (TC-11-e).
  - Otherwise: old thumbnail deleted and timestamp bumped (TC-11-d).

### T-52a-012 — Cover, background and thumbnail edit and removal (manga and anime)
- Priority: P1
- Effort: M
- Depends: T-52a-066, T-52a-011, area:34-images-covers-cache, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt
- Acceptance:
  - `removeCovers` returns self when no file was deleted (TC-12-a) and returns self for local sources (TC-12-b).
  - `editCover` on a favourite writes the custom cover and bumps the timestamp; on a non-favourite remote entry it is a silent no-op and the stream is not read (TC-12-c).
  - `editThumbnail` is a no-op for remote anime (TC-12-d).
  - Background variants use `backgroundLastModified` (ms).

### T-52a-013 — Airing schedule orchestration (loadAiringTime, caller persistence)
- Priority: P1
- Effort: M
- Depends: T-52a-066, T-52a-014, T-52a-015, T-52a-016, area:27-tracking-core, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - Completed anime without manual fetch makes no HTTP request and returns the stored pair (TC-13-a); with manual fetch it requests (TC-13-b).
  - First track among AniList, MAL, Simkl with a non-null track is used; none -> `(1, 0)` (TC-13-c).
  - Concurrent loads for one anime are serialised with `JobRegistry` tag `airing:<id>` (TC-13-d).
  - DECISION RECORDED: "no tracker" overwrites stored airing values with `(1, 0)` (upstream). The Rust port keeps this; a product change to keep old values is a separate item for the user (see topic file section 6).

### T-52a-014 — AniList GraphQL queries and MAL to AniList id lookup
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:28-tracker-anilist, area:29-tracker-myanimelist, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt
- Acceptance:
  - Requests are `POST https://graphql.anilist.co` with `Content-Type: application/json; charset=utf-8` and body `{"query": ...}` (fixture check).
  - Airing response: `(12, 1765000000)` for the ok fixture; `(1, 0)` for `nextAiringEpisode: null` (TC-14-a, TC-14-b).
  - MAL id unknown (`Media: null`) returns 0 and does NOT issue `Media(id:0)` (TC-14-c).
  - Transport error or HTTP 429 returns `(1, 0)` (TC-14-d).

### T-52a-015 — Simkl calendar lookup and parser
- Priority: P1
- Effort: M
- Depends: T-52a-066, T-52a-014, area:32-tracker-mangaupdates-simkl
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt
- Acceptance:
  - Calendars are fetched in order `anime`, `tv`, `movie_release` from `https://data.simkl.in/calendar/<name>.json`.
  - Entry selection uses `serde_json` and picks the earliest entry for the id with date after now (TC-15-a, TC-15-c).
  - A null `mal` never takes another entry's MAL id (TC-15-b, fixture S3).
  - Entries with no offset dates are skipped, not fatal (TC-15-d); bodies over 16 MiB return `(1, 0)` (TC-15-e).

### T-52a-016 — Airing viewer-flag packing (episode bits 8..23, time bits 24..55)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:04-db-anime-schema-migrations, area:05-data-repos-entries-items
- Sources: domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt
- Acceptance:
  - `decode(encode(12, 1765000000)) == (12, 1765000000)` (TC-16-a).
  - Episode 70000 stores and reads back as 4464 (TC-16-b); time 2^32 reads back as 0 (TC-16-c).
  - One `UPDATE` writes both fields (TC-16-d); other bits of `viewer_flags` are unchanged (test with all bits set).

### T-52a-017 — Crash log dump (CrashLogUtil.dumpLogs)
- Priority: P1
- Effort: M
- Depends: T-52a-066, T-52a-018, T-52a-019, area:35-data-misc-updater-notification-export, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt
- Acceptance:
  - Writes `kurobayomi_crash_logs.txt` under the cache dir with sections in order: debug info, problematic extensions (if any), exception (if any), log tail (TC-17-a).
  - Two dumps at once are serialised (TC-17-d); the file is complete after the save completes (TC-17-c).
  - On failure the toast text `Failed to get logs` is shown (LITERAL).
  - Save uses a native save dialog (ui-parity 4), not a share sheet.

### T-52a-018 — Debug information block (CrashLogUtil.getDebugInfo)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt
- Acceptance:
  - Labels appear in the order of behaviour 8.2: `App ID:`, `App version:`, `Android version:` (Linux: `OS:`), ..., `FFmpeg version:` (TC-17-b).
  - mpv, libplacebo and FFmpeg versions are read from libmpv at runtime (no hard-coded `6764488`, `v7.349.0`, `n7.1`).
  - Output has no leading or trailing blank line.

### T-52a-019 — Problematic extensions report (getMangaExtensionsInfo, getAnimeExtensionsInfo)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt
- Acceptance:
  - Lists installed extensions that have a newer available version or are obsolete, sorted by name (TC-19-a, TC-19-d).
  - Missing available version prints `?` (TC-19-b); empty list omits the whole section (TC-19-c).
  - Label is `Obsolete:` for both manga and anime (decision; upstream differs, recorded in behaviour 8.3).

### T-52a-020 — PKCE code verifier (PkceUtil)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:29-tracker-myanimelist
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt
- Acceptance:
  - Verifier length is 67 characters from `[A-Za-z0-9_-]` (TC-20-a).
  - Two calls differ (TC-20-b); randomness from `OsRng`.
  - The verifier is never written to logs (test with a tracing capture).

### T-52a-021 — Human-readable byte sizes (Long.toSize)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:42-ui-updates-history-stats-storage-upcoming
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt
- Acceptance:
  - Decimal units; `999 B`, `1.00 KB`, `1.50 MB`, `2.00 GB`, `-5 B` (TC-21-a, TC-21-c).
  - `999999` prints `1000.00 KB` (quirk kept, TC-21-b).
  - Output uses `.` as decimal separator in every locale.

### T-52a-022 — Recursive directory size (UniFile.size)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt
- Acceptance:
  - Sums regular files, recursing into directories; a missing path returns 0 (EC-55).
  - Symlink loops terminate (TC-22-b): symlinks are not followed.
  - Runs on `spawn_blocking`; the storage screen shows `…` while computing (ui-parity 15).

### T-52a-023 — Stream save with cleanup (BufferedSource.saveTo)
- Priority: P0
- Effort: S
- Depends: T-52a-066, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/storage/OkioExtensions.kt
- Acceptance:
  - Missing parents are created (TC-23-a); destination is replaced atomically by rename (TC-23-c).
  - A failure mid-write removes `<dest>.part` and leaves no destination (TC-23-b).
  - Used by cover, backup and download writers with no change in their output bytes (golden-file test).

### T-52a-024 — Cache-directory file creation (createFileInCacheDir)
- Priority: P0
- Effort: S
- Depends: T-52a-066
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Existing file is deleted, then an empty file is created under `$XDG_CACHE_HOME/kurobayomi/` (TC-24-a).
  - An unwritable cache dir returns an error that the caller turns into `Failed to get logs` (TC-24-b).

### T-52a-025 — Relative date text for dates (LocalDate.toRelativeString)
- Priority: P1
- Effort: S
- Depends: T-52a-066, T-52a-028, area:42-ui-updates-history-stats-storage-upcoming
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt
- Acceptance:
  - Boundary table matches TC-25-a (-8 absolute, -7 `In 7 days`, -1 Tomorrow, 0 Today, 1 Yesterday, 6 `6 days ago`, 7 absolute).
  - Renders through `kuro-i18n` with plural categories (`one`/`other`).

### T-52a-026 — Relative time text for release times (LocalDateTime.toRelativeString)
- Priority: P1
- Effort: M
- Depends: T-52a-066, T-52a-025, area:42-ui-updates-history-stats-storage-upcoming
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt, app/src/main/java/eu/kanade/presentation/components/DateText.kt
- Acceptance:
  - Table TC-26-a passes (30 s ahead and behind -> `Now`; 90 s ahead -> `In 1 minute`; 59 min 59 s behind -> `59 minutes ago`; 1 h behind -> `1 hour ago`; 2 days 1 h behind -> `2 days ago`).
  - DST day case TC-26-c passes (naive arithmetic).
  - Tooltip shows the absolute date-time (ui-parity 13).

### T-52a-027 — Absolute timestamps, zone conversion and local date helpers
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt, app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt
- Acceptance:
  - `short_datetime` produces `"{date} {time}"` with ICU short length (TC-27-a); `short_time` for history rows (TC-27-b).
  - `convert_epoch_millis_zone(0, UTC, Asia/Tokyo) == -32400000` (TC-27-d).
  - Locale-specific output for `en-US` and `de-DE` matches fixtures (TC-27-c).

### T-52a-028 — Language display names, comparator and source group names (LocaleHelper)
- Priority: P0
- Effort: M
- Depends: T-52a-066, area:02-preferences-system, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - `all` sorts first; `compare(all, all)` is `Equal` (TC-28-a).
  - `zh-CN` and `zh-Hans` produce the same name (TC-28-b); `null` yields an empty string (TC-28-d).
  - Sentinels `last_used`, `pinned`, `other`, `all` render `Last used`, `Pinned`, `Other`, `Multi` (TC-28-c).
  - Names are cached per tag; a runtime locale change needs restart (TC-28-e).

### T-52a-029 — Default enabled languages and system language
- Priority: P1
- Effort: S
- Depends: T-52a-066, T-52a-028, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - System `de` yields `{all, de, en}`; empty system language yields `{all, en}` (TC-29-a, TC-29-b).
  - System language read from `glib::language_names()` with `.UTF-8` and `@modifier` stripped.

### T-52a-030 — Closeable array use (CloseableExtensions) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/CloseableExtensions.kt
- Acceptance:
  - Every handle is closed by `Drop`, including when a close fails (close errors do not stop other closes).
  - Loss: none (RAII replaces the helper).

### T-52a-031 — Tap-zone rectangle inversion (RectF.invert)
- Priority: P0
- Effort: S
- Depends: T-52a-066, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Vectors of TC-31: None returns the rect; Horizontal `(0.6, 0.2, 0.9, 0.6)`; Vertical `(0.1, 0.4, 0.4, 0.8)`; Both `(0.6, 0.4, 0.9, 0.8)` for input `(0.1, 0.2, 0.4, 0.6)`.
  - Property test: Both applied twice is identity (TC-31-b).
  - Reader tap zones use the inverted rect for all four modes (area 43 test).

### T-52a-032 — Animation duration scale (animatorDurationScale)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - GTK animations disabled gives scale 0: crossfade 0 ms, page animation minimum 1 ms (TC-32-a).
  - Missing KDE factor gives 1.0 (TC-32-b); factor above 10 clamps to 10 (TC-32-c).
  - Crossfade is `(300 * scale) as i32` ms.

### T-52a-033 — App lock authentication through PAM (AuthenticatorUtil)
- Priority: P2
- Effort: M
- Depends: T-52a-066, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt
- Acceptance:
  - Missing PAM service file makes `is_authentication_supported()` false and the lock setting cannot be enabled (TC-18-a).
  - Wrong password returns the PAM text and keeps the app locked (TC-18-b); empty password is rejected before PAM (TC-18-c).
  - A second lock prompt does not open while one is open (`isAuthenticating`, TC-18-d).
  - Decision recorded: PAM uses the user's own password; fingerprint is optional. Tradeoff stated in behaviour 24.2.

### T-52a-034 — Build-variant flags (BuildConfig helpers)
- Priority: P2
- Effort: S
- Depends: T-52a-066, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt
- Acceptance:
  - `UPDATER_ENABLED` is `false` in every build (TC-34-a).
  - `IS_DEBUG_BUILD` equals `cfg!(debug_assertions)`; `IS_RELEASE_BUILD` is its negation.

### T-52a-035 — Extension class loader (ChildFirstPathClassLoader) — dropped
- Priority: P3
- Effort: S
- Depends: area:20-extension-strategy-redesign
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ChildFirstPathClassLoader.kt
- Acceptance:
  - No DEX or class loader code in the Rust tree (`rg -i "dex|classloader"` empty).
  - Extension loading design lives in area 20; its tests cover the chosen mechanism.
  - Loss: none for users (Android-only mechanism).

### T-52a-036 — Clipboard copy with feedback (copyToClipboard)
- Priority: P1
- Effort: S
- Depends: T-52a-066, T-52a-024, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Blank content: no copy and no toast (TC-36-a).
  - Success: clipboard holds the exact content; toast `Copied to clipboard:` plus truncated content, always shown on Linux.
  - Failure (no display): toast `Failed to copy to clipboard` (TC-36-b).

### T-52a-037 — Runtime permission check (hasPermission) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - No runtime permission code path; file and notification access use portals.
  - Loss: none (Linux has no runtime permission prompts of this kind).

### T-52a-038 — Keep screen awake during playback (powerManager)
- Priority: P2
- Effort: S
- Depends: T-52a-066, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Playing video calls `gtk::Application::inhibit` with `IDLE` and the reason `Playing video`; pausing or stopping uninhibits (test with a fake application).
  - The inhibit cookie is released on player close.

### T-52a-039 — Open URL in default browser (openInBrowser)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - `gtk::UriLauncher` opens the URL through the portal; failure shows the error text as a toast (TC-37-a).
  - No `forceDefaultBrowser` option is exposed (drop noted in ui-parity 3).

### T-52a-040 — Default browser resolver (defaultBrowserPackageName) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt
- Acceptance:
  - No Android resolver code; `xdg-settings` default is used by the portal.
  - Loss: none on KDE; the Android verified-link workaround is not needed.

### T-52a-041 — Reader background choice (createReaderThemeContext)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:43-reader-core, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt
- Acceptance:
  - Table TC-54: theme 0 light, 1 dark, 2 dark, 3 follows theme mode (system uses colour scheme), unknown values light (TC-54-a, TC-54-b).
  - Reader applies CSS class `reader-dark` or `reader-light`; switching repaints immediately.

### T-52a-042 — Content size for file URIs (getUriSize)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - `file://` URIs return the byte length; other schemes return None.
  - Negative or unknown sizes are None (the Kotlin `takeIf { it >= 0 }`).

### T-52a-043 — Android package and installer checks (isPackageInstalled, MIUI, Shizuku, install permission) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - No package-manager or installer-permission code in the Rust tree.
  - Loss: none (Linux extensions do not use Android installers).

### T-52a-044 — Tablet and compact layout breakpoints (isTabletUi, prepareTabletUiContext)
- Priority: P1
- Effort: M
- Depends: T-52a-066, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt, app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - Table TC-53: AUTO 719 px -> Compact, 720 px -> Wide; LANDSCAPE uses `width > height`; ALWAYS Wide; NEVER Compact.
  - Resize across the threshold updates the layout within 100 ms (UI test).
  - Minimum window size 360 x 480 is set.

### T-52a-045 — Night-mode detection from the desktop colour scheme (isNightMode)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt
- Acceptance:
  - `prefer-dark` (value 1) gives dark; `no-preference` (0) and `prefer-light` (2) give light (unit test on the mapping).
  - Missing portal falls back to `gtk-application-prefer-dark-theme`.

### T-52a-046 — Display cutout and navigation-bar scrim checks — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt
- Acceptance:
  - No cutout or scrim logic in the Rust tree.
  - Loss: none on desktop.

### T-52a-047 — Drawable to texture (getBitmapOrNull)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/DrawableExtensions.kt
- Acceptance:
  - Decoded covers and pages are `gdk::Texture`; placeholders return None.
  - Notification large icon is a PNG `gio::BytesIcon`, at most 256 px.

### T-52a-048 — Share and save flows (toShareIntent)
- Priority: P1
- Effort: M
- Depends: T-52a-066, T-52a-023, area:21-backup-format-and-create, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt
- Acceptance:
  - "Save" opens a portal save dialog defaulting to `~/Downloads` and writes the file on confirm (ui-parity 4).
  - http(s) "share" copies the link to the clipboard (T-52a-036 toast).
  - Dialog cancel does nothing.
  - Loss recorded: no third-party share targets on desktop.

### T-52a-049 — Intent extras getters (getParcelableExtraCompat, getSerializableExtraCompat) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt
- Acceptance:
  - No Android Intent code in the Rust tree.
  - Loss: none.

### T-52a-050 — System resource booleans (InternalResourceHelper) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/InternalResourceHelper.kt
- Acceptance:
  - No `Resources.getSystem` access in the Rust tree.
  - Loss: none (only used for the nav-bar scrim, dropped in T-52a-046).

### T-52a-051 — Online and Wi-Fi checks (isOnline, isConnectedToWifi)
- Priority: P1
- Effort: M
- Depends: T-52a-066, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt
- Acceptance:
  - `is_online()` is true only for `gio` connectivity `Full` (EC-81 with `Portal` offline).
  - `is_connected_to_wifi()` reads NetworkManager `PrimaryConnectionType == 802-11-wireless`; missing NetworkManager returns false (TC-40-b).

### T-52a-052 — Network state snapshot and change stream (NetworkStateTracker)
- Priority: P1
- Effort: M
- Depends: T-52a-066, T-52a-051, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt
- Acceptance:
  - `watch_network` emits the current state before any change (EC-84, TC-40-d).
  - Five flaps within 200 ms cause at most two emissions after a 500 ms debounce (TC-40-c).
  - `NetworkState::is_online()` equals `is_connected && is_validated`.

### T-52a-053 — Notifications core (post, cancel, batch) via gio
- Priority: P0
- Effort: M
- Depends: T-52a-066, area:26-library-update-jobs, area:35-data-misc-updater-notification-export, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt
- Acceptance:
  - Post and cancel with the same integer id update and withdraw the same notification (manual test on KDE).
  - Withdrawing an id never posted is not an error (TC-41-b); empty batch makes no call (TC-41-c).
  - A refused post is logged at `warn` and does not panic (TC-41-a).
  - Default action focuses the window and opens the relevant page.

### T-52a-054 — Notification channels mapped to priorities
- Priority: P1
- Effort: S
- Depends: T-52a-066, T-52a-053
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt
- Acceptance:
  - Importance mapping: MIN and LOW -> Low; DEFAULT -> Normal; HIGH and MAX -> High (behaviour 15.2).
  - Channel groups are not created (dropped); per-channel ids are stable enum strings.
  - Accent colour is not applied (dropped; noted in settings help).

### T-52a-055 — TV box detection (isTvBox) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/TvUtils.kt, app/src/main/java/eu/kanade/presentation/more/onboarding/StorageStep.kt
- Acceptance:
  - No TV detection code in the Rust tree; onboarding storage step uses the native folder picker.
  - Loss: none (no Android TV on Linux).

### T-52a-056 — Background job registry (isRunning and start)
- Priority: P0
- Effort: M
- Depends: T-52a-066, area:22-backup-restore, area:23-downloads-manga, area:24-downloads-anime, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/WorkManagerExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - `is_running(tag)` is true while the task runs and false after finish or panic (TC-56-b).
  - A second `start` with the same tag returns false and does not restart the job (TC-56-a).
  - `cancel` on a finished job is a no-op (TC-56-c); `is_running` never blocks the UI thread.

### T-52a-057 — Foreground promotion helper (setForegroundSafely) — dropped
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/WorkManagerExtensions.kt
- Acceptance:
  - No foreground-service code; the 500 ms delay is not reproduced.
  - Busy state is shown through `JobRegistry::is_running` in the UI.
  - Loss: none.

### T-52a-058 — EditText preference listener accessor — dropped
- Priority: P3
- Effort: S
- Depends: area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/EditTextPreferenceExtensions.kt
- Acceptance:
  - No AndroidX preference types in the Rust tree; text settings are GTK entry rows built from the spec.
  - Loss: none.

### T-52a-059 — Popup menu as PopoverMenu (View.popupMenu)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt
- Acceptance:
  - Menu opens from a `gtk::MenuButton`, from `F10` and from the Menu key; items activate actions and the popover closes (ui-parity 20).
  - Right-click on a row opens the same menu.
  - Mockup `ui::menu_button` (src/ui.rs:202) is replaced by `ui::popover::from_menu`.

### T-52a-060 — Viewport visibility check (isVisibleOnScreen)
- Priority: P1
- Effort: S
- Depends: T-52a-066, area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Edge-touching rectangles are not visible; overlapping ones are (TC-13-visibility-a).
  - Reader pauses work for pages that are not visible (area 44 test).

### T-52a-061 — Compose host helpers (setComposeContent) — dropped
- Priority: P3
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt
- Acceptance:
  - No Compose or theme wrapper in the Rust tree; GTK widgets use the user's GTK theme.
  - Loss: none.

### T-52a-062 — Screenshot protection flag (setSecureScreen) — dropped with note
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt
- Acceptance:
  - The `secure_screen_v2` setting is kept and stored; the security page shows the note `Screenshot blocking is not available on Linux`.
  - The app does not claim to block captures.
  - Loss recorded: screenshots of the app are not blocked.

### T-52a-063 — Dynamic colour and app-theme choice (isDynamicColorAvailable, pref_app_theme) — dropped
- Priority: P3
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - No Material You or dynamic-colour code; the app uses the user's GTK theme.
  - The `pref_app_theme` key is not written by the Rust app.
  - Loss: Android wallpaper-derived colours only.

### T-52a-064 — Unit tests for every ported pure function
- Priority: P0
- Effort: M
- Depends: T-52a-003, T-52a-007, T-52a-008, T-52a-009, T-52a-010, T-52a-011, T-52a-016, T-52a-021, T-52a-023, T-52a-025, T-52a-026, T-52a-028, T-52a-031, T-52a-020, T-52a-019
- Sources: app/src/main/java/eu/kanade/core/util/CollectionUtils.kt, app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt
- Acceptance:
  - `cargo test -p kuro-core` passes every TC listed in `p1-edge-cases-and-tests.md` section 4 for the `kuro-core` rows.
  - `cargo test -p kuro-platform` passes the platform rows with the mocks from `kuro-testkit`.
  - Fixtures under `kuro-core/tests/fixtures/` exist and are used by the AniList and Simkl tests.

### T-52a-065 — Preference keys and defaults used by util code (registry)
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Every key in `p1-data-and-config.md` section 4 is registered with the listed default (test that reads defaults).
  - App-state keys use the `__APP_STATE_` prefix in the file store (behaviour of `appStateKey`).
  - `pref_reader_theme_key` defaults to `1`; `secure_screen_v2` defaults to `INCOGNITO`; `lock_app_after` defaults to `0`.

### T-52a-066 — kuro-platform skeleton and Android-to-Linux API map
- Priority: P0
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt
- Acceptance:
  - Crates `kuro-core`, `kuro-platform`, `kuro-i18n` exist in the workspace; `kuro-core` has no `gtk`, `gio` or `glib` in `cargo tree -p kuro-core` (command prints nothing).
  - Every row of the Android-to-Linux table in `p1-rust-design.md` section 6 has a module or a dropped-task reference.
  - `kurobayomi` builds with the new workspace members.
