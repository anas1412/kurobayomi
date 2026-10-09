# 50-android-widgets: overview

Area lead summary for the Android home-screen and lock-screen widgets (`presentation-widget/`) and their Linux replacement in Kurobayomi. Source root for every repo-relative path: `/home/blackbox/aniyomi-src`.

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

The `presentation-widget` module ships four Glance AppWidget providers:

| Provider | Surface | Kind |
|---|---|---|
| `MangaUpdatesGridGlanceWidget` | home screen | manga |
| `AnimeUpdatesGridGlanceWidget` | home screen | anime |
| `MangaUpdatesGridCoverScreenGlanceWidget` | lock screen / keyguard, Samsung cover screen | manga |
| `AnimeUpdatesGridCoverScreenGlanceWidget` | lock screen / keyguard, Samsung cover screen | anime |

Every provider shows the same thing: a grid of covers of the most recent unread chapters (manga) or unseen episodes (anime), one tap per cover, newest first. The feed rule is:

- favourites only (`favorite = 1`), chapter `read = 0` (episode `seen = 0`),
- `dateUpload > now - 3 calendar months` (strict),
- `date_fetch > date_added` (excludes the first library import),
- `LIMIT 500` rows in SQL, then first row per entry wins, capped to `rows * cols` (max 100).

When `use_biometric_lock` is `true`, the widget shows a text placeholder and performs no query. Nothing in this area writes to the database. The only side effect is that Coil writes library cover files (`MangaCoverFetcher.kt:105-145`, see E-7 in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md)).

Sources: `presentation-widget/src/main/java/tachiyomi/presentation/widget/` (abbreviated `W/`), `presentation-widget/src/main/res/`, `presentation-widget/src/main/AndroidManifest.xml`. Full observed behaviour is in [p1-behavior.md](p1-behavior.md).

### 1.2 Why it matters for a usable Kurobayomi (PROPOSAL)

The widget is not on the daily reading or watching path. It is a glance-and-tap entry point to the same "what is new" data the in-app Updates screen (area:42-ui-updates-history-stats-storage-upcoming) shows. It matters for three reasons:

1. The lock rule (`use_biometric_lock`) must hold on every external surface. A desktop surface that shows titles while the app is locked would be a privacy regression. This is the only P1 work in the area (T-50a-008 to T-50a-010).
2. The feed query (window, unread filter, dedupe, cap) is reused by the Updates screen and by the sidebar badge, so the query code is worth building once, correctly.
3. The sidebar badge is a hard-coded `"8"` in the mockup (`src/ui.rs:175`). The count query in this area is the fix.

### 1.3 Scope decision

Android home and lock-screen surfaces are dropped as surfaces (no Linux equivalent for the lock screen, no Glance). What survives is:

- the feed rule and grid geometry (ported, `src/feed/`),
- the lock rule (ported, same key),
- click routing (ported, `--open manga:<id>` / `anime:<id>`),
- a KDE Plasma 6 applet pair plus a `.desktop` action (new, the desktop surface).

Area scope: 36 files in the inventory (`p1-inventory.md`): 19 Kotlin, 1 Gradle, 1 manifest, 2 ProGuard/consumer, 13 resource files. The two binary previews in `presentation-widget/src/main/res/drawable-nodpi/` (`updates_grid_widget_preview.webp`, `updates_grid_coverscreen_widget_preview.webp`) are not in that count; they are dropped under T-50a-032.

## 2. Module maps

### 2.1 Kotlin module map (OBSERVED)

| Package / path (under `presentation-widget/src/main/java/tachiyomi/presentation/widget/`) | Files | Responsibility |
|---|---|---|
| `entries/manga/` | `MangaWidgetManager.kt`, `BaseMangaUpdatesGridGlanceWidget.kt`, `MangaUpdatesGridGlanceWidget.kt`, `MangaUpdatesGridCoverScreenGlanceWidget.kt`, `MangaUpdatesGridGlanceReceiver.kt`, `MangaUpdatesGridCoverScreenGlanceReceiver.kt` | Manga refresh pipeline (`combine` + `distinctUntilChanged`, `MangaWidgetManager.kt:22-45`), the render logic (`provideGlance`, `prepareData`, lock check, window, query), provider configuration (foreground, background, padding), manifest receivers |
| `entries/anime/` | `AnimeWidgetManager.kt`, `BaseAnimeUpdatesGridGlanceWidget.kt`, `AnimeUpdatesGridGlanceWidget.kt`, `AnimeUpdatesGridCoverScreenGlanceWidget.kt`, `AnimeUpdatesGridGlanceReceiver.kt`, `AnimeUpdatesGridCoverScreenGlanceReceiver.kt` | Anime twin of the above. Differs only in `seen`/`episodeId`, `animeId`, and the `SHOW_ANIME` action |
| `components/manga/` | `UpdatesMangaWidget.kt` (grid body and states), `UpdatesMangaCover.kt` (58x87 dp tile), `LockedMangaWidget.kt` (lock text) | Composable UI for the states loading, empty, content and locked |
| `components/anime/` | `UpdatesAnimeWidget.kt`, `UpdatesAnimeCover.kt`, `LockedAnimeWidget.kt` | Anime twin of the above |
| `util/` | `GlanceUtils.kt` | `DpSize.calculateRowAndColumnCount` (row and column formula, `GlanceUtils.kt:29-39`), Glance radius modifiers |

Resources and metadata (`presentation-widget/src/main/`):

| Path | Responsibility |
|---|---|
| `AndroidManifest.xml` | four `<receiver>` entries, each `exported="false"`, `enabled="@bool/glance_appwidget_available"` |
| `res/xml/updates_grid_homescreen_widget_info.xml` | home provider sizes (`minWidth` 240 dp, `minHeight` 80 dp, `targetCellWidth` 4, `targetCellHeight` 2, no `updatePeriodMillis`) |
| `res/xml/updates_grid_lockscreen_widget_info.xml` | keyguard provider (`widgetCategory="keyguard"`) |
| `res/xml/updates_grid_samsung_cover_widget_info.xml` | Samsung sub-screen (`privacyWidget="true"`) |
| `res/values/colors_appwidget.xml`, `res/values-v31/colors_appwidget.xml`, `res/values-night-v31/colors_appwidget.xml` | seven widget colour tokens, three variants |
| `res/values/dimens.xml`, `res/values-v31/dimens.xml` | `appwidget_background_radius` (16 dp, or system value on API 31+), `appwidget_inner_radius` (12 dp) |
| `res/drawable/appwidget_background.xml`, `appwidget_coverscreen_background.xml`, `appwidget_cover_error.xml` | card background, transparent cover-screen background, placeholder cover (`#1F888888` via `cover_placeholder`) |
| `res/layout/appwidget_loading.xml`, `appwidget_coverscreen_loading.xml` | initial RemoteViews, text "Loading..." |

Files outside the module that the widget depends on (not owned here):

| Path | Used for | Owner area |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/App.kt:151-158` | starts both managers on the process lifecycle | area:01-foundation-app-bootstrap |
| `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt`, `MangaCoverKeyer.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt` | cover loading, keying, cache file | area:34-images-covers-cache |
| `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt` | action strings and extra keys | area:36-ui-shell-navigation-security-webview |
| `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt` | `use_biometric_lock`, `lock_app_after` | area:02-preferences-system |
| `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:465-500` | routes for `SHOW_*` actions | area:36-ui-shell-navigation-security-webview |
| `data/src/main/sqldelight/view/updatesView.sq`, `data/src/main/sqldelightanime/view/animeupdatesView.sq` | feed SQL | area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations |
| `i18n/` and `i18n-aniyomi/` (`MR.strings.*`) | message keys | area:51-i18n |

### 2.2 Rust module map (PROPOSAL)

Single crate `kurobayomi` (edition 2024, `gtk` 0.11.5 with `v4_16`, `serde`, `serde_json`). Full signatures are in [p1-rust-design.md](p1-rust-design.md) section 3.

| Path | Responsibility | Replaces (Kotlin) |
|---|---|---|
| `src/main.rs` (early branch) | `--updates-json` returns before any GTK call | `App.kt` widget wiring |
| `src/cli.rs` | argv parsing, exit codes 0/2/3/4, stdout envelope | `Class.forName(MAIN_ACTIVITY)` intent extras |
| `src/feed/mod.rs` | `Kind`, `UpdateRow`, `dedupe_and_cap`, `FeedError`, `Request`, `build_envelope` (lock checked first) | `prepareData`, `provideGlance` |
| `src/feed/window.rs` | `window_start_ms(now)` (`checked_sub_months(Months::new(3))`) | `DateLimit` |
| `src/feed/grid.rs` | `CELL_W_DP`, `CELL_H_DP`, `MAX_ROWS`, `MAX_COLS`, `rows_cols` | `calculateRowAndColumnCount` |
| `src/feed/cover.rs` | cover size, radii, `cached_thumbnail` lookup | `UpdatesMangaCover.kt` constants, dimens, Coil keying |
| `src/feed/query.rs` | `unread_updates(conn, kind, after_ms, limit)`, count query for the badge | `getUpdatesByReadStatus`, `getUpdatesBySeenStatus`, repository mappers |
| `src/feed/lock.rs` | `is_locked(prefs)` for `use_biometric_lock` | `SecurityPreferences.useAuthenticator()` |
| `src/feed/json.rs` | serde types for schema 1 and the error envelope | the Compose/Glance rendering data |
| `dist/plasmoid/org.kurobayomi.updates.manga/` | Plasma 6 applet (QML, `contents/ui/main.qml`) | `UpdatesMangaWidget.kt`, `UpdatesMangaCover.kt`, `LockedMangaWidget.kt`, provider classes |
| `dist/plasmoid/org.kurobayomi.updates.anime/` | anime applet | anime twins |
| `dist/kurobayomi.desktop` (action group only from this area) | `Updates` action | `app/shortcuts.xml` |
| `tests/feed_cli.rs`, `tests/fixtures/*.sql`, `tests/golden/*.json` | golden and fixture tests | none (new) |

Dependencies: no `clap`, no `anyhow`/`thiserror`, no `image`, no `sha2`, no `tokio` in this path (rationale in the rust design, section 13). Thumbnails are rendered with GdkPixbuf, which comes with `gtk4`.

## 3. Reading guide

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | Every file in the area with LOC, top-level declarations, port decision (Port / Rewrite / Replace-with-crate / Drop) and task ids |
| [p1-behavior.md](p1-behavior.md) | The observed behaviour, section by section: startup wiring, manager pipeline, `provideGlance`, `DateLimit`, grid formula, cover preparation, composables, click routing, constants, strings, SQL, lock semantics, error and concurrency model |
| [p1-data-and-config.md](p1-data-and-config.md) | Domain models, constants table, preference keys, provider XML verbatim, colour and dimension tokens, DB objects, cover cache files, network calls, and the desktop JSON feed and cover-thumbnail design |
| [p1-topic-widget-feed-and-grid.md](p1-topic-widget-feed-and-grid.md) | Focused topic on the feed: window, query, dedupe, grid formula, and how each rule maps to the desktop CLI |
| [p1-topic-desktop-surfaces.md](p1-topic-desktop-surfaces.md) | Which desktop surfaces replace the Android ones (Plasma applet, `.desktop` action, no lock-screen, no GNOME Shell extension) and why |
| [p1-ui-parity.md](p1-ui-parity.md) | Visible UI of each state on Android (loading, empty, locked, content, placeholder) and the desktop equivalent, with every deliberate difference marked; gaps in the mockup |
| [p1-rust-design.md](p1-rust-design.md) | Architecture, module layout, Rust signatures, CLI contract, `main.rs` integration, Android-to-Linux mapping table, threading model, contracts with area:02 and area:34, applet skeleton, `.desktop` file, error handling, crates, pitfalls, open questions |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E-1 to E-36 with Android behaviour, desktop handling and a named test for each; failure modes table; the full test list; fixtures; race conditions |
| [p1-tasks.md](p1-tasks.md) | The 35 tasks T-50a-001 to T-50a-035 (lints ok) |

## 4. Key design decisions and open questions

Each item has a recommendation. Items marked "owner call" need the user's answer before implementation.

### D-1 Desktop surface: KDE Plasma 6 applet plus `.desktop` action (decided in the plans, owner call to confirm)

Recommendation: keep. The applet is the only surface with a grid of covers on the desktop. The `.desktop` `Updates` action gives the same entry point on GNOME, XFCE and any launcher menu. A GNOME Shell extension is out of scope.

### D-2 Refresh: 60 s poll, no push (decided)

Recommendation: keep the poll. A push channel needs a running daemon, and the applet must work when the main window is closed. The cost is up to 60 s of latency for a chapter read or a new chapter. The Updates screen (area:42) still refreshes immediately.

### D-3 Lock rule: the flag only, not the in-process unlock state (owner call, Q-1)

Android shows the locked placeholder whenever `use_biometric_lock` is `true`, even while the app is unlocked (`BaseMangaUpdatesGridGlanceWidget.kt:69`). It also ignores `lock_app_after`.

Recommendation: keep the Android rule (fail closed). The CLI would need a shared unlock marker from area:36 to do better, and that adds state for little gain. Revisit once area:36 has a marker.

### D-4 The key name `use_biometric_lock` on Linux (owner call, new)

There is no biometric prompt on a Linux desktop. The key is still the right storage key for the app-lock flag, because area:02 stores it and the Android migration path depends on it.

Recommendation: keep the key name, and have the in-app settings label say what the lock actually does (for example "Require unlock to show widgets"), which is a copy decision for area:47/area:51. The lock itself is area:36's responsibility.

### D-5 Error state on the desktop (deliberate difference, recommend keep)

Android shows nothing for DB or Coil errors (E-6, E-21). The desktop shows "Updates unavailable" with the stderr line in a tooltip, so a broken CLI is not mistaken for an empty library.

### D-6 Query parity: `LIMIT 500` before dedupe (recommend keep parity, opt-in fix later)

A library with many unread chapters for few titles can yield fewer covers than the grid holds (E-5). Recommendation: keep parity as the default. The per-entry `ROW_NUMBER()` query is a later opt-in (proposal in E-5), not part of the first version.

### D-7 Explicit `ORDER BY` (decided, deliberate difference)

Android relies on the view's `ORDER BY date_fetch DESC` surviving the outer `SELECT *`, which SQLite does not guarantee (E-4). The desktop query orders explicitly by `date_fetch DESC, update_id DESC`. This only changes behaviour where Android was undefined.

### D-8 Thumbnail format and location (decided)

PNG at 58x87 logical px times `device_scale` (default 2, so 116x174 px), with the rounded mask baked in, written under `$XDG_CACHE_HOME/kurobayomi/widget-covers/` via temp file and rename. Recommendation: keep PNG. No WebP encoder dependency is worth it for files this small. The directory and key scheme belong to area:34; this area only resolves them.

### D-9 Anime empty-state text (owner call, Q-6)

Android's anime widget shows "No recent updates" (`information_no_recent`), not its own key "Nothing watched recently" (`information_no_recent_anime`, E-24). The code path looks unintended.

Recommendation: parity for the first version (T-50a-011 asserts "No recent updates"), then ask the user whether to switch to "Nothing watched recently". This is a text change, so no code moves.

### D-10 Constant `CARD_RADIUS_DP` (recommend drop)

The rust design defines `CARD_RADIUS_DP = 16.0` for the Android card background (`appwidget_background_radius`), but the desktop applet draws no card (ui-parity section 2.1). The constant has no user. Recommendation: do not define it, and drop it from T-50a-001 and its test. This follows the "no speculative code" rule. The tasks file still lists it; the edit is a follow-up to T-50a-001 and is not made here because task text is owned by the area author.

### D-11 Applet size limits (decided)

Minimum 1x1 cell (64x95 px), default 4x2 cells (256x190 px), maximum 10x10 cells (640x950 px). Android's `minWidth`/`minHeight`/`minResize*`/`maxResize*` values are recorded as the reason, not ported as constants (T-50a-022).

## 5. Cross-area dependencies

### 5.1 This area needs from others

| Need | From | Where it is used | Status in the plans |
|---|---|---|---|
| Binary `kurobayomi` on `PATH`, `.desktop` file, applet packaging, logger | area:01-foundation-app-bootstrap | T-50a-024, T-50a-025, T-50a-034 | written in the plans |
| `PrefsReader` trait (`get_bool`) and the on-disk store format, key `use_biometric_lock` default `false` | area:02-preferences-system | T-50a-008 (P1, blocking) | written in the plans; the trait is a proposal |
| Manga schema: `mangas`, `chapters`, `updatesView`, read-only open, WAL, indexes | area:03-db-manga-schema-migrations | T-50a-005, T-50a-016 | written in the plans |
| Anime schema: `animes`, `episodes`, `animeupdatesView` | area:04-db-anime-schema-migrations | T-50a-006 | written in the plans |
| Repository mapping for update rows | area:05-data-repos-entries-items | T-50a-005 | written in the plans |
| Cover cache directory and key scheme, cover renderer ownership | area:34-images-covers-cache | T-50a-013, T-50a-014 | written in the plans |
| App lock UI and unlock flow, `--open` argument parser, GApplication open targets, routes for `manga:<id>`, `anime:<id>`, `updates` | area:36-ui-shell-navigation-security-webview | T-50a-008, T-50a-020, T-50a-024 | written in the plans |
| Theme tokens (Kirigami colours for the applet) | area:49-theme-and-design-system | T-50a-021 | written in the plans |
| Message keys and translation catalogues | area:51-i18n | T-50a-010, T-50a-011 | not referenced by a task yet (see section 7, risk R-6) |
| Update rows are created by library updates | area:26-library-update-jobs | (behaviour only, no task) | no task needed |

### 5.2 Other areas need from this one

| Consumer | What it needs | Provided by |
|---|---|---|
| area:42-ui-updates-history-stats-storage-upcoming | the same unread-updates query (`unread_updates`) and the unread count for the sidebar badge (rust design section 15) | T-50a-005, T-50a-006, and the count query in `src/feed/query.rs` (no task yet, see R-5) |
| area:36-ui-shell-navigation-security-webview | the widget's open targets (`--open manga:<id>`, `anime:<id>`, `updates`) and the lock flag contract | T-50a-020, T-50a-008 |
| area:35-data-misc-updater-notification-export | notifications open the same routes (`SHOW_MANGA`, `SHOW_ANIME`) | T-50a-020 (route mapping) |
| area:01-foundation-app-bootstrap | `src/main.rs` early branch for `--updates-json`, the `.desktop` `Actions=` line | T-50a-015, T-50a-024 |
| area:34-images-covers-cache | the thumbnail path lookup used by the CLI | T-50a-013 |

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows with `file` starting `50-android-widgets/` in `/home/blackbox/kurobayomi/plans/TASKS.csv`. Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint 50-android-widgets/p1-tasks.md` prints `ok`.

Counts (35 tasks, all in `p1-tasks.md`):

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 3 | 0 | 0 | 0 | 3 |
| P2 | 19 | 4 | 1 | 0 | 24 |
| P3 | 8 | 0 | 0 | 0 | 8 |
| **Total** | **30** | **4** | **1** | **0** | **35** |

P1 tasks: T-50a-008 (lock flag), T-50a-009 (locked payload), T-50a-010 (locked text). All other feature tasks are P2. P3 tasks (T-50a-026 to T-50a-033) record dropped Android surfaces with the user loss stated.

### 6.1 Critical path

There are no P0 tasks in this area. The area is not on the P0 path (reading, library, player and the local DB do not depend on it). Its internal order, with the P1 lock chain first:

1. Lock chain (P1, must land before any external surface ships): T-50a-008 (needs area:02 and area:36) -> T-50a-009 -> T-50a-010.
2. Constants and geometry: T-50a-001 -> T-50a-002 -> T-50a-003; T-50a-001 -> T-50a-004.
3. Query: T-50a-005 (needs area:03 and area:05; also T-50a-004) -> T-50a-006 (needs area:04) and T-50a-007.
4. Cover thumbnails: T-50a-013 (needs area:34 and T-50a-001) -> T-50a-014.
5. CLI: T-50a-015 (needs T-50a-003, 004, 005, 006, 007, 008, 013) -> T-50a-016 (needs area:03), T-50a-017.
6. Applets: T-50a-018 (L, needs T-50a-015) -> T-50a-019, T-50a-020 (needs area:36), T-50a-021 (needs area:49), T-50a-022, T-50a-023, T-50a-025 (needs area:01); T-50a-012 needs T-50a-018.
7. Desktop action: T-50a-024 (needs area:01 and area:36).
8. Tests: T-50a-035 (needs T-50a-015 and T-50a-013) and T-50a-034 (logging).

Longest chain: T-50a-001 -> T-50a-004 -> T-50a-005 -> T-50a-015 -> T-50a-018 -> T-50a-019 (six hops, one L task).

### 6.2 Risks

- R-1 Priority inversion. T-50a-009 (P1) defines the lock contract as "no query when locked" through `build_envelope`, which is built in T-50a-015 (P2). Recommendation: build the lock early return with T-50a-008 (it is three lines in `build_envelope`), and keep T-50a-015 for the rest of the envelope.
- R-2 Plasma 6 API drift. The applet relies on the executable engine (`data["exit code"]`, `data["stdout"]`), `kpackagetool6`, and metadata keys. These must be checked against the installed Plasma version before T-50a-018 is closed. Plasma's API is not covered by cargo tests; only manual `plasmoidviewer` checks cover it.
- R-3 Cross-area blocking. T-50a-008 needs the `PrefsReader` trait and store format from area:02 and the lock UI from area:36. If either is late, the lock chain stalls, and the lock chain gates every external surface.
- R-4 Trust boundary. The CLI prints titles to stdout for any process running as the same user, and the database is unencrypted. The lock flag is a UI gate, not a security boundary (E-33). Document this in the packaging doc (T-50a-025).
- R-5 Missing task for the sidebar badge count. The rust design (section 15) adds `COUNT(*)` to `feed::query` for the sidebar badge that replaces the literal `"8"` (`src/ui.rs:175`), but no task in this area creates it. Add one under the area:42 wiring, or add it to T-50a-005.
- R-6 Missing task for message keys. The applet strings (`No recent updates`, `Widget not available when app lock is enabled`, `Updates unavailable`, `Updates`, `Anime Updates`) need catalogue entries through area:51-i18n. Only the English source strings are referenced; no task creates the keys.
- R-7 Test fixtures. `fixtures/feed-50k.sql` (50 000 chapters) and the E-9 query-plan test depend on a generator script that no task creates.
- R-8 Latency expectations. The 60 s poll means a read in the app is visible on the applet up to a minute later. This is accepted (D-2), but it must be stated in the applet's documentation.
- R-9 Thumbnail renderer threading. GdkPixbuf alpha-mask work must run in `gio::spawn_blocking` (rust design section 8). A main-loop implementation would freeze the window for large grids.

## 7. Definition of done for this area in Kurobayomi

Each item is observable. Items 1 to 6 are the minimum for "usable"; 7 to 10 complete the area.

1. Lock: with `use_biometric_lock` set to `true`, `kurobayomi --updates-json --kind manga --rows 2 --cols 4` exits `0` and prints `"locked":true` with `"items":[]`. No `title`, `cover_path` or `unread_update_id` key appears in the output. The database is not queried (T-50a-009 test: a fixture without the view still exits `0`).
2. Lock text: the manga and anime applets show "Widget not available when app lock is enabled" when `locked` is `true`, and the item model is empty after the flip (within one poll, 60 s).
3. CLI without a display: `env -u DISPLAY -u WAYLAND_DISPLAY kurobayomi --updates-json --kind anime --rows 2 --cols 4` exits `0` with valid JSON. A missing database exits `3` with `"code":"db_missing"` and the file is not created.
4. Feed correctness: with the golden fixture `feed-basic.sql`, the output contains exactly the favourites, unread, in-window rows, deduplicated per entry, in `date_fetch DESC, id DESC` order, capped at `rows * cols`.
5. Applet grid: `plasmoidviewer -a org.kurobayomi.updates.manga` shows covers in a centred grid. A short last row is centred. Resizing from 1x1 to 10x10 cells changes the grid with no overlap; one CLI call is made per 300 ms-settled resize.
6. Click: clicking a cover opens the app on that cover's entry (Library tab for manga, anime library for anime). Each cover opens its own entry, so two covers never open the same one (E-13 test).
7. Error state: with the binary renamed, the applet shows "Updates unavailable" within 6 s and retries on the next poll or on click.
8. Empty state: with an empty database, both applets show "No recent updates".
9. `.desktop` action: `desktop-file-validate dist/kurobayomi.desktop` passes. The `Updates` action opens the Updates screen on a running instance and on a cold start.
10. Tests: `cargo test` passes the grid table, window, dedupe, query fixtures, lock, golden JSON, exit codes, read-only and no-GTK checks, and thumbnail geometry. The 500-row CLI run finishes in under 50 ms (`cargo test -- --ignored`).

Explicitly not required: Android lock-screen widgets, Samsung sub-screen widgets, push refresh, the Glance rendering stack, GNOME widgets.

## 8. Things this overview does not change

- No task IDs changed. The tasks file lints `ok`.
- The `Depends:` tokens in `p1-tasks.md` already used full area ids and all resolve to folders in `/home/blackbox/kurobayomi/plans/`. Short forms in prose (`area:01`, `area:34`, `area:42`, ...) were normalised to full folder ids across this folder's documents. One prose reference to `area:49 / i18n` now reads `area:51-i18n`, because the i18n area is `51-i18n`.
- Source line references were spot-checked against `/home/blackbox/aniyomi-src` (for example `App.kt:151-158`, `GlanceUtils.kt:29-39`, `GetMangaUpdates.kt:21`, `BaseMangaUpdatesGridGlanceWidget.kt:158-161`, `updatesView.sq` `ORDER BY date_fetch DESC`, the `MainActivity.kt` route block, `SecurityPreferences.kt:13`, and the `kotlin_version = "2.4.0"` pin). All matched.
