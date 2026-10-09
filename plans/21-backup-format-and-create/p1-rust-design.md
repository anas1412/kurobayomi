# Area 21: Rust design for backup format, create, validation and auto-backup

Target crate: `kurobayomi` (`/home/blackbox/kurobayomi/Cargo.toml`, edition 2024, `gtk` = gtk4 0.11.5 with feature `v4_16`, `serde`, `serde_json`). Module root: `src/backup/`. Behaviour references are B-nn in p1-behavior.md, data references in p1-data-and-config.md.

---

## 1. Decisions at a glance

1. Wire format: hand-written protobuf codec in `src/backup/wire.rs`. No `prost`, no `protobuf` crate. Reason: kotlinx ProtoBuf has four behaviours that the standard generators get wrong by default: repeated scalars are unpacked here (prost proto3 packs by default), sealed `PreferenceValue` uses a kotlinx-specific layout, enums are ordinals, and absent nullable fields must stay absent. A 300-line codec with explicit field tables is smaller than a generator plus workarounds, and it is fixture-testable (see topic file).
2. Compression: `flate2` (`GzEncoder` / `MultiGzDecoder`). Reason: pure Rust backend (miniz_oxide), no system zlib, gzip is the only container.
3. Scheduling: in-process `tokio` interval loop (10 minutes) driven by a persisted `last_auto_epoch_ms` (not an OS job queue). A systemd user timer is P2 (task T-21a-041).
4. Single-flight: one `tokio::sync::Mutex<()>` job slot shared by manual and automatic runs.
5. Database: `rusqlite` (bundled SQLite), one read transaction per backup, run on `tokio::task::spawn_blocking`.
6. UI thread: GTK main loop. Background work is `tokio`; results return to GTK through an `async_channel` consumed by `glib::spawn_future_local`.
7. Desktop notifications: `notify-rust` (freedesktop `org.freedesktop.Notifications`). "Share" action is replaced by "Show in folder" through `org.freedesktop.FileManager1.ShowItems` (zbus).
8. Battery condition: UPower through zbus.
9. Errors: `thiserror` enums; user-visible text comes from the message key table (B-16), never from `Display` of internal errors.
10. Logging: `tracing` (replaces `logcat`).

---

## 2. Module layout

```
src/backup/
  mod.rs          pub use; BackupError; message keys (B-16)
  wire.rs         Writer, Reader, WireType, Message trait, WireError        (T-21a-001)
  schema.rs       Backup, LegacyBackup, entry structs, enums, Message impls (T-21a-002..006, 009)
  format.rs       sniff(), read_backup_bytes(), legacy detection, gzip      (T-21a-007, 008)
  options.rs      BackupOptions (+ labels, enabled rules, canCreate)        (T-21a-010)
  creator/
    mod.rs        create_bytes(), build_backup(), file write, retention      (T-21a-012, 022, 025, 036)
    manga.rs      manga entries, excluded scanlators, chapters, tracking, history (T-21a-013)
    anime.rs      anime entries, episodes, tracking, history                 (T-21a-014)
    categories.rs manga and anime category lists                             (T-21a-015)
    sources.rs    source references and stub names                           (T-21a-016)
    stores.rs     anime extension stores, manga extension repos              (T-21a-017)
    custom_buttons.rs                                                        (T-21a-018)
    extensions.rs installed extension artifacts (opt-in)                     (T-21a-019)
    preferences.rs app and per-source preferences, key rules                 (T-21a-020, 021, 044)
  validate.rs     preflight (missing sources, trackers), self-check         (T-21a-023, 024)
  auto.rs         automatic scheduler loop, retention, battery, backoff     (T-21a-026, 032, 037, 039)
  job.rs          manual job, single-flight slot, cancellation              (T-21a-027)
  notify.rs       desktop notifications, progress, actions                  (T-21a-028, 029, 030, 042)
  settings.rs     settings rows and keys for the Backup and restore group   (T-21a-031, 032)
  schema_text.rs  generated .proto-style text dump for the debug screen     (T-21a-035)
tests/backup/     fixtures and integration tests                           (T-21a-034, 040)
```

`main.rs` gets `mod backup;`. The mockup's existing `src/settings.rs` arm for `settings_backup_schema` is replaced by `backup::schema_text::TEXT` (T-21a-035).

---

## 3. Wire codec (`src/backup/wire.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireType { Varint = 0, Fixed64 = 1, Len = 2, Fixed32 = 5 }

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("truncated input")]
    Truncated,
    #[error("varint longer than 10 bytes")]
    VarintTooLong,
    #[error("unsupported wire type {0}")]
    BadWireType(u8),
    #[error("field {field} has wire type {got:?}, expected {want:?}")]
    WrongWireType { field: u32, got: WireType, want: WireType },
    #[error("required field {0} missing")]
    MissingField(&'static str),
    #[error("unknown enum ordinal {ordinal} for {name}")]
    BadEnum { name: &'static str, ordinal: u64 },
    #[error("invalid UTF-8 in string field {0}")]
    Utf8(u32),
}

pub struct Writer { buf: Vec<u8> }

impl Writer {
    pub fn new() -> Self { Self { buf: Vec::new() } }
    pub fn int64(&mut self, field: u32, v: i64);          // wire 0, two's complement as u64 (10 bytes if negative)
    pub fn int32(&mut self, field: u32, v: i32);          // wire 0, sign-extended to 64 bits (10 bytes if negative)
    pub fn boolean(&mut self, field: u32, v: bool);       // wire 0, 0/1
    pub fn enum_ordinal(&mut self, field: u32, v: u8);    // wire 0
    pub fn float32(&mut self, field: u32, v: f32);        // wire 5, little endian
    pub fn float64(&mut self, field: u32, v: f64);        // wire 1, little endian
    pub fn string(&mut self, field: u32, v: &str);        // wire 2
    pub fn bytes(&mut self, field: u32, v: &[u8]);        // wire 2
    pub fn message<M: Message>(&mut self, field: u32, m: &M); // wire 2, length prefix from a nested Writer
    pub fn finish(self) -> Vec<u8> { self.buf }
}

pub struct Reader<'a> { buf: &'a [u8], pos: usize }

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self { Self { buf, pos: 0 } }
    /// Next (field number, wire type), or None at end of input.
    pub fn next_tag(&mut self) -> Result<Option<(u32, WireType)>, WireError>;
    pub fn varint(&mut self) -> Result<u64, WireError>;
    pub fn fixed32(&mut self) -> Result<[u8; 4], WireError>;
    pub fn fixed64(&mut self) -> Result<[u8; 8], WireError>;
    pub fn len_delimited(&mut self) -> Result<&'a [u8], WireError>;
    pub fn skip(&mut self, wt: WireType) -> Result<(), WireError>;   // unknown fields
}

pub trait Message: Sized {
    fn encode(&self, w: &mut Writer);
    fn decode(r: &mut Reader<'_>) -> Result<Self, WireError>;
    fn to_bytes(&self) -> Vec<u8> { let mut w = Writer::new(); self.encode(&mut w); w.finish() }
    fn from_bytes(b: &[u8]) -> Result<Self, WireError> { Self::decode(&mut Reader::new(b)) }
}
```

Rules implemented here (all observed in kotlinx ProtoBuf, see p1-data-and-config.md top):
- Field number 0 is invalid: `next_tag` returns `BadWireType`-style error (kotlinx treats it as malformed).
- Unknown field numbers are `skip`ped by the decoder loop in each message (not an error).
- Wrong wire type for a known field number is an error (`WrongWireType`), which becomes "corrupted" at the backup level.
- `Reader::len_delimited` bounds-checks the length against the remaining buffer before slicing (no panic on hostile input).
- Repeated fields: each element is one tag; decoders push into a `Vec`. Scalars in repeated fields are NOT packed on encode. On decode, accept BOTH packed (wire 2) and unpacked (wire 0) for `categories` and similar `Vec<i64>` fields, for robustness (kotlinx-generated files are unpacked; accepting packed costs ten lines).
- Nullable: `Option<T>` is `None` when the tag is absent; encoder writes only `Some`.
- Non-null with default: decoder starts from the default value and overwrites when the tag is present; encoder always writes.
- Non-null required: decoder sets a flag and returns `MissingField` at the end if absent.

Encoder sizes: messages are built into a nested `Writer` then length-prefixed; for a 10 000-manga library this is O(total bytes) with one extra copy per nesting level (acceptable; see pitfalls).

---

## 4. Schema types (`src/backup/schema.rs`)

```rust
pub struct Backup {
    pub manga: Vec<BackupManga>,                       // 1
    pub categories: Vec<BackupCategory>,               // 2
    pub sources: Vec<BackupSource>,                    // 101
    pub preferences: Vec<BackupPreference>,            // 104
    pub source_preferences: Vec<BackupSourcePreferences>, // 105
    pub manga_extension_repos: Vec<BackupExtensionRepos>, // 106
    pub is_legacy: bool,                               // 500 (default true when absent; false when written)
    pub anime: Vec<BackupAnime>,                       // 501
    pub anime_categories: Vec<BackupCategory>,         // 502
    pub anime_sources: Vec<BackupAnimeSource>,         // 503
    pub extensions: Vec<BackupExtension>,              // 504
    pub anime_extension_stores: Vec<BackupExtensionStore>, // 505
    pub custom_buttons: Vec<BackupCustomButtons>,      // 506
}

pub struct LegacyBackup { /* fields 1,2,3,4,101,103,104,105,106,107,108,109 exactly as p1-data-and-config.md 1.2 */ }
impl LegacyBackup { pub fn into_current(self) -> Backup; }   // lossless, B-12 / data 1.2

pub struct BackupManga {
    pub source: i64,                  // 1, required
    pub url: String,                  // 2, required
    pub title: String,                // 3, ""
    pub artist: Option<String>,       // 4
    pub author: Option<String>,       // 5
    pub description: Option<String>,  // 6
    pub genre: Vec<String>,           // 7
    pub status: i32,                  // 8, 0
    pub thumbnail_url: Option<String>,// 9
    pub date_added: i64,              // 13, 0
    pub viewer: i32,                  // 14, 0
    pub chapters: Vec<BackupChapter>, // 16
    pub categories: Vec<i64>,         // 17 (category ORDER values)
    pub tracking: Vec<BackupTracking>,// 18
    pub favorite: bool,               // 100, true
    pub chapter_flags: i32,           // 101, 0
    pub viewer_flags: Option<i32>,    // 103
    pub history: Vec<BackupHistory>,  // 104
    pub update_strategy: UpdateStrategy, // 105, AlwaysUpdate
    pub last_modified_at: i64,        // 106, 0
    pub favorite_modified_at: Option<i64>, // 107
    pub excluded_scanlators: Vec<String>,  // 108
    pub version: i64,                 // 109, 0
}

pub struct BackupAnime {
    pub source: i64,                  // 1, required
    pub url: String,                  // 2, required
    pub title: String,                // 3, ""
    pub artist: Option<String>, pub author: Option<String>, pub description: Option<String>,
    pub genre: Vec<String>, pub status: i32, pub thumbnail_url: Option<String>,
    pub date_added: i64,              // 13
    pub episodes: Vec<BackupEpisode>, // 16
    pub categories: Vec<i64>,         // 17
    pub tracking: Vec<BackupAnimeTracking>, // 18
    pub favorite: bool,               // 100, true
    pub episode_flags: i32,           // 101, 0
    pub viewer_flags: i32,            // 103, 0
    pub history: Vec<BackupAnimeHistory>, // 104
    pub update_strategy: UpdateStrategy,  // 105
    pub last_modified_at: i64,        // 106
    pub favorite_modified_at: Option<i64>, // 107
    pub version: i64,                 // 109
    pub memo: Vec<u8>,                // 112, b"{}"
    pub background_url: Option<String>, // 500 (never written by Aniyomi)
    pub parent_id: Option<i64>,       // 502
    pub id: Option<i64>,              // 503
    pub season_flags: i64,            // 504, 0
    pub season_number: f64,           // 505, -1.0 (fixed64)
    pub season_source_order: i64,     // 506, 0
    pub fetch_type: FetchType,        // 507, Episodes
}

#[repr(u8)] #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UpdateStrategy { #[default] AlwaysUpdate = 0, OnlyFetchOnce = 1 }

#[repr(u8)] #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FetchType { Seasons = 0, #[default] Episodes = 1 }
```

Enum decode: an ordinal outside the declared range is `WireError::BadEnum`. Kotlin would throw too (corrupted file message).

Remaining entry structs follow the same pattern and field tables: `BackupChapter`, `BackupEpisode`, `BackupTracking`, `BackupAnimeTracking`, `BackupHistory`, `BackupAnimeHistory`, `BackupCategory`, `BackupSource`, `BackupAnimeSource`, `BackupExtensionRepos`, `BackupExtensionStore`, `BackupExtension`, `BackupCustomButtons`, `BackupPreference`, `BackupSourcePreferences`. Their exact field numbers, types and defaults are in p1-data-and-config.md sections 2-4.

Preference value (sealed) in Rust:

```rust
pub enum PrefValue {
    Int(i32), Long(i64), Float(f32), String(String), Boolean(bool), StringSet(BTreeSet<String>),
}
pub struct BackupPreference { pub key: String, pub value: PrefValue }
pub struct BackupSourcePreferences { pub source_key: String, pub prefs: Vec<BackupPreference> }
```
The nested message layout of `PrefValue` is the one open point of the format; it is decided by fixtures (T-21a-034). Until then `encode` writes the subclass tag as `PREF_VALUE_TAG[variant]` from a `const` table that the fixture test fills in. Do not guess the numbers in code without a fixture.

Entry conversion functions: one `from_row` per entry type (`BackupManga`, `BackupChapter`, `BackupHistory`, `BackupTracking`, `BackupAnime`, `BackupEpisode`, `BackupAnimeHistory`, `BackupAnimeTracking`) live in `creator/*.rs` (write side); the `into_domain` counterparts live in `schema.rs` for restore (area:22).

---

## 5. Format layer (`src/backup/format.rs`)

```rust
pub enum Sniffed { Gzip, JsonForbidden, Raw }

pub fn sniff(first_two: &[u8]) -> Result<Sniffed, BackupError>;
// [0x1f,0x8b] -> Gzip; [0x7b,0x7d] | [0x7b,0x22] | [0x7b,0x0a] -> JsonForbidden;
// fewer than 2 bytes -> BackupError::Truncated (Kotlin: EOFException); else Raw.

pub fn read_backup_bytes(path: &Path) -> Result<Vec<u8>, BackupError>;   // sniff + gunzip (MultiGzDecoder)

pub fn decode_file(path: &Path) -> Result<Backup, BackupError> {
    let bytes = read_backup_bytes(path)?;
    if is_legacy(&bytes)? { LegacyBackup::from_bytes(&bytes)?.into_current() }
    else { Backup::from_bytes(&bytes)? }
}
```

Mapping of errors to user keys:
- `JsonForbidden` -> `invalid_backup_file_json` ("JSON backup not supported").
- `WireError` (any) and `gzip` failures on decode -> `invalid_backup_file_unknown` ("Backup file is corrupted"). Kotlin only maps `SerializationException`; a corrupt gzip stream surfaces as a raw IO error. PROPOSAL: map both to the same key (E-11).
- `Truncated` on sniff -> "Backup file is corrupted".

Legacy detection (B-12): partial decode with a `DetectBackup { is_legacy: Option<bool>, backup_anime_sources_count: usize }` reading tags 500 and 103 only, skipping everything else. `is_legacy = is_legacy_field.unwrap_or(true) && count > 0`.

Gzip writing:

```rust
pub fn gzip_encode(payload: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut enc = flate2::write::GzEncoder::new(Vec::with_capacity(payload.len() / 4), flate2::Compression::default());
    enc.write_all(payload)?;
    enc.finish()
}
```
(`Compression::default()` = level 6, same as Kotlin.) Streaming alternative for very large libraries: `GzEncoder` directly over the `BufWriter<File>`; choose streaming in the file path (see creator `write_file`).

---

## 6. Creator (`src/backup/creator/mod.rs`)

### 6.1 Inputs as traits (no Injekt; constructor injection)

```rust
pub trait SourceRegistry: Send + Sync {
    fn is_installed(&self, source_id: i64) -> bool;
    /// Display name for the backup entry (installed name, else stub name, else "").
    fn name_of(&self, source_id: i64) -> String;
    /// "Name (LANG)" for a valid stub, else the numeric id as text (B-14).
    fn stub_label(&self, source_id: i64) -> String;
}

pub trait TrackerRegistry: Send + Sync {
    fn name(&self, tracker_id: i32) -> Option<String>;
    fn logged_in(&self, tracker_id: i32) -> bool;
}

pub trait ExtensionArtifacts: Send + Sync {
    /// (package id, artifact bytes) for every installed extension, anime first then manga.
    fn installed(&self) -> Result<Vec<(String, Vec<u8>)>, BackupError>;
}

pub trait PrefStore: Send + Sync {
    /// Every app-level key/value. The creator applies the key rules.
    fn all(&self) -> Vec<(String, PrefValue)>;
}

pub trait SourcePrefStore: Send + Sync {
    /// (source_key like "source_<id>", prefs) for every configurable installed source.
    fn configurable(&self, anime: bool) -> Vec<(String, Vec<(String, PrefValue)>)>;
}
```

`SourceRegistry::name_of` implements `getOrStub(id).name` (empty string for an unknown stub, B-08d).

### 6.2 Build and write

```rust
pub struct CreateInputs<'a> {
    pub db: &'a rusqlite::Connection,        // used inside one read transaction
    pub sources: &'a dyn SourceRegistry,
    pub trackers: &'a dyn TrackerRegistry,
    pub extensions: &'a dyn ExtensionArtifacts,
    pub prefs: &'a dyn PrefStore,
    pub source_prefs: &'a dyn SourcePrefStore,
}

/// Pure function: builds the protobuf payload (uncompressed). Unit-testable with an in-memory DB.
pub fn build_backup(inputs: &CreateInputs<'_>, opts: &BackupOptions) -> Result<Backup, BackupError>;

/// Encode + gzip + write + self-check. Returns the final path.
pub fn create_file(inputs: &CreateInputs<'_>, opts: &BackupOptions, target: &Target, progress: &dyn Fn(Progress)) -> Result<PathBuf, BackupError>;

pub enum Target { Manual(PathBuf), Automatic { dir: PathBuf } }

pub struct Progress { pub phase: Phase, pub done: u32, pub total: u32 }   // Phase: Reading, Encoding, Writing, Verifying
```

`build_backup` steps (mirror B-06.2..5, B-08):
1. Start a read transaction: `BEGIN DEFERRED` then run all queries inside it, `COMMIT` at the end (consistent snapshot, E-05 fix).
2. `non_fav_anime = if opts.read_entries { query_watched_anime_not_in_library() } else { vec![] }`; same for manga.
3. `anime_entries = if opts.library_entries { favourites + non_fav_anime } else { vec![] }` and convert each with `anime::entry(...)`, flags per B-08b. Do not run step 2 when `library_entries` is false (E-09).
4. Manga the same with `manga::entry(...)`.
5. Categories when `opts.categories`: `categories::manga()` and `categories::anime()` (non-system only).
6. Sources: `sources::manga(&manga_entries, sources)`, `sources::anime(&anime_entries, sources)` (first-seen distinct).
7. App prefs when `opts.app_settings`: `preferences::app(prefs, opts.private_settings)`.
8. Source prefs when `opts.source_settings`: `preferences::sources(source_prefs, opts.private_settings)`.
9. Extension stores when `opts.extension_stores`: `stores::anime(...)`, `stores::manga_repos(...)`.
10. Custom buttons when `opts.custom_button`.
11. Extensions when `opts.extensions`: `extensions::artifacts(...)`.
12. Return `Backup { is_legacy: false, ... }`.

`create_file` steps (mirror B-06.1 and .6..10):
1. Target: Manual: `path` as chosen (truncate on open; the GTK dialog already confirmed overwrite). Automatic: `dir` create_dir_all; `prune_automatic(dir)` happens AFTER step 5 (E-04 fix); then the file name is computed.
2. Encode: `let payload = backup.to_bytes();` (empty check kept for parity; unreachable).
3. Write to `<final>.part` in the same directory: `GzEncoder` over `BufWriter<File>`, `write_all(&payload)`, `finish()`, `File::sync_all()`.
4. Self-check: `format::decode_file(&part)`; on error delete `part`, return `Corrupt`. (Kotlin's `BackupFileValidator.validate`. Result lists of missing sources and trackers are computed but do not fail the run.)
5. Atomic publish: `std::fs::rename(part, final)`. For automatic runs: `prune_automatic(dir, keep = 4)` (delete matching files beyond the newest 4 excluding the new one). Then update `last_auto_epoch_ms` (B-10).
6. On any error after step 3: remove `part`. Manual target: also remove the partially written final file if it was created by this run (Kotlin deletes the file).

Note on step 3 in SAF/portal paths: the GTK dialog returns a `gio::File`; use `file.path()` (may be None for non-local URIs; then fall back to `file.read/write` via `gio::FileOutputStream`). T-21a-022 covers this.

### 6.3 Retention and names

```rust
pub const MAX_AUTO_BACKUPS: usize = 4;
pub fn automatic_file_name(now: chrono::DateTime<chrono::Local>) -> String {
    format!("kurobayomi_{}.tachibk", now.format("%Y-%m-%d_%H-%M-%S"))
}
pub fn is_automatic_name(name: &str) -> bool {
    // ^kurobayomi_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}(-\d{2})?\.tachibk$  (hand-written; no regex dependency)
}
pub fn prune_automatic(dir: &Path, keep: usize) -> Result<usize, BackupError>;
// lists dir, filters is_automatic_name, sorts by name descending (lexicographic == chronological), deletes all beyond `keep`.
```
Lexicographic order is chronological because the name is zero-padded and local-time based. A DST fall-back repeats an hour; the seconds suffix keeps names unique in practice. If the target name already exists, append `-2`, `-3` before `.tachibk` (the regex does not accept that: make the pattern `(-\d{2})?(-\d+)?` in the hand-written checker, E-03).

---

## 7. Validation (`src/backup/validate.rs`)

```rust
pub struct Preflight { pub missing_sources: Vec<String>, pub missing_trackers: Vec<String> }

pub fn preflight(backup: &Backup, sources: &dyn SourceRegistry, trackers: &dyn TrackerRegistry) -> Preflight {
    // manga sources: backup.sources where !is_installed(source_id)
    //   label = if name.parse::<i64>().is_ok() { stub_label(id) } else { name.clone() }   (B-13.2)
    //   distinct, sorted; then anime sources the same; concatenated manga-first.
    // trackers: distinct syncIds from anime and manga tracking; keep those with name(id) Some and !logged_in(id); names sorted.
}
```

`BackupError` for the restore UI wraps decode errors with the same key mapping as B-11. The Kotlin `IllegalStateException(e).message` ("java.io.IOException: ...") is NOT reproduced: PROPOSAL shows the localized key text only (E-12).

---

## 8. Automatic scheduler (`src/backup/auto.rs`)

```rust
pub const TICK: std::time::Duration = std::time::Duration::from_secs(600);     // matches the 10-minute flex
pub const RETRY_BASE: std::time::Duration = std::time::Duration::from_secs(600); // Kotlin backoff base

pub struct AutoConfig { pub interval_hours: u32, pub last_auto_epoch_ms: i64, pub on_battery_min_percent: u8 /* 15 */ }

pub enum AutoDecision { Off, NotDue { next_at_ms: i64 }, Defer(Reason), Run }
pub enum Reason { RestoreRunning, BatteryLow { percent: u8 }, ManualRunning, Backoff { until_ms: i64 } }

pub fn decide(now_ms: i64, cfg: &AutoConfig, restore_running: bool, power: PowerState, manual_running: bool, retry: &RetryState) -> AutoDecision;

pub async fn run_loop(cfg: watch::Receiver<AutoConfig>, job: Arc<tokio::sync::Mutex<()>>, runner: Arc<dyn AutoRunner>);
```

Rules (PROPOSAL, with the Kotlin rule each replaces):
- `interval_hours == 0` -> `Off` (Kotlin `cancelUniqueWork`).
- Due when `now - last_auto_epoch_ms >= interval_hours * 3600_000`. If `last_auto_epoch_ms == 0` (never ran) the first due time is "enable time + interval" (E-08). Kotlin relies on WorkManager's first run; this removes the immediate first run after enabling.
- `restore_running` -> `Defer(RestoreRunning)`, retry after `RETRY_BASE` (Kotlin `Result.retry()` with backoff).
- Battery: `PowerState { on_battery: bool, percent: Option<u8> }`. `on_battery && percent < on_battery_min_percent` -> `Defer(BatteryLow)`. Kotlin's `requiresBatteryNotLow` has no exact percent; 15 is the Android "low battery" default. area:26 proposes 20; the two must be reconciled in the cross-area review (flagged in p1-topic-autobackup-scheduling.md).
- Backoff after a failed automatic run: `RETRY_BASE * 2^(attempt-1)` capped at the interval (Kotlin EXPONENTIAL with 10-minute base). The attempt counter resets on success.
- A failed automatic run does NOT advance `last_auto_epoch_ms`, so the next tick retries; this differs from Kotlin, where a `Result.failure()` waits a full period (E-10).
- Manual run in progress -> `Defer(ManualRunning)` (skip, do not queue).
- The loop holds the job mutex only while running; `try_lock` semantics for the decision.

`AutoRunner::run(&self) -> Result<PathBuf, BackupError>` is implemented by the app: it builds `CreateInputs`, calls `create_file` with `BackupOptions::default()` and `Target::Automatic { dir }` where `dir = <data>/autobackup`.

Options used by automatic runs: `BackupOptions::default()` (B-02). This is a documented limitation shown in the settings info text (p1-ui-parity.md).

---

## 9. Manual job (`src/backup/job.rs`)

```rust
pub struct JobSlot(Arc<tokio::sync::Mutex<()>>);

pub enum JobEvent { Started, Progress(Progress), Finished(Result<PathBuf, BackupError>) }

pub fn start_manual(slot: &JobSlot, runtime: &tokio::runtime::Handle, inputs: Arc<OwnedInputs>, opts: BackupOptions, target: PathBuf, tx: async_channel::Sender<JobEvent>) -> Result<(), BackupError /* InProgress */> {
    let guard = slot.0.clone().try_lock_owned().map_err(|_| BackupError::InProgress)?;
    runtime.spawn(async move { let _g = guard; /* spawn_blocking(create_file) */ ... });
    Ok(())
}
```
- A second `start_manual` while a run is active returns `InProgress` immediately. The UI shows the toast `backup_in_progress` (B-03). This replaces WorkManager `KEEP` plus `isManualJobRunning`.
- Cancellation: none in Kotlin for backup (no cancel button). PROPOSAL: no cancel in P1 (matches Kotlin). If added later: `CancellationToken` checked between phases; `.part` removed.
- Events arrive on the GTK side through `async_channel`; `glib::spawn_future_local` awaits `rx.recv()` and updates notifications and the window.

---

## 10. Notifications (`src/backup/notify.rs`)

```rust
pub trait Notifier: Send + Sync {
    fn backup_progress(&self) -> NotificationHandle;      // indeterminate, persistent
    fn backup_failed(&self, reason: Option<&str>);
    fn backup_created(&self, path: &Path);                // actions: "reveal" -> ShowItems
    fn restore_progress(&self, content: &str, done: u32, max: u32, sync: bool);
    fn restore_failed(&self, reason: Option<&str>);
    fn restore_done(&self, elapsed: std::time::Duration, errors: u32, log: Option<&Path>, sync: bool);
}
```
Implementation `DesktopNotifier { hide_content: bool, ... }` with `notify_rust::Notification`:
- Summary/body texts are the English strings of B-15 (through the same key table). `hide_content` (pref `hide_notification_content`) suppresses restore body text only (Kotlin behaviour; backup texts are never hidden).
- Urgency: `Low` for progress (Kotlin `IMPORTANCE_LOW`), `Normal` for completion (Kotlin `IMPORTANCE_HIGH` would be `Critical` on KDE; use Normal to avoid pop-up spam).
- Progress replace: send with the same `id()` (freedesktop notification id) to update in place; close it with `NotificationHandle::close()` at the end (replaces `cancelNotification`).
- Icons: `document-save`, `dialog-information`, `dialog-error`, `folder` (freedesktop icon names, resolved by KDE's icon theme). The app icon is passed as `app_icon("kurobayomi")`.
- Actions: `reveal` labelled "Show in folder" (backup created) and `details` labelled "Tap to see details" (restore error log). Both are handled on a dedicated thread (`notify-rust` `wait_for_action` blocks) which sends the action back over `async_channel` to GTK.
- `reveal` calls `org.freedesktop.FileManager1.ShowItems(["file:///..."], "")` over the session bus (zbus `Proxy`). If the call fails (no file manager implements it), fall back to `gio::AppInfo::launch_default_for_uri` on the parent directory.

Permission: no permission model on Linux; the `POST_NOTIFICATIONS` check is dropped. If the notification daemon is absent, `notify-rust` returns an error which is logged (`tracing::warn!`) and ignored, matching Kotlin's silent return.

---

## 11. Settings glue (`src/backup/settings.rs`)

Keys (Kurobayomi config, owned by area:02, read here):
- `backup.interval_hours: u32` default 12, allowed {0, 6, 12, 24, 48, 168}. Setting a value outside the allowed set is rejected by the UI (the Kotlin accepts any Int; E-13).
- `state.backup.last_auto_epoch_ms: i64` default 0 (app state, not exported in backups).
- `backup.directory: Option<PathBuf>` default `None` = `$XDG_DATA_HOME/kurobayomi/autobackup` (PROPOSAL; Kotlin uses `<storage base>/autobackup`).
- `notifications.hide_content: bool` default false.

Setting change hook: when `backup.interval_hours` changes, the `watch::Sender<AutoConfig>` is updated; the loop picks it up on the next tick, or immediately via `tokio::select!` on the watch (Kotlin re-enqueues immediately).

---

## 12. Threading model (summary)

| Work | Thread | Mechanism |
|---|---|---|
| GTK widgets, dialogs, file chooser | GTK main thread | `gtk::FileDialog` callbacks |
| Backup build, encode, gzip, write, self-check | tokio blocking pool | `spawn_blocking` (SQLite and file I/O are blocking) |
| Auto loop, decisions | tokio runtime (2 workers) | `tokio::spawn` with `interval(TICK)` |
| UPower / FileManager1 D-Bus calls | tokio | zbus async proxies |
| Notification action wait | dedicated OS thread | `notify-rust` `wait_for_action` (blocking) |
| Events to UI | glib main context | `async_channel::Receiver` + `glib::spawn_future_local` |

Rules:
- No GTK object is touched off the main thread.
- No `rusqlite::Connection` is shared across threads; the job opens its own read connection (WAL mode assumed; area:03 owns the pragmas).
- Progress is coalesced: at most one notification update per 500 ms (the Kotlin `setForegroundSafely` delay is 500 ms; the progress notification itself is indeterminate, so the rate limit is for the restore path only).

---

## 13. Android API to Linux mapping (this area)

| Android / Kotlin (source) | Kurobayomi replacement | Notes |
|---|---|---|
| `kotlinx.serialization.protobuf.ProtoBuf` (`AppModule.kt:190`) | `src/backup/wire.rs` | Own codec; see section 3 |
| `okio.gzip()` / `GZIPInputStream` | `flate2::write::GzEncoder`, `flate2::read::MultiGzDecoder` | Level 6 |
| `UniFile` / `DocumentFile` / SAF `CreateDocument` | `std::path::PathBuf`, `gtk::FileDialog::save` | Portal-aware |
| `ContentResolver.openInputStream` | `std::fs::File` | |
| `Context.contentResolver.takePersistableUriPermission` | not needed | Paths are plain |
| `WorkManager` periodic work + constraints | `tokio::time::interval` + `auto::decide` | Persisted timestamp |
| `Constraints(requiresBatteryNotLow)` | UPower `OnBattery`, `DisplayDevice.Percentage` (zbus) | Threshold 15% (section 8) |
| `ForegroundInfo` / `FOREGROUND_SERVICE_TYPE_DATA_SYNC` | none | No service model on Linux |
| `NotificationCompat` / channels / groups | `notify-rust` | Urgency instead of channel importance |
| `NotificationReceiver` pending broadcasts | in-process action callbacks | |
| `PackageManager.getApplicationInfo().publicSourceDir` | extension artifact path from the extension manager (areas 17-19) | |
| `Injekt.get<...>()` | constructor injection (`CreateInputs`) | |
| `Instant.now()`, `Date`, `SimpleDateFormat` | `chrono::Utc::now()`, `chrono::Local` | Local time for file names (B-07) |
| `logcat` | `tracing` | |
| `Context.stringResource(MR.strings.*)` | `backup::messages::text(Key)` (table of B-16) | English only in P1; i18n is out of scope |
| `DeviceUtil.isMiui` | none | Dropped |
| `Preference.isAppState/isPrivate` | key prefix rules in `creator/preferences.rs` | `__APP_STATE_`, `__PRIVATE_` |
| `MemoColumnAdapter` (`JsonObject` <-> bytes) | `serde_json::Value` <-> UTF-8 bytes | Empty object `b"{}"` |

---

## 14. Error type

```rust
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("Couldn't create a backup file")]          CreateFile,            // create_backup_file_error
    #[error("No library entries to back up")]          EmptyBackup,           // empty_backup_error
    #[error("Backup is already in progress")]          InProgress,            // backup_in_progress
    #[error("JSON backup not supported")]              JsonNotSupported,      // invalid_backup_file_json
    #[error("Backup file is corrupted")]              Corrupt(#[source] Box<dyn std::error::Error + Send + Sync>), // invalid_backup_file_unknown
    #[error("Backup failed")]                         Io(#[from] std::io::Error),       // creating_backup_error on screen
    #[error(transparent)]                             Wire(#[from] wire::WireError),
    #[error("database: {0}")]                         Db(#[from] rusqlite::Error),
}
```
`Display` strings are for logs only; user text comes from the key mapping (`backup::messages`), which is the only path to notifications and dialogs.

---

## 15. Pitfalls

1. Never use `prost` derive here: proto3 packed-by-default and missing presence rules break compatibility.
2. Local-time file names: a DST fall-back repeats an hour. The seconds suffix plus the `-2`, `-3` collision suffix keeps names unique. Lexicographic sort is chronological only within one timezone; cross-timezone files may sort wrong (acceptable; document).
3. `rusqlite::Connection` is `!Sync`; never move it across `await` points on a shared handle. One connection per job, opened inside `spawn_blocking`.
4. Nested `Writer`s double memory per nesting level. For 10 000 entries with chapters, expect a few hundred MB peak if built fully in memory; mitigation: stream entries into the top-level `Writer` and write the gzip stream as you go (Proposal for v2; P1 uses in-memory and documents the limit).
5. `notify-rust` `wait_for_action` blocks: run it on a thread, never on the tokio runtime or the GTK thread.
6. GTK `FileDialog` in a Flatpak returns document-portal paths (`/run/user/<uid>/doc/...`). Those are valid for the process but not for other apps; "Show in folder" may open the document folder instead of the real location. Documented limitation.
7. A `.part` file left by a crash is harmless; on startup, delete `*.tachibk.part` in the automatic directory (T-21a-025).
8. Do not parse JSON. Kotlin rejects JSON by magic bytes; Rust must not try to be helpful (no silent JSON import).
9. `i32` truncation: `status`, `chapter_flags`, `viewer_flags` are `Int` in the format but `Long` in the DB. Use `i32::try_from(...)` and fail the backup with `Corrupt` rather than wrap (Kotlin wraps silently; E-14 notes the difference).
10. Do not start the auto loop before the database is open and migrated (area:03/04), or the first tick can read an old schema.
11. Sealed `PreferenceValue` numbers: do not hard-code guesses. T-21a-034 must land before any export is shipped.

---

## 16. Test seams

- `CreateInputs` with fakes for `SourceRegistry`, `TrackerRegistry`, `ExtensionArtifacts`, `PrefStore`, `SourcePrefStore` plus an in-memory SQLite (`rusqlite::Connection::open_in_memory`) loaded from the area:03/04 schema.
- `build_backup` is pure over those inputs, so creator tests need no GTK and no D-Bus.
- `decide(...)` is a pure function: table tests over `now`, `last`, battery, restore, manual, backoff.
- Wire tests: round-trip every struct; golden bytes from Aniyomi fixtures (topic file, T-21a-034).
