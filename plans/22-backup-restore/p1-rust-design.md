# 22-backup-restore — Part 1 Rust design (Kurobayomi)

Target: `/home/blackbox/kurobayomi` (crate `kurobayomi`, edition 2024, `gtk4` 0.11.5 with `v4_16`, `serde`, `serde_json`). The existing mockup has `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/settings.rs`, `src/spec.rs`, `src/entry.rs`, `src/player.rs`, `src/clamp.rs`; no backup code exists yet.

Decision summary:
- Restore runs on one dedicated OS thread (`std::thread`), not on the GTK main loop. All database work is synchronous (rusqlite), so tokio is not needed for restore. Tokio stays available for network code owned by other areas.
- One SQLite connection owned by the restore thread for the duration of the job. Each entry is one `rusqlite::Transaction` (Immediate behaviour).
- Progress and log events flow over `async_channel` to the GTK main context via `glib::spawn_future_local`.
- Cancellation is an `Arc<AtomicBool>` checked before every entry and section (replaces WorkManager cancel + `ensureActive`).
- Single-job guard: an app-level `Mutex<Option<RunningRestore>>` (replaces `ExistingWorkPolicy.KEEP`).
- Notifications use GIO `gio::Notification` through `gtk::Application::send_notification` (no progress bar in GNotification; progress text is in the body and an in-window banner shows the bar).

---

## 1. Module layout

```
src/
  backup/
    mod.rs              pub use + module docs
    options.rs          RestoreOptions (7 bools), labels, defaults, bit array (de)serialization
    format.rs           (owned by area:21) protobuf structs for Backup/LegacyBackup/...; gzip sniff; decode()
    validate.rs         ValidationResult { missing_sources, missing_trackers }; validate(path) -> Result<..>
    error_log.rs        ErrorLog { entries: Vec<(DateTime<Local>, String)> }; write(path) -> Option<PathBuf>
    restore/
      mod.rs            RestoreRequest, RestoreEvent, RestoreSummary, RestoreError, run(); section order
      ctx.rs            RestoreCtx { conn, cancel, events, amount, progress, now, fetch_window, source_names }
      categories.rs     restore_categories(tx, backup_list, kind) (manga and anime share the code via Kind)
      manga.rs          restore_manga_entry(tx, ctx, entry, backup_categories)
      anime.rs          restore_anime_entry(tx, ctx, entry, backup_categories, seasons)
      merge.rs          Chapter/Episode merge rules, Track merge rules (shared helpers)
      prefs.rs          app + source preferences restore, category id remap
      extensions.rs     anime stores, manga repos, custom buttons, APK skip counter
    job.rs              RunningRestore, JobSlot (single job guard), start()/stop()
    ui/
      restore_page.rs   GTK page (options, validation card, action bar)
      create_page.rs    GTK page (create options, save dialog)   [owned by area:21 UI pieces; listed here for the screen]
      progress.rs       in-window progress banner (Revealer + ProgressBar + Cancel)
  metadata/
    mod.rs
    comicinfo.rs        ComicInfo model + quick-xml (de)serialize + copy mappings
    details.rs          MangaDetails / AnimeDetails / ChapterDetails / EpisodeDetails (serde)
  storage_info.rs       StorageInfo rows (statvfs) for the storage section
```

Changes to existing mockup files:
- `src/main.rs`: register app actions `cancel-restore` and `open-error-log` (see §8); create the `JobSlot` and store it in the application state (`Rc`/`glib` object data on the `gtk::Application`).
- `src/settings.rs`: `body()` gets no new arm for `settings_restore_backup`, which already exists in the spec. The restore flow starts from the "Restore backup" action row, not from a page id.
- `src/ui.rs`: the spec action row "Restore backup" (`spec/settings.json` line 2412, no `target`) must start the file dialog. Wire it where action rows are dispatched (`resolve`, `src/ui.rs:437`; verify the exact dispatch point when implementing). Do not change `Item` in `src/spec.rs`.
- `spec/settings.json`: no content change required for restore; the notes text for `settings_restore_backup` (line 2663) is already the spec for the validation card.

---

## 2. Crates

| Crate | Version policy | Why | Replaces (Android/Kotlin) |
|---|---|---|---|
| `rusqlite` (feature `bundled` optional) | pin when added | Synchronous SQLite with transactions; schema owned by areas 03/04 | SQLDelight `AndroidMangaDatabaseHandler`, `AndroidAnimeDatabaseHandler` |
| `prost` | pin when added | Protobuf decode of the wire format; hand-written structs (no build script) | kotlinx `ProtoBuf` |
| `flate2` | pin when added | gzip (`GzDecoder`) | `okio.gzip()` |
| `quick-xml` (feature `serialize`) | pin when added | ComicInfo.xml read/write | `nl.adaptivity.xmlutil` |
| `chrono` | pin when added | Local date/time, start-of-day windows, `%Y-%m-%d %H:%M:%S%.3f` formatting | `ZonedDateTime`, `SimpleDateFormat` |
| `async-channel` | pin when added | Restore thread -> GTK main context | (Kotlin `Flow`/WorkManager progress) |
| `thiserror` | pin when added | Typed `RestoreError` | exceptions |
| `log` | pin when added | Log facade for `Log.e`-equivalent | `android.util.Log`, `logcat` |
| `directories` or `glib::user_cache_dir()` | prefer `glib::user_cache_dir()` (no extra crate) | error log location | `externalCacheDir` |
| `nix` (feature `fs`) or `rustix` | prefer `rustix::fs::statvfs` | free/total bytes for StorageInfo | `StatFs` |

Not used: tokio (not needed for restore), libadwaita (forbidden by the brief), any Android crate.

GTK/GLib/GIO APIs (no extra crate): `gtk::FileDialog` (open/save), `gtk::UriLauncher` (open the error log), `gtk::AlertDialog`, `gio::Notification`, `gtk::Application::send_notification` / `withdraw_notification`, `glib::format_size` (SI sizes), `glib::spawn_future_local`, `glib::user_cache_dir`.

---

## 3. Types and signatures

### 3.1 Options

```rust
// src/backup/options.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestoreOptions {
    pub library_entries: bool,   // default true
    pub categories: bool,        // default true
    pub app_settings: bool,      // default true
    pub extension_stores: bool,  // default true
    pub custom_buttons: bool,    // default true
    pub source_settings: bool,   // default true
    pub extensions: bool,        // default false (APK; skipped on Linux)
}

impl Default for RestoreOptions { /* values above */ }

impl RestoreOptions {
    pub const LABELS: [&'static str; 7] = [
        "Library", "Categories", "App settings", "Extension stores",
        "Custom buttons", "Source settings", "Extensions",
    ];
    pub fn to_bits(self) -> [bool; 7];            // index order = field order above
    pub fn from_bits(b: [bool; 7]) -> Self;
    pub fn can_restore(self) -> bool;              // any field true
    pub fn set(&mut self, index: usize, value: bool); // checkbox handler
}
```

Labels are `MR.strings.label_library`, `categories`, `app_settings`, `extensionStores`, `AYMR.strings.custom_button_settings`, `source_settings`, `label_extensions`. Keep the English text as the Kurobayomi key values.

### 3.2 Request, events, summary, errors

```rust
// src/backup/restore/mod.rs
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RestoreRequest {
    pub path: PathBuf,                 // selected .tachibk (any name; content is sniffed)
    pub options: RestoreOptions,
}

#[derive(Debug, Clone)]
pub enum RestoreEvent {
    /// Emitted after each completed item or section. `done` counts completed steps; `total` is the fixed amount.
    Progress { title: String, done: u32, total: u32 },
    /// Emitted for each collected entry error (same text as the log line, without timestamp).
    EntryError { message: String },
    /// Final: completed normally (may have errors).
    Finished(RestoreSummary),
    /// Cancel was requested and honoured.
    Canceled { error_log: Option<PathBuf> },
    /// Decode or unexpected failure (aborted restore). `message` is user-facing text.
    Failed { message: String },
}

#[derive(Debug, Clone)]
pub struct RestoreSummary {
    pub elapsed: Duration,
    pub error_count: usize,
    pub error_log: Option<PathBuf>,   // Some only when error_count > 0 and the write succeeded
    pub skipped_apk_extensions: usize,
    pub skipped_unknown_prefs: usize, // PROPOSAL (5.5 in behaviour)
}

#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("{0}")]
    Decode(String),                   // maps to invalid_backup_file_* texts
    #[error("Canceled restore")]
    Cancelled,
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Io(String),
}

/// Runs the full restore on the calling thread. Call from a dedicated `std::thread`.
pub fn run(
    db_path: &std::path::Path,
    req: &RestoreRequest,
    cancel: &std::sync::atomic::AtomicBool,
    events: &async_channel::Sender<RestoreEvent>,
) -> Result<RestoreSummary, RestoreError>;
```

`run` is a sequence: open the connection, decode, count, run sections, write the error log, emit `Finished`, `Canceled` or `Failed`. The emit order and the rule "no error log and no Finished event on cancel" follow D17 (fix: write the log in all paths, see §6).

### 3.3 Restore context

```rust
// src/backup/restore/ctx.rs
use chrono::{DateTime, Local};

pub struct RestoreCtx<'a> {
    pub conn: &'a mut rusqlite::Connection,
    pub cancel: &'a std::sync::atomic::AtomicBool,
    pub events: &'a async_channel::Sender<RestoreEvent>,
    pub amount: u32,
    pub progress: u32,
    pub now: DateTime<Local>,          // captured once per run (as in MangaRestorer.kt:37-43)
    pub fetch_window: (i64, i64),      // epoch ms, see §4.4 of behaviour (MangaFetchInterval.getWindow)
    pub errors: crate::backup::error_log::ErrorLog,
    pub source_names: SourceNames,     // id -> name, manga and anime (fixes D1)
}

pub struct SourceNames {
    pub manga: std::collections::HashMap<i64, String>,
    pub anime: std::collections::HashMap<i64, String>,
}

impl RestoreCtx<'_> {
    /// Fails with RestoreError::Cancelled when the cancel flag is set. Call before each item.
    pub fn check_cancel(&self) -> Result<(), RestoreError>;
    /// Increments progress by `by` and emits Progress.
    pub fn advance(&mut self, title: &str, by: u32);
}
```

### 3.4 Category restore (shared for manga and anime)

```rust
// src/backup/restore/categories.rs
#[derive(Clone, Copy)]
pub enum Kind { Manga, Anime }

pub struct BackupCategory { pub name: String, pub order: i64, pub id: i64, pub flags: i64 }

/// Returns the list of (backup order, db category id) pairs used by entries.
pub fn restore_categories(
    tx: &rusqlite::Transaction<'_>,
    kind: Kind,
    backup: &[BackupCategory],
) -> rusqlite::Result<std::collections::HashMap<i64 /*backup order*/, i64 /*db id*/>>;
```
Returns the order-to-id map so the entry code does not query again. Implements behaviour §4.2 exactly, including `categorized_display` (see §5 for preferences).

### 3.5 Entry restore

```rust
// src/backup/restore/manga.rs
pub struct MangaEntry { /* fields of format::BackupManga, see p1-data-and-config.md §2.1 */ }

pub fn restore_manga_entry(
    tx: &rusqlite::Transaction<'_>,
    ctx: &RestoreCtx<'_>,           // read-only parts: now, fetch_window
    entry: &MangaEntry,
    category_map: &std::collections::HashMap<i64, i64>, // backup order -> db id (empty when categories off)
) -> Result<(), RestoreError>;
```
The caller wraps the call in `tx`; on `Err` the transaction is dropped (rollback) and the error goes to `ctx.errors` as `"{title} [{source}]: {message}"`.

```rust
// src/backup/restore/anime.rs
pub fn restore_anime_entry(
    tx: &rusqlite::Transaction<'_>,
    ctx: &RestoreCtx<'_>,
    entry: &AnimeEntry,
    seasons: &[AnimeEntry],          // rows whose parentId == entry.backup_id (PROPOSAL: from one pre-pass)
    category_map: &std::collections::HashMap<i64, i64>,
    backup_to_db: &mut std::collections::HashMap<i64, i64>, // backup id -> db id (fixes D7)
) -> Result<(), RestoreError>;
```

### 3.6 Merge helpers (shared rules)

```rust
// src/backup/restore/merge.rs
pub struct ItemState {               // the fields compared by forComparison(), excluding id/parent/dates/version
    pub name: String, pub url: String, pub number: f64,
    pub scanlator: Option<String>, pub read: bool /* seen for episodes */,
    pub bookmark: bool, pub last_pos: i64,  // lastPageRead or lastSecondSeen
    pub source_order: i64,
}

/// Returns true when backup and db rows are equal under forComparison().
pub fn same_state(a: &ItemState, b: &ItemState) -> bool;

/// The chapter/episode merge rule from behaviour §4.3.5 / §4.4.3. Returns the merged state to write.
pub fn merge_existing_item(backup: &ItemState, db: &ItemState) -> ItemState;
```
Keep the behaviour exact: name, url, number, scanlator come from the DB row (copyFrom); read/seen and last position follow the two-branch rule; bookmark is OR. The episode variant also ORs `fillermark`. Implement as two small functions rather than a trait (two call sites).

### 3.7 Preferences

```rust
// src/backup/restore/prefs.rs
pub enum PrefValue { Int(i32), Long(i64), Float(f32), String(String), Bool(bool), StringSet(Vec<String>) }

pub trait PrefStore {                 // implemented by the app's settings backend (area:02)
    fn get_type_name(&self, key: &str) -> Option<&'static str>;  // "int" | "long" | "float" | "string" | "bool" | "stringset" | None
    fn set(&mut self, key: &str, value: PrefValue);
    fn append_stringset(&mut self, key: &str, ids: &[String]);  // PROPOSAL: replace instead (D12)
}

pub fn restore_app_prefs(
    store: &mut dyn PrefStore,
    prefs: &[(String, PrefValue)],
    category_map_manga: Option<&std::collections::HashMap<String, i64>>, // backup id -> db id (None when categories off)
    category_map_anime: Option<&std::collections::HashMap<String, i64>>,
) -> PrefsSummary;
```
Type gating (`get_type_name` must equal the value's type or be `None`) follows behaviour §5.2 exactly.

### 3.8 Extensions and custom buttons

```rust
// src/backup/restore/extensions.rs
pub fn restore_anime_store(tx: &rusqlite::Transaction<'_>, s: &AnimeStore) -> rusqlite::Result<()>;      // UPSERT
pub fn restore_manga_repo(tx: &rusqlite::Transaction<'_>, r: &MangaRepo) -> Result<(), RestoreError>;    // conflict errors
pub fn restore_custom_buttons(tx: &rusqlite::Transaction<'_>, buttons: &[CustomButtonBackup]) -> rusqlite::Result<()>;
```

### 3.9 Job slot

```rust
// src/backup/job.rs
use std::sync::{atomic::AtomicBool, Arc, Mutex};

pub struct RunningRestore {
    pub cancel: Arc<AtomicBool>,
    pub handle: std::thread::JoinHandle<()>,
}

#[derive(Default)]
pub struct JobSlot(Mutex<Option<RunningRestore>>);

impl JobSlot {
    /// Starts the job, or returns Err(AlreadyRunning) when one is active (PROPOSAL: visible
    /// message; Kotlin drops the second request silently with ExistingWorkPolicy.KEEP).
    pub fn start(&self, req: RestoreRequest, db_path: PathBuf, events: async_channel::Sender<RestoreEvent>)
        -> Result<(), AlreadyRunning>;
    pub fn is_running(&self) -> bool;
    /// Sets the cancel flag. The job emits Canceled and ends.
    pub fn stop(&self);
}

#[derive(Debug)] pub struct AlreadyRunning;
```
The thread clears the slot on exit (it holds a clone of the `Arc<JobSlot>` or it sends a final event that the GTK side uses to clear the slot). Choose the GTK-side clear: on `Finished`, `Canceled` or `Failed`, call `slot.clear()`. Do not rely on `JoinHandle` drop.

### 3.10 Validation

```rust
// src/backup/validate.rs
pub struct ValidationResult {
    pub missing_sources: Vec<String>,   // manga missing first, then anime, each distinct + sorted
    pub missing_trackers: Vec<String>,  // sorted
}
pub fn validate(path: &std::path::Path, sources: &dyn SourceLookup, trackers: &dyn TrackerLookup)
    -> Result<ValidationResult, RestoreError>;

pub trait SourceLookup {
    fn manga_source_exists(&self, id: i64) -> bool;
    fn anime_source_exists(&self, id: i64) -> bool;
    fn stub_name(&self, id: i64, kind: crate::backup::restore::categories::Kind) -> String; // "Name (id)" style; area:17
}
pub trait TrackerLookup {
    /// Returns Some(name) when the tracker exists and is NOT logged in; None otherwise.
    fn not_logged_in_name(&self, tracker_id: i64) -> Option<String>;
}
```
Algorithm exactly as behaviour §3.2 (names that parse as Long are replaced by `stub_name`).

### 3.11 Error log

```rust
// src/backup/error_log.rs
pub struct ErrorLog { entries: Vec<(chrono::DateTime<chrono::Local>, String)> }

impl ErrorLog {
    pub fn push(&mut self, message: String) { /* stamps Local::now() */ }
    pub fn len(&self) -> usize;
    /// Writes `<cache>/aniyomi_restore_error.txt` (overwrite). Returns None when empty or on IO error (logged).
    pub fn write(&self, dir: &std::path::Path) -> Option<std::path::PathBuf>;
}
```
Line format: `[{:%Y-%m-%d %H:%M:%S%.3f}] {message}\n` (behaviour §7.2). Location: `glib::user_cache_dir().join("kurobayomi")` with the same file name (kept for parity; it is not Android-specific). Do not swallow errors silently (D15): `log::warn!` the failure.

---

## 4. Threading model

```
GTK main thread                                  Restore thread (std::thread)
---------------                                  ----------------------------
RestorePage "Restore" click
  -> JobSlot::start(req, db_path, tx_events)  -> spawn thread
  -> pop page                                      open Connection (WAL), decode file
glib::spawn_future_local(rx_events) <-----  send Progress / EntryError / Finished ...
  update banner + notification                     per entry: conn.transaction() ...
                                                   check cancel flag before each entry
  Cancel (banner button or notification action) -> cancel.store(true)
  on Finished/Canceled/Failed: slot.clear(),
       send final notification, show toast        return
```

Rules:
- Only the main thread touches GTK widgets and `gio::Notification`. The restore thread sends `RestoreEvent` values only.
- The validation (`validate()`) runs on a short-lived `std::thread` launched when the restore page opens; the page shows a spinner row until the result arrives on an `async_channel`.
- The restore thread opens its own SQLite connection (not shared with the UI's connection). Two connections writing concurrently is avoided because the UI is not writing during a restore (the restore page is closed; the library view should refresh via a DB change notification after `Finished`).
- Every step emits a `Progress` event (no throttling in the channel). Only the GNotification update is throttled to at most one per 500 ms, plus a final update at 100 percent. The Kotlin version re-posts the notification on every item; this is a deliberate change.

Blocking inside the restore thread is fine. Do not call `glib::MainContext::block_on` from it.

---

## 5. Restore pipeline (Rust version of BackupRestorer.restoreFromFile)

```rust
fn run_inner(ctx: &mut RestoreCtx, backup: &format::Backup, opts: RestoreOptions) -> Result<(), RestoreError> {
    // 1. amount, exactly as behaviour §1.4 step 3, but categories add 2 and the categories section emits 2 progress steps (fix D2)
    // 2. ordered sections (fix D3):
    if opts.categories {
        ctx.check_cancel()?;
        let manga_map = restore_categories(&tx_for_categories, Kind::Manga, &backup.backup_categories)?;
        let anime_map = restore_categories(&tx_for_categories, Kind::Anime, &backup.backup_anime_categories)?;
        ctx.advance("Categories", 2);
    }
    if opts.app_settings { restore_app_prefs(..., category maps when opts.categories else None); ctx.advance("App settings", 1); }
    if opts.source_settings { for s in &backup.backup_source_preferences { ... } ctx.advance("Source settings", 1); }
    if opts.library_entries { for each anime (sortByNew) { entry tx ... }; for each manga (sortByNew) { entry tx ... } }
    if opts.extension_stores { anime stores then manga repos; per item advance }
    if opts.custom_buttons { restore_custom_buttons(...); ctx.advance("Custom buttons", 1); }
    if opts.extensions { ctx.skipped_apk = backup.backup_extensions.len(); ctx.advance("Extensions", 1); }
    Ok(())
}
```

Notes on the order:
- Anime and manga are processed sequentially (anime first, as in the Kotlin launch order, to keep stable behaviour for seasons). A single transaction per entry.
- `sortByNew` for manga and anime is computed against the DB snapshot at the start of each section (behaviour §4.1).
- With `categories` disabled, pass empty maps; entries then skip membership writes (same as Kotlin, which passes `emptyList()`).
- The "extensions" section counts APK entries as skipped (not errors), replacing the Android installer.

Season pre-pass (fix D7): before processing anime, build `children: HashMap<i64 /*backup parent id*/, Vec<usize>>` from rows with `parent_id.is_some()`. Process only rows with `parent_id.is_none()` at top level; restore children inside the parent's transaction with `parentId = backup_to_db[parent]`. Rows whose parent id is not present in the backup become top-level entries with `parentId = null` (Proposal; Kotlin would keep the raw id).

Note: this changes behaviour for orphaned seasons (D7 class). Record this as a deliberate deviation in the test plan.

---

## 6. Cancellation and error log (fixes D4, D17)

- `check_cancel()` returns `Err(RestoreError::Cancelled)` before each entry and each section.
- Entry errors are caught with `match` on the `Result` returned by the entry function; `RestoreError::Cancelled` is re-raised (never recorded as an entry error). This is the D4 fix.
- The error log is written in every exit path (`Finished`, `Canceled`, and `Failed`) before the final event. This is the D17 fix. On cancel the log contains the errors collected so far.
- A `Failed` restore also writes the log (errors collected before the failure).

---

## 7. GTK UI

### 7.1 Restore page (`src/backup/ui/restore_page.rs`)
Structure (widgets; names are GTK 4 types):
- `gtk::Box` (vertical) inside `gtk::ScrolledWindow`, margins 16 px (phone width is not a target; the desktop window min width still applies).
- Validation card: `gtk::Frame` or `gtk::ListBox` row containing a `gtk::Label` with `set_selectable(true)`, `set_wrap(true)`, `set_xalign(0.0)`, markup from the text tables in behaviour §12 (bold via Pango markup, escaped with `glib::markup_escape_text`).
- Options: `gtk::ListBox` (`selection-mode none`) with 7 `gtk::CheckButton` rows (labels in §3.1), `set_active` from `RestoreOptions`, `connect_toggled` -> `options.set(i, active)` and re-evaluate the action button sensitivity.
- Action bar: `gtk::Button` "Restore" with CSS class `suggested-action`, full width, at the bottom (`gtk::Box` at the end). `set_sensitive(can_restore && options.can_restore())`.
- Loading: while validation runs, show a `gtk::Spinner` row; hide the options list until `can_restore` is known (Kotlin shows checkboxes only when `canRestore`).

State machine (matches behaviour §1.2):
```
Loading --(validate Err)--> Invalid { canRestore: false, error }   // options hidden, action disabled
Loading --(validate Ok, missing)--> Ready { canRestore: true, error: Missing(sources, trackers) }
Loading --(validate Ok, none)--> Ready { canRestore: true, error: None }
Ready --(toggle)--> Ready (action enabled iff options.can_restore())
Ready --(Restore)--> pop page; JobSlot::start
```

Error text (exact, behaviour §1.2 and data file §12):
- Missing: `"You may need to install any missing extensions and log in to tracking services afterwards to use them."`, then bold `"Missing sources:"` and `"- name"` lines, then bold `"Trackers not logged into:"` and `"- name"` lines.
- Invalid: bold `"Invalid backup file:"`, the path (not the URI), blank, bold `"Full error:"`, the message.

### 7.2 File chooser
- `gtk::FileDialog::new()`, `set_title("Select backup file")` (`MR.strings.file_select_backup`), `set_filters` with one `gtk::FileFilter` named "Aniyomi backup" (pattern `*.tachibk`) and one "All files" (`*`). The Kotlin accepts `*/*`; the content is sniffed anyway (behaviour §3.1), so "All files" is the default filter.
- Cancel of the dialog: no toast needed. The "No file selected" text (`file_null_uri_error`) is shown as a `gtk::AlertDialog` only when the dialog returns an error other than cancel (PROPOSAL; Kotlin shows a toast for a null result).
- Single-instance check before opening the dialog: if `JobSlot::is_running()`, show `restore_in_progress` text ("Restore is already in progress") and do not open the dialog (behaviour §1.1).

### 7.3 Progress banner (`src/backup/ui/progress.rs`)
- A `gtk::Revealer` at the top of the main window content, with a `gtk::Box` holding: `gtk::Label` (title, e.g. "Restoring backup"), `gtk::Label` (current item, hidden when `hideNotificationContent`), `gtk::ProgressBar` (fraction = done/total, `set_show_text(true)` text "done / total"), `gtk::Button` "Cancel".
- Shown while the job runs; revealed on start; hidden on `Finished`, `Canceled`, `Failed`.
- The same content goes to the desktop notification (§8).

### 7.4 Create page (`src/backup/ui/create_page.rs`, area:21)
- Option groups as in behaviour §9 (Library, Settings, Extensions), checkbox sensitivity rules as in §9.
- "Create" button sensitive iff `canCreate()`.
- Click: if the manual create job is running, show the `backup_in_progress` text; else `gtk::FileDialog::save` with `set_initial_name("kurobayomi_yyyy-MM-dd_HH-mm.tachibk")`. Name prefix: `APPLICATION_ID` of Kurobayomi (not `eu.kanade...`), recorded in p1-data-and-config.md §7.
- Result: start the create job (area:21), pop the page. Cancelled dialog: stay on the page.

### 7.5 Storage info (`src/storage_info.rs`)
One row per mounted root: path heading, `gtk::ProgressBar` (fraction clamped to 0.0..=1.0; hidden when total is 0 or unknown), caption `"Available: {avail} / Total: {total}"` with `glib::format_size`. Roots from `/proc/self/mountinfo` filtered to real local block filesystems (PROPOSAL; the Android source uses app-specific external dirs). Keep it to the user's home mount and each mounted external disk.

---

## 8. Notifications (GIO)

Registered in `main.rs` on the `gtk::Application`:
- `app.cancel-restore` (no parameter): calls `JobSlot::stop()`.
- `app.open-error-log` (string parameter = file path): `gtk::UriLauncher::new(&format!("file://{path}"))` `.launch(None::<&gtk::Window>, None::<&gio::Cancellable>, |r| ...)`.

Notifications:

| Phase | id (string) | Priority | Title | Body | Buttons |
|---|---|---|---|---|---|
| Progress | `restore-progress` | Low | "Restoring backup" | "{done} / {total}: {title}" (title omitted when hide-content is on) | "Cancel" -> `app.cancel-restore` |
| Complete (no errors) | `restore-complete` | Normal | "Restore completed" | "Done in {mm} min, {ss} sec" | none |
| Complete (errors) | `restore-complete` | High | "Restore completed" | "Done in {mm} min, {ss} sec with {n} error(s)" (singular/plural by n) | "Tap to see details" -> `app.open-error-log` with the path |
| Canceled | `restore-complete` | Normal | "Restoring backup failed" | "Canceled restore" | none |
| Failed | `restore-complete` | High | "Restoring backup failed" | the failure message (user-facing) | none |

- The progress notification is withdrawn (`withdraw_notification("restore-progress")`) before the completion notification is sent.
- Plural text uses the two English forms from `restore_completed_message` (behaviour §8.2).
- Send with `app.send_notification(Some(id), &n)` (GTK 4 `gio::Application` method; the id replaces the previous one).

---

## 9. Scheduler re-arm (after app preference restore)

```rust
// implemented by the owning areas (area:08 library updates, area:21 auto backup)
pub trait Scheduler {
    fn rearm_library_updates(&self);   // reads auto-update interval (hours) and restrictions; 0 disables
    fn rearm_auto_backup(&self);       // reads backup_interval (hours, default 12); 0 disables
}
```
Called once after `restore_app_prefs` returns, on the GTK main thread (via the `Finished` handler), mirroring `PreferenceRestorer.restoreApp` (behaviour §5.4). Implementation may be a no-op stub until the owning area lands; the restore code must not depend on the scheduler's internals.

---

## 10. Android to Linux mapping (touched by this area)

| Android / Kotlin | Linux / Rust replacement | Notes |
|---|---|---|
| `CoroutineWorker` + `WorkManager` unique work (KEEP) | `JobSlot` + `std::thread` | single-job guard; no OS job scheduler needed |
| `OneTimeWorkRequest` input data | `RestoreRequest` struct (in memory) | no serialization needed; the job is not persisted across app restarts (Kotlin WorkManager persisted it). PROPOSAL: accept this loss (a restore is interactive) |
| `WorkManager.cancelUniqueWork` | `AtomicBool` cancel flag | checked between items |
| `setForeground(ForegroundInfo)` + `FOREGROUND_SERVICE_TYPE_DATA_SYNC` | not needed; the process is a desktop app | user keeps the app open or the restore is cancelled on close (see §11) |
| `NotificationCompat` progress with `setProgress` | GNotification body text + in-window `gtk::ProgressBar` | GNotification has no progress bar |
| `PendingIntent` broadcast for cancel | `app.cancel-restore` GAction | |
| `ACTION_VIEW` text/plain for the error log | `gtk::UriLauncher` (or `gio::AppInfo::launch_default_for_uri`) | |
| `Notification channels` (LOW / HIGH) | GNotification `priority` (low / high) | channels do not exist on Linux |
| `ActivityResultContracts.GetContent("*/*")` | `gtk::FileDialog::open` with filters | |
| `ActivityResultContracts.CreateDocument` | `gtk::FileDialog::save` | |
| `takePersistableUriPermission` | not needed (plain paths) | |
| `Context.createFileInCacheDir` (`externalCacheDir`) | `glib::user_cache_dir()/kurobayomi/` | |
| `File.getUriCompat` (FileProvider) | `gio::File::for_path(...).uri()` | |
| `Formatter.formatFileSize` | `glib::format_size` | SI units (kB, MB, GB) in both |
| `StatFs` | `rustix::fs::statvfs` | |
| `DeviceUtil.isMiui` / `isMiuiOptimizationDisabled` | removed | drop T-22a-039 |
| `PackageManager.getInstalledPackages` / APK installer intent | removed | drop T-22a-029 |
| `SharedPreferences` per source (`source_<id>`) | Kurobayomi preference store per source (area:02) | keys identical |
| `Log.e` / `logcat` | `log::error!` | |
| `SimpleDateFormat(Locale.getDefault())` | `chrono` with fixed ASCII digits | locale-independent output (fix in §3.11) |

---

## 11. Lifecycle and shutdown

- Closing the main window while a restore runs: show a `gtk::AlertDialog` "A restore is in progress. Cancel it and close?" with buttons "Keep running" and "Cancel restore and close" (PROPOSAL; Kotlin's WorkManager job survives the UI).
- Cancel on close waits for the restore thread to reach its next check point (entries are short; SQLite commits are atomic per entry), then closes. Never kill the thread; the per-entry transaction guarantees no partial entry.
- If the app is killed mid-restore, the database is consistent per entry (each entry committed or rolled back) but the restore is not resumable. The next restore re-runs and is idempotent per entry (merge rules keep existing rows; see behaviour §4).

---

## 12. Pitfalls

1. SQLite transaction behaviour: use `conn.transaction_with_behavior(TransactionBehavior::Immediate)` for entry transactions so a writer lock is taken at BEGIN (avoids `SQLITE_BUSY` on upgrade if a UI read transaction is open).
2. Schema coupling: the SQL in §5 of p1-data-and-config.md is copied from the Kotlin `.sq` files. If area:03/04 change column names, update the Rust queries from the Kurobayomi schema, not from the Kotlin text.
3. Float fields: chapter numbers and track scores are `f32` on the wire; convert with `f64::from(x)` to match Kotlin's `toDouble()` (it yields the same float artefacts; E-27 in the edge file). PROPOSAL: round to 6 decimals on restore; document the choice in the test plan.
4. `prost` default handling: a missing `favorite` must be `true` (D22). Do not use `#[prost(bool, optional)]` with `unwrap_or_default`; store `Option<bool>` and apply the default in an accessor.
5. The sealed `PreferenceValue` wire form is not reproduced by `prost` derive; implement a small hand-written decoder after capturing a fixture (T-22a-051).
6. `Option<String>` for `genre`: the domain `genre` is a list, the DB stores `", "` joined text. Split only for display; never re-split the DB text for restore.
7. Locale: the error log timestamp must not depend on the system locale. Use `chrono::format` with ASCII digits.
8. Progress counters are updated only on the restore thread; events are the single source of truth for the GTK side.
9. `gtk::FileDialog` is async; never block the main thread waiting for it. Use the callback or `glib::spawn_future_local` with `dialog.open_future(...)` (GTK 4.10+ `*_future` variants exist for FileDialog; verify the exact method name against gtk4-rs 0.11 docs when implementing).
10. Do not expand the entry restore into a generic trait with two implementations unless a third caller appears (simplicity rule). Two concrete functions are cheaper to read.
