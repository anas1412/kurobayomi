# 50-android-widgets — Part 1 tasks

Task ids are `T-50a-` followed by three digits. Scope: the Android Glance widgets in `presentation-widget/` (36 files). Port decisions are in `p1-inventory.md`; the feed algorithm is in `p1-topic-widget-feed-and-grid.md`; the surfaces are in `p1-topic-desktop-surfaces.md`.

Groups:

- 001-004: shared constants, grid geometry, window.
- 005-007: unread feed query and dedupe.
- 008-010: app-lock rule (P1: must hold before any external surface ships).
- 011-014: states and cover rendering.
- 015-017: CLI contract, read-only access, errors.
- 018-025: Plasma applets, click routing, colours, size, accessibility, `.desktop` action, packaging.
- 026-033: Android surfaces and resources dropped (P3, each with the user loss stated in the task).
- 034-035: logging replacement and tests.

## Foundations

### T-50a-001 — Shared widget constants (cover, cell pitch, radii, caps, window, row limit)
- Priority: P2
- Effort: S
- Depends: none
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaCover.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/anime/UpdatesAnimeCover.kt, presentation-widget/src/main/res/values/dimens.xml, presentation-widget/src/main/res/values-v31/dimens.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt
- Acceptance:
  - A single Rust module (`src/feed/cover.rs`, `src/feed/grid.rs`) defines `COVER_W_DP = 58.0`, `COVER_H_DP = 87.0`, `CELL_W_DP = 64.0`, `CELL_H_DP = 95.0`, `INNER_RADIUS_DP = 12.0`, `CARD_RADIUS_DP = 16.0`, `MAX_ROWS = 10`, `MAX_COLS = 10`, window `3` months, DB limit `500`.
  - A unit test asserts each value equals the Android value in `p1-behavior.md` section 12.
  - No other file in `src/` repeats these numbers as literals.

### T-50a-002 — Grid geometry: rows and columns from a size (port of calculateRowAndColumnCount)
- Priority: P2
- Effort: S
- Depends: T-50a-001
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt
- Acceptance:
  - `grid::rows_cols(width, height, pad_top, pad_bottom)` returns `rows = clamp(trunc((height - pad_top - pad_bottom) / 95), 1, 10)` and `cols = clamp(trunc(width / 64), 1, 10)`.
  - Table test passes for: (64,95,0,0)->(1,1); (128,190,0,0)->(2,2); (256,190,0,0)->(2,4); (256,190,24,24)->(1,4); (640,950,0,0)->(10,10); (2000,2000,0,0)->(10,10); (10,10,0,0)->(1,1); (256,100,10,0)->(1,4).
  - Negative height input returns rows 1 (no panic).

### T-50a-003 — Per-request grid size instead of the largest-area instance
- Priority: P2
- Effort: S
- Depends: T-50a-002
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt
- Acceptance:
  - The CLI takes `--rows` and `--cols` from the caller and never derives them from other instances (Android `maxBy` at `BaseMangaUpdatesGridGlanceWidget.kt:79-82` is not ported).
  - Golden test: `--rows 3 --cols 1` over a fixture with 5 distinct entries returns exactly 3 items in feed order.
  - Missing `--rows` or `--cols` uses `2` and `4` (documented default), and values outside 1..10 are a usage error (exit 2).

### T-50a-004 — Recent window: now minus three calendar months, strict bound
- Priority: P2
- Effort: S
- Depends: T-50a-001
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/BaseAnimeUpdatesGridGlanceWidget.kt
- Acceptance:
  - `window::window_start_ms(now)` uses `checked_sub_months(Months::new(3))` in the local zone and returns epoch milliseconds.
  - Tests: 2026-10-08 12:00 -> 2026-07-08 12:00; 2026-05-31 00:00 -> 2026-02-28 00:00; 2028-05-31 00:00 -> 2028-02-29 00:00.
  - A row with `dateUpload == window_start_ms` is excluded (strict comparison).
  - The window is recomputed on every CLI call (no value cached between calls).

## Feed query and dedupe

### T-50a-005 — Manga unread-updates query (updatesView semantics)
- Priority: P2
- Effort: M
- Depends: area:05-data-repos-entries-items, area:03-db-manga-schema-migrations, T-50a-004
- Sources: data/src/main/sqldelight/view/updatesView.sq, data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt
- Acceptance:
  - `query::unread_updates(conn, Kind::Manga, after_ms, 500)` returns only favourites, `read = 0`, `dateUpload > after_ms`, and `date_fetch > date_added`.
  - The SQL has an explicit `ORDER BY dateFetch DESC, chapterId DESC` (Android relies on the view order; E-4).
  - `LIMIT 500` is applied before dedupe (E-5), verified by a 600-row fixture test.
  - The fixture test for the filters passes on `feed-basic.sql`.

### T-50a-006 — Anime unread-updates query (animeupdatesView semantics)
- Priority: P2
- Effort: S
- Depends: area:04-db-anime-schema-migrations, T-50a-005
- Sources: data/src/main/sqldelightanime/view/animeupdatesView.sq, data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt, domain/src/main/java/tachiyomi/domain/updates/anime/interactor/GetAnimeUpdates.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/BaseAnimeUpdatesGridGlanceWidget.kt
- Acceptance:
  - `Kind::Anime` uses `seen = 0`, `episodeId AS update_id`, `ORDER BY dateFetch DESC, episodeId DESC`.
  - Fixture test: an anime with `seen = 1` episodes is absent; with unseen episodes present, the feed item's `unread_update_id` is the newest unseen episode id.

### T-50a-007 — Dedupe by entry and cap (first occurrence wins)
- Priority: P2
- Effort: S
- Depends: T-50a-005
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/BaseAnimeUpdatesGridGlanceWidget.kt
- Acceptance:
  - `dedupe_and_cap` keeps the first row per `entry_id` in input order and stops at `cap`.
  - Test with input [(1,a),(2,b),(1,c)] returns [(1,a),(2,b)]; with 20 distinct entries and cap 8 returns the first 8.
  - A row whose `thumbnail_url` is null is kept (E-14 behaviour).

## App-lock rule (P1)

### T-50a-008 — Read the lock flag exactly as Android does (use_biometric_lock, fail closed)
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system, area:36-ui-shell-navigation-security-webview
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt
- Acceptance:
  - `lock::is_locked` reads the Boolean `use_biometric_lock` with default `false`, through the area:02-preferences-system `PrefsReader` trait.
  - A read error returns `FeedError::Prefs`, and the CLI exits 4 with no items printed (fail closed).
  - Tests: flag true -> locked; flag false -> open; key missing -> open; `lock_app_after` value changes nothing (E-8).

### T-50a-009 — Locked payload: no query and no titles or covers leave the process
- Priority: P1
- Effort: S
- Depends: T-50a-008
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/LockedMangaWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/anime/LockedAnimeWidget.kt
- Acceptance:
  - `build_envelope` checks the lock before any query; when locked it returns `locked: true, items: []` and the query function is never called (test: fixture DB without the view still gives exit 0).
  - No `title`, `cover_path` or `unread_update_id` key appears in JSON when locked (golden test).
  - The applet clears its item model when `locked` becomes true (QML test: `items.length == 0` after a locked payload).

### T-50a-010 — Locked text uses the same message key on every surface
- Priority: P1
- Effort: S
- Depends: T-50a-009
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/LockedMangaWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/anime/LockedAnimeWidget.kt
- Acceptance:
  - The applet's locked text is "Widget not available when app lock is enabled" (the Android key `appwidget_unavailable_locked`, `i18n/src/commonMain/moko-resources/base/strings.xml:990`).
  - The text is set through `i18nd("kurobayomi", ...)` with that English string as the source.
  - Clicking the locked placeholder runs `kurobayomi` with no arguments (raises the window; the normal app lock then applies, area:36-ui-shell-navigation-security-webview).

## States and cover rendering

### T-50a-011 — Empty state text for both kinds
- Priority: P2
- Effort: S
- Depends: T-50a-007
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/anime/UpdatesAnimeWidget.kt
- Acceptance:
  - When `items` is empty and `locked` is false, both applets show "No recent updates" (`information_no_recent`).
  - The anime applet does not use the anime-only text "Nothing watched recently" (E-24); a test asserts the string.

### T-50a-012 — Loading and error states with a timeout
- Priority: P2
- Effort: S
- Depends: T-50a-018
- Sources: presentation-widget/src/main/res/layout/appwidget_loading.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt
- Acceptance:
  - Before the first result the applet shows a busy indicator; it never shows it for more than 6 s (CLI killed after 6 s, E-21).
  - A non-zero exit or killed process shows "Updates unavailable" with the stderr line as a tooltip; a click retries.
  - Test with a fake CLI that sleeps 10 s shows the error state within 6.5 s.

### T-50a-013 — Thumbnail renderer and cache-only cover lookup
- Priority: P2
- Effort: M
- Depends: area:34-images-covers-cache, T-50a-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt, app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaCover.kt
- Acceptance:
  - The GTK app renders `<cover-cache>/<key>.png` at 58x87 x `device_scale` (default 2 -> 116x174 px), centre-cropped, with a rounded alpha mask of radius 12 x `device_scale`, written to a temp file then renamed.
  - The CLI never downloads, decodes, or writes; with no network the CLI returns `cover_path` null for missing covers and exit 0 (E-7 test under strace).
  - The CLI never writes to the library cover cache (the Android side effect at `MangaCoverFetcher.kt:105-145` is not reproduced).
  - Test: scale 1 gives 58x87 px; scale 2 gives 116x174 px; a corner pixel has alpha 0; the centre has alpha 255.
  - A `file://` local cover produces a thumbnail; a `content://` URL produces `cover_path` null (E-27).

### T-50a-014 — Placeholder tile matching the Android error cover
- Priority: P2
- Effort: S
- Depends: T-50a-013
- Sources: presentation-widget/src/main/res/drawable/appwidget_cover_error.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaCover.kt
- Acceptance:
  - A cell with `cover_path` null shows a rounded rectangle filled `#1F888888` (radius 12 px) with a centred icon of about 10 x 10 px.
  - The placeholder is shown for: null path, load failure of the image (QML `Image.Error`), and a missing file.
  - Screenshot comparison against the Android placeholder spec (p1-ui-parity 1.4) is a manual check.

## CLI contract

### T-50a-015 — `kurobayomi --updates-json` argument parsing and schema-1 JSON envelope
- Priority: P2
- Effort: M
- Depends: T-50a-003, T-50a-004, T-50a-005, T-50a-006, T-50a-007, T-50a-008, T-50a-013
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/BaseAnimeUpdatesGridGlanceWidget.kt
- Acceptance:
  - `src/cli.rs::run` accepts `--updates-json --kind <manga|anime> --rows <1..10> --cols <1..10> [--db] [--cover-cache] [--device-scale 1..4] [--now-ms]`.
  - `src/main.rs` returns before any GTK call when `--updates-json` is present; `env -u DISPLAY -u WAYLAND_DISPLAY kurobayomi --updates-json ...` exits 0 with valid JSON.
  - Golden files for normal, locked, empty envelopes match byte for byte (`schema: 1`, field order as in `p1-data-and-config.md` section 11.1).
  - Titles with quotes, newlines, and RTL override characters round-trip through `serde_json` and `JSON.parse`.

### T-50a-016 — Read-only database access from the CLI (WAL, busy timeout, never create)
- Priority: P2
- Effort: S
- Depends: area:03-db-manga-schema-migrations, T-50a-015
- Sources: data/src/main/sqldelight/view/updatesView.sq, data/src/main/sqldelightanime/view/animeupdatesView.sq
- Acceptance:
  - The CLI opens with `SQLITE_OPEN_READ_ONLY` (no `SQLITE_OPEN_CREATE`) and `busy_timeout` 250 ms.
  - A missing database gives exit 3 with code `db_missing` and does not create the file (test: path absent before and after).
  - A test with a writer holding a transaction for 1 s gives exit 3 (busy) or a result within 300 ms, never a torn row (E-34).

### T-50a-017 — Error envelope, exit codes, and stderr diagnostics
- Priority: P2
- Effort: S
- Depends: T-50a-015
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt
- Acceptance:
  - Exit codes: 0 ok (including locked and empty), 2 usage, 3 database, 4 preferences.
  - On failure stdout is `{"schema":1,"error":"<text>","code":"<code>"}` and stderr has one human line.
  - Golden tests for each exit code.

## Plasma applets

### T-50a-018 — Two Plasma 6 applets (manga and anime) with the grid and placeholders
- Priority: P2
- Effort: L
- Depends: T-50a-015
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaCover.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/LockedMangaWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/res/xml/updates_grid_homescreen_widget_info.xml
- Acceptance:
  - `dist/plasmoid/org.kurobayomi.updates.manga/` and `dist/plasmoid/org.kurobayomi.updates.anime/` each have `metadata.json` with `KPackageStructure` "Plasma/Applet" and Ids as named.
  - Names and descriptions: "Updates" / "Anime Updates" and "See your recently updated library entries".
  - `plasmoidviewer -a org.kurobayomi.updates.manga` renders the grid from a fixture CLI output; the grid is centred; a short last row is centred.
  - No configuration UI exists (no `ConfigGeneral.qml`).

### T-50a-019 — Applet refresh: 60 s poll, resize debounce, header refresh in error or empty state
- Priority: P2
- Effort: S
- Depends: T-50a-018
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeWidgetManager.kt
- Acceptance:
  - A `Timer` with interval 60000 ms, `triggeredOnStart: true`, runs the CLI.
  - Resizing triggers one CLI call 300 ms after the last size change (20 size changes over 1 s gives exactly 2 calls).
  - Clicking the header area refreshes immediately; the header is visible only in error and empty states.
  - A lock flip is shown within one poll interval (E-22).

### T-50a-020 — Cell activation opens the entry (`--open manga:<id>` / `anime:<id>`)
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview, T-50a-018
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/anime/UpdatesAnimeWidget.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - Activating a cell runs `kurobayomi --open manga:<entry_id>` (or `anime:`) built from the feed's `entry_id`, with no `Class.forName` lookup (E-12).
  - Each cell's command differs by id (E-13 test).
  - Mapping to the Android routes: `SHOW_MANGA` -> Library tab at entry; `SHOW_ANIME` -> anime library at entry (`MainActivity.kt:474-482`), verified by area:36-ui-shell-navigation-security-webview tests.

### T-50a-021 — Colours from the Plasma theme (replace Material You and fixed tokens)
- Priority: P2
- Effort: S
- Depends: area:49-theme-and-design-system, T-50a-018
- Sources: presentation-widget/src/main/res/values-v31/colors_appwidget.xml, presentation-widget/src/main/res/values-night-v31/colors_appwidget.xml, presentation-widget/src/main/res/values/colors_appwidget.xml, presentation-widget/src/main/res/drawable/appwidget_background.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaUpdatesGridGlanceWidget.kt
- Acceptance:
  - Applet text and background use `Kirigami.Theme` colours; no wallpaper-derived palette is read.
  - The placeholder fill `#1F888888` is the only fixed colour.
  - Manual check: switching the Plasma colour scheme between light and dark changes the applet text colour within one poll.

### T-50a-022 — Applet size rules (1x1 minimum, 4x2 default, 10x10 maximum)
- Priority: P2
- Effort: S
- Depends: T-50a-001, T-50a-018
- Sources: presentation-widget/src/main/res/xml/updates_grid_homescreen_widget_info.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt
- Acceptance:
  - The applet's minimum is one cell (64 x 95 px), default 256 x 190 px (4 x 2 cells), maximum 640 x 950 px (10 x 10 cells).
  - Android `minWidth`/`minHeight`/`minResize*`/`maxResize*` values are recorded as the reason for these numbers in the applet's code comment (and not ported as separate constants).
  - A test at each boundary gives the expected rows and columns.

### T-50a-023 — Accessibility and pointer affordances for covers
- Priority: P2
- Effort: S
- Depends: T-50a-018
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaCover.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt
- Acceptance:
  - Each cover is keyboard focusable (Tab), activated by Enter and Space, has accessible name = entry title, and shows the title as a tooltip after 500 ms of hover.
  - Pointer cursor is a pointing hand over covers.
  - Android `contentDescription = null` is not copied; this difference is listed in `p1-ui-parity.md` section 2.3.

### T-50a-024 — `.desktop` action `Updates` (and `--open updates`)
- Priority: P2
- Effort: S
- Depends: area:01-foundation-app-bootstrap, area:36-ui-shell-navigation-security-webview
- Sources: app/shortcuts.xml, presentation-widget/src/main/res/xml/updates_grid_homescreen_widget_info.xml
- Acceptance:
  - `dist/kurobayomi.desktop` contains `Actions=Updates;` and a `[Desktop Action Updates]` group with `Exec=kurobayomi --open updates` and `Name=Updates`.
  - `desktop-file-validate dist/kurobayomi.desktop` passes.
  - The action opens the Updates screen on a running instance and on a cold start.

### T-50a-025 — Applet packaging (kpackagetool6, optional package, Flatpak note)
- Priority: P2
- Effort: S
- Depends: T-50a-018, area:01-foundation-app-bootstrap
- Sources: presentation-widget/src/main/AndroidManifest.xml
- Acceptance:
  - A build step installs both applets with `kpackagetool6 --type Plasma/Applet --install`; `--remove` uninstalls.
  - The applet packages are optional: the app runs with them absent, and the `.desktop` action still works.
  - The packaging doc states that the Flatpak build ships the `.desktop` action only, and the applet is a host package.

## Dropped Android surfaces (P3, each states the loss)

### T-50a-026 — Drop the four Android home and lock receivers and their manifest registrations
- Priority: P3
- Effort: S
- Depends: T-50a-018
- Sources: presentation-widget/src/main/AndroidManifest.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaUpdatesGridGlanceReceiver.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeUpdatesGridGlanceReceiver.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaUpdatesGridGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeUpdatesGridGlanceWidget.kt
- Acceptance:
  - No Rust or QML file references these classes; the replacement is the applet pair (T-50a-018).
  - Loss recorded in this task: the home-screen grid on Android launchers. Replacement: the KDE applet and the `.desktop` action on other desktops.

### T-50a-027 — Drop the keyguard (lock-screen) providers
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-widget/src/main/res/xml/updates_grid_lockscreen_widget_info.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaUpdatesGridCoverScreenGlanceReceiver.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeUpdatesGridCoverScreenGlanceReceiver.kt, presentation-widget/src/main/res/layout/appwidget_coverscreen_loading.xml
- Acceptance:
  - No Linux lock-screen widget is built (KDE and GNOME have no API for it).
  - Loss recorded: seeing recent covers without unlocking the session. No replacement; the Updates tab after unlock.

### T-50a-028 — Drop the Samsung cover-screen metadata and cover-screen classes
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-widget/src/main/res/xml/updates_grid_samsung_cover_widget_info.xml, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaUpdatesGridCoverScreenGlanceWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeUpdatesGridCoverScreenGlanceWidget.kt, presentation-widget/src/main/res/drawable/appwidget_coverscreen_background.xml
- Acceptance:
  - No Samsung SDK metadata, transparent cover background, or white-on-cover colour is ported.
  - Loss recorded: sub-screen covers on Samsung devices (privacy-widget behaviour, E-32). Replacement: none.

### T-50a-029 — Drop push refresh (MangaWidgetManager / AnimeWidgetManager) in favour of the poll
- Priority: P3
- Effort: S
- Depends: T-50a-019
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeWidgetManager.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - No `combine`/`distinctUntilChanged` refresh pipeline is ported; the App wiring (`App.kt:151-158`) has no Kurobayomi counterpart.
  - Loss recorded: a refresh within a few milliseconds of a chapter read or a new chapter (now up to 60 s). The Updates tab keeps immediate updates (area:42-ui-updates-history-stats-storage-upcoming).

### T-50a-030 — Drop the Glance modifier helpers and rendering stack
- Priority: P3
- Effort: S
- Depends: T-50a-018
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaCover.kt
- Acceptance:
  - `appWidgetBackgroundRadius`, `appWidgetInnerRadius`, Glance `Box`/`Column`/`Row`/`Text`/`Image` are not ported; their geometry lives in T-50a-001 and T-50a-022.
  - Loss recorded: none visible; the rendering is replaced one-to-one in QML.

### T-50a-031 — Drop unused colour tokens
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-widget/src/main/res/values/colors_appwidget.xml, presentation-widget/src/main/res/values-v31/colors_appwidget.xml, presentation-widget/src/main/res/values-night-v31/colors_appwidget.xml
- Acceptance:
  - `appwidget_background` (colour), `appwidget_on_background`, `appwidget_surface_variant`, `appwidget_on_surface_variant` are not ported (no code or drawable reads them, E-18).
  - Loss recorded: none (dead tokens).

### T-50a-032 — Drop the widget-picker preview images
- Priority: P3
- Effort: S
- Depends: T-50a-018
- Sources: presentation-widget/src/main/res/xml/updates_grid_homescreen_widget_info.xml, presentation-widget/src/main/res/xml/updates_grid_lockscreen_widget_info.xml
- Acceptance:
  - `drawable-nodpi/updates_grid_widget_preview.webp` and `updates_grid_coverscreen_widget_preview.webp` are not shipped by Kurobayomi.
  - Loss recorded: a preview picture in the Android widget picker. Replacement: an applet screenshot in the package metadata (later, not in this plan).

### T-50a-033 — Drop the Gradle module wiring and empty or comment-only ProGuard files
- Priority: P3
- Effort: S
- Depends: none
- Sources: presentation-widget/build.gradle.kts, presentation-widget/proguard-rules.pro, presentation-widget/consumer-rules.pro
- Acceptance:
  - No Gradle or R8 file is created; the Cargo manifest replaces the module.
  - Loss recorded: none (the consumer file is empty and the ProGuard file has only comments).

## Logging and tests

### T-50a-034 — Replace logcat error logging with the app logger and CLI stderr
- Priority: P2
- Effort: S
- Depends: area:01-foundation-app-bootstrap, T-50a-015
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/anime/AnimeWidgetManager.kt
- Acceptance:
  - A failed CLI run writes one line to stderr, and the applet logs the stderr line through the Plasma debug output (`console.warn`).
  - The app-side thumbnail renderer logs failures through the logger chosen by area:01-foundation-app-bootstrap; a test asserts one log line per failed thumbnail.

### T-50a-035 — Test suite for the feed, CLI, and thumbnails
- Priority: P2
- Effort: M
- Depends: T-50a-015, T-50a-013
- Sources: presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt, presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt
- Acceptance:
  - `cargo test` runs: grid table (T-50a-002), window (T-50a-004), dedupe (T-50a-007), query fixtures (T-50a-005/006), lock (T-50a-008/009), golden JSON (T-50a-015), exit codes (T-50a-017), read-only and no-GTK checks (T-50a-016, T-50a-015), thumbnail geometry (T-50a-013).
  - Fixtures from `p1-edge-cases-and-tests.md` section 4 are checked in under `tests/fixtures/`.
  - The 500-row CLI run finishes under 50 ms (ignored benchmark test).
