# 50-android-widgets — Part 1 Rust design

Target: `/home/blackbox/kurobayomi` (single crate `kurobayomi`, edition 2024, `gtk = gtk4 0.11.5` with feature `v4_16`, `serde`, `serde_json`; entry `src/main.rs:12` `fn main() -> glib::ExitCode` creating `gtk::Application` id `io.github.blackbox.Kurobayomi`).

Scope recap: the Android widgets are dropped as Android hosts. What survives is the data rule (window, unread filter, dedupe, cap), the grid geometry, the lock rule, and the click routing. They are delivered as:

1. a headless CLI mode of the same binary (`kurobayomi --updates-json ...`), and
2. two Plasma 6 applets (`org.kurobayomi.updates.manga`, `org.kurobayomi.updates.anime`) that run the CLI on a timer, plus
3. a `.desktop` action (`Updates`) for launcher menus on any desktop.

No Glance, no Compose, no tokio in the CLI path.

## 1. Architecture

```
                 +-------------------+
 SQLite (WAL) -->| src/feed/query.rs |--+
                 +-------------------+  |   src/feed/mod.rs
 prefs store  -->| src/feed/lock.rs  |--+--> build_envelope() --> src/feed/json.rs --> stdout
                 +-------------------+  |         ^
 now (chrono) -->| src/feed/window.rs|--+         |
                 +-------------------+            |
 argv         -->| src/cli.rs  (parse args) ------+
                 +-------------------+

 In-app GTK:  Updates screen (area:42-ui-updates-history-stats-storage-upcoming) and the cover loader (area:34-images-covers-cache) call the same
              query.rs / cover.rs functions and write thumbnails into the widget cache.
 Applet (QML): Plasma5Support executable engine runs the CLI every 60 s; renders JSON.
```

The CLI and the GTK app share the crate; the CLI path returns before GTK initialises, so it works without a display.

## 2. Module layout (PROPOSAL)

| File | Responsibility | Replaces (Android) |
|---|---|---|
| `src/feed/mod.rs` | `Kind`, `UpdateRow`, `dedupe_and_cap`, `build_envelope` | `BaseMangaUpdatesGridGlanceWidget.prepareData` (+ anime twin) |
| `src/feed/window.rs` | `window_start_ms(now)` (now minus 3 calendar months) | `DateLimit` companion |
| `src/feed/grid.rs` | `CELL_W_DP`, `CELL_H_DP`, `MAX_ROWS`, `MAX_COLS`, `rows_cols` | `GlanceUtils.kt:calculateRowAndColumnCount` |
| `src/feed/cover.rs` | cover size and radius constants; cache-path lookup (delegates the key scheme to area:34-images-covers-cache) | `UpdatesMangaCover.kt` constants, `appwidget_*_radius` dimens, Coil keying |
| `src/feed/query.rs` | `unread_updates(conn, kind, after_ms, limit)` | `getUpdatesByReadStatus`, `getUpdatesBySeenStatus`, repository mappers |
| `src/feed/lock.rs` | `is_locked(prefs)` for `use_biometric_lock` | `SecurityPreferences.useAuthenticator()` |
| `src/feed/json.rs` | serde output types (schema 1) and the error envelope | Compose/Glance rendering of the data |
| `src/cli.rs` | argv parsing, exit codes, stdout/stderr | `Class.forName(MAIN_ACTIVITY)` intent extras (replaced by `--open`) |
| `dist/plasmoid/org.kurobayomi.updates.manga/` | Plasma applet package (QML) | `UpdatesMangaWidget.kt`, `UpdatesMangaCover.kt`, `LockedMangaWidget.kt`, provider classes |
| `dist/plasmoid/org.kurobayomi.updates.anime/` | anime applet package | anime twins |
| `dist/kurobayomi.desktop` | `.desktop` file with the `Updates` action | `app/shortcuts.xml` `show_recently_updated` |
| `tests/feed_cli.rs` | golden tests against a fixture database | (new; see the edge-cases file) |

`src/feed/` is a directory module. Nothing outside `src/feed/` and `src/cli.rs` should depend on rusqlite directly; GTK code uses `feed::query` through `feed::mod`.

## 3. Key types and signatures

```rust
// src/feed/mod.rs
pub mod cover;
pub mod grid;
pub mod json;
pub mod lock;
pub mod query;
pub mod window;

use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Manga,
    Anime,
}

impl Kind {
    /// Value written to JSON and used as the `--open` prefix.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Manga => "manga",
            Kind::Anime => "anime",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "manga" => Some(Kind::Manga),
            "anime" => Some(Kind::Anime),
            _ => None,
        }
    }
}

/// One unread chapter or episode, joined with its entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateRow {
    pub entry_id: i64,           // mangaId or animeId
    pub update_id: i64,          // chapterId or episodeId
    pub title: String,           // mangaTitle or animeTitle (tooltip only)
    pub source_id: i64,
    pub thumbnail_url: Option<String>,
    pub cover_last_modified: i64,
    pub date_fetch_ms: i64,
}

/// Keeps the first row per entry (input order), stops at `cap` rows.
/// Rows whose cover is missing are kept (they render as placeholders), as on Android.
pub fn dedupe_and_cap(rows: impl IntoIterator<Item = UpdateRow>, cap: usize) -> Vec<UpdateRow> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(cap.min(100));
    for row in rows {
        if out.len() == cap {
            break;
        }
        if seen.insert(row.entry_id) {
            out.push(row);
        }
    }
    out
}
```

```rust
// src/feed/window.rs
use chrono::{DateTime, Local, Months};

/// Lower bound (exclusive) for `dateUpload`, in epoch milliseconds.
/// Android: ZonedDateTime.now().minusMonths(3) in the system zone.
pub fn window_start_ms(now: DateTime<Local>) -> i64 {
    now.checked_sub_months(Months::new(3))
        .expect("three months before now is always representable")
        .timestamp_millis()
}
```

```rust
// src/feed/grid.rs
pub const CELL_W_DP: f32 = 64.0; // cover 58 + 2 x 3 horizontal padding
pub const CELL_H_DP: f32 = 95.0; // cover 87 + 2 x 4 vertical padding
pub const MAX_ROWS: u32 = 10;
pub const MAX_COLS: u32 = 10;

/// Same arithmetic as GlanceUtils.kt:calculateRowAndColumnCount.
/// `as i32` truncates toward zero like Kotlin's Float.toInt().
pub fn rows_cols(width: f32, height: f32, pad_top: f32, pad_bottom: f32) -> (u32, u32) {
    let h = height - pad_top - pad_bottom;
    let rows = ((h / CELL_H_DP) as i32).clamp(1, MAX_ROWS as i32) as u32;
    let cols = ((width / CELL_W_DP) as i32).clamp(1, MAX_COLS as i32) as u32;
    (rows, cols)
}
```

```rust
// src/feed/cover.rs
pub const COVER_W_DP: f32 = 58.0;
pub const COVER_H_DP: f32 = 87.0;
pub const INNER_RADIUS_DP: f32 = 12.0; // appwidget_inner_radius
pub const CARD_RADIUS_DP: f32 = 16.0;  // appwidget_background_radius (fixed, no system value)

use std::path::{Path, PathBuf};

/// Path of the rendered 58x87 thumbnail, or None when it has not been rendered.
/// The key scheme is owned by area:34-images-covers-cache; this function only resolves it.
pub fn cached_thumbnail(cache_dir: &Path, thumbnail_url: &str, cover_last_modified: i64, device_scale: u32) -> Option<PathBuf>;
```

```rust
// src/feed/query.rs
use rusqlite::Connection;
use super::{Kind, UpdateRow};

/// Unread rows newer than `after_ms`, newest fetch first.
/// `limit` is applied in SQL before dedupe (Android uses 500).
pub fn unread_updates(conn: &Connection, kind: Kind, after_ms: i64, limit: i64) -> rusqlite::Result<Vec<UpdateRow>>;
```

```rust
// src/feed/lock.rs
use crate::prefs::PrefsReader; // trait owned by area:02-preferences-system (src/prefs.rs); see section 7
use super::FeedError;

/// `use_biometric_lock`, default false. Missing key = false.
/// A read error from the store maps to FeedError::Prefs (fail closed, see section 12).
pub fn is_locked(prefs: &dyn PrefsReader) -> Result<bool, FeedError> {
    prefs.get_bool("use_biometric_lock", false).map_err(FeedError::Prefs)
}
```

```rust
// src/feed/json.rs
use serde::Serialize;

#[derive(Serialize)]
pub struct Envelope {
    pub schema: u32,               // 1
    pub generated_at_ms: i64,
    pub kind: &'static str,        // "manga" | "anime"
    pub locked: bool,
    pub grid: GridOut,
    pub window_after_ms: i64,
    pub items: Vec<ItemOut>,
}

#[derive(Serialize)]
pub struct GridOut { pub rows: u32, pub cols: u32, pub cap: u32 }

#[derive(Serialize)]
pub struct ItemOut {
    pub entry_id: i64,
    pub kind: &'static str,
    pub title: String,
    pub unread_update_id: i64,
    pub cover_path: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorOut<'a> { pub schema: u32, pub error: &'a str, pub code: &'a str }
```

```rust
// src/feed/mod.rs (continued): the one function both CLI and GTK call
#[derive(Debug)]
pub enum FeedError {
    Db(rusqlite::Error),
    Prefs(String),
    Usage(String),
}

impl From<rusqlite::Error> for FeedError {
    fn from(e: rusqlite::Error) -> Self { FeedError::Db(e) }
}

pub struct Request<'a> {
    pub kind: Kind,
    pub rows: u32,
    pub cols: u32,
    pub now: chrono::DateTime<chrono::Local>,
    pub cover_cache: &'a std::path::Path,
    pub device_scale: u32,
}

/// Builds the schema-1 envelope. When locked, returns an envelope with `items: []`
/// and does not run the update query at all (no titles or covers are read).
pub fn build_envelope(conn: &Connection, prefs: &dyn crate::prefs::PrefsReader, req: &Request) -> Result<json::Envelope, FeedError>;
```

Rule in `build_envelope`: check the lock FIRST and return early. Reading the query first and discarding it would still load titles into memory, so the early return is part of the contract, not an optimisation.

```rust
// src/cli.rs
/// Parses `--updates-json --kind <manga|anime> --rows <1..10> --cols <1..10>
///         [--db <path>] [--cover-cache <dir>] [--device-scale <1..4>] [--now-ms <i64>]`.
/// Returns the process exit code: 0 ok, 2 usage, 3 database, 4 prefs.
/// Writes the JSON envelope (or error envelope) to stdout, diagnostics to stderr.
pub fn run(args: &[String]) -> u8;
```

Defaults: `--rows 2 --cols 4` (applet passes its own values), `--device-scale 2`, `--db` = the app's default database path (area:03-db-manga-schema-migrations), `--cover-cache` = `$XDG_CACHE_HOME/kurobayomi/widget-covers` (area:34-images-covers-cache path), `--now-ms` absent = wall clock. `--now-ms` is an undocumented test hook used by golden tests; keep it, it costs one line.

Argument parsing: plain `std::env::args`, no clap (one flag family, nothing speculative).

## 4. `main.rs` integration (PROPOSAL)

Current `src/main.rs:12-20` builds the GTK application directly. The change is a single early branch, before any GTK call:

```rust
fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--updates-json") {
        return glib::ExitCode::from(cli::run(&args));
    }
    // ... existing GTK application code unchanged ...
}
```

Add `mod cli; mod feed;` next to the existing `mod` lines. `--open <kind>:<id>` and `--open updates` are handled by the GApplication command-line path (area:36-ui-shell-navigation-security-webview owns the parser); the applet and the `.desktop` action call the same binary.

## 5. Android API to Linux mapping

| Android / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| Glance `GlanceAppWidget` + `provideGlance` | Plasma 6 `PlasmoidItem` in QML (`dist/plasmoid/*/contents/ui/main.qml`) | KDE only. GNOME has no equivalent without a Shell extension (dropped there). |
| `GlanceAppWidgetManager.getAppWidgetSizes` / `LocalSize` | `root.width`, `root.height` of the applet | The applet sizes its own grid, so no max-area guess. |
| `GlanceAppWidgetReceiver` + manifest `<receiver>` | Plasma package `metadata.json` (`"KPackageStructure": "Plasma/Applet"`) | Installed with `kpackagetool6 --type Plasma/Applet --install`. Verify the key names against the installed Plasma version. |
| `updateAll(context)` | QML `Timer` (60 s, `triggeredOnStart: true`) + click on the header to refresh | No push channel. |
| `MangaWidgetManager` / `AnimeWidgetManager` (`combine` + `distinctUntilChanged`) | Dropped; the poll re-reads the same data every 60 s | Latency up to 60 s instead of immediate. |
| `actionStartActivity(intent)` with `SHOW_MANGA` extra | `Plasma5Support.DataSource { engine: "executable" }` runs `kurobayomi --open manga:<id>` | GApplication forwards it to the running instance. |
| `ImageRequest` + Coil `executeBlocking` | Cache-only read: `thumbnail` PNG path from `cover::cached_thumbnail`, rendered by QML `Image` | The CLI never downloads. |
| `RoundedCornersTransformation` (API < 31) / `cornerRadius` | Alpha mask baked into the PNG thumbnail | One code path, no QML layer. |
| `Precision.EXACT`, `Scale.FILL` + `ContentScale.Crop` | Centre-crop to 58:87 in the thumbnail renderer (GdkPixbuf) | |
| `MR.strings.*` (`information_no_recent`, ...) | QML `i18nd("kurobayomi", "No recent updates")` with the same English text | Translation catalogues belong to area:51-i18n. |
| `Material You` dynamic colours (`m3_sys_color_dynamic_*`) | Plasma colour scheme via `Kirigami.Theme.backgroundColor`, `Kirigami.Theme.textColor`, `Kirigami.Theme.highlightColor` | Follows the user's active scheme (Tokyo Night per the user's setup). |
| `SecurityPreferences.useAuthenticator()` | `is_locked(prefs)` reading `use_biometric_lock` (area:02-preferences-system store) | Same key, same default. |
| `logcat(LogPriority.ERROR, e)` | `eprintln!` in the CLI; the logger chosen by area:01-foundation-app-bootstrap in the app | CLI errors also go to the stdout envelope. |
| `ProcessLifecycleOwner.lifecycleScope` | Applet lifetime (plasmashell) | |
| `Dispatchers.IO` / `Default` | Synchronous rusqlite in the CLI (one short query per poll); GTK side uses `gio::spawn_blocking` for thumbnail IO | |
| `DensityExtensions.dpToPx` (`Resources.getSystem()` density) | `device_scale` argument (default 2) | No global density lookup. |
| `R.dimen.appwidget_*_radius` and `values-v31` system radius | `CARD_RADIUS_DP = 16.0`, `INNER_RADIUS_DP = 12.0` constants | System value dropped. |
| `consumerProguardFiles`, R8 | none | |

## 6. Async and threading model

- CLI: single-threaded, synchronous. Open the database read-only, run one query, serialize, exit. Target total time under 50 ms with 500 rows.
- Applet: QML only. The executable engine is asynchronous; the `onNewData` handler parses stdout and updates the model. Disconnect the source after each result (`disconnectSource`) so the engine does not keep old commands alive.
- GTK app: thumbnail rendering runs in `gio::spawn_blocking` (IO and decode off the main loop); results return through `glib::MainContext` futures. No tokio is needed for this area.
- Database: the app keeps its own connection(s) (area:03-db-manga-schema-migrations). The CLI opens a separate read-only connection. WAL allows concurrent readers; the CLI sets `busy_timeout` (proposal: 250 ms) and treats `SQLITE_BUSY` as exit code 3.

## 7. Preferences reader (contract with area:02-preferences-system)

This area needs one read: a Boolean by key with a default. Proposal: area:02-preferences-system exposes

```rust
pub trait PrefsReader {
    fn get_bool(&self, key: &str, default: bool) -> Result<bool, String>;
}
```

and a constructor that opens the on-disk store read-only from the CLI. If area:02-preferences-system chooses a different storage format, only `lock.rs` changes. The key name `use_biometric_lock` is fixed (it is the Android key and is kept so a future migration is trivial).

## 8. Cover thumbnails (contract with area:34-images-covers-cache)

- The GTK app renders thumbnails when covers are cached (area:34-images-covers-cache decides when). Renderer: GdkPixbuf (already a dependency through gtk4), no extra crate.
- Steps: load the cached cover file; scale so the cover covers the 58x87 x `device_scale` box; centre-crop; apply a rounded-rect alpha mask (radius 12 x `device_scale`); save PNG to `<cover-cache>/<key>.png` via a temporary name and `rename` (atomic, so the CLI never reads a partial file).
- The CLI only checks `Path::exists` and reports the path. A missing file is `cover_path: null`.
- No WebP encoder dependency. PNG is larger but the thumbnails are small (116x174 px at scale 2).

Pitfall: GdkPixbuf `add_alpha` followed by per-pixel alpha writes is O(w x h) on the main thread if called there. Keep it inside `spawn_blocking`.

## 9. Applet skeleton (PROPOSAL, `dist/plasmoid/org.kurobayomi.updates.manga/contents/ui/main.qml`)

```qml
import QtQuick
import org.kde.plasma.plasmoid
import org.kde.plasma.plasma5support as Plasma5Support
import org.kde.kirigami as Kirigami

PlasmoidItem {
    id: root
    readonly property string kind: Plasmoid.metaData.pluginId.endsWith(".anime") ? "anime" : "manga"
    readonly property int cellW: 64
    readonly property int cellH: 95
    readonly property int rows: Math.max(1, Math.min(10, Math.floor(height / cellH)))
    readonly property int cols: Math.max(1, Math.min(10, Math.floor(width / cellW)))
    property var payload: ({ locked: false, items: [], schema: 0 })
    property string errorText: ""

    function refresh() {
        executable.connectSource("kurobayomi --updates-json --kind " + kind
                                 + " --rows " + rows + " --cols " + cols);
    }

    Plasma5Support.DataSource {
        id: executable
        engine: "executable"
        onNewData: (source, data) => {
            executable.disconnectSource(source);
            if (data["exit code"] !== 0) { root.errorText = data["stderr"]; return; }
            root.payload = JSON.parse(data["stdout"]);
            root.errorText = "";
        }
    }

    Timer {
        interval: 60000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: root.refresh()
    }
}
```

The full UI (grid of `Image` cells, placeholders, locked and empty text, click handlers that run `kurobayomi --open <kind>:<id>`) is specified in `p1-ui-parity.md`. Keep `rows`/`cols` changes debounced: `onRowsChanged` / `onColsChanged` call `refresh()` once after a resize settles (proposal: 300 ms).

Note: `data["exit code"]` and `data["stdout"]` are the executable engine's keys; verify against the Plasma version in use.

## 10. Click routing

- Applet cell click: `executable.connectSource("kurobayomi --open " + kind + ":" + entryId)`.
- `.desktop` action: `Exec=kurobayomi --open updates`.
- Handling: the running instance (GApplication) receives the command line. `manga:<id>` opens the Library tab at the entry (equivalent of `HomeScreen.Tab.Library(id)`), `anime:<id>` opens the anime library, `updates` opens the Updates screen. Unknown targets are ignored and logged; the window is still raised. The parser and its tests are owned by area:36-ui-shell-navigation-security-webview.
- If no instance is running, `--open` starts the app and then applies the target (same GApplication path).

## 11. `.desktop` file (PROPOSAL, `dist/kurobayomi.desktop`, the part owned by this area)

```ini
[Desktop Entry]
Name=Kurobayomi
Exec=kurobayomi %U
Icon=io.github.blackbox.Kurobayomi
Type=Application
Categories=AudioVideo;Graphics;
Actions=Updates;

[Desktop Action Updates]
Name=Updates
Exec=kurobayomi --open updates
```

The main entry and the other fields belong to area:01-foundation-app-bootstrap packaging; this area adds only the `Actions=` line and the `[Desktop Action Updates]` group.

## 12. Error handling

- `FeedError` variants: `Db`, `Prefs`, `Usage`. No `anyhow`, no `thiserror` (three variants, hand-written `Display` is shorter than a derive plus a dependency).
- CLI maps: `Usage` -> exit 2, `Db` -> exit 3, `Prefs` -> exit 4. Each writes `ErrorOut` to stdout and a human line to stderr.
- A failure to read the lock flag is an error (exit 4), not "unlocked". Failing open would show titles while the user expects the lock.
- Missing cover file is not an error (`cover_path: null`).
- Database missing (file not found) is exit 3 with code `db_missing`. The CLI must not create the database (`SQLITE_OPEN_READ_ONLY` without `SQLITE_OPEN_CREATE`).

## 13. Crates

| Crate | Use | Why this one |
|---|---|---|
| `rusqlite` | queries (read-only in CLI) | the planned DB layer (area:03-db-manga-schema-migrations); `bundled` feature decided there |
| `chrono` | local time and calendar-month subtraction | `checked_sub_months` clamps month ends like `java.time` |
| `serde` + `serde_json` | envelope output | already in the mockup `Cargo.toml` |
| `gtk` (gtk4 0.11.5) | GdkPixbuf thumbnail renderer and the app | already a dependency |

Not used: `clap` (argument surface is four flags), `anyhow`/`thiserror` (see section 12), `image` (GdkPixbuf covers it), `sha2` (key scheme belongs to area:34-images-covers-cache), `tokio` (CLI is synchronous).

## 14. Pitfalls

- The CLI must not link or initialise GTK on the early branch. `main` returns before `gtk::Application::builder()`. Check with `env -u DISPLAY -u WAYLAND_DISPLAY kurobayomi --updates-json ...`.
- The applet's `height`/`width` are the applet's current size; during a resize the values change many times. Debounce (section 9).
- Plasma applets run with the user's session; a relative `kurobayomi` depends on `PATH`. The installed binary path must be in the default `PATH` (Arch: `/usr/bin/kurobayomi`). Document it.
- Flatpak builds: the CLI must see the same database and cache paths as the app; use `--filesystem=xdg-data/kurobayomi` (area:01-foundation-app-bootstrap) and document that the applet needs the binary on the host.
- `json.rs` field names are a contract with the QML. Changing one bumps `schema`.
- Sorting: `ORDER BY date_fetch DESC` is explicit in our query (Android relies on the view's order, see the behaviour file E-4). Result order is therefore defined, which differs from Android only where Android was undefined.
- Title strings can contain quotes and newlines; `serde_json` escapes them, and QML uses the parsed object, never string concatenation.
- The window is recomputed on every CLI call, so the applet's view slides without a restart (Android only recomputed per render; the manager kept the start-of-process bound).

## 15. How this plugs into the mockup

- `src/main.rs`: add the early branch (section 4) and the `mod` lines. Nothing else changes in `main`.
- `src/screens.rs:277` `fn updates()`: the Updates screen (area:42-ui-updates-history-stats-storage-upcoming) reads `feed::query::unread_updates` with the same window as the applet. Its anime/manga switch maps to `Kind`.
- `src/ui.rs:175`: the sidebar badge is currently a literal `"8"` for `"updates"`. Proposal: the badge = number of unread rows in the window for both kinds (Android: "Updates icon shows a numeric Badge = new anime + new manga updates count", `spec/app.json` Updates notes). The count query is `SELECT COUNT(*)` with the same filters; add it to `feed::query` (one function, no new file).
- `spec/app.json`: no widget entries exist. The applets have no spec screens; they are described here only.

## 16. Open questions for the author

- Q-1: the lock check is the flag, not the in-process unlock state (Android behaviour). Should the CLI show data when the user has unlocked the app in this session? That needs a shared unlock marker (area:36-ui-shell-navigation-security-webview). Default in this plan: keep Android behaviour (locked while the flag is on).
- Q-2: poll (60 s) vs push over D-Bus. Push would need a running daemon. Default: poll, because the applet must work when the main window is closed without a background service.
- Q-3: GNOME users get only the `.desktop` action. Acceptable for this plan; a GNOME Shell extension is out of scope.
