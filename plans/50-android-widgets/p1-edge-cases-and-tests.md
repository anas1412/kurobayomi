# 50-android-widgets — Part 1 edge cases, failure modes, tests

Source of truth for the Android behaviour: `p1-behavior.md`. Identifiers `E-n` are referenced from there. Upstream test files: none exist in this scope (`presentation-widget/` has no `src/test` or `src/androidTest` tree in the scope list), so every test below is new.

## 1. Edge cases (Android behaviour, with desktop handling)

### E-1 Refresh key ignores anything except ids and the lock flag

- Android: `MangaWidgetManager.kt:31-34` drops emissions where the lock flag is equal and the set of `chapterId`s is equal. A changed title or cover (same chapter ids) does not refresh the widget until the next `updateAll` for another reason.
- Desktop: no push refresh. The applet polls every 60 s and renders whatever the CLI returns, so covers and titles refresh within 60 s.
- Test: `T-poll-cover-change` (section 3) changes only `thumbnail_url` and `cover_last_modified` and asserts the next CLI call returns the new `cover_path`.

### E-2 Grid size taken from the largest-area instance of the provider

- Android: `BaseMangaUpdatesGridGlanceWidget.kt:79-82` = `ids.flatMap { manager.getAppWidgetSizes(it) }.maxBy { it.height.value * it.width.value }.calculateRowAndColumnCount(...)`. The feed is cut to that instance's `rows * cols`. A tall narrow instance can have more rows than the largest-area instance, so it shows fewer rows than it could. A small instance shows only the front of the feed.
- Desktop: the applet passes its own `--rows` and `--cols`, so no instance is compared.
- Test: `T-grid-per-instance` (the CLI with rows=3 cols=1 returns exactly 3 items when the fixture has 5).

### E-3 `DateLimit` frozen at manager start

- Android: `MangaWidgetManager.kt:26` evaluates `DateLimit` once in `init`. A process that stays up for days keeps the old bound for the subscription, while each widget render recomputes it.
- Desktop: the window is computed per CLI call.
- Test: `T-window-per-call` (two CLI calls with `--now-ms` 1 day apart return different `window_after_ms`).

### E-4 Ordering not guaranteed

- Android: `updatesView` has `ORDER BY date_fetch DESC`, but `getUpdatesByReadStatus` (`updatesView.sq`) has no ORDER BY. SQLite does not guarantee the view's order survives the outer `SELECT *`. The "newest first" feed and the `distinctBy` winner both rely on it.
- Desktop: the query has `ORDER BY date_fetch DESC, update_id DESC` explicitly (tie-break on the id so equal timestamps are deterministic).
- Test: `T-order-deterministic` (two rows with equal `date_fetch` come back in descending id order on every run).

### E-5 `LIMIT 500` before dedupe

- Android: the SQL `LIMIT 500` counts chapter rows, and `distinctBy` runs afterwards (`BaseMangaUpdatesGridGlanceWidget.kt:123-124`). A library where 500 unread chapters belong to 3 titles yields 3 covers, though the grid could show 8.
- Desktop: keep parity by default (`limit = 500` chapter rows). Proposal (opt-in, not default): `ROW_NUMBER() OVER (PARTITION BY entry_id ORDER BY date_fetch DESC, update_id DESC) = 1` in SQL, so the limit applies to entries.
- Test: `T-limit-before-dedupe` (fixture with 600 unread rows of 3 entries: default returns 3 items; with the opt-in query returns min(cap, entries)).

### E-6 Upstream flow errors end auto-refresh silently

- Android: an exception from the DB or preferences flow is not inside the `try` in `MangaWidgetManager.kt:35-42`. `combine` terminates and `launchIn` ends. No more automatic refresh until the process restarts. Logged nowhere.
- Desktop: the poll is independent; an error in one run shows the error placeholder and the next poll retries (60 s).
- Test: `T-poll-recovers` (make the DB briefly locked, CLI returns exit 3, then succeeds on the next call).

### E-7 Widget loads write the library cover cache and use the network

- Android: the factory passes `isLibraryManga = data.isMangaFavorite` (`MangaCoverFetcher.kt:330`), with `isMangaFavorite = true` hard-coded (`BaseMangaUpdatesGridGlanceWidget.kt:131`). So a background widget render can download and store covers in the library cover cache (`httpLoader`, lines 105-145), even for entries the user never opened.
- Desktop: the CLI never downloads and never writes. Covers without a rendered thumbnail show the placeholder. The Updates tab (area:42-ui-updates-history-stats-storage-upcoming) still fetches normally.
- Test: `T-cli-no-network` (run the CLI with no network and a missing cover: exit 0, `cover_path` null, no sockets opened; check with `strace -e trace=network` in the test script).

### E-8 Lock state is the flag, not the unlock state

- Android: `BaseMangaUpdatesGridGlanceWidget.kt:69` reads only `use_biometric_lock`. The widget shows the locked placeholder even when the user unlocked the app a moment ago, and it ignores `lock_app_after` (`SecurityPreferences.kt:15`).
- Desktop: same rule (`is_locked` reads the flag). Open question Q-1 in the Rust design asks whether to relax this.
- Test: `T-lock-flag-only` (flag on, no unlock marker: `locked: true`; flag off: `locked: false`; `lock_app_after` value changes nothing).

### E-9 Partial index may not be used by the view join

- Android: `chapters_unread_by_manga_index ON chapters(manga_id, read) WHERE read = 0` exists (`data/src/main/sqldelight/data/chapters.sq`), but the query starts from `updatesView`, which joins `mangas` and `chapters` on `manga_id`. The planner may scan. Not measured in this review.
- Desktop: run `EXPLAIN QUERY PLAN` on the query at implementation time; add an index only if a scan shows up on the fixture of 50 000 chapters.
- Test: `T-query-plan` (asserts the plan contains `SEARCH` on `chapters` and not `SCAN chapters` for the 50 000-row fixture; if it does not, the test documents the index to add).

### E-10 Density used for cover bitmap is the system density

- Android: `dpToPx` uses `Resources.getSystem().displayMetrics.density` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt:8`). On a device with a different launcher density the bitmap is sized for the system screen, and Glance `Crop` fits it.
- Desktop: `device_scale` argument (default 2). Thumbnails are 116 x 174 px at scale 2 and are center-cropped to the cell.
- Test: `T-thumb-scale` (scale 1 gives 58 x 87 px; scale 2 gives 116 x 174 px; both have aspect 58:87 within 1 px).

### E-11 All covers are awaited before anything renders

- Android: `prepareData` runs `executeBlocking` for every cover sequentially inside `withIOContext`, and `collectAsState` gets the list once (`BaseMangaUpdatesGridGlanceWidget.kt:121-155`). A slow source delays the whole grid, up to 100 covers. The user sees the spinner for the whole time.
- Desktop: the CLI does no image work (it reports paths only). Missing thumbnails show the placeholder and the GTK side renders them in the background; the grid appears at once.
- Test: `T-no-blocking-on-covers` (fixture with 100 entries whose thumbnails are absent: CLI finishes in under 50 ms).

### E-12 `Class.forName` on a string constant

- Android: `Class.forName(Constants.MAIN_ACTIVITY)` at composition time (`UpdatesMangaWidget.kt:76`, `LockedMangaWidget.kt:27`). A rename of the activity breaks the widget at runtime, not at compile time.
- Desktop: the applet calls the binary name, `kurobayomi`. A rename of the binary breaks it; the `.desktop` file and applet share one constant in `dist/`.
- Test: `T-applet-binary` (the QML command string equals the installed binary name in the `.desktop` `Exec=` line; a grep check in CI).

### E-13 PendingIntent uniqueness

- Android: each cell's intent has a unique category `mangaId.toString()` as a workaround for issuetracker 238793260 (`UpdatesMangaWidget.kt:83-84`). Without it, Android reuses the PendingIntent and every cell opens the same entry.
- Desktop: each cell runs its own command with its own id. No shared state.
- Test: `T-cell-targets` (render the applet command for ids 1 and 2 and check they differ).

### E-14 Missing cover on the first row wins the dedupe

- Android: `distinctBy { mangaId }` keeps the first row even when its `thumbnailUrl` is null. The view joins the cover columns from the entry row, so every chapter row of one entry carries the same `thumbnailUrl`; the null case therefore shows the placeholder for the whole entry, and the dedupe rule has no second row to fall back to.
- Desktop: same rule (parity). The cover is per entry, so the first row's cover is the entry's cover.
- Test: `T-dedupe-first-row` (two rows for entry 1, first with null thumbnail, second with URL: the item uses the first row's `unread_update_id` and `cover_path: null`).

### E-15 Duplicate picker labels

- Android: `label_recent_updates` is used by both the manga home and manga lockscreen providers (`AndroidManifest.xml`), so the picker shows two "Updates" entries.
- Desktop: applet names are "Updates" (manga) and "Anime Updates" (anime) only; no lock-screen applet.
- Test: `T-applet-names` (metadata names are unique across the two applets).

### E-16 Home manga and anime providers share one size XML

- Android: `updates_grid_homescreen_widget_info.xml` is used by both home providers, so anime and manga widgets are sized identically (fine, intentional).
- Desktop: both applets share the default size (4 x 2 cells) and the minimum (1 x 1). Constant `CELL_*` values live in Rust, read by both.
- Test: none needed beyond the grid test.

### E-17 Empty ProGuard and consumer rules

- Android: `consumer-rules.pro` has 0 lines and `proguard-rules.pro` has only comments. No rule is active.
- Desktop: nothing to port (dropped, see inventory).
- Test: none.

### E-18 Declared but unused colour tokens

- Android: `appwidget_background` (colour), `appwidget_on_background`, `appwidget_surface_variant`, `appwidget_on_surface_variant` are declared in `values/colors_appwidget.xml` and the v31 and night variants, but no code or drawable references them (grep of scope: only `appwidget_secondary_container`, `appwidget_on_secondary_container`, `appwidget_coverscreen_background` are used).
- Desktop: not ported (no token table in Rust).
- Test: none; the inventory records the drop.

### E-19 Corner radius from the app context vs the launcher

- Android: `roundPx = context.resources.getDimension(R.dimen.appwidget_inner_radius)` (`BaseMangaUpdatesGridGlanceWidget.kt:120`) uses the app's density to build the pre-API-31 cover transform, while the Glance `cornerRadius` is applied by the launcher.
- Desktop: radius is baked into the PNG at 12 x `device_scale` px. One place, one value.
- Test: `T-thumb-radius` (a corner pixel of the rendered PNG has alpha 0; the centre has alpha 255).

### E-20 API-level branch for corners

- Android: below API 31 the transform rounds covers, at API 31 and up the launcher clips them (`BaseMangaUpdatesGridGlanceWidget.kt:141-145`). The look differs slightly by version.
- Desktop: always baked. No branch.
- Test: `T-thumb-radius` as E-19.

### E-21 Spinner with no timeout

- Android: `data == null` shows `CircularProgressIndicator` until the first emission. If the flow never emits (e.g., a stuck DB), the spinner stays.
- Desktop: the applet shows the spinner until the first CLI result or the first error; a CLI exit (success or error) always ends the loading state. A command that hangs longer than 5 s is killed by the applet's timeout (proposal) and shows the error placeholder.
- Test: `T-applet-timeout` (a fake CLI that sleeps 10 s causes the error placeholder within 6 s).

### E-22 Lock toggled while the card is on screen

- Android: the card keeps its last content until Glance re-renders; the manager refreshes both providers when the flag changes, so the swap happens quickly.
- Desktop: the flag is read per poll, so the swap happens within 60 s. Refresh triggers are the 60 s timer, the debounced resize (E-36), and a click on the header in the error or empty state. No focus-based refresh.
- Test: `T-lock-swap` (flag set to on between two polls: the second CLI call returns `locked: true` and the applet clears `items`).

### E-23 `maxBy` on empty input

- Android: `ids.flatMap { manager.getAppWidgetSizes(it) }.maxBy { ... }` (`BaseMangaUpdatesGridGlanceWidget.kt:79-81`). Kotlin 2.4.0 (`gradle/kotlinx.versions.toml:2`): `Iterable.maxBy` is non-nullable and throws `NoSuchElementException` on an empty list. If `getGlanceIds` or `getAppWidgetSizes` returns nothing, `provideGlance` throws.
- Desktop: the grid comes from the applet size (`max(1, floor(...))`), so no empty-list case.
- Test: `T-no-empty-max` (not applicable on desktop; the Android behaviour is recorded here as the reason the size is no longer derived from a list).

### E-24 Anime empty state uses the manga-neutral key

- Android: `UpdatesAnimeWidget.kt:46` uses `MR.strings.information_no_recent` ("No recent updates"), not the anime-specific `information_no_recent_anime` ("Nothing watched recently", `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:534`). Observed; the anime-specific key is unused by the widget.
- Desktop: same as Android (parity). Changing it would be a UI decision for the owner; recorded here.
- Test: `T-empty-text-key` (the anime applet's empty text equals "No recent updates").

### E-25 Recomposition creates a new flow

- Android: the DB flow is inside `remember { }` in `provideContent` (`BaseMangaUpdatesGridGlanceWidget.kt:94-100`), so each new composition subscribes again. This is correct but means a new query per composition; there is no shared cache.
- Desktop: one query per poll. No shared state.
- Test: `T-poll-query-count` (count SQL statements in one CLI call: exactly one).

### E-26 Time zone of the window

- Android: system zone (`ZonedDateTime.now()`). Across a DST change the bound moves by an hour at most; the 3-month calendar subtraction is unaffected.
- Desktop: `chrono::Local`. Same.
- Test: `T-window-dst` (a fixed `--now-ms` on a DST boundary in the test zone `TZ=Europe/Berlin` gives the expected local date three months earlier).

### E-27 Cover URLs that are not HTTP

- Android: `MangaCoverFetcher` handles `file://` (`Type.File`) and `content://`/`URI` (`Type.URI`, `uniFileLoader`) (`MangaCoverFetcher.kt:75-78`). `content://` comes from storage-access framework folders.
- Desktop: no `content://`. A `file://` thumbnail URL is read from disk by the GTK side; `content://` URLs get no thumbnail (placeholder).
- Test: `T-local-cover` (a local source with `file:///...` URL renders a thumbnail; a `content://` URL gives `cover_path: null`).

### E-28 Titles with special characters

- Android: titles are not shown on covers (no text). Only `contentDescription = null`.
- Desktop: titles go in JSON (tooltip). Quotes, backslashes, newlines, emoji, and RTL text must round-trip.
- Test: `T-title-escaping` (title `He said "hi"\n‮` round-trips through `serde_json` and QML `JSON.parse`).

### E-29 Un-favourite removes rows

- Android: the view requires `favorite = 1`, so un-favouriting removes the entry's rows. The manager sees the set change and refreshes.
- Desktop: the next poll drops the entry. Up to 60 s later.
- Test: `T-unfavorite` (set `favorite = 0` and the next CLI call omits the entry).

### E-30 Rows dated before the entry was added

- Android: the view requires `date_fetch > date_added`, so the initial library import is excluded.
- Desktop: same filter (parity).
- Test: `T-date-fetch-added` (a row with `date_fetch == date_added` is excluded; `date_fetch = date_added + 1` is included).

### E-31 Exactly at the window bound

- Android: `dateUpload > :after` (strict). A row with `dateUpload == after` is excluded.
- Desktop: same strict comparison.
- Test: `T-window-bound-strict` (a row at exactly `window_after_ms` is excluded).

### E-32 Cover-screen widget may show covers on a locked device

- Android: the keyguard providers are `widgetCategory="keyguard"` and the Samsung sub-screen metadata sets `privacyWidget="true"` (`updates_grid_samsung_cover_widget_info.xml`). From the metadata alone, covers can appear on a locked device when the app lock flag is off. Device behaviour not verified in this review.
- Desktop: no keyguard surface. Nothing is shown on a locked session (the applet runs inside the user's Plasma session, and a locked Plasma session does not run applets).
- Test: none (dropped surface). Recorded as the privacy reason for the drop.

### E-33 Local access to the CLI output

- Android: widget content is visible to the launcher process.
- Desktop: any process running as the same user can run `kurobayomi --updates-json` or read the database directly. The database is not encrypted; the lock flag only gates the widget surface. This is the same trust level as the database itself.
- Test: none. Documented in the Rust design under security notes (no separate control here).

### E-34 CLI runs while the app writes

- Desktop: WAL readers do not block the writer. A checkpoint can make a read see the last committed state only. `SQLITE_BUSY` is possible during a schema change; mapped to exit 3 after a 250 ms busy timeout.
- Test: `T-cli-during-write` (a writer thread holds a write transaction for 1 s; the CLI returns exit 3 or a result within 300 ms and never a torn row).

### E-35 Atomic thumbnail writes

- Desktop: thumbnails are written to a temporary file in the same directory and renamed. The CLI reads paths only and checks `exists`, so a half-written file is never reported.
- Test: `T-thumb-atomic` (kill the renderer mid-write: no file with the final name exists; the temp file is removed on the next run).

### E-36 Applet resize flapping

- Desktop: a drag-resize emits many size changes. The applet debounces the CLI call (300 ms after the last change).
- Test: `T-resize-debounce` (simulate 20 size changes over 1 s: exactly 2 CLI calls).

## 2. Failure modes summary

| Failure | Android result | Desktop result | Exit / code |
|---|---|---|---|
| DB missing | not applicable in the widget code (the app always owns the database) | error placeholder "Updates unavailable" | 3 / `db_missing` |
| DB busy | silent stop of auto-refresh (E-6) | error placeholder; next poll retries | 3 / `db_busy` |
| Prefs unreadable | no error path in the widget code; the flag is read synchronously | error placeholder; fails closed | 4 / `prefs_unreadable` |
| Cover missing | placeholder | placeholder | 0 / `cover_path: null` |
| Cover network error | placeholder | not applicable (CLI never downloads) | 0 |
| Activity class missing | `ClassNotFoundException` at composition | not applicable | not applicable |
| Empty `ids` list | `NoSuchElementException` (E-23) | not applicable | not applicable |
| CLI hangs | not applicable | applet timeout 6 s, error placeholder | not applicable |
| Usage error (bad args) | not applicable | exit 2 with `usage` envelope | 2 / `usage` |

## 3. Tests to write (new)

All Rust tests live in `tests/feed_cli.rs` and `src/feed/*.rs` `#[cfg(test)]` modules. Fixtures are built by a helper that applies the schema SQL from area:03-db-manga-schema-migrations/04 to a temporary database file (never the user's database).

Unit tests (`src/feed/grid.rs`, `window.rs`, `mod.rs`):

- `T-grid-table`: `rows_cols` for these inputs, expected (rows, cols):
  - (w 64, h 95, 0, 0) -> (1, 1)
  - (w 127.9, h 189.9, 0, 0) -> (1, 1)
  - (w 128, h 190, 0, 0) -> (2, 2)
  - (w 640, h 950, 0, 0) -> (10, 10)
  - (w 2000, h 2000, 0, 0) -> (10, 10)
  - (w 10, h 10, 0, 0) -> (1, 1)
  - (w 256, h 190, 0, 0) -> (2, 4)
  - (w 256, h 190, 24, 24) -> (1, 4) (h - 48 = 142 -> 1 row)
  - (w 256, h 100, 10, 0) -> (1, 4) (h = 90 -> clamped to 1)
- `T-window-month`: now 2026-10-08T12:00 Europe/Berlin -> 2026-07-08T12:00 local.
- `T-window-clamp`: now 2026-05-31T00:00 -> 2026-02-28T00:00 (2026 not leap).
- `T-window-leap`: now 2028-05-31T00:00 -> 2028-02-29T00:00.
- `T-window-bound-strict` (see E-31).
- `T-dedupe-first`: input [(1,a),(2,b),(1,c)] -> [(1,a),(2,b)].
- `T-dedupe-cap`: input of 20 distinct entries, cap 8 -> 8 items in input order.
- `T-dedupe-empty`: empty input -> empty output.

Integration tests (`tests/feed_cli.rs`, fixture DB):

- `T-query-filter`: favourites only (E-30 and the favourite filter), `read = 0` only, `dateUpload > after` strict, and `date_fetch > date_added`.
- `T-order-deterministic` (E-4).
- `T-limit-before-dedupe` (E-5).
- `T-grid-per-instance` (E-2).
- `T-window-per-call` (E-3).
- `T-lock-flag-only` (E-8).
- `T-lock-no-query` (locked: the query function is not called; test by making the fixture DB lack the view and asserting exit 0 with `locked: true`).
- `T-lock-swap` (E-22).
- `T-prefs-error-closed` (prefs store returns an error: exit 4, no items printed).
- `T-cover-missing` / `T-cover-present` / `T-local-cover` (E-27).
- `T-thumb-scale` (E-10), `T-thumb-radius` (E-19/E-20), `T-thumb-atomic` (E-35) - these run the thumbnail renderer under `xvfb-run` or a headless GdkPixbuf test, since GdkPixbuf does not need a display.
- `T-title-escaping` (E-28): JSON output parsed back with `serde_json`.
- `T-unfavorite` (E-29).
- `T-cli-readonly` (CLI never creates the database): path absent before, still absent after an error run.
- `T-cli-no-gtk` (E-12 related): run the built binary with `DISPLAY` and `WAYLAND_DISPLAY` unset; expect exit 0 and valid JSON.
- `T-cli-exit-codes`: usage (2), missing DB (3), prefs (4).
- `T-json-golden`: golden files for (a) normal feed with 3 items, (b) locked, (c) empty, (d) error envelope. The `--now-ms` flag fixes time; the cover cache is a temporary directory.
- `T-query-plan` (E-9): `EXPLAIN QUERY PLAN` on the 50 000-row fixture; the test prints the plan and fails only if a full scan of `chapters` appears.
- `T-no-blocking-on-covers` (E-11): 100 entries without thumbnails finish in under 50 ms.
- `T-cli-during-write` (E-34).
- `T-poll-recovers` (E-6).
- `T-cell-targets` (E-13) and `T-applet-binary` (E-12): string checks on the QML and `.desktop` files.
- `T-applet-names` (E-15).
- `T-empty-text-key` (E-24).
- `T-poll-query-count` (E-25): one SQL statement per call (count with `rusqlite` trace hook).
- `T-window-dst` (E-26): `TZ=Europe/Berlin` with the DST date.
- `T-cli-no-network` (E-7): no socket is opened during a CLI call (run under `strace -f -e trace=network` in CI; skipped when `strace` is missing).

Performance (ignored by default, run with `cargo test -- --ignored`):

- `T-perf-500`: 500 unread rows, 3 entries, CLI returns in under 50 ms on the reference machine.

GUI and applet checks (manual, scripted where possible):

- `plasmoidviewer -a org.kurobayomi.updates.manga` shows: loading, then content, then empty (with an empty DB), then locked (with the flag set), then error (with the binary renamed).
- Resize the applet from 1x1 to 10x10 cells; no overlap, no clipped covers beyond the grid.
- Keyboard: Tab through covers; Enter opens the entry.
- Hover shows the title tooltip after 500 ms.
- Dark and light Plasma colour schemes: placeholder and text readable.

## 4. Fixtures needed

| Fixture | Contents | Used by |
|---|---|---|
| `fixtures/feed-basic.sql` | 3 favourite manga with 2 unread chapters each; 1 non-favourite with unread; 1 favourite whose chapter has `date_fetch == date_added`; 1 chapter older than the window; 1 read chapter | query filter, dedupe, window tests |
| `fixtures/feed-500.sql` | 600 unread chapters across 3 manga, dates spread over 4 months | E-5, perf |
| `fixtures/feed-100.sql` | 100 favourites, no thumbnails | E-11 |
| `fixtures/feed-anime.sql` | 2 anime with unseen episodes; 1 with `seen = 1` | anime kind tests |
| `fixtures/feed-50k.sql` | 50 000 chapters over 500 manga | E-9 query plan |
| `fixtures/feed-unicode.sql` | titles with `"`, `\n`, `‮`, emoji, CJK | E-28 |
| `fixtures/covers/` | 3 small PNG covers (58 x 87 source ratio), one `file://` local cover | cover tests |
| `fixtures/prefs-locked.toml` (or the area:02-preferences-system store format) | `use_biometric_lock = true` | lock tests |
| `fixtures/prefs-open.toml` | `use_biometric_lock = false` | lock tests |
| `fixtures/prefs-broken` | unreadable store | E-prefs error test |
| `tests/golden/feed-normal.json`, `feed-locked.json`, `feed-empty.json`, `error-db-missing.json` | golden outputs | `T-json-golden` |

## 5. Race conditions

- R-1 Lock toggled between the CLI reading the flag and the query: the CLI reads the flag first and returns early when locked, so a lock turned on mid-call still returns the feed if the flag read was before the change. Acceptable: the next poll (60 s) corrects it. Documented.
- R-2 DB updated between the count and the list (if the count is added for the badge): counts can differ by one. Badge is refreshed on the next poll; no correctness impact.
- R-3 Two applets (manga and anime) polling at the same time: both open read-only connections; WAL allows it.
- R-4 Applet removed while the CLI is running: the executable engine's `onNewData` fires for a source the applet no longer needs; the handler checks `root` is alive (QML object destroyed means no update). No crash expected; verify in the manual checks.
- R-5 Thumbnail renamed while the applet displays it: the QML `Image` already decoded the old file; the next poll shows the new path. The old file is not deleted while referenced (the GTK side deletes only files older than 7 days, owned by area:34-images-covers-cache).

## 6. Things deliberately not tested

- Android Glance rendering on device: dropped surface.
- Samsung cover-screen metadata: dropped surface.
- Coil fetch behaviour: owned by area:34-images-covers-cache and its tests.
- Material You dynamic colours: dropped.
