# Area 42 (part 2): Rust design, Updates and Upcoming

Target: Kurobayomi (`/home/blackbox/kurobayomi`), Rust stable, gtk4-rs 0.11 (GTK 4.16+, no libadwaita), rusqlite, tokio, chrono. Behaviour contract: `p2-behavior.md`. Data contract: `p2-data-and-config.md`. Widget contract: `p2-ui-parity.md`.

## 1. Crate and module layout

Existing mockup: `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/spec.rs`, `src/entry.rs`, `src/player.rs`, `src/settings.rs`, `src/clamp.rs`. The mockup `updates()` in `src/screens.rs:277` is a stub with hard-coded rows and must be replaced.

New modules (all under `src/`):

```
src/updates/mod.rs            pub use; page builders updates::anime::page(), updates::manga::page()
src/updates/model.rs          MangaUpdate, AnimeUpdate, Row<T>, DownloadState, Ui<T>, group_by_day
src/updates/store.rs          UpdatesStore trait + SqliteUpdatesStore (rusqlite, read-only queries)
src/updates/selection.rs      Selection engine (exact port of section 5 of p2-behavior.md)
src/updates/actions.rs        read/seen, bookmark, fillermark, download, delete dispatch
src/updates/feed.rs           subscription: change signal -> re-query -> Vec<Row<T>>
src/updates/fmt.rs            progress formatting (formatProgress), last-updated text
src/updates/manga.rs          manga page: list widget, toolbar, bottom action bar
src/updates/anime.rs          anime page: same skeleton, anime-specific rows and actions
src/upcoming/mod.rs           page builders upcoming::manga::page(), upcoming::anime::page()
src/upcoming/model.rs         Entry, UpcomingRow, build_upcoming (grouping + events + header_indexes)
src/upcoming/store.rs         upcoming queries (status sets, start-of-day ms)
src/upcoming/calendar.rs      calendar math (week start, week days, leading blanks, month length)
src/upcoming/calendar_ui.rs   gtk::Grid month view, header with Stack transition, day cells, dots
src/upcoming/page.rs          page layout (two-panel vs single column), day-jump scroll
src/time_fmt.rs               relative_date(), relative_time_span() shared with history/other areas
```

Rule: `updates::model`, `updates::selection`, `updates::actions` (decision logic) have no GTK imports so they can be unit-tested without a display. Only `*.rs` files named `*_ui.rs`, `page.rs`, `manga.rs`, `anime.rs` import `gtk4`.

## 2. Crates

| Need | Crate | Why |
|---|---|---|
| GTK4 widgets | `gtk4 = "0.11"` (+ `glib`, `gio`) | Required target |
| SQLite | `rusqlite` (already in Kurobayomi, `bundled` feature) | Same DB file as the rest of the app; SQL is hand-written, not SQLDelight |
| Dates | `chrono = "0.4"` | `NaiveDate`, `Local`, `Weekday`, `Datelike`; replaces `java.time` |
| Async | `tokio` (multi-thread runtime for IO), `async-channel` for GTK delivery | Keep rusqlite calls off the GTK thread |
| Errors | `thiserror` | Typed `UpdatesError` |
| Logging | `tracing` | Replaces `logcat` |
| Week start | `chrono` + a Sunday-first region table (see section 8.2) | No ICU dependency for one value; ceiling noted |
| Opening URLs | `gtk4::UriLauncher` (GTK 4.10+) | Replaces `LocalUriHandler` |

No new crate is needed for the calendar grid: `gtk::Grid` is enough.

## 3. Types

### 3.1 Feed rows

```rust
// src/updates/model.rs
use chrono::NaiveDate;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MangaUpdate {
    pub manga_id: i64,
    pub manga_title: String,
    pub chapter_id: i64,
    pub chapter_name: String,
    pub scanlator: Option<String>,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: i64,
    pub source_id: i64,
    pub date_fetch_ms: i64,
    pub cover_url: Option<String>,
    pub cover_last_modified: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeUpdate {
    pub anime_id: i64,
    pub anime_title: String,
    pub episode_id: i64,
    pub episode_name: String,
    pub scanlator: Option<String>,
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub last_second_seen_ms: i64,
    pub total_ms: i64,
    pub source_id: i64,
    pub date_fetch_ms: i64,
    pub cover_url: Option<String>,
    pub cover_last_modified: i64,
}

/// Mirrors MangaDownload.State / AnimeDownload.State numeric values.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState {
    NotDownloaded = 0,
    Queue = 1,
    Downloading = 2,
    Downloaded = 3,
    Error = 4,
}

/// One list row. `progress` is the download percent as provided by area:23-downloads-manga, area:24-downloads-anime.
#[derive(Clone, Debug)]
pub struct Row<T> {
    pub update: T,
    pub download: DownloadState,
    pub progress: i32,
    pub selected: bool,
}

/// Abstraction used by the selection engine and grouping.
pub trait FeedItem {
    fn key(&self) -> i64;               // chapter_id or episode_id
    fn date_fetch_ms(&self) -> i64;
    fn is_read(&self) -> bool;          // read or seen
}

impl FeedItem for MangaUpdate {
    fn key(&self) -> i64 { self.chapter_id }
    fn date_fetch_ms(&self) -> i64 { self.date_fetch_ms }
    fn is_read(&self) -> bool { self.read }
}
impl FeedItem for AnimeUpdate {
    fn key(&self) -> i64 { self.episode_id }
    fn date_fetch_ms(&self) -> i64 { self.date_fetch_ms }
    fn is_read(&self) -> bool { self.seen }
}

/// Display list item. Items index into the rows vector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ui {
    Header(NaiveDate),
    Item(usize),
}
```

### 3.2 Store

```rust
// src/updates/store.rs
use crate::updates::model::{AnimeUpdate, MangaUpdate};

pub const UPDATES_LIMIT: u32 = 500;          // GetMangaUpdates / GetAnimeUpdates limit = 500
pub const UPDATES_WINDOW_MONTHS: u32 = 3;    // ZonedDateTime.now().minusMonths(3)

#[derive(thiserror::Error, Debug)]
pub enum UpdatesError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
}

pub trait UpdatesStore: Send + Sync {
    /// `after_ms` is compared against chapters.date_upload (strictly greater).
    fn recent_manga(&self, after_ms: i64, limit: u32) -> Result<Vec<MangaUpdate>, UpdatesError>;
    fn recent_anime(&self, after_ms: i64, limit: u32) -> Result<Vec<AnimeUpdate>, UpdatesError>;
}
```

SQL (Proposal, explicit ORDER BY, see `p2-data-and-config.md` section 3.2):

```sql
SELECT mangas._id, mangas.title, chapters._id, chapters.name, chapters.scanlator,
       chapters.read, chapters.bookmark, chapters.last_page_read,
       mangas.source, chapters.date_fetch, mangas.thumbnail_url, mangas.cover_last_modified
FROM mangas JOIN chapters ON mangas._id = chapters.manga_id
WHERE mangas.favorite = 1
  AND chapters.date_fetch > mangas.date_added
  AND chapters.date_upload > ?1
ORDER BY chapters.date_fetch DESC
LIMIT ?2;
```

### 3.3 Selection engine (exact port)

```rust
// src/updates/selection.rs
use std::collections::HashSet;
use crate::updates::model::{FeedItem, Row};

/// Port of MangaUpdatesScreenModel.selectedPositions (Array<Int>(2) = [-1, -1]).
/// Signed on purpose: -1 is the "no anchor" sentinel and comparisons against it
/// are part of the observed behaviour (p2-behavior.md section 5.2 notes).
#[derive(Default, Debug)]
pub struct Selection {
    anchors: [isize; 2],
    ids: HashSet<i64>,
}

impl Selection {
    pub fn new() -> Self { Self { anchors: [-1, -1], ids: HashSet::new() } }

    pub fn selected_ids(&self) -> &HashSet<i64> { &self.ids }

    /// Port of toggleSelection(item, selected, userSelected, fromLongPress).
    pub fn toggle<T: FeedItem>(
        &mut self,
        rows: &mut [Row<T>],
        id: i64,
        selected: bool,
        user_selected: bool,
        from_long_press: bool,
    ) {
        let Some(idx) = rows.iter().position(|r| r.update.key() == id) else { return };
        if rows[idx].selected == selected { return; }
        let first_selection = !rows.iter().any(|r| r.selected);
        rows[idx].selected = selected;
        if selected { self.ids.insert(id); } else { self.ids.remove(&id); }
        let i = idx as isize;

        if selected && user_selected && from_long_press {
            if first_selection {
                self.anchors = [i, i];
            } else {
                let range: std::ops::Range<isize> = if i < self.anchors[0] {
                    let r = (i + 1)..self.anchors[0];
                    self.anchors[0] = i;
                    r
                } else if i > self.anchors[1] {
                    let r = (self.anchors[1] + 1)..i;
                    self.anchors[1] = i;
                    r
                } else {
                    0..0
                };
                for j in range {
                    let j = j as usize;
                    if j < rows.len() && !rows[j].selected {
                        rows[j].selected = true;
                        self.ids.insert(rows[j].update.key());
                    }
                }
            }
        } else if user_selected && !from_long_press {
            if !selected {
                if i == self.anchors[0] {
                    self.anchors[0] = rows.iter().position(|r| r.selected).map_or(-1, |p| p as isize);
                } else if i == self.anchors[1] {
                    self.anchors[1] = rows.iter().rposition(|r| r.selected).map_or(-1, |p| p as isize);
                }
            } else if i < self.anchors[0] {
                self.anchors[0] = i;
            } else if i > self.anchors[1] {
                self.anchors[1] = i;
            }
        }
    }

    /// Port of toggleAllSelection(selected).
    pub fn set_all<T: FeedItem>(&mut self, rows: &mut [Row<T>], selected: bool) {
        for r in rows.iter_mut() {
            r.selected = selected;
            if selected { self.ids.insert(r.update.key()); } else { self.ids.remove(&r.update.key()); }
        }
        self.anchors = [-1, -1];
    }

    /// Port of invertSelection().
    pub fn invert<T: FeedItem>(&mut self, rows: &mut [Row<T>]) {
        for r in rows.iter_mut() {
            r.selected = !r.selected;
            if r.selected { self.ids.insert(r.update.key()); } else { self.ids.remove(&r.update.key()); }
        }
        self.anchors = [-1, -1];
    }

    /// Re-apply the id set after a feed refresh (toUpdateItems semantics).
    pub fn apply_to<T: FeedItem>(&self, rows: &mut [Row<T>]) {
        for r in rows.iter_mut() { r.selected = self.ids.contains(&r.update.key()); }
    }

    pub fn anchors(&self) -> [isize; 2] { self.anchors }
}
```

Notes on the port:
- `rows.iter().position` replaces `indexOfFirst`; `rposition` replaces `indexOfLast`.
- `apply_to` mirrors `toUpdateItems` computing `selected` from the set; anchors are intentionally not recomputed, matching Aniyomi.
- Kotlin `IntRange` with exclusive end is `Range<isize>` here. `(i + 1)..anchor0` is empty when `i + 1 >= anchor0`, same as Kotlin.

### 3.4 Grouping

```rust
// src/updates/model.rs (continued)
use chrono::{Local, TimeZone};

/// Port of State.getUiModel(): header before the first item of each local date of dateFetch.
pub fn group_by_day<T: FeedItem>(rows: &[Row<T>]) -> Vec<Ui> {
    let mut out = Vec::with_capacity(rows.len() + 8);
    let mut prev: Option<NaiveDate> = None;
    for (i, r) in rows.iter().enumerate() {
        let d = local_date(r.update.date_fetch_ms());
        if prev != Some(d) {
            out.push(Ui::Header(d));
        }
        out.push(Ui::Item(i));
        prev = Some(d);
    }
    out
}

/// Epoch millis to local calendar date (Long.toLocalDate in Kotlin).
pub fn local_date(ms: i64) -> NaiveDate {
    Local.timestamp_millis_opt(ms).single().map(|t| t.date_naive())
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}
```

Check against `insertSeparators`: header is emitted when `beforeDate != afterDate` and `afterDate != null`. The first item has `before = null` so `prev` starts as `None` and always produces a header, identical to the Kotlin. `Local.timestamp_millis_opt(...).single()` returns `None` in a DST gap (never for an instant), so the fallback is only for out-of-range millis.

### 3.5 Upcoming model

```rust
// src/upcoming/model.rs
use chrono::NaiveDate;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: i64,
    pub title: String,
    pub next_update_ms: i64,      // expectedNextUpdate, already null-checked
    pub cover_url: Option<String>,
    pub cover_last_modified: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpcomingRow {
    Header { date: NaiveDate, count: u32 },
    Item(usize),                  // index into the entries Vec (ascending order)
}

#[derive(Default, Debug)]
pub struct UpcomingIndex {
    pub rows: Vec<UpcomingRow>,           // ascending, headers before their group
    pub events: BTreeMap<NaiveDate, u32>, // day -> count (calendar dots)
    pub header_index: BTreeMap<NaiveDate, usize>, // day -> index in rows
}

/// `entries` must be sorted ascending by next_update_ms (the SQL ORDER BY guarantees it).
pub fn build_upcoming(entries: &[Entry]) -> UpcomingIndex {
    // Compute group counts exactly as insertSeparatorsReversed does: a header carries
    // the number of items whose local date equals the header's date.
    let dates: Vec<NaiveDate> = entries.iter().map(|e| crate::updates::model::local_date(e.next_update_ms)).collect();
    let mut rows = Vec::with_capacity(entries.len() + 8);
    let mut index = UpcomingIndex::default();
    let mut i = 0;
    while i < entries.len() {
        let d = dates[i];
        let mut j = i;
        while j < entries.len() && dates[j] == d { j += 1; }
        let count = (j - i) as u32;
        rows.push(UpcomingRow::Header { date: d, count });
        index.events.insert(d, count);
        index.header_index.insert(d, rows.len() - 1);
        for k in i..j { rows.push(UpcomingRow::Item(k)); }
        i = j;
    }
    index.rows = rows;
    index
}
```

Equivalence with the Kotlin: for an ascending list the reversed-separator algorithm yields `Header(d, group_count)` before each group, which is what the forward loop above produces. Header index used for day jump is the index in `rows` (the Kotlin uses the index in `items`, which equals `rows` since the calendar is not part of the `items` list).

### 3.6 Upcoming store

```rust
// src/upcoming/store.rs
pub const UPCOMING_MANGA_STATUSES: [i64; 2] = [1, 4];       // ONGOING, PUBLISHING_FINISHED
pub const UPCOMING_ANIME_STATUSES: [i64; 3] = [1, 4, 7];    // + UPCOMING

/// Local midnight today in epoch milliseconds (Kotlin: toEpochSecond() * 1000).
pub fn start_of_today_ms(now: chrono::DateTime<chrono::Local>) -> i64 {
    use chrono::TimeZone;
    let midnight = now.date_naive().and_hms_opt(0, 0, 0).expect("midnight");
    chrono::Local.from_local_datetime(&midnight).earliest()
        .map(|t| t.timestamp_millis())
        .unwrap_or_else(|| now.timestamp_millis())
}

pub trait UpcomingStore: Send + Sync {
    fn upcoming_manga(&self, start_of_day_ms: i64, statuses: &[i64]) -> rusqlite::Result<Vec<crate::upcoming::model::Entry>>;
    fn upcoming_anime(&self, start_of_day_ms: i64, statuses: &[i64]) -> rusqlite::Result<Vec<crate::upcoming::model::Entry>>;
}
```

SQL per table (`mangas`, `animes` the same shape):
```sql
SELECT _id, title, thumbnail_url, cover_last_modified, next_update, status
FROM mangas
WHERE next_update >= ?1 AND favorite = 1 AND status IN (?2, ?3)  -- placeholder list length = statuses.len()
ORDER BY next_update ASC;
```
Build the `IN` list with `rusqlite::params_from_iter`. Do not string-interpolate ids.

Important: `expectedNextUpdate` returns null when `status == COMPLETED (2)`. The status set never includes 2, so `next_update` is always used. Keep a debug assert.

### 3.7 Calendar math

```rust
// src/upcoming/calendar.rs
use chrono::{Datelike, NaiveDate, Weekday};

pub const DAYS_OF_WEEK: usize = 7;
pub const MAX_DOTS: usize = 3;

/// Port of WeekFields.of(Locale).firstDayOfWeek. Region table in section 8.2.
/// `locale` is a POSIX locale string such as "en_US.UTF-8" (LC_TIME, falling back to LANG).
pub fn week_start(locale: &str) -> Weekday {
    let base = locale.split('.').next().unwrap_or("");
    let region = base.split('_').nth(1).unwrap_or("").to_ascii_uppercase();
    match region.as_str() {
        "AF" | "IR" => Weekday::Sat,
        "US" | "CA" | "JP" | "IL" | "BR" | "IN" | "KR" | "PH" | "MX" | "TW" | "ZA" | "SA" | "HK" => Weekday::Sun,
        _ => Weekday::Mon,
    }
}

/// Port of weekDays: (0 until 7).map { DayOfWeek.of((first - 1 + it) % 7 + 1) }
pub fn week_days(first: Weekday) -> [Weekday; DAYS_OF_WEEK] {
    let f = first.num_days_from_monday() as usize; // Mon=0
    std::array::from_fn(|i| {
        let idx = (f + i) % DAYS_OF_WEEK;
        match idx { 0 => Weekday::Mon, 1 => Weekday::Tue, 2 => Weekday::Wed, 3 => Weekday::Thu,
                    4 => Weekday::Fri, 5 => Weekday::Sat, _ => Weekday::Sun }
    })
}

/// Port of emptyFieldCount = weekDays.indexOf(first-of-month.dayOfWeek).
pub fn leading_blanks(week: &[Weekday; DAYS_OF_WEEK], month: NaiveDate) -> usize {
    let first = NaiveDate::from_ymd_opt(month.year(), month.month(), 1).unwrap();
    week.iter().position(|w| *w == first.weekday()).unwrap_or(0)
}

/// Port of YearMonth.lengthOfMonth().
pub fn days_in_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 { NaiveDate::from_ymd_opt(year + 1, 1, 1) }
               else { NaiveDate::from_ymd_opt(year, month + 1, 1) };
    (next.unwrap() - NaiveDate::from_ymd_opt(year, month, 1).unwrap()).num_days() as u32
}

/// Port of YearMonth.minusMonths(1) / plusMonths(1) (no clamping needed: day is always 1).
pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + (month as i32 - 1) + delta;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}
```

Note: `YearMonth` is modelled as `(year, month)` tuples (or `NaiveDate` first of month) to keep ordering trivial (`(y, m)` tuple comparison matches `YearMonth.compareTo`).

### 3.8 Relative date text

```rust
// src/time_fmt.rs
use chrono::NaiveDate;

pub enum DateStyle { Relative, Absolute }

/// Port of LocalDate.toRelativeString(context, relative, dateFormat).
/// `format_abs` formats the absolute date (pref app_date_format, default locale SHORT).
pub fn relative_date(date: NaiveDate, today: NaiveDate, relative: bool, format_abs: impl Fn(NaiveDate) -> String) -> String {
    if !relative { return format_abs(date); }
    let diff = (today - date).num_days();   // Kotlin: ChronoUnit.DAYS.between(this, now)
    match diff {
        d if d < -7 => format_abs(date),
        d if d < 0 => plural_upcoming(d.unsigned_abs()),      // "Tomorrow" | "In %d days"
        0 => "Today".to_string(),
        d if d < 7 => plural_relative(d as u64),              // "Yesterday" | "%d days ago"
        _ => format_abs(date),
    }
}

fn plural_upcoming(n: u64) -> String { if n == 1 { "Tomorrow".into() } else { format!("In {n} days") } }
fn plural_relative(n: u64) -> String { if n == 1 { "Yesterday".into() } else { format!("{n} days ago") } }

/// Port of relativeTimeSpanString(epochMillis).
pub fn relative_time_span(epoch_ms: i64, now_ms: i64) -> String {
    if epoch_ms <= 0 { return "Never".into(); }
    if now_ms - epoch_ms < 60_000 { return "Just now".into(); }
    let mins = (now_ms - epoch_ms) / 60_000;
    let (n, unit) = match mins {
        m if m < 60 => (m, "minute"),
        m if m < 60 * 24 => (m / 60, "hour"),
        m => (m / (60 * 24), "day"),
    };
    if n == 1 { format!("1 {unit} ago") } else { format!("{n} {unit}s ago") }
}
```

Proposal caveat: exact `DateUtils.getRelativeTimeSpanString` wording and plural rules are Android-specific. The three cases (Never, Just now, relative) are preserved; ceiling documented in `p2-behavior.md` section 6.5.

### 3.9 Actions

```rust
// src/updates/actions.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadAction { Start, StartNow, Cancel, Delete, ShowQualities /* anime only */ }

/// Port of ChapterDownloadAction / EpisodeDownloadAction dispatch.
/// `Outcome::clear_selection` reflects the observed early-return that skips toggleAllSelection
/// for multi-select START_NOW / CANCEL / SHOW_QUALITIES.
pub struct Outcome { pub clear_selection: bool }

pub fn dispatch_download_precondition(action: DownloadAction, count: usize) -> Option<Outcome> {
    match (action, count) {
        (_, 0) => None,
        (DownloadAction::Start | DownloadAction::Delete, _) => Some(Outcome { clear_selection: true }),
        (DownloadAction::StartNow | DownloadAction::Cancel | DownloadAction::ShowQualities, 1) => Some(Outcome { clear_selection: true }),
        (DownloadAction::StartNow | DownloadAction::Cancel | DownloadAction::ShowQualities, _) => Some(Outcome { clear_selection: false }),
    }
}

/// Port of the enqueue loop: group by manga id (first-seen order), skip a group when
/// `source_installed(manga_id)` is false (caller resolves manga -> source and checks the extension).
pub fn plan_enqueue<T, F>(items: &[T], manga_of: impl Fn(&T) -> i64, source_installed: F) -> Vec<(i64, Vec<usize>)>
where F: Fn(i64) -> bool,
{
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    let mut order: Vec<i64> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let m = manga_of(it);
        if !groups.contains_key(&m) { order.push(m); }
        groups.entry(m).or_default().push(i);
    }
    order.into_iter().filter_map(|m| {
        let src_ok = source_installed(m);       // caller resolves manga->source; false = skip group
        src_ok.then(|| (m, groups.remove(&m).unwrap()))
    }).collect()
}
```

Read/seen and bookmark/fillermark planners (pure functions, unit-tested):

```rust
/// Port of SetReadStatus filter. Returns indices of chapters that will be written.
pub fn read_targets(read: bool, rows: &[(bool /*read*/, i64 /*last_page_read*/)]) -> Vec<usize> {
    rows.iter().enumerate().filter(|(_, (r, lpr))| if read { !*r } else { *r || *lpr > 0 }).map(|(i, _)| i).collect()
}
/// lastPageRead written: 0 when marking unread, untouched when marking read.
pub fn read_update_value(read: bool) -> Option<i64> { if read { None } else { Some(0) } }

/// Port of bookmarkUpdates / fillermarkUpdates filter (filterNot { flag == target }).
pub fn flag_targets(target: bool, current: &[bool]) -> Vec<usize> {
    current.iter().enumerate().filter(|(_, c)| **c != target).map(|(i, _)| i).collect()
}
```

Same for `seen`/`lastSecondSeen` (anime) with `lastSecondSeen` written as `0` when unseen.

### 3.10 Feed subscription (threading)

```rust
// src/updates/feed.rs
use std::sync::Arc;

/// A change signal from the DB layer (writes through the app) or download manager.
#[derive(Clone, Copy, Debug)]
pub enum ChangeKind { Chapters, Episodes, Downloads, Favorites }

pub struct Feed<S: UpdatesStore + 'static> {
    store: Arc<S>,
    runtime: tokio::runtime::Handle,
}

impl<S: UpdatesStore + 'static> Feed<S> {
    /// Re-query on the blocking pool and deliver to the GTK main loop.
    pub fn refresh_manga(&self, window_after_ms: i64, tx: async_channel::Sender<Result<Vec<MangaUpdate>, UpdatesError>>) {
        let store = self.store.clone();
        self.runtime.spawn_blocking(move || {
            let res = store.recent_manga(window_after_ms, UPDATES_LIMIT);
            let _ = tx.send_blocking(res);
        });
    }
}
```

Consumer side (GTK thread):

```rust
glib::spawn_future_local(async move {
    while let Ok(res) = rx.recv().await {
        match res {
            Ok(list) => page.set_rows(list),          // replaces rows; re-applies selection set
            Err(e) => { tracing::error!(?e); page.show_toast(INTERNAL_ERROR); }  // keep loop alive
        }
    }
});
```

Rules:
- The Kotlin `catch` terminates the flow. The Rust consumer keeps the loop alive, so an error does not freeze the list (intentional deviation, recorded in `p2-edge-cases-and-tests.md`).
- Change signals are coalesced: at most one query in flight per feed; a new signal during a query sets a `dirty` flag and triggers one more query after completion. Proposal: 100 ms debounce on change signals.
- Download progress is NOT a re-query. It patches the row in place by `chapter_id`, like `updateDownloadState`. Progress ticks come from the download manager on the GTK thread through the same `async_channel`.

### 3.11 Window and cut-off time

`window_after_ms` is computed once when the page is constructed: `Local::now()` minus 3 calendar months, converted to ms. Use `chrono::Months::new(3)` via `Local::now().checked_sub_months(Months::new(3))`. Matches `ZonedDateTime.now().minusMonths(3)` (computed once per ScreenModel).

## 4. Page wiring into the mockup

`src/screens.rs` has `fn updates() -> Option<Built>` (line 277). Replace the body with:

```rust
fn updates() -> Option<Built> {
    let s = screen("updates");
    let content = kind_tabs(|anime| {
        if anime { crate::updates::anime::page() } else { crate::updates::manga::page() }
    });
    let mut end = toolbar(&s);
    end.reverse();
    built(content, end)
}
```

- `kind_tabs` already produces the Anime | Manga switcher (`src/screens.rs:70`). Keep it.
- The mockup's hard-coded rows (`for (day, range) in [...]`) and "Library last updated: 5 minutes ago" are removed.
- The toolbar actions "View Upcoming Updates" (calendar icon) and "Update library" (refresh icon) must be wired: the first calls `ui::push(&crate::upcoming::page(kind))`, the second calls the library update API.
- Spec ids used by the mockup's `spec/app.json` (`updates`, `updates_delete_episodes`, `updates_delete_chapters`, `"empty": "No recent updates"`) are read through `spec::with_spec` and `ui::screen("updates")`. Keep them.

## 5. GTK widget choices

| Aniyomi element | GTK4 widget | Notes |
|---|---|---|
| FastScrollLazyColumn of updates | `gtk::ListBox` with `SelectionMode::None` and our own `Selection` | ListBox selection would fight the custom engine. Header rows are non-activatable `ListBoxRow`s with `ListGroupHeader` style (label, `heading` CSS class). |
| Row 56 px tall | `gtk::ListBoxRow` with `set_size_request(-1, 56)` | Cover `gtk::Picture` 40 px wide, fills height minus 6 px margins |
| Long press | `gtk::GestureLongPress` (`set_delay_factor` default) | Maps to `toggle(user=true, long_press=true)` |
| Ctrl/Shift click | `gtk::GestureClick` with `current_event_state()` modifiers | Desktop equivalent of range select (section 7) |
| Pull-to-refresh | none | Refresh button + F5 (drop, see `p2-behavior.md` section 14) |
| Snackbar | `gtk::Overlay` + `gtk::Revealer` (SlideUp) with `gtk::Label` | Auto-hide 4000 ms |
| Calendar grid | `gtk::Grid` with 7 columns | Header row labels from `calendar::week_days` |
| Calendar month title change | `gtk::Stack` with `transition_type(SlideUp / SlideDown)` and `transition_duration(200)` | Forward = SlideUp, backward = SlideDown |
| Two-panel layout | `gtk::Paned` (position = calendar width) or horizontal `gtk::Box` | Use `Paned` so the user can drag the divider |
| Badge in date header | `gtk::Label` with CSS class `badge` and `background` = accent | Tokyo Night accent via the user's theme |
| Dot indicator | `gtk::DrawingArea` (Cairo circles) or CSS `border-radius: 50%` box | Cairo keeps alpha exact |

## 6. Async and threading model

- GTK main loop: all widget work, selection state, `Rc<RefCell<PageState>>`.
- DB: `rusqlite::Connection` opened read-only for queries (WAL), in a `tokio::task::spawn_blocking` closure. One connection per page is fine; cache connection in a `thread_local` pool if needed.
- Writes (read/seen, bookmark, fillermark, delete): `spawn_blocking`, fire-and-forget like Kotlin's `launchIO`, then `dirty` the feed. Mirror `withNonCancellableContext`: do not cancel writes on page close (run them to completion on the blocking pool).
- Downloads: calls into the download module (area:23-downloads-manga, area:24-downloads-anime) are non-blocking enqueue calls; progress arrives via `async_channel` to the GTK thread.
- Library update: `updates::actions::trigger_library_update() -> bool` returns `false` when a run is active (single-flight `AtomicBool` guard owned by the library update module, area:26-library-update-jobs). The page shows "Updating library" or "An update is already running".
- Never call `Connection` on the GTK thread. A debug assertion `glib::MainContext::default().is_owner()` is cheap and catches this.

## 7. Keyboard and mouse mapping (desktop)

| Action | Keys / pointer |
|---|---|
| Toggle one row | Ctrl+click |
| Range select | Shift+click (from the anchor) |
| Select all | Ctrl+A |
| Invert selection | Ctrl+I |
| Clear selection | Escape |
| Open update | Enter or double-click (activate) |
| Open entry | Click on cover |
| Refresh library | F5 or refresh button |
| Long press select (Aniyomi parity) | Long press on the row (touch screens only) |

Shift+click uses the same arithmetic as long-press range selection (`toggle(user=true, long_press=true)`), so it reproduces Aniyomi exactly. Ctrl+click uses the tap branch (`user=true, long_press=false`) with `selected = !row.selected`.

## 8. Platform mapping (Android to Linux)

### 8.1 API mapping

| Android / Kotlin | Linux / Rust |
|---|---|
| Voyager `Screen`, `ScreenModel`, `Navigator.push/pop` | Rust structs + `ui::push` / `ui::pop` (mockup `src/ui.rs:34`, `:40`) |
| `StateScreenModel` + `MutableStateFlow` | `Rc<RefCell<State>>` + explicit `render()` |
| `Flow.combine`, `collectLatest` | `async_channel` + `glib::spawn_future_local` |
| `Channel(Int.MAX_VALUE)` events | `async_channel::unbounded` |
| `SnackbarHostState` | `gtk::Overlay` + `gtk::Revealer` toast |
| `LocalUriHandler.openUri` | `gtk::UriLauncher::new(uri).launch(...)` |
| `WorkManager.isRunning(TAG)` | `AtomicBool` in the library update module |
| `ExistingWorkPolicy.KEEP` unique work | single-flight guard, return false when running |
| `LocalHapticFeedback` | none (drop) |
| `BackHandler` | `EventControllerKey` Escape handler on the page |
| `combinedClickable` | `GestureClick` + `GestureLongPress` |
| `Injekt.get()` | constructor injection (`Arc<dyn UpdatesStore>` and the same pattern for the other services) |
| `java.time.*` | `chrono::{NaiveDate, Local, Weekday, Months}` |
| `ZoneId.systemDefault()` | `chrono::Local` |
| `DateUtils.getRelativeTimeSpanString` | `time_fmt::relative_time_span` |
| `WeekFields.of(Locale).firstDayOfWeek` | `calendar::week_start(locale)` (section 8.2) |
| `DateTimeFormatter.ofPattern("MMMM yyyy")` | `chrono` format `"%B %Y"` (localised names need `LC_TIME`; English fallback) |
| `Configuration.smallestScreenWidthDp` | GTK window width (section 9 of behaviour) |
| `TwoPanelBox` | `gtk::Paned` |
| `FastScrollLazyColumn` | `gtk::ScrolledWindow` + `gtk::ListBox` |
| `ItemCover.Square/Book` | `gtk::Picture` in a fixed-size `gtk::Frame` (area:34-images-covers-cache loader) |

### 8.2 Week start

`calendar::week_start(locale: &str) -> Weekday`:
- Parse the region from the locale string (`LC_TIME` or `LANG`, e.g. `en_US.UTF-8` -> `US`).
- Sunday-first regions (small table, ceiling documented): `US, CA, JP, IL, BR, IN, KR, PH, MX, TW, ZA, SA, HK`. Saturday-first regions: `AF, IR`. Everything else: Monday. The table is the `match` in `calendar::week_start` (section 3.7).
- Proposal: user override `upcoming.week_start` (section 4.1 of data doc).
- Ceiling: a full table would come from CLDR `firstDay` data (`icu` or `cldr-week` crate). Add when a user with a non-listed locale asks.

Regions with Saturday-first (`IR`, `AF`) are mapped to Saturday, not Sunday. Document in the table.

## 9. Error handling

- `UpdatesError` (thiserror) for DB errors. Logged with `tracing::error!`. User sees a toast with the exact Aniyomi text `InternalError: Check crash logs for further information` (MR.strings.internal_error). Proposal: same text, because it is the string key's value; a log path hint can be added in the toast detail.
- Missing source or missing manga during enqueue or delete: skip group, no toast (Aniyomi silent skip). Proposal: `tracing::warn!` only.
- Write failures (`SetReadStatus.Result.InternalError`) are silent in Aniyomi. Proposal: show the internal-error toast, since silent data loss is worse than an extra toast. Recorded as a deliberate deviation in the edge-case file.

## 10. Pitfalls

1. Time zones: all day grouping uses `Local`. Do not use `Utc` date for headers, or late-evening updates land on the wrong day.
2. Milliseconds everywhere: `next_update`, `date_fetch`, `date_upload`, `last_second_seen`, `total_seconds` are ms. `start_of_today_ms` converts from seconds internally. Do not mix with seconds.
3. The SQL view `ORDER BY` is not guaranteed in the outer query. Always write `ORDER BY` explicitly.
4. The `IN (...)` list needs one placeholder per status. Use `params_from_iter`.
5. Selection anchors are indices into the current list. Any refresh that reorders rows must call `Selection::apply_to`, and anchors may point to stale indices (preserved quirk). Consider resetting anchors on refresh as a documented improvement; default is to match Aniyomi.
6. `DownloadState` values must stay 0..4 (matching the Kotlin `value` ints) because the download module may persist or compare them.
7. GTK `ListBox` rows are reused by the widget system only when using a factory; with `ListBox::append` every row is a widget. 500 rows is fine. Above ~2000 rows switch to `gtk::ListView` with a model.
8. Upcoming `start_of_today_ms` is computed at page creation. Re-query on day change, or keep as Aniyomi (no re-query until the page is reopened). Proposal: re-query when the window regains focus after midnight.
9. Two `Rc` references to the same `PageState` inside a GTK closure leak if not `Weak`-downgraded; use `Weak` in signal handlers.
10. `Selection` is pure; keep it free of GTK types so the unit tests in `p2-edge-cases-and-tests.md` run on CI without a display.

## 11. Public API summary (what other areas call)

```rust
pub fn updates::manga::page() -> gtk4::Widget;
pub fn updates::anime::page() -> gtk4::Widget;
pub fn upcoming::manga::page() -> gtk4::Widget;
pub fn upcoming::anime::page() -> gtk4::Widget;
pub fn updates::actions::trigger_library_update(kind: Kind) -> bool;   // used by toolbar and F5
pub fn updates::feed::reset_new_count(kind: Kind);                      // on enter/leave
pub fn time_fmt::relative_date(date: NaiveDate, today: NaiveDate, relative: bool, format_abs: impl Fn(NaiveDate) -> String) -> String;
pub fn time_fmt::relative_time_span(epoch_ms: i64, now_ms: i64) -> String;
```

`Kind` is `enum Kind { Manga, Anime }`, defined in `src/updates/mod.rs`.
