# Area 13 — Rust design (core utilities, storage, archives, images, torrent helpers)

Scope and sources: see p1-inventory.md. Behaviour: p1-behavior.md. Data: p1-data-and-config.md. This document fixes the module layout, the public types and trait signatures, the threading model, the crate choices, the error model and the Android-to-Linux mapping. It assumes the workspace from area:01-foundation-app-bootstrap (`crates/kuro-core`, `crates/kuro-platform`, `crates/kuro-testkit`, GUI package `kurobayomi` in the repository root) and the rules in `plans/addenda/requirements-cross-platform.md`.

## 1. Current mockup (what exists today)

`/home/blackbox/kurobayomi/Cargo.toml` is a single package `kurobayomi` (edition 2024) with `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde` 1.0.229 and `serde_json` 1.0.151. There is no workspace yet. The mockup files are `src/main.rs`, `clamp.rs`, `entry.rs`, `player.rs`, `screens.rs`, `settings.rs`, `spec.rs`, `ui.rs` (UI only, no data layer). Nothing in the mockup reads files, decodes images, talks to TorrServer or stores preferences. The mockup spec files (`spec/*.json`) describe screens; the relevant parts for this area are:

- `spec/settings.json`: the torrent rows ("Torrent trackers", "settings_player_torrents", the notice dialog `torrent_notice`, and the "Reset default torrent trackers string" action).
- `spec/app.json`: the storage screen (Data and storage, delete-by-entry) and the first-run "Storage" step that requires a folder.
- `spec/reader_player.json`: the reader's double-page and background options (consumed by the reader area).

The mockup therefore needs: a folder chooser (storage step), toast messages, and the torrent settings rows (validators from this area). The UI wiring is in p1-ui-parity.md.

## 2. Crate and module layout

All code of this area lives in `kuro-core` (no GTK) except the toast (in the GUI crate) and the WebView helpers (in `kuro-platform` behind the `WebViewHost` trait).

```
crates/kuro-core/src/
  util/
    mod.rs            // re-exports
    text.rs           // chop, truncate_center, byte_size, take_bytes, html_decode_legacy   (T-13a-008, 011, 012)
    natural.rs        // natural_cmp_ci over UTF-16 units                                  (T-13a-009)
    collate.rs        // icu_collator primary compare                                       (T-13a-010)
    hash.rs           // md5_hex, sha256_hex, disk_key                                       (T-13a-013)
    json.rs           // empty_object()                                                     (T-13a-051)
    lang.rs           // bool_to_i64                                                         (T-13a-057)
  prefs/
    mod.rs            // PrefStore (RawStore), Pref<T>, PrefValue, EnumPref                 (T-13a-002, 004, 005, 007)
    memory.rs         // InMemoryStore (tests and previews)                                 (T-13a-003)
    filter.rs         // TriState, CheckboxState                                            (T-13a-006)
  storage/
    mod.rs            // FolderProvider                                                      (T-13a-015)
    filename.rs       // sanitize_component, MAX_FILE_NAME_BYTES, RESERVED_WINDOWS_NAMES     (T-13a-014)
    space.rs          // free_bytes, total_bytes (statvfs)                                    (T-13a-016)
    dirs.rs           // dir_size                                                            (T-13a-016)
    nomedia.rs        // create_nomedia                                                      (T-13a-018)
    temp.rs           // TempFiles                                                           (T-13a-019)
    copy.rs           // copy_read_only                                                      (T-13a-020)
    names.rs          // extension, name_without_extension, display_path                    (T-13a-017)
  archive/
    mod.rs            // ArchiveReader, EntryInfo, ArchiveError                              (T-13a-031, 032)
    libarchive.rs     // FFI wrapper (private)                                               (T-13a-031)
    epub.rs           // EpubReader                                                          (T-13a-033)
    zip_writer.rs     // CbzWriter                                                           (T-13a-034)
  image/
    mod.rs            // ImageKind, Sniff                                                     (T-13a-021)
    sniff.rs          // magic-byte table                                                     (T-13a-021)
    decode.rs         // decode_rgba, decoder selection, dimensions                           (T-13a-022, 024)
    ops.rs            // split_in_half, rotate_quarter, split_and_merge, split_tall           (T-13a-025..028)
    background.rs     // choose -> Background                                                  (T-13a-029)
    props.rs          // is_animated_supported, is_wide                                       (T-13a-023, 024)
  torrent/
    mod.rs
    bencode.rs        // Value, parse, encode                                                 (T-13a-036, 037)
    meta.rs           // parse_torrent_file -> TorrentMeta                                    (T-13a-038)
    model.rs          // serde DTOs                                                           (T-13a-039)
    client.rs         // TorrServerClient (reqwest)                                           (T-13a-040)
    supervisor.rs     // TorrServerSupervisor (child process)                                 (T-13a-041)
    prefs.rs          // typed torrent prefs, validators                                      (T-13a-042, 043)
    route.rs          // classify, play-link                                                  (T-13a-044)
  error.rs            // UtilError umbrella (thiserror)
crates/kuro-core/tests/   // ported tests, see p1-edge-cases-and-tests.md
crates/kuro-core/tests/fixtures/torrent/   // the two .torrent fixtures, copied from core/common/src/test/resources
```

Toast and the folder chooser sit in the GUI crate:

```
src/toast.rs          // in-app overlay toast (T-13a-053)
```

WebView helpers are in `kuro-platform` (trait) and `kuro-core::web` (pure functions: UA transform and version parsing, T-13a-052):

```
crates/kuro-core/src/web.rs     // infer_user_agent, parse_chrome_major, MIN_WEBVIEW_MAJOR
```

Dependency direction: `kuro-core` must not depend on GTK. `kuro-platform` is depended on by `kuro-core` only for the `Paths` trait and `WebViewHost` trait objects passed in by the app context (rule 3 of the cross-platform addendum: services receive collaborators as constructor arguments, never as globals).

## 3. Key types and signatures

### 3.1 Errors (`error.rs`)

```rust
// thiserror-based, one enum per module, all converted into UtilError at the boundary.
#[derive(Debug, thiserror::Error)]
pub enum UtilError {
    #[error("text: {0}")]              Text(#[from] text::TextError),
    #[error("storage: {0}")]           Storage(#[from] storage::StorageError),
    #[error("archive: {0}")]           Archive(#[from] archive::ArchiveError),
    #[error("image: {0}")]             Image(#[from] image::ImageError),
    #[error("torrent: {0}")]           Torrent(#[from] torrent::TorrentError),
    #[error("preference: {0}")]        Pref(#[from] prefs::PrefError),
}

#[derive(Debug, thiserror::Error)]
pub enum TextError {
    #[error("replacement ({replacement_len}) longer than limit ({limit})")]
    ReplacementTooLong { replacement_len: usize, limit: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("source does not exist: {0}")]          SourceMissing(std::path::PathBuf),
    #[error("destination exists: {0}")]             DestinationExists(std::path::PathBuf),
    #[error("cannot create directory {0}")]         CreateDir(std::path::PathBuf, #[source] std::io::Error),
    #[error("io error on {path}: {source}")]        Io { path: std::path::PathBuf, #[source] source: std::io::Error },
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("cannot open archive {path}: {reason}")]   Open { path: std::path::PathBuf, reason: String },
    #[error("libarchive: {0}")]                        Backend(String),
    #[error("epub package missing: {0}")]              PackageMissing(String),
    #[error("entry not utf-8")]                        NonUtf8Name,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("unknown image format")]                   UnknownFormat,
    #[error("decode failed: {0}")]                     Decode(String),
    #[error("encode failed: {0}")]                     Encode(String),
    #[error("region outside image")]                   OutOfBounds,
    #[error("io: {0}")]                                Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum TorrentError {
    #[error("invalid torrent: {0}")]                   InvalidTorrent(String),
    #[error("bencode: {0}")]                           Bencode(String),
    #[error("torrent server is not running")]          ServerUnavailable,
    #[error("torrent server timed out")]               ServerTimeout,       // mapped from SocketTimeout
    #[error("torrent server rejected request: {0}")]   ServerRejected(u16),
    #[error("invalid port {0}")]                       InvalidPort(String),
    #[error("invalid proxy url: {0}")]                 InvalidProxy(String),
    #[error("http: {0}")]                              Http(#[from] reqwest::Error),
}
```

The mapping from Kotlin exceptions: `DisabledTorrServerException` becomes `TorrentError::ServerUnavailable`; `DeadTorrentException` (from `SocketTimeoutException`) becomes `TorrentError::ServerTimeout` (`TorrentUtils.kt:28-33`). Error messages shown to the user come from message keys (p1-data-and-config.md section 7), not from `Display`.

### 3.2 Preferences (`prefs/mod.rs`)

The raw store is the seam to area:02-preferences-system. This area needs only typed get/set/delete and change notification.

```rust
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::watch;

/// Untyped value as stored. Mirrors the Android SharedPreferences types plus an object (JSON string) kind.
#[derive(Debug, Clone, PartialEq)]
pub enum RawValue {
    Str(String),
    Int(i64),        // Kotlin Int/Long both fit
    Float(f32),
    Bool(bool),
    StrSet(BTreeSet<String>),
    Object(String),  // serialized enum or object preference
}

#[derive(Debug, thiserror::Error)]
pub enum PrefError {
    #[error("stored type mismatch for {key}")]  TypeMismatch { key: String },
    #[error("backend: {0}")]                    Backend(String),
}

/// Implemented by area:02-preferences-system's store. Synchronous, cheap, thread-safe.
pub trait RawStore: Send + Sync + 'static {
    fn read(&self, key: &str) -> Result<Option<RawValue>, PrefError>;
    /// `None` deletes the key.
    fn write(&self, key: &str, value: Option<RawValue>) -> Result<(), PrefError>;
    /// Every write (including delete) publishes its key. `None` payload means "all keys may have changed" (Android clear()).
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Option<String>>;
}

/// Values that can be stored under a key.
pub trait PrefValue: Clone + PartialEq + Send + Sync + 'static {
    fn to_raw(&self) -> RawValue;
    /// `Err(TypeMismatch)` means the stored value has the wrong kind (primitive kinds delete the key, as Android does).
    fn from_raw(raw: &RawValue) -> Result<Self, PrefError>;
}

impl PrefValue for String { /* RawValue::Str */ }
impl PrefValue for i64 { /* RawValue::Int */ }
impl PrefValue for bool { /* RawValue::Bool */ }
impl PrefValue for BTreeSet<String> { /* RawValue::StrSet */ }

/// Typed handle. Clone is cheap (Arc).
#[derive(Clone)]
pub struct Pref<T: PrefValue> {
    store: Arc<dyn RawStore>,
    key: Arc<str>,
    default: T,
    kind: Kind,
    tx: watch::Sender<T>,                 // holds the last known value; shared per key via the registry
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind { Primitive, Object }      // controls the mismatch policy (see below)

impl<T: PrefValue> Pref<T> {
    pub fn key(&self) -> &str;
    /// Primitive kind: on TypeMismatch the key is deleted and default returned. Object kind: default returned, key kept.
    pub fn get(&self) -> T;
    pub fn set(&self, value: T) -> Result<(), PrefError>;   // synchronous write
    pub fn is_set(&self) -> bool;
    pub fn delete(&self) -> Result<(), PrefError>;
    pub fn default_value(&self) -> &T;
    /// Receiver that starts with the current value and conflates updates (Kotlin changes()).
    pub fn watch(&self) -> watch::Receiver<T>;
    pub fn get_and_set(&self, f: impl FnOnce(T) -> T) -> Result<(), PrefError>;
}

impl Pref<bool> {
    /// Kotlin toggle(): writes !get() and returns the new value.
    pub fn toggle(&self) -> Result<bool, PrefError>;
}

impl Pref<BTreeSet<String>> {
    pub fn insert(&self, item: String) -> Result<(), PrefError>;   // Kotlin plusAssign
    pub fn remove(&self, item: &str) -> Result<(), PrefError>;     // Kotlin minusAssign
}

/// Enum preference: stored as the constant name (Kotlin getEnum). Unknown name -> default, stored value kept.
pub trait NamedEnum: Copy + Eq + Send + Sync + 'static {
    fn name(self) -> &'static str;
    fn from_name(s: &str) -> Option<Self>;
}

pub struct EnumPref<E: NamedEnum> { inner: Pref<String>, default: E }  // stored as Object kind
impl<E: NamedEnum> EnumPref<E> {
    pub fn get(&self) -> E { E::from_name(&self.inner.get()).unwrap_or(self.default) }
    pub fn set(&self, v: E) -> Result<(), PrefError> { self.inner.set(v.name().to_owned()) }
}

/// Private and app-state key helpers (behaviour 1.1). Same literal prefixes as Android.
pub mod keys {
    pub const PRIVATE_PREFIX: &str = "__PRIVATE_";
    pub const APP_STATE_PREFIX: &str = "__APP_STATE_";
    pub fn is_private(key: &str) -> bool { key.starts_with(PRIVATE_PREFIX) }
    pub fn private_key(key: &str) -> String { format!("{PRIVATE_PREFIX}{key}") }
    pub fn is_app_state(key: &str) -> bool { key.starts_with(APP_STATE_PREFIX) }
    pub fn app_state_key(key: &str) -> String { format!("{APP_STATE_PREFIX}{key}") }
}
```

Registry: `PrefRegistry` (in `prefs/mod.rs`) owns one `watch::Sender<T>` per key, created on first use, so every handle for the same key observes the same updates. It forwards `RawStore::subscribe` events for matching keys (and for `None`, re-reads all registered keys). The registry is the only place with a `Mutex<HashMap<..>>`.

The Kotlin `changes()` emits "ignition" first; the Rust `watch::Receiver` already holds the current value, so no extra event is needed (conflation is `watch` semantics).

### 3.3 Filters (`prefs/filter.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TriState { #[default] Disabled, EnabledIs, EnabledNot }
impl TriState { pub fn next(self) -> Self { match self { Self::Disabled => Self::EnabledIs, Self::EnabledIs => Self::EnabledNot, Self::EnabledNot => Self::Disabled } } }

#[derive(Debug, Clone, PartialEq)]
pub enum CheckState<T> { Checked(T), Unchecked(T) }                 // Kotlin CheckboxState.State
impl<T> CheckState<T> { pub fn next(self) -> Self; pub fn is_checked(&self) -> bool; }

#[derive(Debug, Clone, PartialEq)]
pub enum TriCheck<T> { Include(T), Exclude(T), Unset(T) }          // Kotlin CheckboxState.TriState
impl<T> TriCheck<T> { pub fn next(self) -> Self; }                 // Unset -> Include -> Exclude -> Unset

pub fn as_check_state<T>(item: T, cond: impl Fn(&T) -> bool) -> CheckState<T>;
pub fn map_as_check_state<T>(items: Vec<T>, cond: impl Fn(&T) -> bool) -> Vec<CheckState<T>>;
```

### 3.4 Text (`util/text.rs`, `util/natural.rs`, `util/collate.rs`)

```rust
pub fn chop(s: &str, count: usize, replacement: &str) -> Result<String, TextError>;          // default replacement "…"
pub fn truncate_center(s: &str, count: usize, replacement: &str) -> Result<String, TextError>; // default "..."
pub fn byte_size(s: &str) -> usize { s.len() }                                                  // UTF-8 bytes
pub fn take_bytes(s: &str, n: usize) -> String;                                                 // cut at char boundary, no U+FFFD stripping
pub fn html_decode_legacy(s: &str) -> String;                                                   // see behaviour 2.6

/// Exact port of SimpleNaturalComparator + case-insensitive compareChars. Operates on UTF-16 code units.
pub fn natural_cmp_ci(a: &str, b: &str) -> std::cmp::Ordering;

/// Java-compatible primary-strength collation via ICU.
pub struct Collator { inner: icu_collator::Collator }
impl Collator {
    pub fn for_locale(locale: &icu_locale_core::Locale) -> Result<Self, CollateError>;
    pub fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering;
}
```

`natural_cmp_ci` uses `str::encode_utf16().collect::<Vec<u16>>()` on both inputs (allocation is acceptable: archive listings are small), then the line-by-line port. Lowercasing per unit: `char::from_u32(u as u32).map(|c| c.to_lowercase().next().unwrap_or(c) as u32)` for BMP scalars; lone surrogates compare as themselves (Java `Character.toLowerCase(char)` on a surrogate is identity).

### 3.5 Hashes (`util/hash.rs`)

```rust
pub fn md5_hex(bytes: &[u8]) -> String;      // lowercase, 32 chars
pub fn sha256_hex(bytes: &[u8]) -> String;   // lowercase, 64 chars
pub fn sha1_hex(bytes: &[u8]) -> String;     // torrent info hash, 40 chars
pub fn disk_key(key: &str) -> String { md5_hex(key.as_bytes()) }   // DiskUtil.hashKeyForDisk
```

Hex is written by hand (`format!("{:02x}")` over the digest) to keep the exact lowercase output; no `hex` crate needed.

### 3.6 Storage (`storage/*`)

```rust
// storage/mod.rs
pub trait FolderProvider: Send + Sync {
    fn directory(&self) -> std::path::PathBuf;
    fn url(&self) -> String;                 // file:// URL of directory()
}
pub struct DefaultFolder { root: std::path::PathBuf }   // built from kuro_platform::Paths::downloads_default()
impl FolderProvider for DefaultFolder { /* ... */ }

// storage/filename.rs
pub const MAX_FILE_NAME_BYTES: usize = 250;
pub const MAX_COMPONENT_UTF16: usize = 240;
pub const RESERVED_WINDOWS_NAMES: &[&str] = &["CON","PRN","AUX","NUL","COM1","COM2","COM3","COM4","COM5","COM6","COM7","COM8","COM9","LPT1","LPT2","LPT3","LPT4","LPT5","LPT6","LPT7","LPT8","LPT9"];
/// Behaviour 3.1. Returns "(invalid)" for empty results.
pub fn sanitize_component(orig: &str) -> String;
/// base name budget helper used by reader and player filenames (takeBytes + sanitize + suffix)
pub fn page_filename(title_and_chapter: &str, page_number: u32) -> String;

// storage/space.rs
pub fn total_bytes(path: &std::path::Path) -> Option<u64>;     // f_blocks * f_frsize
pub fn free_bytes(path: &std::path::Path) -> Option<u64>;      // f_bavail * f_frsize (non-root)

// storage/dirs.rs
pub fn dir_size(path: &std::path::Path) -> std::io::Result<u64>;   // regular files only, no symlink follow

// storage/nomedia.rs
pub const NOMEDIA_FILE: &str = ".nomedia";
pub fn create_nomedia(dir: &std::path::Path) -> std::io::Result<bool>;   // true when created

// storage/temp.rs
pub struct TempFiles { dir: std::path::PathBuf }   // <cache>/tmp
impl TempFiles {
    pub fn new(cache: &std::path::Path) -> Self;
    pub fn create_from(&self, source: &std::path::Path) -> std::io::Result<tempfile::NamedTempFile>; // prefix padded to 3 bytes, suffix ".tmp"
    pub fn clear(&self) -> std::io::Result<()>;                                                          // remove_dir_all, ignore NotFound
}

// storage/copy.rs
pub fn copy_read_only(source: &std::path::Path, target: &std::path::Path, overwrite: bool) -> Result<(), StorageError>;

// storage/names.rs (UniFile helpers, Rust Path versions)
pub fn extension_quirk(name: &str) -> &str;        // substringAfterLast('.'): whole name when no dot
pub fn name_without_extension_quirk(name: &str) -> &str; // substringBeforeLast('.')
```

### 3.7 Archives (`archive/*`)

```rust
// archive/mod.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo { pub name: String, pub is_file: bool }

/// A memory-mapped archive. Every operation opens its own libarchive handle over the same mapping.
pub struct ArchiveReader { map: std::sync::Arc<memmap2::Mmap>, path: std::path::PathBuf }

impl ArchiveReader {
    pub fn open(path: &std::path::Path) -> Result<Self, ArchiveError>;
    /// Lists entries in archive order. Entries with non-UTF-8 names are skipped (behaviour 4.1).
    pub fn entries(&self) -> Result<Vec<EntryInfo>, ArchiveError>;
    /// Streams one entry by exact name. The returned reader owns its libarchive handle.
    pub fn open_entry(&self, name: &str) -> Result<Option<EntryReader>, ArchiveError>;
    /// Reads a whole entry into memory (used by the image sniff path and small XML files).
    pub fn read_entry(&self, name: &str) -> Result<Option<Vec<u8>>, ArchiveError>;
}

pub struct EntryReader { /* owns libarchive handle; impl std::io::Read */ }
impl std::io::Read for EntryReader { fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize>; }
impl Drop for EntryReader { /* archive_read_free */ }

// archive/epub.rs
pub struct EpubReader { archive: ArchiveReader, sep: PathSep }
#[derive(Clone, Copy, PartialEq, Eq)] pub enum PathSep { Slash, Backslash }
impl EpubReader {
    pub fn open(path: &std::path::Path) -> Result<Self, ArchiveError>;
    pub fn package_href(&self) -> Result<String, ArchiveError>;      // container.xml rootfile full-path, fallback OEBPS/content.opf
    pub fn image_paths(&self) -> Result<Vec<String>, ArchiveError>;  // spine order, deduplicated (behaviour 4.5)
    pub fn read_entry(&self, name: &str) -> Result<Option<Vec<u8>>, ArchiveError>;
}
/// Pure string helper, unit-tested: resolve(base, rel, sep) with clamped ".." and the absolute-path rule.
pub fn resolve_zip_path(base: &str, rel: &str, sep: PathSep) -> String;
pub fn parent_directory(path: &str, sep: PathSep) -> String;

// archive/zip_writer.rs
pub struct CbzWriter { /* zip::ZipWriter<std::fs::File> */ }
impl CbzWriter {
    /// Creates `<target>_tmp`; `finish()` renames it to `<target>` (behaviour 4.6, suffix "_tmp").
    pub fn create(target: &std::path::Path) -> std::io::Result<Self>;
    /// Adds one file as a stored entry named by its basename; mtime and unix mode from the source.
    pub fn add_file(&mut self, source: &std::path::Path) -> std::io::Result<()>;
    pub fn finish(self) -> std::io::Result<std::path::PathBuf>;
}
```

### 3.8 Images (`image/*`)

```rust
// image/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind { Avif, Gif, Heif, Jpeg, Jxl, Png, Webp }
impl ImageKind {
    pub fn mime(self) -> &'static str;     // "image/avif", "image/gif", "image/heif", "image/jpeg", "image/jxl", "image/png", "image/webp"
    pub fn extension(self) -> &'static str; // "avif","gif","heif","jpg","jxl","png","webp"
    pub const ALL: [ImageKind; 7] = [...];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sniff { pub kind: ImageKind, pub animated: bool }

// image/sniff.rs
/// Behaviour 5.1 / 2.4. Needs at least 32 bytes; shorter input is zero-padded like the Kotlin buffer.
pub fn sniff(head: &[u8]) -> Option<Sniff>;
pub fn kind_from_mime(mime: &str) -> Option<ImageKind>;   // exact, case-sensitive
/// ImageUtil.isImage: extension (case-sensitive, whole name when no dot) or sniff.
pub fn is_image_name(name: &str, head: impl FnOnce() -> Option<Vec<u8>>) -> bool;
/// ImageUtil.getExtensionFromMimeType, fallback "jpg".
pub fn extension_for(mime: Option<&str>, head: impl FnOnce() -> Option<Vec<u8>>) -> &'static str;

// image/decode.rs
pub struct Rgba8 { pub width: u32, pub height: u32, pub pixels: Vec<u8> }   // or image::RgbaImage
/// Decodes the whole image to RGBA8. Uses the `image` crate for JPEG/PNG/GIF/WebP; jxl-oxide for JXL; libheif-rs for HEIF/AVIF.
pub fn decode_rgba(bytes: &[u8]) -> Result<image::RgbaImage, ImageError>;
/// Header dimensions without a full decode (ImageUtil.extractImageOptions).
pub fn dimensions(bytes: &[u8]) -> Result<(u32, u32), ImageError>;
/// Encode JPEG at quality 100 (all re-encodes in the source use 100).
pub fn encode_jpeg_q100(img: &image::RgbaImage) -> Result<Vec<u8>, ImageError>;

// image/props.rs
pub fn is_animated_supported(sniff: Option<Sniff>) -> bool;   // behaviour 5.5: GIF true, animated WebP true, HEIF/AVIF false
pub fn is_wide(width: u32, height: u32) -> bool { width > height }

// image/ops.rs
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Side { Right, Left }
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Quarter { Cw90, Ccw90 }
pub fn split_in_half(img: &image::RgbaImage, side: Side) -> image::RgbaImage;        // W/2 x H
pub fn rotate_quarter(img: &image::RgbaImage, q: Quarter) -> image::RgbaImage;        // H x W
pub fn split_and_merge(img: &image::RgbaImage, upper: Side) -> image::RgbaImage;      // W/2 x 2H
/// Behaviour 5.10. Returns written part paths; on error removes all parts for the prefix.
pub fn split_tall(src: &[u8], dst_dir: &std::path::Path, prefix: &str, optimal_height: u32)
    -> Result<Vec<std::path::PathBuf>, ImageError>;
pub fn tall_split_plan(width: u32, height: u32, optimal_height: u32) -> Vec<SplitPart>;
pub struct SplitPart { pub index: u32, pub top: u32, pub height: u32, pub bottom: u32 }  // index 0-based
pub fn split_part_name(prefix: &str, index: u32) -> String;   // "{prefix}__{index+1:03}.jpg"

// image/background.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Background {
    Solid(Rgba),
    /// Four-stop vertical bands as the Android GradientDrawable paints them (see behaviour 5.12).
    VerticalBands { top: Rgba, bottom: Rgba },
}
/// Pure port of ImageUtil.chooseBackground after decoding. `landscape` = viewport wider than tall.
pub fn choose_background(img: &image::RgbaImage, landscape: bool) -> Background;
```

### 3.9 Torrent helpers (`torrent/*`)

```rust
// torrent/bencode.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Dict(std::collections::BTreeMap<Vec<u8>, Value>),   // unsigned byte order (BEP 3), see behaviour 6.1
}
pub fn parse(input: &[u8]) -> Result<Value, TorrentError>;     // requires full consumption
pub fn encode(v: &Value) -> Result<Vec<u8>, TorrentError>;     // rejects unsorted maps only if constructed externally (never for BTreeMap)

// torrent/meta.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat { pub id: Option<u32>, pub path: String, pub length: u64 }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentMeta {
    pub title: String,
    pub hash: String,             // 40 lowercase hex
    pub size: u64,
    pub trackers: Vec<String>,    // announce first, then announce-list flattened, duplicates kept
    pub files: Vec<FileStat>,
}
pub fn parse_torrent_file(bytes: &[u8]) -> Result<TorrentMeta, TorrentError>;

// torrent/model.rs (serde; kotlinx field names)
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TorrentDto {
    pub title: String,
    #[serde(default)] pub hash: Option<String>,
    #[serde(default, rename = "torrent_size")] pub torrent_size: Option<i64>,
    #[serde(default)] pub trackers: Option<Vec<String>>,
    #[serde(default, rename = "file_stats")] pub file_stats: Option<Vec<FileStatDto>>,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct FileStatDto { #[serde(default)] pub id: Option<i32>, pub path: String, pub length: i64 }
#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct TorrentRequest {
    pub action: &'static str,                       // "add"
    #[serde(skip_serializing_if = "String::is_empty")] pub hash: String,
    #[serde(skip_serializing_if = "String::is_empty")] pub link: String,
    #[serde(skip_serializing_if = "String::is_empty")] pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")] pub poster: String,
    #[serde(skip_serializing_if = "String::is_empty")] pub data: String,
    #[serde(rename = "save_to_db", skip_serializing_if = "std::ops::Not::not")] pub save_to_db: bool,
}

// torrent/client.rs
pub struct TorrServerClient { base: reqwest::Url, http: reqwest::Client }
impl TorrServerClient {
    pub fn new(port: u16, http: reqwest::Client) -> Self;           // base http://127.0.0.1:{port}
    pub async fn echo(&self) -> bool;                              // never errors; false = not running
    pub async fn add(&self, link: &str, title: &str, save: bool) -> Result<TorrentDto, TorrentError>;
    pub async fn upload(&self, bytes: Vec<u8>, title: &str, save: bool) -> Result<TorrentDto, TorrentError>;
    pub fn stream_url(&self, torrent: &TorrentDto, index: u32) -> String;   // behaviour 6.6
}

// torrent/supervisor.rs
pub struct TorrServerSupervisor { /* Arc<Mutex<State>>, child handle, port, trackers */ }
impl TorrServerSupervisor {
    pub async fn ensure_running(&self) -> Result<u16, TorrentError>;   // idempotent, 10-poll readiness wait
    pub async fn stop(&self) -> Result<(), TorrentError>;
    pub fn status(&self) -> ServerStatus;                              // for the UI row
}
pub enum ServerStatus { Stopped, Starting, Running { port: u16 }, Failed(String) }

// torrent/prefs.rs
pub struct TorrentPrefs { /* six Pref handles */ }
pub fn validate_port(s: &str) -> Result<u16, TorrentError>;             // 0..=65535, toIntOrNull semantics
pub fn validate_proxy_url(s: &str) -> Result<(), TorrentError>;         // scheme/host/protocol rules, behaviour 6.8
pub const DEFAULT_TRACKERS: &str = "...";                              // exact 26 lines, see p1-data-and-config.md section 6
pub const DEFAULT_PORT: &str = "8090";

// torrent/route.rs
pub enum Route { Direct, TorrServer }
pub fn classify(url: &str, server_base: &str, enabled: bool) -> Route;  // behaviour 6.10 predicate
pub fn magnet_index(url: &str) -> u32;                                  // "index=" parse, default 0
```

### 3.10 Web helpers (`web.rs`, behaviour 7.3)

```rust
pub const MIN_WEBVIEW_MAJOR: u32 = 118;   // Chrome-based minimum kept as the UA baseline; WebKitGTK gate is separate
pub fn infer_user_agent(default_ua: &str) -> String;     // the two regex transforms
pub fn parse_chrome_major(ua: &str) -> u32;              // 0 when unparseable
```

The WebKitGTK version gate and the settings live behind the `WebViewHost` trait in kuro-platform (area:12-network-stack owns the Cloudflare flow).

### 3.11 Logging and runtime helpers

```rust
// kuro-core::log
pub fn priority_to_level(p: LogPriority) -> tracing::Level;      // VERBOSE->TRACE, DEBUG->DEBUG, INFO->INFO, WARN->WARN, ERROR/ASSERT->ERROR
pub enum LogPriority { Verbose, Debug, Info, Warn, Error, Assert }
```

Runtime helpers (`kuro-core::rt`): `spawn_io<F, T>(f: F) -> JoinHandle<T>` = `tokio::task::spawn_blocking`; `spawn_net(fut)` = `tokio::spawn`; `on_ui(f)` = `glib::MainContext::default().invoke(f)` (only from non-GTK threads; the GUI crate provides it); `NonCancellable` = a detached task tracked in a `JoinSet` on the app context and drained at shutdown (behaviour 7.6).

## 4. Threading and async model

| Work | Where it runs | Why |
|---|---|---|
| Archive listing, entry reads, EPUB parsing | `spawn_blocking` (tokio blocking pool) | libarchive and zip are synchronous; mapping is read-only, so no lock needed |
| Image decode, crop, rotate, split, background | `spawn_blocking`; at most `num_cpus` concurrent decodes via a `tokio::sync::Semaphore` in the image service | CPU bound and memory heavy (a 1000 x 10000 px page is about 40 MB as RGBA8); the semaphore limits peak memory |
| Directory size, free space, nomedia, temp files | `spawn_blocking` | blocking filesystem calls |
| TorrServer HTTP (`echo`, `add`, `upload`) | tokio, shared `reqwest::Client` from area 12 | async network |
| TorrServer child process supervision | one tokio task per supervisor; the child is owned by the task; `kill_on_drop(true)` | process lifetime tied to the app |
| Preference reads and writes | synchronous on the calling thread (small SQLite or file writes; area 02 decides the store) | Android code calls them from main; Rust keeps them cheap |
| Preference change notifications | `tokio::sync::watch` channels; the GUI crate awaits each receiver's `changed()` inside a future spawned with `glib::MainContext::default().spawn_local` so UI updates run on the main thread | UI refresh on the main thread only |
| Toast | main thread (GTK) | GTK widgets are not thread-safe |
| WebView | main thread (WebKitGTK) through `WebViewHost` | WebKitGTK requirement |

Rules
- Nothing in `kuro-core` blocks the GTK main loop: every public async function is `async` or returns a value computed in `spawn_blocking`.
- No `block_on` inside GTK callbacks.
- Cancellation: image and archive jobs check a `CancellationToken` between parts (tall split) and between entries (listing). A cancelled split removes its partial parts (same as the Kotlin failure path).

## 5. Crates (chosen, with reasons)

| Need | Crate (version range to pin at implementation) | Reason | Android source replaced |
|---|---|---|---|
| Async runtime | `tokio` 1.x (rt-multi-thread, sync, time, process, fs) | used by all other areas | Kotlin coroutines |
| HTTP | `reqwest` 0.12.x (shared client from area 12) | TorrServer HTTP | OkHttp (`network.client`) |
| Serde JSON | `serde`, `serde_json` (already in `Cargo.toml`) | DTOs | kotlinx.serialization |
| Errors | `thiserror` 2.x | library errors | exceptions |
| Logging | `tracing` 0.1.x | structured logs | logcat |
| Zip read and write | `zip` 2.x with feature `deflate` (needed to read compressed CBZ and EPUB) and only `CompressionMethod::Stored` used for writing | CBZ, EPUB containers | libarchive zip path |
| libarchive (all formats) | `libarchive3-sys` (FFI, pkg-config `libarchive`) behind a small safe wrapper, or the `libarchive` crate if its API covers `read_open_memory` (verify in spike T-13a-031) | matches Aniyomi format coverage (the full set is fixed by the AAR build; OQ-13a-6) | `me.zhanghai.android.libarchive` |
| Memory map | `memmap2` 0.9.x | read-only mapping | `Os.mmap` |
| HTML parse (EPUB, lenient) | `scraper` 0.20+ (html5ever) | jsoup-like lenient HTML parser | jsoup |
| Images decode/encode | `image` 0.25.x with features `jpeg`, `png`, `gif`, `webp`; AVIF decoding needs an AV1 decoder whose choice is deferred to the spike (OQ-13a-5) | the Rust image stack | `BitmapFactory`, `Bitmap.compress` |
| JPEG XL | `jxl-oxide` 0.11.x (pure Rust decoder) | no system dependency | tachiyomi image-decoder JXL |
| HEIF/AVIF containers | `libheif-rs` (system `libheif`, optional feature `heif`) | the only mature HEIF decoder binding | tachiyomi image-decoder HEIF |
| Collation | `icu_collator` 2.x with `icu_locale_core` | Java PRIMARY collation equivalent | `java.text.Collator` |
| MD5 / SHA-256 / SHA-1 | `md-5` 0.10.x, `sha2` 0.10.x, `sha1` 0.10.x (RustCrypto) | exact digests | `java.security.MessageDigest` |
| Temp files | `tempfile` 3.x | `createTempFile` semantics | `File.createTempFile` |
| Dir walk | `walkdir` 2.x (`follow_links(false)`) | safe recursion | `File.listFiles` recursion |
| statvfs | `rustix` 0.38+ (feature `fs`) | no libc unsafe in our code | `StatFs` |
| URL form encoding | `form_urlencoded` 1.x (`byte_serialize`) | matches `URLEncoder.encode(_, "utf8")` (space to `+`) | `java.net.URLEncoder` |
| URL parsing (proxy validation) | `url` 2.x | `Uri.scheme` and `host` | `android.net.Uri` |
| Tests | `proptest` (dev-dependency) for natural order and bencode round trip | property tests | JUnit `@TestFactory` |
| JSON | `serde_json` | `{}` constant | `JsonObject` |

Crates considered and rejected:
- `natord`: case-sensitive by default, digit rules differ from the Java comparator; the port is 60 lines.
- `rust-icu` / `rust_icu_ucol`: C dependency; `icu_collator` is pure Rust.
- `unrar`: licence (non-free C++ code) and an extra dependency; RAR goes through libarchive instead.
- `hex` crate: `format!("{:02x}")` is enough.
- `bencode` crates: none keep the strict grammar and the BTreeMap key rule; a 120-line parser is simpler than auditing a crate.
- `lazy_static` / `once_cell` for the collator: `std::sync::OnceLock` (std) is enough.

Dropped (no Rust crate needed): RxJava bridge, EGL, hardware bitmap, FFmpeg SAF helper, FileProvider, MediaScanner, DeviceUtil vendor checks, dpToPx.

## 6. Android to Linux mapping (this area)

| Android API or library | Linux / Rust replacement | Owning module | Notes |
|---|---|---|---|
| `Environment.getExternalStorageDirectory()` | `kuro_platform::Paths::downloads_default()` | storage/mod.rs | XDG download dir |
| `ContextCompat.getExternalFilesDirs` / `Environment.getExternalStorageState` | dropped; GTK folder chooser / portal | (none) | SD card listing not needed |
| `StatFs` | `rustix::fs::statvfs` | storage/space.rs | f_bavail for free space |
| `MediaScannerConnection.scanFile` | no-op (document) | storage/nomedia.rs | no media indexer contract on Linux |
| `ParcelFileDescriptor` / `Os.mmap` | `memmap2::Mmap` | archive/mod.rs | read only, shared |
| `ContentResolver.openInputStream` / SAF `content://` | `std::fs::File` (paths only) | archive, storage | SAF removed |
| `FileProvider.getUriForFile` | dropped; portal `OpenURI` / `FileChooser` via `kuro-platform::Opener` | (none) | the shared-image cache directory is dropped |
| `Context.externalCacheDir` | `kuro_platform::Paths::cache()` | storage/temp.rs | |
| `Bitmap` / `BitmapFactory` / `Canvas` / `Matrix` | `image::RgbaImage`, `imageops::crop_imm`, quarter rotation | image | identical pixel rectangles |
| `BitmapRegionDecoder` | full decode then `crop_imm` | image/ops.rs | memory cost acceptable at 16 MB-class pages (noted) |
| `Bitmap.compress(JPEG, 100)` | `JpegEncoder::new_with_quality(w, 100)` | image/decode.rs | libjpeg-turbo output differs by a few bytes from Skia (noted) |
| `ImageDecoder` (tachiyomi native) | `image` + `jxl-oxide` + `libheif-rs` | image/decode.rs | sniff table ported exactly |
| `GradientDrawable` / `ColorDrawable` | `Background` enum, painted by the reader widget (cairo or CSS) | image/background.rs | |
| `Configuration.orientation` | viewport aspect ratio passed as `landscape: bool` | image/background.rs | |
| `Toast` | `src/toast.rs` in-app overlay | GUI crate | |
| `WebView` | `WebViewHost` (kuro-platform) on `webkit6` | web.rs + kuro-platform | area:12-network-stack owns flow |
| `Build.VERSION.SDK_INT` gates | dropped (single platform) | image/props.rs | the SDK 28 and 30 rules become "supported by the decoder" |
| `ActivityManager.MemoryInfo.totalMem` | `/proc/meminfo` `MemTotal` via kuro-platform | (dropped, see T-13a-045) | |
| `Resources.getSystem().displayMetrics` (max dimension) | `gdk::Monitor::geometry()` max of width and height at startup | image/ops.rs (parameter) | device value, as in Android |
| `Dispatchers.IO` / `Main` | tokio blocking pool / glib main context | rt helpers | section 4 |
| `GlobalScope` / `launchNow` | no equivalent; direct call or tracked task | rt helpers | section 7.6 of behaviour |
| `SharedPreferences` / `PreferenceManager` | `RawStore` implemented by area:02 | prefs | |
| `Flow` / `StateFlow` | `watch::Receiver<T>` | prefs | conflation identical |
| `kotlinx.serialization` | `serde` | torrent/model.rs | rename attributes |
| `okio.BufferedSource` | `Vec<u8>` / `bytes::Bytes` | image, archive | the whole page is read once |
| `logcat` | `tracing` | log.rs | priority table in 3.11 |
| `moko-resources` | i18n crate (area i18n); no backslash stripping | i18n | Localize.kt workaround dropped |
| `java.text.Collator` | `icu_collator` primary | util/collate.rs | |
| `java.security.MessageDigest` | RustCrypto digests | util/hash.rs | |
| `java.net.URLEncoder` | `form_urlencoded::byte_serialize` | torrent/route.rs | |
| `android.net.Uri` (proxy validation) | `url::Url` | torrent/prefs.rs | scheme and host checks |
| `DeviceUtil.isMiui`, `isSamsung`, `oneUiVersion`, `invalidDefaultBrowsers` | dropped | (none) | |
| EGL `DEVICE_TEXTURE_LIMIT` | dropped | (none) | |
| `Context.toast` | `src/toast.rs` | GUI | |

## 7. Pitfalls (must be handled in code review)

1. Path separators: always `PathBuf::join`. Archive entry names and EPUB paths use `/` (or `\` for the Windows-zipped EPUB case); they are strings, never `Path`.
2. UTF-16 counts: `chop`, `truncate_center` and the filename component limit count UTF-16 units in the source. The Rust port keeps that count (`encode_utf16`), so names are the same length as on Android for the same input.
3. Surrogates: a cut at 240 UTF-16 units can split a surrogate pair; move the cut back by one unit (behaviour 3.1).
4. Byte budgets: `take_bytes` cuts at a char boundary; never slice a `str` at a byte index that is not a boundary (use `is_char_boundary`).
5. Windows names: `CON`, `PRN`, `AUX`, `NUL`, `COM1-9`, `LPT1-9` get a leading `_` (cross-platform rule 5). This changes nothing on Linux users' existing folders unless the name is one of those tokens.
6. Natural order: the port must keep the unsigned 64-bit numeric compare and the digit-before-non-digit rule; the test vectors in p1-edge-cases-and-tests.md pin both.
7. Bencode key order: unsigned byte order (deliberate deviation, behaviour 6.1). The info-hash result is unchanged for canonical input.
8. JPEG quality 100 everywhere a page is re-encoded (split, merge, rotate): changing the quality changes cache sizes and visible artefacts.
9. Background: `choose_background` must not index out of range; the offset-column rule in behaviour 5.12 applies.
10. Tall-split failure: remove partial parts and keep the original page (the downloader depends on the original file surviving).
11. Temp dir cleanup on start: `TempFiles::clear` must not run while a download is using the directory (the Kotlin does not guard this; the Rust app clears only at startup, before the downloader starts).
12. Child process: `kill_on_drop(true)`, and on app exit the supervisor sends a stop request then waits up to 5 seconds before killing (proposal).
13. The TorrServer port is decided by the server when `0` is configured: always read the port from the readiness probe result, never assume the configured value.
14. Read-only copies (extensions): `set_permissions` after the copy (behaviour 3.8 proposal), so a failed copy is not left read-only.
15. `Pref::set` writes synchronously; calls from the GTK thread must stay short (no network inside a preference write).

## 8. Plug-in points in the existing mockup

- `src/settings.rs` (torrent rows and dialogs) will call `torrent::prefs::validate_port` and `validate_proxy_url` from the row validators. The mockup currently has plain entries; the change is the validator closure only.
- `src/screens.rs` (storage screen) will read `storage::space::free_bytes` and `storage::dirs::dir_size` through the storage service. Until the service exists, the mockup keeps its static numbers (no change required in this area).
- `src/ui.rs` has the `Built` / `Screen` helpers; the toast is a new `src/toast.rs` module that the screens call. It does not change the spec format.
- `spec/settings.json` keys used by this area (`settings_player_torrents`, `torrent_notice`, trackers, reset action) are unchanged; their English strings must match section 7 of p1-data-and-config.md.
- The mockup `Cargo.toml` gets `kuro-core = { path = "crates/kuro-core" }` only after the workspace from area:01 exists (T-01a-001). Nothing in this area requires a change to the mockup's `[dependencies]` before that.

## 9. Testing hooks

- `kuro-testkit` provides `TempTree` (temporary directories with fixture files), `FakeRawStore` (wraps `InMemoryStore` with broadcast), and `fixture(path)` for the torrent and archive fixtures.
- Every pure function in section 3 has a unit test written from the behaviour spec, with the vectors in p1-edge-cases-and-tests.md.
- The TorrServer client is tested against a local `axum` test server that answers `/echo`, `/torrents`, `/torrent/upload` with fixed JSON (no real TorrServer needed).
- The image background algorithm is tested with synthetic images (solid, two-band, dark border, white page, 100-109 px edge case).

## 10. Open design questions (answer before the matching task starts)

- OQ-13a-1: how TorrServer receives the tracker list on Linux (the Android AAR function `addTrackers` has no documented HTTP equivalent in this repository). Plan: test the bundled TorrServer build for a settings endpoint first; fallback appends trackers to magnet links at add time. Owner: T-13a-041.
- OQ-13a-2: index `0` for multi-file torrents (see behaviour 6.6). Owner: T-13a-044.
- OQ-13a-3: proxy mode `None` mapping (behaviour 6.8). Owner: T-13a-043.
- OQ-13a-4: whether the Linux WebView user agent keeps the Android-spoof transform (behaviour 7.3). Owner: T-13a-052 with area:12.
- OQ-13a-5: whether HEIF and AVIF are required at all for manga and anime sources (both are rare in practice); the decision sets whether `libheif-rs` is a default feature. Owner: T-13a-022.
