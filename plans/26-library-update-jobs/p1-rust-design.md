# Area 26 - Library update jobs: Rust design

Scope reminder: `p1-inventory.md`. Behaviour being reproduced: `p1-behavior.md`. Data: `p1-data-and-config.md`. This file is the implementation design. Everything marked **Proposal:** is a decision; everything else restates the Kotlin behaviour being preserved.

Current mockup facts (checked): `Cargo.toml` is a single crate `kurobayomi` (edition 2024) with `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde`, `serde_json`. Source files: `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/spec.rs`, `src/entry.rs`, `src/player.rs`, `src/settings.rs`, `src/clamp.rs`. Spec files: `spec/app.json`, `spec/entries.json`, `spec/reader_player.json`, `spec/settings.json`, `spec/SCHEMA.md`. The mockup is UI only; there is no data layer, no tokio, no rusqlite yet.

---

## 1. Crate and module layout

**Proposal:** keep one crate for the first iteration and split later only if compile times demand it. Add a `core` module tree that has no `gtk` imports so it can be unit-tested headless.

```
src/
  main.rs                       (existing; starts runtime + GTK app)
  core/
    mod.rs
    error.rs                    UpdateError (thiserror)
    library_update/
      mod.rs                    pub use; PassKind, PassTrigger, PassRequest
      queue.rs                  build_queue(): category selection, restrictions, sort   (behaviour 3)
      pass.rs                   run_pass(): per-source semaphore, per-entry flow, error collection (behaviour 6)
      sync.rs                   chapter/episode sync adapter (behaviour 5); pure diff function + DB calls
      fetch_interval.rs         FetchInterval (behaviour 4); pure, chrono-based, heavily tested
      metadata.rs               metadata-only pass (behaviour 9)
      error_log.rs              writeErrorFile equivalent (behaviour 7)
    scheduler.rs                auto schedule (behaviour 1.1), startNow/stop (1.2/1.3), lock (7.3 table)
    notify/
      mod.rs                    trait Notifier + NotificationText (pure string builders, behaviour 8.6)
      desktop.rs                notify-rust implementation
    chapter_util.rs             next_unread, dedupe_by_number, filter_downloaded (behaviour 11)
    episode_util.rs             next_unseen, filter_downloaded
    datasaver.rs                DataSaver trait + three proxies (behaviour 10)
    net_state.rs                NetworkManager + UPower probes (zbus), fallback = "always satisfied"
    prefs.rs                    typed view over the preference store (keys from p1-data section 3)
  ui/ (or existing ui.rs / screens.rs)
    updates_screen.rs           shows last run time, counts, job progress, error log button
    settings_library.rs         interval choices, device restrictions, category pickers
```

Repo-relative Kotlin counterparts are listed in `p1-inventory.md`. Do not port `DataSaver` into `core::library_update`; it lives in `core::datasaver` because the reader and downloader use it too.

---

## 2. Key types and trait signatures

All signatures are Rust-proposal code. Async traits use native `async fn` in traits (stable since 1.75) with `Send` bounds where spawned.

### 2.1 Errors

```rust
// src/core/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    /// Maps to MR `no_chapters_error` ("No chapters found") / AYMR `no_episodes_error`.
    #[error("No chapters found")]
    NoChapters,
    #[error("No episodes found")]
    NoEpisodes,
    /// Maps to MR `loader_not_implemented_error` ("Source not found").
    #[error("Source not found")]
    SourceNotInstalled,
    /// Any other failure; the message is shown verbatim (Kotlin uses e.message).
    #[error("{0}")]
    Other(String),
}

impl UpdateError {
    /// Text written to the error log and to the per-entry failure list.
    pub fn user_message(&self) -> String { self.to_string() }
}
```

`Other(String)` must never be an empty string; when the underlying error has no message, use `format!("{:?}", err)` (Kotlin would store `null`, see behaviour P-9).

### 2.2 Entry model consumed by the queue

```rust
// src/core/library_update/queue.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind { Manga, Anime }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStrategy { AlwaysUpdate, OnlyFetchOnce }   // stored 0 / 1

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchType { Seasons, Episodes }                   // stored 0 / 1

#[derive(Debug, Clone)]
pub struct LibraryEntry {
    pub id: i64,
    pub source: i64,
    pub title: String,
    pub kind: EntryKind,
    pub category: i64,            // 0 = default/no category; -1 = season child
    pub update_strategy: UpdateStrategy,
    pub status: i64,              // SManga/SAnime status; COMPLETED == 2
    pub unread_or_unseen: i64,    // totalCount - readCount
    pub total: i64,               // totalChapters / totalCount
    pub has_started: bool,
    pub next_update_ms: i64,
    pub fetch_type: FetchType,    // Episodes for manga
    pub parent_id: Option<i64>,   // Some for anime season children
    pub fetch_interval: i32,
    pub last_update_ms: i64,
}

#[derive(Debug, Clone, Default)]
pub struct QueueRestrictions {
    pub skip_completed: bool,           // "manga_ongoing"
    pub skip_unread: bool,              // "manga_fully_read"
    pub skip_not_started: bool,         // "manga_started"
    pub skip_outside_release: bool,     // "manga_outside_release_period"
}
```

Note on the not-always-update check: in Kotlin it is **unconditional** (`it.manga.updateStrategy != ALWAYS_UPDATE -> skip` is not guarded by a restriction token). The struct above therefore has no field for it; `build_queue` applies it always.

```rust
pub enum SkipReason {
    NotAlwaysUpdate, Completed, NotCaughtUp, NotStarted, NotInReleasePeriod,
}

impl SkipReason {
    /// English text from MR keys (behaviour section 3.3). Only written to the debug log.
    pub fn message(self) -> &'static str {
        match self {
            Self::NotAlwaysUpdate => "Skipped because series does not require updates",
            Self::Completed => "Skipped because series is complete",
            Self::NotCaughtUp => "Skipped because there are unread chapters",
            Self::NotStarted => "Skipped because no chapters are read",
            Self::NotInReleasePeriod => "Skipped because no release was expected today",
        }
    }
}

pub struct QueueInput<'a> {
    pub library: &'a [LibraryEntry],
    pub explicit_category: Option<i64>,     // Some(id) for manual per-category runs
    pub include_categories: &'a [i64],      // prefs library_update_categories
    pub exclude_categories: &'a [i64],      // prefs library_update_categories_exclude
    pub restrictions: QueueRestrictions,
    pub include_season_children: bool,      // anime only
    pub release_window_upper_ms: i64,       // getWindow(now).upper
}

/// Pure function. Output sorted by title (byte-wise natural string order, same as Kotlin's String.compareTo for BMP text).
pub fn build_queue(input: &QueueInput, season_children: &dyn Fn(i64) -> Vec<LibraryEntry>)
    -> (Vec<LibraryEntry>, Vec<(i64, SkipReason)>);
```

Rules to implement exactly (behaviour 3.2 and 3.3):
- `explicit_category == Some(c)`: `list = library.filter(category == c)`; no distinct; update-category prefs ignored.
- Else as in behaviour 3.2 (include/exclude, exclusion wins, `distinct_by(id)` keeping the first occurrence).
- Anime season expansion (behaviour 3.4) happens before restrictions: `Seasons` parent is replaced by its child list, or by nothing when `include_season_children == false`.
- Restriction checks in the exact order of behaviour 3.3 table. First match wins.
- `title` sort: `sort_by(|a, b| a.title.cmp(&b.title))`.

Rust `String::cmp` is byte-wise UTF-8 order. For ASCII this equals Kotlin's ordering. For non-ASCII the order can differ from UTF-16 code-unit order only for supplementary characters; accepted (document in tests, T-26a-013).

### 2.3 Fetch interval (pure)

```rust
// src/core/library_update/fetch_interval.rs
use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};

pub const MAX_INTERVAL: i32 = 28;
pub const GRACE_PERIOD_DAYS: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window { pub lower_ms: i64, pub upper_ms: i64 }   // upper is inclusive: tomorrow 00:00 - 1 ms

pub struct Dated<'a> { pub date_upload_ms: i64, pub date_fetch_ms: i64, pub _p: std::marker::PhantomData<&'a ()> }

/// Mirrors MangaFetchInterval.getWindow.
pub fn window_for<Tz: TimeZone>(now: DateTime<Tz>) -> Window;

/// Mirrors calculateInterval(chapters, zone). `items` = (date_upload_ms, date_fetch_ms) per chapter/episode.
pub fn calculate_interval<Tz: TimeZone>(items: &[(i64, i64)], zone: &Tz) -> i32;

/// Mirrors toMangaUpdate / toAnimeUpdate. Returns (next_update_ms, fetch_interval).
pub fn next_update<Tz: TimeZone>(
    stored_fetch_interval: i32,
    items: &[(i64, i64)],
    stored_next_update_ms: i64,
    stored_last_update_ms: i64,
    now: DateTime<Tz>,
    window: Option<Window>,          // None = (0,0) in Kotlin = compute from now
) -> (i64, i32);

/// Mirrors increaseInterval(delta, timeSinceLatest, 10).
pub fn increase_interval(delta: i32, days_since_latest: i64, increase_when_over: i64) -> i32;
```

Implementation rules (must match behaviour section 4 exactly):
- Local dates: use `DateTime::with_timezone(&zone).date_naive()`, and "start of day" as `NaiveDate::and_hms_opt(0,0,0).and_local_timezone(zone).earliest()`. Kotlin `atStartOfDay(zone)` uses the first valid instant; `earliest()` is the equivalent.
- `calculate_interval`: `chapter_window = if items.len() <= 8 {3} else {10}`; upload dates: filter `upload > 0`, sort desc by upload, map to local start-of-day `NaiveDate`, dedupe keeping order (`Vec` + `contains` is fine: window <= 10 after `take`; to match Kotlin `distinct()` then `take(window)`, dedupe first over the full sorted sequence, then truncate), `take(window)`. Fetch dates: same without the `> 0` filter, sorted by `date_fetch` desc.
- `delta` = `(last - ... )`: `days = (first_date - last_date).num_days()` where `first` is newest, `last` oldest. Kotlin `last.until(first, DAYS)` gives `first - last`. Divide by `period = list.len() - 1` using **floor division**; for positive divisor `i64::div_euclid` equals floor division, and `period >= 2` always here.
- `coerce_in(1, 28)`.
- `calculate_next_update`: keep `stored_next_update_ms` if `lower <= x <= upper` (inclusive both ends: Kotlin `rangeTo(window.second + 1)`). Else `latest = start_of_day(local date of (last_update > 0 ? last_update : now))`, `days_since = (local date of now - latest.date).num_days()` i.e. `ChronoUnit.DAYS.between(latestDate, dateTime)` where `dateTime` is the zoned now and `latestDate` is start-of-day; careful: Kotlin compares a `LocalDateTime` (start of day) with a `ZonedDateTime` via `DAYS.between`, which truncates toward zero on the difference of the two instants converted to LocalDateTime; implement as `(now_local_naive_datetime - latest_naive_datetime).num_days()` on naive local values, which matches for all non-DST-crossing cases; DST edge case is documented in tests (T-26a-012).
- `cycle = days_since.div_euclid(effective)` with `effective = if interval < 0 { abs(interval) } else { increase_interval(interval, days_since, 10) }`. `effective` is always >= 1 when `interval != 0` (calculate path guarantees 1..=28). Guard: if `stored == 0` and computed value is somehow 0 (should not happen), return error rather than divide by zero.
- Result `next = (latest.date + (cycle + 1) * abs(interval)) start-of-day in zone` converted to ms.
- `increase_interval`: iterative version of the recursion; `if delta >= 28 { return 28 }`; `cycle = days.div_euclid(delta) + 1`; `if cycle > 10 { delta *= 2; loop } else { delta }`.

Reference table for tests (computed by hand from the Kotlin logic):

| delta | days_since | cycle = days/delta + 1 | result |
|---|---|---|---|
| 7 | 0 | 1 | 7 |
| 7 | 70 | 11 (>10) -> delta 14: 70/14+1 = 6 (<=10) | 14 |
| 1 | 9 | 10 (not > 10) | 1 |
| 1 | 10 | 11 (> 10) -> 2: 10/2+1 = 6 | 2 |
| 16 | 200 | 13 (>10) -> 32 >= 28 -> 28 | 28 |

### 2.4 Source and store traits

```rust
// src/core/library_update/pass.rs
use std::sync::Arc;

pub struct SourceChapter { pub url: String, pub name: String, pub scanlator: Option<String>,
    pub chapter_number_hint: f64, pub date_upload_ms: i64 }  // SChapter subset

pub struct SourceEntryDetails { pub title: Option<String>, pub thumbnail_url: Option<String>,
    pub background_url: Option<String>, pub author: Option<String>, pub artist: Option<String>,
    pub description: Option<String>, pub genres: Vec<String>, pub status: i64,
    pub update_strategy: UpdateStrategy, pub memo: serde_json::Value }

#[async_trait_free] // native async fn in trait
pub trait Source: Send + Sync {
    fn id(&self) -> i64;
    fn is_local(&self) -> bool;
    fn is_unmetered(&self) -> bool;                 // UnmeteredSource marker
    async fn details(&self, url: &str) -> Result<SourceEntryDetails, UpdateError>;
    async fn chapters(&self, url: &str) -> Result<Vec<SourceChapter>, UpdateError>;     // manga
    async fn episodes(&self, url: &str, fetch_details: bool) -> Result<(Option<SourceEntryDetails>, Vec<SourceChapter>), UpdateError>; // anime
}

pub trait SourceRegistry: Send + Sync {
    /// Returns a stub that yields UpdateError::SourceNotInstalled on every call when the source is missing.
    fn get_or_stub(&self, source_id: i64) -> Arc<dyn Source>;
    /// None when not installed (metadata jobs silently skip, behaviour 9.1).
    fn get(&self, source_id: i64) -> Option<Arc<dyn Source>>;
}
```

Note: `async fn` in a trait used as `Arc<dyn Source>` is not object-safe. **Proposal:** use the `async-trait` crate (`#[async_trait]`) for `Source` and `SourceRegistry::get_or_stub` returning `Arc<dyn Source>`; accepted cost is one `Box` per call, negligible versus network. Native `async fn` is fine for the store traits below since they are used generically.

```rust
pub trait LibraryStore: Send + Sync + 'static {
    async fn library(&self) -> Result<Vec<LibraryEntry>, UpdateError>;     // libraryView / animelibView
    async fn season_children(&self, parent_id: i64) -> Result<Vec<LibraryEntry>, UpdateError>; // [] on error (behaviour 0)
    async fn is_favorite(&self, id: i64, kind: EntryKind) -> Result<bool, UpdateError>;
    async fn load_for_sync(&self, id: i64, kind: EntryKind) -> Result<Option<LibraryEntry>, UpdateError>;
}

pub trait ItemSync: Send + Sync + 'static {
    /// Returns the new items (after excluded-scanlator / changed-or-duplicate-read filter) in the order defined by behaviour 5.4 step 19.
    async fn sync_chapters(&self, entry: &LibraryEntry, raw: Vec<SourceChapter>, source: &dyn Source,
        manual: bool, window: Window, now_ms: i64) -> Result<Vec<NewItem>, UpdateError>;
    async fn sync_episodes(&self, entry: &LibraryEntry, raw: Vec<SourceChapter>, source: &dyn Source,
        manual: bool, window: Window, now_ms: i64) -> Result<Vec<NewItem>, UpdateError>;
}

#[derive(Debug, Clone)]
pub struct NewItem { pub id: i64, pub url: String, pub name: String, pub number: f64,
    pub scanlator: Option<String>, pub source_order: i64, pub date_upload_ms: i64 }
```

### 2.5 Pass runner

```rust
pub struct PassRequest {
    pub kind: EntryKind,
    pub trigger: PassTrigger,              // Auto | Manual | Category(i64)
    pub auto_update_metadata: bool,        // prefs auto_update_metadata
    pub include_season_children: bool,     // anime only
    pub restrictions: QueueRestrictions,
    pub include_categories: Vec<i64>,
    pub exclude_categories: Vec<i64>,
    pub auto_download_new: bool,           // download_new (chapter) / download_new_episode
    pub only_unread_for_download: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassTrigger { Auto, Manual, Category(i64) }

pub struct PassOutcome {
    pub checked: u32,
    pub new_items: Vec<(LibraryEntry, Vec<NewItem>)>,     // sorted desc by source_order inside each Vec (behaviour 6.2)
    pub failures: Vec<(LibraryEntry, String)>,            // message never empty (see 2.1)
    pub cancelled: bool,
}

pub struct PassDeps {
    pub store: Arc<dyn LibraryStore>,
    pub sources: Arc<dyn SourceRegistry>,
    pub sync: Arc<dyn ItemSync>,
    pub prefs: Arc<dyn Prefs>,
    pub downloads: Arc<dyn DownloadQueue>,   // enqueue(paused=true) + start()
    pub notifier: Arc<dyn Notifier>,
    pub progress: async_channel::Sender<PassEvent>,
}

pub async fn run_pass(req: PassRequest, deps: PassDeps, cancel: tokio_util::sync::CancellationToken)
    -> PassOutcome;

#[derive(Debug, Clone)]
pub enum PassEvent {
    Started { total: u32 },
    EntryStarted { id: i64, title: String, done: u32, total: u32 },
    EntryFinished { id: i64, ok: bool, new_count: u32, done: u32, total: u32 },
    Finished(PassOutcome),                  // boxed in practice
}
```

Pass algorithm (behaviour 6.1 - 6.5, Rust-specific):
1. Acquire the kind lock (section 5.2). If already held, return `Skipped` (UI shows "An update is already running").
2. `let library = store.library()`. Build queue (`build_queue`). Persist `library_job_run` row (state running, total = queue.len()).
3. `set_last_timestamp(now)` (pref `library_update_last_timestamp`), before queueing, same as Kotlin.
4. Group by `source` preserving first-appearance order (`IndexMap`, or `Vec<(i64, Vec<..>)>` built with a `HashMap<i64, usize>`).
5. `let sem = Arc::new(tokio::sync::Semaphore::new(5));` spawn one task per group: `let _p = sem.acquire().await` held for the whole group; iterate entries sequentially; for each entry `if cancel.is_cancelled() { break }`; library re-check (`is_favorite` for manga; for anime `parent_id.is_none() && !is_favorite` skip; seasons exempt).
6. Per-entry block: `update_entry(...)` returns `Result<Vec<NewItem>, UpdateError>`; errors go to `failures` with the message from section 2.1. Use `Result` not `catch_unwind`; cancellation is handled by the token, never converted into a failure (fixes P-10).
7. Progress events after start and after finish of each entry; `done` increments.
8. After all groups: notifier `cancel_progress()`; if `new_items` non-empty: `notifier.show_summary(...)`, then if any download was enqueued, `downloads.start()`; if failures non-empty write error log and `notifier.show_error(count, path)`.
9. Update `library_job_run` to success/failure/cancelled; release lock.

`update_entry` for manga (behaviour 5.1 and 6.2):
```rust
async fn update_entry_manga(deps: &PassDeps, e: &LibraryEntry, window: Window, now_ms: i64, metadata: bool)
    -> Result<Vec<NewItem>, UpdateError> {
    let source = deps.sources.get_or_stub(e.source);
    if metadata {
        let d = source.details(&e.url).await?;               // url: from store
        deps.store.apply_details(e.id, &d, /*manual=*/false).await?;   // UpdateManga.awaitUpdateFromSource
    }
    let raw = source.chapters(&e.url).await?;               // NoChapters thrown by sync when empty & !local
    let Some(db) = deps.store.load_for_sync(e.id, EntryKind::Manga).await?.filter(|x| x.favorite) else {
        return Ok(vec![]);                                   // removed mid-pass
    };
    let mut new = deps.sync.sync_chapters(&db, raw, &*source, false, window, now_ms).await?;
    new.sort_by(|a, b| b.source_order.cmp(&a.source_order)); // sortedByDescending { sourceOrder }
    Ok(new)
}
```
Anime version uses `episodes` and skips the favourite gate before the sync (Kotlin P-7 behaviour). **Proposal:** apply the gate in both (fixes P-7); the decision is recorded as a deliberate deviation in T-26a-024.

Note the `sort_by` on `source_order` descending: Kotlin applies `sortedByDescending { it.sourceOrder }` to the list returned by sync; keep that exact order.

Then per entry after `new` non-empty (manga): `filter_for_download(...)` -> `downloads.enqueue(entry, chapters, autostart=false)`; `hasDownloads = true`; counter `prefs.new_manga_updates_count += new.len()` (note: `new.len()` after the sort, same count).

### 2.6 Notifier

```rust
pub trait Notifier: Send + Sync {
    fn progress(&self, titles: &[String], current: u32, total: u32);       // ID -101
    fn cancel_progress(&self);
    fn queue_size_warning(&self);                                          // ID -103, timeout 30 s
    fn summary(&self, kind: EntryKind, updates: &[(String, Vec<f64>)]);     // ID -301 / -1301
    fn per_entry(&self, kind: EntryKind, entry: &LibraryEntry, items: &[NewItem]);  // id = entry.id hashed
    fn error(&self, kind: EntryKind, failed: u32, log_path: Option<std::path::PathBuf>);  // ID -102
}
```

Pure text builders (no toolkit) in `core::notify::text`:

```rust
/// Mirrors getNewChaptersDescription / getNewEpisodesDescription (behaviour 8.6).
pub fn new_items_description(kind: EntryKind, numbers: &[f64], count: usize) -> String;
/// Mirrors percentFormatter (RoundingMode.DOWN, 0 fraction digits).
pub fn percent(current: u32, total: u32) -> String;   // floor(current*100/total) + "%"
/// Kotlin chop(n): proposal = take n chars, append "…"? -> see behaviour 8.7; implement as "first n chars" with ellipsis "..." only when truncated; verify in T-26a-037.
pub fn chop(s: &str, n: usize) -> String;
pub fn format_number(n: f64) -> String;  // formatChapterNumber: "2.5", "3" (no trailing .0)
```

`percent(current, total)`: `if total == 0 { "0%" }` (Kotlin would produce NaN; guard for safety).

Desktop implementation (`notify/desktop.rs`) uses `notify-rust`:
- Progress: `Notification::new().summary("Aniyomi")` with body = title text (already "Updating library… (42%)" in the `summary` field, body = titles joined by newline when not hidden) and `.hint(Hint::Category("transfer"))`, `.urgency(Low)`, `.id(stored_id)` to update in place (freedesktop replace-id), action "Cancel" (`add_action("cancel", "Cancel")`) -> sends on the event channel `PassEvent::CancelRequested`.
- Summary and per-entry notifications: **Proposal:** urgency `Normal` (Kotlin uses IMPORTANCE_DEFAULT with priority HIGH; `Critical` is too aggressive for a desktop).
- Actions per entry: `"read"` -> "Mark as read" (`action_mark_as_read`), `"view"` -> "View chapters", `"download"` -> "Download" (only when count <= threshold, 15 chapters / 10 episodes). Action callbacks send `NotifyAction { kind, entry_id, item_ids, action }` to the GTK side (see section 4).
- Error: body "N update(s) failed" + action "Tap to see details" opens the log via `gio::AppInfo::launch_default_for_uri` on `file://` URI (`text/plain` default app).
- Hidden content (`hideNotificationContent`): generic text only.
- Icons: `notify-rust` `.icon("kurobayomi")` (app icon name); large icon = cover file path with circular crop precomputed to 192 px PNG in cache (`$XDG_CACHE_HOME/kurobayomi/covers/notif/<id>.png`), via `gdk-pixbuf` (crate `gdk-pixbuf`, transitively present with gtk4).

The in-app equivalent (toast): `adw`-free `gtk::Revealer` + `gtk::Label` overlay (no libadwaita). Text identical.

### 2.7 Scheduler

```rust
pub struct Scheduler {
    handle: tokio::runtime::Handle,
    interval_hours: Arc<tokio::sync::watch::Sender<u32>>,   // 0 = off
    restrictions: Arc<tokio::sync::watch::Sender<DeviceRestrictions>>,
    cancel_auto: Mutex<Option<CancellationToken>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceRestrictions { pub wifi_only: bool, pub unmetered_only: bool, pub when_charging: bool }

impl Scheduler {
    /// Mirrors setupTask(context, prefInterval). Replaces any existing auto loop (UPDATE policy).
    pub fn setup_task(&self, interval_hours: Option<u32>);
    /// Mirrors startNow(context, category). Returns false when a pass of that kind is running.
    pub fn start_now(&self, kind: EntryKind, category: Option<i64>) -> bool;
    /// Mirrors stop(context): cancels running pass of kind; re-arms auto loop if the running pass was automatic.
    pub fn stop(&self, kind: EntryKind);
}
```

Auto loop (per kind), tokio task:
```
loop {
    if interval == 0 { wait for change; continue }
    wait until constraints_satisfied() (poll net_state every 30 s; cancel on setup_task)
    if manual pass of this kind running -> sleep 10 min (LINEAR backoff step) and retry   // Kotlin Result.retry()
    run_pass(Auto)
    sleep(interval_hours * 3600 s  - jitter in [0, 10 min])   // flex: the next run occurs within [interval-10min, interval]
}
```
- Retry/backoff: when a pass returns `Retry` (constraint not met, or manual running), delay `10 min * attempt` (LINEAR; attempt starts at 1; cap at 5 h as WorkManager's max backoff is 5 h). Reset attempt on success.
- Constraint check (`constraints_satisfied`): if `wifi_only` and not Wi-Fi -> false; if `unmetered_only` and metered -> false; if `when_charging` and on battery -> false. Battery-not-low is **not enforced** on desktop (Proposal; there is no equivalent that is meaningful on mains-powered desktops; document the deviation).
- Anime and manga each have their own loop and own lock. Metadata jobs are manual only (no loop).
- Startup: `Scheduler::new` reads prefs; if interval > 0 arms loops. First auto run happens as soon as constraints are satisfied (WorkManager parity, see T-26a-010 decision).

Persistence of the schedule across restarts: the schedule is re-armed from prefs at startup, so nothing else needs storage. Missed runs are not replayed (WorkManager parity).

Background running: the process must be alive for auto loops. **Proposal (default):** the app keeps running in the background when the window is closed if the user enabled "Automatic updates" (use `org.freedesktop.portal.Background.RequestBackground` with `autostart = true`; on KDE Plasma this shows the standard permission dialog). Fallback when portal is unavailable (no sandbox): the loop only runs while the process exists. Optional alternative documented in p1-topic-scheduling.md: a systemd user timer `kurobayomi-update.timer` launching `kurobayomi --library-update=auto`.

### 2.8 Net and power state (`core::net_state`)

```rust
pub struct NetState { pub on_wifi: bool, pub metered: bool, pub on_battery: bool }
#[async_trait_free] pub trait NetProbe: Send + Sync { async fn snapshot(&self) -> NetState; }
```
Implementation: `zbus` system bus; NetworkManager `org.freedesktop.NetworkManager` property `Metered` (`NMMetered`, data doc 6), `PrimaryConnectionType` string (`"802-11-wireless"` = Wi-Fi), UPower `org.freedesktop.UPower` `OnBattery`. If the bus or service is missing, return `NetState { on_wifi: true, metered: false, on_battery: false }` (no constraint blocks the pass; log once at WARN).

Unit-test the mapping `(PrimaryConnectionType, Metered) -> NetState` with table-driven tests.

### 2.9 Data saver

```rust
pub enum DataSaverMode { None, BandwidthHero, WsrvNl, ReSmushIt }

pub struct DataSaverConfig {
    pub mode: DataSaverMode,
    pub excluded_sources: std::collections::HashSet<i64>,
    pub bandwidth_server: String,      // trailing '/' trimmed
    pub ignore_jpeg: bool, pub ignore_gif: bool,
    pub format_jpeg: bool, pub quality: u8, pub color_bw: bool,
}

#[async_trait]
pub trait ImageCompressor: Send + Sync {
    /// Mirrors DataSaver.compress. Async so reSmush.it does not block.
    async fn compress(&self, image_url: &str) -> String;
}

pub fn compressor_for(source_id: i64, cfg: &DataSaverConfig, http: reqwest::Client) -> Arc<dyn ImageCompressor>;
```
- `NoOp` when `mode == None`, or when `mode != None` and `excluded_sources.contains(source_id)`.
- Each proxy implements the exact URL templates in `p1-behavior.md` 10.3-10.5 and the jpg/gif decision.
- The page loader fetch wrapper (behaviour 10.2) is `async fn get_image_with_compressor(source, page, compressor) -> Response` with the same save/restore of `page.image_url` (implemented by cloning the page, avoiding shared mutable state; this is a deliberate change: Kotlin mutates and restores, which is racy under concurrency).
- reSmush.it: `reqwest` GET `http://api.resmush.it/ws.php?img={url}&qlty={quality}` with the image URL percent-encoded via `url::form_urlencoded::byte_serialized` (Kotlin does not encode; P-14/P-15 fix). Parse JSON with `serde_json` (`{"dest": "..."}`) instead of substring extraction; on any error or missing `dest` return the original URL. Timeout 10 s (proposal).
- Bandwidth Hero: `format_jpeg`/`color_bw` as `"1"`/`"0"`; `URLEncoder.encode(url, "utf-8")` equivalent: `url::form_urlencoded::byte_serialized`? Note: Java `URLEncoder` encodes space as `+` and `*` unchanged; `form_urlencoded::byte_serialized` encodes space as `+` and leaves `*-._` unencoded — compatible. Test with a space and `&` in the URL.
- wsrv.nl: encode the image URL (proposal fix of P-14) using `form_urlencoded::byte_serialized`; the suffix rules unchanged.

### 2.10 Chapter and episode utilities

```rust
pub fn next_unread<'a>(chapters_after_filters_and_sort: &'a [ChapterRow], descending: bool) -> Option<&'a ChapterRow>;
pub fn dedupe_by_number<'a>(chapters: &'a [ChapterRow], current_id: i64, current_scanlator: Option<&str>) -> Vec<&'a ChapterRow>;
pub fn filter_downloaded(chapters: Vec<ChapterRow>, is_local: bool, is_downloaded: impl Fn(&ChapterRow) -> bool) -> Vec<ChapterRow>;
```
- `next_unread`: `descending ? iter().rev().find(!read) : iter().find(!read)` (Kotlin `findLast` = Rust `rev().find`).
- `dedupe_by_number`: group by `number` (f64 key via `ordered_float` or by bit pattern of normalized `f64`; use `number.to_bits()` after mapping `-0.0` to `0.0`), group order = first appearance; per group pick `find(id == current_id) .or_else(find(scanlator == current_scanlator)) .unwrap_or(first)`.
- The episode versions are identical with `seen`/`EpisodeRow`.
- `applyFilters` (view filters and sort) is owned by the list-view area; these functions take its output.

---

## 3. Async and threading model

- Process layout: GTK main thread (glib main loop) owns all widgets. A dedicated multi-thread `tokio::runtime::Runtime` (`worker_threads = 4`, proposal) is created once in `main` and its `Handle` is passed to `Scheduler` and `run_pass`. Blocking SQLite I/O (`rusqlite`) runs through `tokio::task::spawn_blocking` inside the store implementation, using a small pool of connections (`r2d2` not needed; use one writer `Mutex<Connection>` plus WAL readers opened per call).
- Events flow background -> UI through `async_channel::Sender<PassEvent>`; the GTK side consumes with `glib::spawn_future_local(async move { while let Ok(ev) = rx.recv().await { ... } })`. Never touch widgets from tokio tasks.
- UI -> background commands (start/stop/setup) go through the `Scheduler` methods which are `Send + Sync` and internally use `tokio::sync::watch` and `CancellationToken`.
- Concurrency limit: `tokio::sync::Semaphore::new(5)` per pass (not global; the two kinds run in parallel with their own semaphore, same as Kotlin where each job has its own semaphore). Across kinds, a per-source politeness rule would need a global per-source semaphore (**Proposal:** add a process-wide `DashMap<i64, Arc<Semaphore>>` with 1 permit per source so manga and anime passes never hit the same source simultaneously; this is a deliberate improvement; record in T-26a-022).
- Cancellation: `CancellationToken` per pass, cancelled by `stop(kind)`; checked before each entry and in `select!` around network calls. Cancelled entries are not failures (fix P-10).
- Timers: `tokio::time::sleep` and `interval`; monotonic clock for waits, wall clock (`chrono::Utc::now()`) only for stored timestamps.

---

## 4. Integration with the existing Kurobayomi mockup

| Mockup file | Current content (checked) | Change for this area |
|---|---|---|
| `src/screens.rs` (fn `updates()`, around line 277-282) | static text "Library last updated: 5 minutes ago" | Read `library_update_last_timestamp` and render relative time; add a "Update library" button calling `Scheduler::start_now(kind, None)` and disabled state when running (`library_job_run` state) |
| `src/main.rs` (line 175) | sidebar badge `"updates" => "8"` placeholder | Bind badge to `new_manga_updates_count + new_anime_updates_count` pref values; hide when zero |
| `src/settings.rs` (line 17) | `settings_worker_info` returns a static text dump "Enqueued/Finished/Tags: LibraryUpdate/State: SUCCEEDED" | Replace with live dump of `library_job_run` rows (kind, trigger, state, started_at, done/total/failed); this is the "Worker info" screen |
| `src/screens.rs` (`info_screen` at ~line 492-498) | onboarding/new_update/crash | not this area |
| `spec/settings.json` | not inspected in this pass | When implementing T-26a-060-series settings, add entries for: interval (0/12/24/48/72/168 hours), device restrictions (wifi, network_not_metered, ac), entry restrictions (four tokens), categories include/exclude (per kind), season-children toggle, auto metadata toggle, data saver section. Keys must match p1-data section 3 |
| `src/ui.rs` | generic widgets (`page`, `row`, `boxed_list`, `show_dialog`, `show_sheet`) | Use `show_dialog` for the error-log opener and the "already running" message; use `row` for settings rows |

Mockup "dialogs" in `spec` (`Dialog` struct in `src/spec.rs:59`) are the vehicle for the category include/exclude picker (multi-state checkboxes: included / excluded / neutral, mirroring `TriStateListDialog`).

Snackbar equivalent: `ui.rs` has no toast yet; add `toast(overlay, text)` implemented with `gtk::Revealer` plus `gtk::Label`, 4 s auto-hide (decision: matches the Material Snackbar short duration of 4 s), placed bottom-centre of the window.

---

## 5. Persistence and locking

### 5.1 Preferences

A `Prefs` trait returns typed values with the defaults from `p1-data-and-config.md` section 3. Storage backend belongs to area:02-preferences-system; this area only consumes it. Used keys listed in `p1-data-and-config.md`.

```rust
pub trait Prefs: Send + Sync {
    fn auto_update_interval_hours(&self) -> u32;                 // default 0
    fn device_restrictions(&self) -> DeviceRestrictions;          // default wifi_only = true
    fn item_restrictions(&self) -> QueueRestrictions;             // defaults per data doc 2.5
    fn update_categories(&self, kind: EntryKind) -> (Vec<i64>, Vec<i64>);
    fn auto_update_metadata(&self) -> bool;
    fn include_season_children(&self) -> bool;
    fn set_last_update_ms(&self, ms: i64);
    fn bump_new_count(&self, kind: EntryKind, by: u32);
    fn new_count(&self, kind: EntryKind) -> u32;
}
```
(`device_restrictions` must parse the StringSet tokens `wifi`, `network_not_metered`, `ac`; unknown tokens are ignored, as Kotlin does.)

### 5.2 Single-writer lock per kind

`library_job_run` table (data doc 7.3) is the lock: the pass inserts a row with `state='running'` in the same transaction that checks there is no other `running` row for the kind. Startup recovery sets stale `running` rows to `cancelled`. (The Kotlin WorkManager lock is process-scoped; the DB lock also survives a crash.)

---

## 6. Error handling

- Pass-level errors: `run_pass` returns `PassOutcome`; it never panics on source errors. Store/DB errors inside an entry become entry failures with the error text; a DB error during the final notification/error log step is logged (`tracing::error!`) and the pass is marked `failure`.
- Error log file: `core::library_update::error_log::write(path, failures, help_url)` writes exactly the format in `p1-behavior.md` section 7; failures to write are logged and the error notification is shown with `log_path = None` (action omitted).
- Logging: `tracing` with `tracing-subscriber` (env filter `kurobayomi=info`). Skip reasons logged at `debug!` (mirrors logcat block).
- Never log source credentials or proxy URLs at info level (DataSaver URLs can contain image hosts; log host only).

---

## 7. Crates to use and why

| Need | Crate (version proposal) | Why |
|---|---|---|
| async runtime | `tokio` 1.x (`rt-multi-thread`, `time`, `sync`, `macros`) | required by the brief |
| cancellation | `tokio-util` (`CancellationToken`) | per-pass cancel |
| background -> UI channel | `async-channel` 2.x | works with `glib::spawn_future_local` |
| GTK | existing `gtk4` 0.11.5 (v4_16) | existing |
| SQLite | `rusqlite` (bundled feature off if system `libsqlite3` used; decide in area:03-db-manga-schema-migrations) | required by the brief |
| HTTP | `reqwest` (`rustls-tls`, `json`), 0.12.x | data saver, source HTTP |
| JSON | `serde`, `serde_json` | existing |
| dates | `chrono` 0.4 | local start-of-day math |
| errors | `thiserror` 2.x | typed errors |
| desktop notifications | `notify-rust` 4.x | freedesktop Notifications with actions |
| D-Bus (NM/UPower/portal) | `zbus` 5.x | pure Rust, async, no libdbus |
| URL encoding | `url` (`form_urlencoded::byte_serialized`) | matches Java URLEncoder for the cases used |
| image cover crop for notifications | `gdk-pixbuf` (via gtk4 dependency) | already in the GTK stack |
| tracing | `tracing`, `tracing-subscriber` | logs |
| tests | built-in `#[test]` and `tokio::test` | no extra crate |

Not used: `async-trait` only for `Source`/`ImageCompressor` (object safety); everything else native async fn.

---

## 8. Android API -> Linux mapping (behaviour side)

| Kotlin/Android | Rust/Linux | Notes |
|---|---|---|
| `CoroutineWorker.doWork` | `run_pass` future | returns `PassOutcome` |
| `setForeground` / `ForegroundInfo` | not needed; optional portal background request | see 2.7 |
| `PeriodicWorkRequestBuilder` + constraints | scheduler loop + `net_state` probe | 2.7, 2.8 |
| `ExistingPeriodicWorkPolicy.UPDATE` | `setup_task` replaces the loop | |
| `ExistingWorkPolicy.KEEP` (startNow) | `start_now` returns false if lock held | |
| `WorkManager.getWorkInfosByTag(...RUNNING)` | `library_job_run` query | |
| `cancelWorkById` | `CancellationToken::cancel` | |
| `NotificationManager.notify` | `notify-rust` | 2.6 |
| `PendingIntent` + `BroadcastReceiver` actions | `notify-rust` action callback -> `PassEvent` / `NotifyAction` on channel | |
| `FileProvider` URI | `gio::File::for_path(p).uri()` | |
| `Intent(ACTION_VIEW, text/plain)` | `gio::AppInfo::launch_default_for_uri` | |
| `Context.createFileInCacheDir` | `glib::user_cache_dir().join("kurobayomi")` | |
| `isConnectedToWifi` | NM `PrimaryConnectionType == "802-11-wireless"` | 2.8 |

---

## 9. Pitfalls for the implementer

1. Do not reproduce P-1 (anime downloads never queued). Wire it.
2. Keep the per-source semaphore semantics (one group per source, sequential inside). Do not parallelise entries of one source.
3. The metadata passes must not share a lock with the update passes; they must not share a name with each other either.
4. Restriction token `manga_ongoing` means "skip COMPLETED". Do not "fix" the name.
5. `unreadCount != 0` (not `> 0`) for the caught-up check; `totalChapters > 0` gate for the not-started check.
6. The release-window check uses tomorrow 00:00 - 1 ms as the cutoff for manual runs too.
7. Sorting of the queue is by title and is applied **after** restrictions; sorting of new items inside a notification is by number (summary) and source order (fetch result).
8. `fetch_interval` negative = manual override. Do not clamp negatives to 1..28.
9. Interval math uses local calendar days, not 24-hour multiples. DST: `NaiveDate` arithmetic, convert once.
10. Notification ids: `entry.id.hashCode()` is Java's int hash of a Long (`(int)(v ^ (v >>> 32))`); Rust port uses a 32-bit fold of the same formula only if reproducing ids is needed (it is not; ids are internal). Use `(kind, id)` mapped to a `u32` via FNV-1a to avoid collisions across kinds (fixes P-11).
11. Error file: Kotlin groups by message including the `null` case; Rust messages are never empty (see 2.1).
12. The "Download" notification action appears only up to 15/10 items; keep the thresholds distinct per kind.
13. Do not call `startDownloads` when nothing was enqueued (Kotlin does this only when `hasDownloads` is true; keep).
14. Keep `library_update_last_timestamp` set at pass start, even when the queue is empty.

---

## 10. Test plan hook

Unit tests (pure): fetch_interval (tables from 2.3, window edges, DST day), build_queue (restriction order, category combinations, season expansion), `new_items_description` (every branch), `percent`, `dedupe_by_number`, `next_unread`, datasaver URL builders (golden strings). Integration (headless, in-memory SQLite + fake Source): run_pass per-source sequencing, error collection, cancellation, lock behaviour. Details in `p1-edge-cases-and-tests.md`.
