# Area 34 — Images, covers and cache: Rust design

Targets: crate `kuro-images` (new; module tree `src/images/` inside the mockup crate until the workspace exists, see §14.1), with `kuro-core` (names, errors, `ArtRef`), `kuro-domain` (number and time text), and `kuro-platform` (`Paths`, `Notifier`, `Opener`, share). Behavior contract: `p1-behavior.md` (cited as "behavior §N"). Data shapes: `p1-data-and-config.md` (cited as "data §N"). Tasks: `p1-tasks.md`.

Rules from `addenda/requirements-cross-platform.md` apply: no `#[cfg(target_os)]` in these crates; every directory comes from `Paths`; file names pass one sanitiser; notifications, opening and sharing go through the platform traits; the GTK texture work stays on the main thread.

Design decisions that differ from Kotlin are labelled **Deviation** and listed with the user-visible effect.

---

## 1. Crate and module layout

```
kuro-images/
  Cargo.toml                    features: default = ["avif", "jxl"]; avif, jxl, heif (off)
  src/lib.rs                    pub use: ImageService, ArtRef, ArtKind, Owner, LoadOptions, CachePolicy, LoadedImage, DataSource
  src/art.rs                    ArtRef, ArtKind, Owner (unifies Manga/MangaCover/Anime/AnimeCover inputs)  (behavior §4, §10)
  src/keys.rs                   cache_key(&ArtRef, custom_exists) -> String                          (behavior §10)
  src/options.rs                LoadOptions, CachePolicy, RequestSize, Scale                          (behavior §3, data §7)
  src/resource.rs               classify(&str) -> Resource                                            (behavior §5.2)
  src/sniff.rs                  ImageFormat, sniff(&[u8]) -> Option<ImageFormat>, is_image()         (behavior §17)
  src/sample.rs                 in_sample_size(), target_dim()                                        (behavior §15)
  src/decode/mod.rs             decode(bytes, DecodeOptions) -> Result<Decoded, ImageError>           (behavior §13-§14)
  src/decode/raster.rs          JPEG, PNG, GIF (first frame), WebP (first frame), BMP via `image`
  src/decode/avif.rs            AVIF stills (feature avif)
  src/decode/jxl.rs             JPEG XL stills (feature jxl)
  src/decode/heif.rs            HEIF stills (feature heif)
  src/decode/orient.rs          EXIF orientation (§4.5)
  src/fetch.rs                  ImageService::load: resolve -> cache layers -> network -> decode   (behavior §5-§9)
  src/http.rs                   request construction, status acceptance                            (behavior §7)
  src/cover_file.rs             library cover file cache: write_source_to_cover_file, promote       (behavior §8)
  src/disk_image_cache.rs       Coil-equivalent disk cache for non-library images                   (§6.2)
  src/memory_cache.rs           decoded image LRU by bytes                                          (§6.1)
  src/chapter_cache.rs          page-list and page-image cache                                      (§6.3, behavior §20)
  src/cover_paths.rs            cover, custom cover, background file names over Paths              (behavior §18)
  src/invalidate.rs             prep_update_cover, remove_covers decision logic                     (behavior §19)
  src/save.rs                   save_image(), staging, Pictures target                              (§8.1, behavior §21)
  src/share.rs                  staging of the last shared image                                    (§8.3)
  src/error.rs                  ImageError, user-facing mapping                                     (§12)
  src/testing.rs                fixture helpers (cfg(test) only)

kuro-core/src/fsname.rs         build_valid_filename(), take_bytes(), byte_size()                    (§8.2, behavior §22)
kuro-core/src/errors.rs         AppError, user_message()                                             (§12, behavior §28)
kuro-domain/src/format.rs       chapter_number_text(), episode_number_text(), duration_text(), relative_time_text()  (§11)
kuro-platform/src/lib.rs        trait Paths, trait Notifier, trait Opener, trait Sharer (new, see §10)
```

Ponytail: the four Kotlin cover fetchers (manga cover, manga projection, anime image, anime projection) collapse into one `ArtRef` and one `fetch_art` function. The Kotlin factories differ only in which URL and which cache directory they select, so one function with a `kind` argument is enough. The three cover-cache classes become one `cover_paths` module with three functions. Upgrade path: a separate `CoverSource` trait only when a second cover source type exists (none planned).

### 1.1 Cargo dependencies (crate names and the major version line to pin on first `cargo add`)

| Crate | Line | Used by | Why |
|---|---|---|---|
| `image` | 0.25 (features `jpeg`, `png`, `gif`, `webp`, `bmp`) | `decode/raster.rs` | pure Rust decoders; `AnimationDecoder` for GIF/WebP first frame |
| `avif-decode` | current 0.x (feature `avif`) | `decode/avif.rs` | pure Rust AV1 AVIF decode; alternative `libavif` C binding if the crate lacks a feature (spike task implied by T-34a-028) |
| `jxl-oxide` | 0.x (feature `jxl`) | `decode/jxl.rs` | pure Rust JPEG XL decoder |
| `libheif-rs` | 2.x (feature `heif`, off) | `decode/heif.rs` | needs system `libheif`; off by default (T-34a-029, P2) |
| `md-5` | 0.10 | `keys.rs`, `cover_paths.rs`, `chapter_cache.rs` | MD5 for file names (behavior §18.2) |
| `serde`, `serde_json` | 1 | `chapter_cache.rs` | page-list JSON (data §6.2) |
| `reqwest` | line used by area:12-network-stack | `http.rs` | shared client (`kuro-network`) |
| `tokio` | 1 (`rt-multi-thread`, `sync`, `fs`, `io-util`, `macros`) | `fetch.rs`, `disk_image_cache.rs` | async I/O, `Semaphore` (8 fetch, 3 decode) |
| `tokio-util` | 0.7 | `fetch.rs` | `CancellationToken` for cancelled cover loads |
| `bytes` | 1 | `fetch.rs` | cheap body sharing |
| `glib` | line from gtk4 0.11 | `decode/mod.rs` (`glib::Bytes`) | GTK-compatible byte container (Send + Sync) |
| `directories` | 6 | `kuro-platform` Linux impl | XDG base dirs and `UserDirs::picture_dir()` |
| `thiserror` | 2 | `error.rs` | error enums |
| `tracing` | 0.1 | all | replaces logcat (behavior §2.5) |
| `fs4` | 0.x | `disk_image_cache.rs` (free space) | replaces `StatFs.availableBlocksLong` (behavior §9.2) |
| `chrono` | 0.4 | `kuro-domain/format.rs` | relative time, local date (O-7) |
| `rust_decimal` | 1 | `kuro-domain/format.rs` | HALF_EVEN rounding for chapter numbers (§11) |
| `humansize` | 2 | `kuro-domain/format.rs` | decimal file sizes for the cache size label (§15) |
| `lru` | 0.12 | `memory_cache.rs` | recency ordering for decoded images (§6.1) |
| `parking_lot` | 0.12 | `memory_cache.rs` | cheap mutex for the memory cache |
| `async-channel` | 2 | `kuro-images` to GTK bridge | runtime-agnostic channel (§13.2) |
| `lcms2` | 6 (feature `icc`, off) | `decode/mod.rs` | ICC display profile, Deviation D-3 (§9) |
| `hyper` | 1 (dev-dependency) | tests | local HTTP fixture server (§17) |
| `open` | 5.x | `kuro-platform` Opener | `xdg-open` via the platform crate only |

Crates forbidden in these modules: anything that touches `xdg`, `zbus`, or `libsecret` directly (those belong to `kuro-platform`).

---

## 2. Wiring into the app

### 2.1 Replacement for `ImageLoader.Builder` (behavior §2)
One `ImageService` is created at start, after the tokio runtime and the HTTP client exist, and stored in the application context (the same place as the Injekt replacement from area:01-foundation-app-bootstrap):

```rust
pub struct ImageConfig {
    pub fetch_concurrency: usize,      // 8  (behavior §2.3)
    pub decode_concurrency: usize,     // 3  (behavior §2.3)
    pub memory_budget_bytes: u64,      // 64 MiB (§6.1)
    pub disk_image_budget_bytes: u64,  // 250 MiB upper clamp, 2 % of free space (§6.2, behavior §9.2)
    pub disk_image_min_bytes: u64,     // 10 MiB
    pub verbose: bool,                 // verbose_logging (behavior §2.5)
    pub low_ram: bool,                 // false on Linux (Deviation D-1, behavior §2.2)
}

impl ImageService {
    pub fn new(cfg: ImageConfig, http: reqwest::Client, paths: Arc<dyn Paths>) -> Result<Self, ImageError>;
}
```
Dropped from Kotlin: `allowRgb565` (D-1), `DebugLogger` (replaced by `tracing` filter), `OkHttpNetworkFetcherFactory` (the `reqwest` client is passed in directly).

### 2.2 Who calls what
- Lists and detail headers call `ImageService::load(ArtRef, LoadOptions, Size)` through `CoverImage` (§5.4).
- Reader pages call `decode_bytes` directly with the bytes produced by `ChapterCache` (§6.3).
- Save and share call `save::save_image` (§8.1).

### 2.3 Lifecycle
Created once at start. Dropped at shutdown after cancelling all loads. The memory and disk caches are owned by the service. No global statics (addendum rule 3).

---

## 3. Path mapping (Android → Linux via `Paths`)

`Paths` (in `kuro-platform`, Linux implementation with `directories`):
```rust
pub trait Paths: Send + Sync {
    fn data_dir(&self) -> PathBuf;      // XDG_DATA_HOME/kurobayomi
    fn cache_dir(&self) -> PathBuf;     // XDG_CACHE_HOME/kurobayomi
    fn pictures_dir(&self) -> Option<PathBuf>; // UserDirs::picture_dir()/Kurobayomi (created on save)
}
```
Mapping (also data §4.2):
| Kotlin root | Rust function |
|---|---|
| `covers`, `covers/custom` | `cover_paths::manga_cover(&paths)`, `manga_custom_cover(&paths)` → `<data>/covers`, `<data>/covers/custom` |
| `animecovers`, `animecovers/custom` | `anime_cover`, `anime_custom_cover` |
| `animebackgrounds`, `animebackgrounds/custom` | `anime_background`, `anime_custom_background` |
| `cacheDir/chapter_disk_cache` | `<cache>/chapter_images` (§6.3) |
| `cacheDir/image_cache` | `<cache>/images` (§6.2) |
| `cacheDir/shared_image` | `<cache>/shared_image` (§8.3) |
| `Pictures/Aniyomi` | `pictures_dir()` (fallback §8.1) |

Directories are created on first use, not at start (behavior §18.1 creates them on construction; Deviation D-2: lazily, to avoid empty folders for users who never save).

---

## 4. Decoding

### 4.1 Pipeline
`bytes` → `sniff` (§4.4) → if `custom_decoder` or format ∈ {Avif, Jxl, Heif} use the dedicated decoder, else `raster` → RGBA8 → `orient` (§4.5) → sample (§4.6 and behavior §15) → `Decoded`.

### 4.2 Types
```rust
pub struct DecodeOptions {
    pub target: TargetSize,            // from LoadOptions.size and scale (behavior §15.2)
    pub scale: Scale,                  // Fit | Fill
    pub crop_borders: bool,            // behavior §14.3, algorithm O-2
    pub display_profile: Option<Arc<[u8]>>, // Deviation D-3 (§9), None by default
    pub max_pixels: u64,               // 40_000_000 for covers, 120_000_000 for pages (§4.6)
}

pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub rgba: glib::Bytes,             // row-major RGBA8, straight alpha, stride = width * 4
    pub is_sampled: bool,              // behavior §14.5
    pub format: ImageFormat,
}

pub fn decode(bytes: &[u8], opts: &DecodeOptions) -> Result<Decoded, ImageError>;
```
Error mapping (behavior §14): zero dimension → `ImageError::DecodeInit` (text `Failed to initialize decoder`); no pixels → `ImageError::DecodeFailed` (`Failed to decode image`); unsupported sniff → `ImageError::UnsupportedFormat`.

### 4.3 Feature gates
Default features `avif`, `jxl`. Without a feature the sniffed type returns `UnsupportedFormat`, which the reader shows as the error placeholder (same as Coil's error state). CI builds the default features and `--no-default-features`.

### 4.4 Sniffing
```rust
pub fn sniff(head: &[u8]) -> Option<ImageFormat>;   // head = first min(32, len) bytes
pub enum ImageFormat { Avif, Gif, Heif, Jpeg, Jxl, Png, Webp }
impl ImageFormat {
    pub fn mime(self) -> &'static str;      // data §3.1 table
    pub fn extension(self) -> &'static str; // "jpg" for Jpeg
}
pub fn is_image(name: Option<&str>, head: impl FnOnce() -> Option<Vec<u8>>) -> bool; // behavior §17.3
```
Magic rules (Proposal; the Kotlin uses the native decoder, see O-3/O-4): `\xFF\xD8\xFF` Jpeg; `\x89PNG\r\n\x1a\n` Png; `GIF87a`/`GIF89a` Gif; `RIFF....WEBP` Webp; `ftyp` box with brand `avif` or `avis` Avif; `ftyp` with `heic`, `heix`, `hevc`, `hevx`, `heim`, `heis`, `mif1`, `msf1` Heif; `\xFF\x0A` or the JXL container signature `\x00\x00\x00\x0CJXL \x0D\x0A\x87\x0A` Jxl. Otherwise none. Only the first 32 bytes are read. Deviation D-18: BMP (`BM` magic, extension `bmp`) is decoded by the raster path and accepted by `is_image`. Kotlin's `ImageType` has no BMP entry, so its name and sniff checks reject BMP, although Coil's platform decoder still draws it. Rust accepts it for parity with what the user sees. Fixture F-07.

### 4.5 EXIF orientation (Deviation D-4, proposal)
Read the EXIF `Orientation` tag from JPEG (APP1) and apply rotation/flip before sampling. Behavior parity unknown (O-1); the Kotlin path does not do it explicitly. Ships behind a flag `exif_orientation` default true (T-34a-032).

### 4.6 Sampling and limits
- Sample size is computed in `sample::in_sample_size` exactly as behavior §15.1 (integer divide, highest-bit, Fit = max, Fill = min, floor 1, zero dimension treated as undefined).
- Decode at reduced size: `image`'s JPEG decoder supports scaled decoding (`scale_factor`); for others, decode full and downscale with `image::imageops::thumbnail` only when `sample > 1`. Output size is `ceil(src / sample)`.
- Memory guard: if `width * height > max_pixels`, the decode is refused with `ImageError::TooLarge` (text `Image too large`). Proposal to avoid crashes that Android avoids by OOM handling; not in Kotlin.

---

## 5. Loading and widgets

### 5.1 Fetch API
```rust
pub struct LoadRequest {
    pub art: ArtRef,
    pub options: LoadOptions,
    pub size: RequestSize,             // Original or Pixels
    pub cancel: CancellationToken,
}

impl ImageService {
    pub async fn load(&self, req: LoadRequest) -> Result<LoadedImage, ImageError>;
}

pub struct LoadedImage {
    pub image: Arc<Decoded>,           // shared with the memory cache
    pub source: DataSource,            // Memory | Disk | Network  (behavior §4.3, §6.2)
    pub key: String,                   // cache key (keys.rs)
}
```
Order of operations inside `load` (behavior §5.1, §6.2):
1. Build the key: `keys::cache_key(&art, custom_exists)` where `custom_exists = cover_paths::custom_file(&art, paths).exists()` (blocking `stat`, run on `spawn_blocking`).
2. If `memory_cache` read enabled and key in `memory_cache` → return `Memory`.
3. If `use_custom_cover` and custom file exists → load file (`DataSource::Disk`), decode, insert to memory cache.
4. Classify URL (`resource::classify`); invalid → `ImageError::InvalidImage`; null → `ImageError::NoCoverSpecified`.
5. File → read file (`DataSource::Disk`). Content URI → `ImageError::InvalidImage` (Deviation D-5).
6. HTTP → `fetch_http` (§5.2 flow, behavior §6.2).

### 5.2 `fetch_http`
```text
if art.favorite and cover_file exists and disk_cache.read_enabled  -> read cover file (Disk)
snapshot = disk_image_cache.open(key) if read_enabled              (Disk)
if snapshot:
    if art.favorite and cover_file path exists-able:
        copy snapshot -> cover file (cover_file::write), remove snapshot entry, read cover file
    else -> read snapshot (Disk)
response = http::get(url, headers, cache_control)                  (behavior §7)
if art.favorite and write enabled -> cover_file::write_from_stream(response) -> read cover file (Network)
if disk_image_cache.write_enabled -> disk_image_cache.put(key, response) -> read (Network)
else -> decode from in-memory bytes (Network)
```
Mirrors behavior §6.2 step by step, including the `cacheResponse != null → DISK` distinction: the Rust code sets `Disk` when the `reqwest` response was served from a Rust-side cache (never, since the HTTP-level cache is off, behavior §7.2), so `Network` is always reported. Deviation D-6: `DataSource` for 304 responses is `Network`.

### 5.3 Texture creation (GTK thread only)
```rust
fn texture(decoded: &Decoded) -> gdk::Texture {
    gdk::MemoryTexture::new(
        decoded.width as i32,
        decoded.height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &decoded.rgba,
        (decoded.width * 4) as usize,
    ).upcast()
}
```
`glib::Bytes` is shared, so the texture does not copy the pixel data. Memory: one RGBA copy per decoded image, held by the memory cache (§6.1). Textures are dropped when the widget unbinds.

### 5.4 `CoverImage` widget
A GTK widget that replaces the mockup `cover(w, h)` (`src/ui.rs:176-182`).
```rust
pub enum CoverRatio { Square, Book, Thumb }   // 1:1, 2:3, 16:9 (behavior §25.1)
pub struct CoverImage { /* gtk::Stack(placeholder | picture | error) inside gtk::AspectFrame */ }
impl CoverImage {
    pub fn new(ratio: CoverRatio, corner_radius_css_class: &str) -> Self;
    pub fn bind(&self, art: ArtRef, options: LoadOptions, size: RequestSize, service: Rc<ImageService>);
    pub fn unbind(&self);   // cancels the in-flight token, drops the texture
}
```
- Visual states: placeholder (CSS class `kuro-cover-placeholder`, background `rgba(136, 136, 136, 0.1216)` = `#1F888888`), picture (`gtk::Picture`, `content_fit = Cover`), error (bundled symbolic SVG for `cover_error`, behavior §25.3).
- Crossfade: `gtk::Stack` with `transition_type = Crossfade` and `transition_duration = 300` ms (behavior §2.4). GTK disables stack transitions when `gtk-enable-animations` is false (to verify in the spike, T-34a-054).
- `bind` spawns the load on the tokio runtime and applies the result on the GTK main context (§13.2). A stale result (widget re-bound to another key) is dropped by comparing keys.

### 5.5 Full viewer
`CoverViewer` is a modal `gtk::Window` (transient for the main window) with a `gtk::Picture` for the image, an overlay pill at the top right (Share, Save, Edit), and for anime a left pill with Close and the page switch. It uses `LoadOptions { memory_cache: Disabled, size: Original, use_background }` (behavior §26.2). Zoom: `gtk::Picture` inside `gtk::ScrolledWindow` with `Ctrl+scroll` zoom; the 500 ms zoom animation of Kotlin is replaced by a 500 ms `adw`-free `glib::timeout` ease on the scale factor (Deviation D-7). Full UI rules: `p1-ui-parity.md`.

---

## 6. Caches

### 6.1 Memory cache (decoded images)
```rust
pub struct MemoryCache { /* LRU keyed by String, value Arc<Decoded>, weighted by rgba.len() */ }
impl MemoryCache {
    pub fn new(budget_bytes: u64) -> Self;     // 64 MiB (Proposal; Coil default is 20 % of heap, behavior §9.3)
    pub fn get(&self, key: &str) -> Option<Arc<Decoded>>;
    pub fn insert(&self, key: String, value: Arc<Decoded>);
}
```
Eviction by least-recently-used until total ≤ budget. Thread-safe with `parking_lot::Mutex` (or `std::sync::Mutex`). Crate for the LRU ordering: `lru` 0.12 with manual weight accounting. Deviation D-8: decoded-texture budget is fixed, not a fraction of the Java heap.

### 6.2 Disk image cache (non-library images; Coil-equivalent)
Layout: `<cache>/images/<md5(key)>` (value file) and `<cache>/images/<md5(key)>.part` (during write). The key is the cache key string from `keys.rs`, not the URL (behavior §9.1).
```rust
pub struct DiskImageCache { root: PathBuf, max_bytes: AtomicU64, inner: tokio::sync::Mutex<LruIndex> }
impl DiskImageCache {
    pub async fn open(root: PathBuf, min: u64, max_clamp: u64, free_fraction: f64) -> io::Result<Self>;
    pub async fn get(&self, key: &str) -> Option<PathBuf>;      // touch mtime on hit
    pub async fn put_stream<S>(&self, key: &str, body: S) -> io::Result<PathBuf>
        where S: Stream<Item = io::Result<Bytes>>;              // writes .part then rename
    pub async fn remove(&self, key: &str) -> io::Result<bool>;
}
```
Sizing (behavior §9.2): `max = clamp(free_fraction * free_bytes(root), min, max_clamp)` with `free_fraction = 0.02`, `min = 10 MiB`, `max_clamp = 250 MiB`; if free space cannot be read, `max = min`.
Eviction: after each put, if the sum of file sizes exceeds `max`, remove oldest-mtime files until under `max`. Sum is kept in memory and recomputed at `open`. Ponytail: mtime LRU without an index; upgrade path is an SQLite index when the directory has more than 10 000 entries (noted, not built).
Write semantics: `put_stream` writes `<md5>.part`; on success `rename` to `<md5>` (atomic on the same filesystem); on error the `.part` file is removed (matches Coil's abort, behavior §9.1 step 6). A reader never sees a partial value.
Concurrency: an in-flight set guards same-key writes; a second writer for a key already being written waits for the first (Kotlin skips silently for the page cache; for image cache Coil's editor is also exclusive, so waiting is the Coil-equivalent behavior).

### 6.3 Chapter cache (reader pages and page lists)
Layout under `<cache>/chapter_images`:
- `pages/<md5(manga_id as decimal + chapter_url)>.json` — page list (data §6.2).
- `img/<md5(image_url)>` — page image bytes (no `.0` suffix; Deviation D-9, the DiskLruCache suffix is replaced by the file name).
- `img/<md5(image_url)>.tmp` — temp file during write.
```rust
pub struct ChapterCache { root: PathBuf, limit_bytes: u64 /* 100 MiB, data §8.3 */, index: tokio::sync::Mutex<LruIndex> }
impl ChapterCache {
    pub async fn open(root: PathBuf) -> io::Result<Self>;
    pub async fn get_page_list(&self, manga_id: i64, chapter_url: &str) -> Result<Vec<Page>, CacheError>;
    pub async fn put_page_list(&self, manga_id: i64, chapter_url: &str, pages: &[Page]) -> Result<(), CacheError>;
    pub async fn is_image_cached(&self, url: &str) -> bool;
    pub fn image_path(&self, url: &str) -> PathBuf;   // no existence check (behavior §20.4)
    pub async fn put_image<S>(&self, url: &str, body: S) -> Result<(), CacheError> where S: Stream<Item = io::Result<Bytes>>;
    pub async fn clear(&self) -> io::Result<usize>;   // behavior §20.6
    pub async fn size(&self) -> io::Result<u64>;      // behavior §20.8 (readable size)
}
```
- Page-list miss returns `CacheError::Miss`; a corrupt JSON file is deleted and treated as a miss (Deviation D-10; Kotlin leaves the file).
- `put_page_list` failures are logged at warn and ignored at the call site (behavior §20.3).
- Eviction: LRU by mtime, both `pages/` and `img/` count toward `limit_bytes`; `put_*` evicts when over the limit.
- `clear` removes every file in both folders and returns the number of files removed. There is no journal file, so the Kotlin journal exception has no equivalent. Deviation D-11: `.tmp` leftovers are removed too; Kotlin leaves them behind (behavior §20.6).
- Reader integration: `HttpPageLoader` equivalent calls `get_page_list` / `is_image_cached` / `put_image` (behavior §20.7).

### 6.4 Library cover file cache
No separate cache type. `cover_file::write_from_stream(path, body)` implements behavior §8.3 (mkdirs, delete, create, copy, delete on failure). `cover_file::promote_from_disk(snapshot_path, cover_path)` implements behavior §8.2 (copy then remove the disk entry). Covers are plain files under `<data>/covers`, `<data>/animecovers` and `<data>/animebackgrounds` (with their `custom` subfolders) and survive cache clearing (§3).

---

## 7. Keys

```rust
pub fn cache_key(art: &ArtRef, custom_exists: bool) -> String {
    let owner = match art.owner { Owner::Manga => "manga", Owner::Anime => "anime" };
    if custom_exists {
        format!("{owner};{};{}", art.entry_id, art.last_modified)
    } else {
        format!("{owner};{};{}", art.url.as_deref().unwrap_or("null"), art.last_modified)
    }
}
```
Equivalence with the Kotlin keyers (behavior §10): the four domain keyers and the two projection keyers reduce to this function. The background branch of `AnimeKeyer` (behavior §10.2) uses `art.kind == Background`, `art.url = backgroundUrl`, `art.last_modified = backgroundLastModified`, and `custom_exists = animebackgrounds/custom/<md5(id)>.exists()`. Test vectors for equivalence are in `p1-edge-cases-and-tests.md` §3 (C12).

```rust
pub struct ArtRef {
    pub owner: Owner,                  // Manga | Anime
    pub entry_id: i64,
    pub source_id: i64,                // for HttpSource client lookup (behavior §7.1)
    pub kind: ArtKind,                 // Cover | Background (Background only for Anime)
    pub url: Option<String>,           // thumbnail_url or background_url
    pub favorite: bool,                // isLibraryManga / isLibraryAnime (behavior §4)
    pub last_modified: i64,            // cover_last_modified or background_last_modified (epoch ms)
}
```
Mapping from the Kotlin inputs: `Manga → ArtRef{Owner::Manga, Cover, thumbnail_url, favorite, cover_last_modified}`; `MangaCover → same`; `Anime → ArtRef{Owner::Anime, kind = if use_background {Background} else {Cover}, url = background_url or thumbnail_url, last_modified accordingly}`; `AnimeCover → Cover`.

---

## 8. Names and saving

### 8.1 Save API
```rust
pub enum SaveTarget {
    Pictures { folder: Option<String> },   // folder = folder-per-title (behavior §21.8)
    Staging,                                // <cache>/shared_image (behavior §21.7)
}
pub enum SaveSource {
    Page(Bytes),                            // original bytes, no re-encode (behavior §21.2)
    CoverRgba(Arc<Decoded>),                // re-encoded as JPEG quality 100
}
pub struct SaveRequest { pub name: String, pub source: SaveSource, pub target: SaveTarget }

pub fn save_image(paths: &dyn Paths, req: SaveRequest) -> Result<PathBuf, ImageError>;
```
Algorithm (behavior §21.4-21.5):
1. Bytes: sniff; no image → `ImageError::NotAnImage` (text `Not an image`).
   CoverRgba: encode JPEG quality 100 via `image::codecs::jpeg::JpegEncoder::new_with_quality(w, 100)`; type is Jpeg.
2. `file_name = fsname::build_valid_filename(&format!("{}.{}", req.name, ext))` (§8.2).
3. Directory: `Pictures` → `paths.pictures_dir()` (or `$HOME/Pictures` fallback if `None`) joined with `Kurobayomi`, then `folder` if present; `Staging` → `<cache>/shared_image` after `remove_dir_all` (behavior §21.7; the caller does the delete).
4. `create_dir_all`; write to `<dir>/<file_name>.part`, then `rename` over `<dir>/<file_name>` (overwrite semantics, behavior §21.5). No media scan (Deviation D-12).
5. Return the absolute path. The UI turns it into a `gio::File` when needed.
Error mapping: write failure → `ImageError::SaveFailed` (text `Error saving picture`, behavior §23.1 key `error_saving_picture`).

### 8.2 File-name sanitiser (`kuro-core/src/fsname.rs`)
```rust
pub fn build_valid_filename(orig: &str) -> String;   // Android-compatible base
pub fn take_bytes(s: &str, n: usize) -> String;      // behavior §22.2
pub fn byte_size(s: &str) -> usize { s.len() }       // UTF-8 length
```
`build_valid_filename` = behavior §22.1 steps 1-4 (trim `.` and space, forbidden characters to `_`, empty → `(invalid)`, `take(240)` in chars). Addendum extension (applied after the base, changes only invalid results, so Android-valid names are unchanged):
- Forbidden on NTFS: `<>:"/\|?*` and control characters. (`:` and `\` are already covered by the base.)
- Reserved device names (case-insensitive, with or without extension): `CON, PRN, AUX, NUL, COM1-COM9, LPT1-LPT9` → prefix `_`.
- Trailing dots and spaces removed (already done by the base trim).
- Length: at most 255 UTF-16 code units; the base `take(240)` already keeps the result under that.
Deviation D-13: names with a leading dot lose it (base behavior), reserved names get `_` prefix (new).

### 8.3 Share staging
- The last share is kept: `Staging` target deletes `<cache>/shared_image` first (behavior §21.7).
- The share action is `Sharer::share(path)` (§10). The staged file is not removed on exit (behavior §21.7 comment).
- Cover share uses `Staging` without the delete (Kotlin behavior, behavior §21.7 last sentence). Deviation D-14: the cover share also clears the staging folder first, to stop accumulation (edge E-IMG-15).

### 8.4 `.nomedia`
`cover_paths::create_nomedia(dir)`: if `dir` exists and `dir/.nomedia` is absent, create it empty (behavior §18 analog to `DiskUtil.createNoMediaFile`, DiskUtil.kt:87-98). Called only by area:14-source-local-and-metadata (local source folders) and by downloads (area:23-downloads-manga) when they create library folders. It is not called for `Pictures` saves (Kotlin does not call it there either).

---

## 9. Display profile (ICC) — Deviation D-3

Kotlin sets an ICC profile byte array on the native decoder and on `SubsamplingScaleImageView` (behavior §14.6). GTK 4.16 does not apply ICC profiles to `GdkTexture` content. Proposal: keep the setting `pref_display_profile_key` in the schema, read it, but apply it only through `lcms2` (crate `lcms2`) in `decode/mod.rs` when the feature `icc` is enabled (not built by default; P2, T-34a-031). User-visible effect until then: the setting does nothing on Linux, and the settings screen says so.

---

## 10. Platform traits (`kuro-platform`)

Added to the addendum's trait list (addendum table, rows 1-13 unchanged):

```rust
pub trait Notifier: Send + Sync {
    /// Shows "Picture saved" with an optional preview and open/share actions (behavior §23.2).
    fn picture_saved(&self, path: &Path, preview: Option<&gdk::Texture>);
    /// Shows "Error" with the given text or "Unknown error" (behavior §23.2).
    fn picture_error(&self, text: Option<&str>);
    fn clear_picture(&self);
}

pub trait Opener: Send + Sync {
    fn open(&self, path: &Path) -> Result<(), PlatformError>;     // default app (xdg-open / portal)
    fn reveal(&self, path: &Path) -> Result<(), PlatformError>;   // file manager, parent folder
}

pub trait Sharer: Send + Sync {
    /// Linux has no system share sheet. Linux impl: open the file with the default viewer
    /// through `Opener::open` and copy its URI to the clipboard. Returns Ok when either works.
    fn share_file(&self, path: &Path) -> Result<(), PlatformError>;
    fn copy_image(&self, texture: &gdk::Texture) -> Result<(), PlatformError>; // clipboard
}
```
Linux implementations: `Notifier` via `gio::Notification` sent through the `gtk::Application` (`send_notification` with id `download-image`, default action opens the file, button `Share`); `Opener` via `open` crate (`xdg-open`) or `ashpd` OpenURI in a sandbox; `Sharer` as described; `copy_image` via `gdk::Display::clipboard().set_texture(...)`.
Windows (later): `Notifier` → toast, `Opener` → ShellExecute, `Sharer` → Windows share UI (WinRT `DataTransferManager`) (later milestone).

---

## 11. Formatting (`kuro-domain/src/format.rs`)

```rust
pub fn chapter_number_text(n: f64) -> String;   // == episode_number_text
pub fn episode_number_text(n: f64) -> String;
pub fn duration_text(d: std::time::Duration, fallback: &str) -> String;   // behavior §27.2
pub fn relative_time_text(epoch_ms: i64, now_ms: i64) -> String;          // behavior §27.3
```
- `chapter_number_text`: format with ASCII digits, `.` decimal separator, HALF_EVEN rounding to 3 fraction digits, trailing zeros removed, no grouping; `NaN` → `NaN`; positive infinity → `∞`; negative infinity → `-∞`; `-0.0` → `0` (Java prints `-0` for negative zero? Verified only for +0; Proposal: normalise to `0`). Implementation: `let r = round_half_even(n, 3)`; then `format!("{:.3}")` and strip zeros.
  Deviation D-15: digits are always ASCII (Java uses the locale's digits, e.g. Arabic-Indic for `ar`, behavior §27.1). Open question O-5.
- `duration_text`: exactly behavior §27.2 with the four unit strings as `format!("{}d")`, `format!("{}h")`, `format!("{}m")`, `format!("{}s")`, joined by a single space, `fallback` when empty.
- `relative_time_text`: behavior §27.3 steps 1-4. Absolute date after 7 days uses `chrono` local date `%b %-d` (current year) or `%b %-d, %Y` (other year); the exact Android format is O-7.

Rounding helper: `round_half_even(x, digits)` = `(x * 10^digits)` then ties-to-even, matching Java `HALF_EVEN` on the decimal expansion of the double (use `rust_decimal` crate with `RoundingStrategy::MidpointNearestEven` on `Decimal::from_f64_retain`; the tests pin the golden table).

---

## 12. Errors

```rust
#[derive(thiserror::Error, Debug)]
pub enum ImageError {
    #[error("Invalid image")] InvalidImage,
    #[error("No cover specified")] NoCoverSpecified,
    #[error("HTTP {0}")] Http(u16, String),                 // reason phrase kept for logs
    #[error("Null response source")] NullBody,
    #[error("Failed to initialize decoder")] DecodeInit,
    #[error("Failed to decode image")] DecodeFailed,
    #[error("Unsupported image format")] UnsupportedFormat,
    #[error("Image too large")] TooLarge,
    #[error("Not an image")] NotAnImage,
    #[error("Error saving picture")] SaveFailed,
    #[error(transparent)] Io(#[from] std::io::Error),
    #[error("cancelled")] Cancelled,
}
```
- Cover display: any error except `Cancelled` shows the error placeholder (behavior §25.3). No text is shown on covers.
- Save and share: user text is `Error saving picture` (picture), or the snackbar keys from behavior §23.1; the notification shows `Error` with the error's display text (behavior §23.2).
- Logging: `tracing::error!` with the same texts as the Kotlin `logcat` lines (behavior §8.1, §8.2, §20.3).
- `AppError` for the exception mapping (behavior §28) lives in `kuro-core/src/errors.rs`:
```rust
pub enum AppError {
    TorrServerDisabled,
    Http { code: u16 },
    UnknownHost { host: String, offline: bool },
    NoResults,
    SourceNotInstalled,
    Other { kind: &'static str, message: Option<String> }, // kind mirrors the Kotlin simple name
}
pub fn user_message(e: &AppError) -> String;   // exact texts of behavior §28.1
```
The `Other` arm reproduces `"Exception"`/`"IOException"` → message or kind, otherwise `"{kind}: {message}"` with `message` printed as `null` when absent (behavior §28.1 step 6).

---

## 13. Async and threading model

### 13.1 Threads
- GTK main thread: widgets, textures, notifications, snackbars (`adw`-free toasts via `gtk::Overlay` labels).
- tokio multi-thread runtime (`tokio::runtime::Runtime` created in `main`, handle stored in the service): network, file I/O, decode (on `spawn_blocking` behind `decode_permits: Semaphore(3)`), fetch permits `fetch_permits: Semaphore(8)`.
- No work on the GTK thread except texture creation and widget updates.

### 13.2 Bridging results to GTK
```rust
let (tx, rx) = async_channel::bounded::<Result<LoadedImage, ImageError>>(1);
let svc = service.clone();
runtime.spawn(async move { let r = svc.load(req).await; let _ = tx.send(r).await; });
glib::spawn_future_local(async move {
    if let Ok(result) = rx.recv().await { widget.apply(result); }
});
```
`async_channel` is used because it is runtime-agnostic (works from both threads). Cancellation: the `CancellationToken` is checked before the decode permit and before writing to a cache; the HTTP future is dropped when cancelled.

### 13.3 Saves and edits
Save, share and cover edits run on the tokio runtime; the UI update is posted back with the same channel pattern. Edits of covers run one at a time per entry (a `tokio::sync::Mutex` keyed by entry id; Kotlin has no such lock, the risk is in edge E-IMG-8).

### 13.4 Limits (summary)
| Limit | Value | Source |
|---|---|---|
| Concurrent fetches | 8 | behavior §2.3 |
| Concurrent decodes | 3 | behavior §2.3 |
| Memory cache budget | 64 MiB | §6.1 (Proposal) |
| Disk image cache | 2 % free, clamp 10-250 MiB | behavior §9.2 |
| Chapter cache | 100 MiB | data §8.3 |
| Max decoded pixels (cover) | 40 M | §4.6 (Proposal) |
| Max decoded pixels (page) | 120 M | §4.6 (Proposal) |
| Notification preview bound | 720 x 1280 | behavior §23.2 |
| Crossfade | 300 ms | behavior §2.4 |
| Viewer zoom | 500 ms | behavior §26.2 |

---

## 14. Integration with the mockup

### 14.1 Mockup today
Single crate `kurobayomi` (`Cargo.toml`: `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, serde, serde_json). Images are placeholders:
- `src/ui.rs:176-182`: `pub fn cover(w: i32, h: i32) -> gtk::Frame` with the comment "ponytail: grey frame stands in for cover art until there is an image loader".
- `src/entry.rs:53-67`: `cover_grid(n, w, on)` calls `cover(w, w * 10 / 7)`.
- `src/entry.rs:91`: detail header `cover(130, 190)`; `entry.rs:93` picks `full_images` (anime) or `full_cover` (manga) as the viewer id.
- `src/entry.rs:398`: `cover_grid(24, 130, ...)` (library grid).
- `src/screens.rs:99`, `:184`: `cover(w, h)` for the list and the selection bar cover.
- `spec/entries.json`: dialogs `full_cover` (line 235) and `full_images` (line 849).

### 14.2 Changes (tasks T-34a-052, T-34a-053)
- Replace `ui::cover(w, h)` with `CoverImage::new(ratio)` + `bind(...)`. The caller chooses the ratio: grid covers `Book` (2:3), the detail header `Book`. Where Kotlin uses `ItemCover.Thumb` (16:9) or `ItemCover.Square` (1:1), the Rust caller uses the same ratio (grep `ItemCover.` in `app/src/main/java/eu/kanade/presentation` before wiring). The fixed `w * 10 / 7` ratio in `src/entry.rs:67` is retired.
- Add `src/images/` with the modules of §1 (feature-gated decoders).
- Replace the grey frame comment; remove the `ponytail:` marker when the service exists.
- The `full_cover`/`full_images` dialogs become `CoverViewer` (§5.5) fed by the spec's item list; the spec file stays the single source for labels.

### 14.3 Spec references
The spec strings for Edit cover, Edit background, Delete, Share, Save, Close come from `spec/entries.json` (dialog ids above); the labels must match behavior §23.1 and §24.1 exactly. Items not in the spec (switch arrow) use the aria label text from behavior §26.1.

---

## 15. Android API → Linux mapping

| Android / Kotlin | Kurobayomi |
|---|---|
| Coil `ImageLoader` | `ImageService` (§2) |
| Coil `Fetcher` / `Fetcher.Factory` | `fetch_art` inside `ImageService::load` (§5.1) |
| Coil `Keyer` | `keys::cache_key` (§7) |
| Coil `Decoder` / `TachiyomiImageDecoder` | `decode::decode` (§4) |
| Coil `DiskCache` | `DiskImageCache` (§6.2) |
| Coil `MemoryCache` | `MemoryCache` (§6.1) |
| `ImageRequest.size(Size.ORIGINAL)` | `RequestSize::Original` |
| `Scale.FIT`/`FILL` | `Scale::Fit`/`Fill` |
| `AsyncImage` + `ColorPainter` placeholder | `gtk::Stack` + CSS placeholder (§5.4) |
| `ContentScale.Crop` | `gtk::Picture::set_content_fit(Cover)` |
| `crossfade(300)` | `gtk::Stack` crossfade 300 ms (§5.4) |
| `SubsamplingScaleImageView` | `gtk::Picture` in `gtk::ScrolledWindow` + zoom (§5.5) |
| `ImageDecoder` (native) | `image`, `avif-decode`, `jxl-oxide`, `libheif-rs` (§1.1) |
| `Bitmap.compress(JPEG, 100)` | `image::codecs::jpeg::JpegEncoder` quality 100 |
| `Bitmap.copy(HARDWARE)` | none (Deviation; GTK texture) |
| `DiskLruCache` (chapter cache) | `ChapterCache` (§6.3) |
| `Context.cacheDir` / `filesDir` / `getExternalFilesDir` | `Paths::cache_dir()` / `Paths::data_dir()` |
| `Environment.getExternalStoragePublicDirectory(PICTURES)` | `Paths::pictures_dir()` (`directories::UserDirs::picture_dir`) |
| `MediaScannerConnection.scanFile` | none (Deviation D-12) |
| MediaStore insert/query (Q+) | none (plain write) |
| `FileProvider.getUriForFile` | `gio::File::for_path(...).uri()` where a URI is needed |
| `StatFs.availableBlocksLong` | `fs4::available_space` |
| `Formatter.formatFileSize` | `humansize`-style decimal formatter (KB/MB/GB) in `kuro-domain` |
| `NotificationCompat.BigPictureStyle` | `gio::Notification` with `gio::BytesIcon` of the preview |
| `Intent.ACTION_SEND` chooser | `Sharer::share_file` (§10) |
| `ClipData.newRawUri` / clipboard | `gdk::Clipboard::set_texture` (`Sharer::copy_image`) |
| `ActivityResultContracts.GetContent("image/*")` | `gtk::FileDialog::open` with a `gtk::FileFilter` (`image/*` MIME list) |
| `Settings.Global` animator scale | `gtk::Settings::is_gtk_enable_animations` |
| `ActivityManager.isLowRamDevice`, `memoryClass` | not used (Deviation D-1) |
| `GLUtil` EGL texture size | not used (GTK) |
| `Build.VERSION.SDK_INT` checks | not used |
| `R.drawable.cover_error` | bundled symbolic SVG via `gio::Resource` (`gtk::Image::from_resource`) |
| `Dispatchers.IO.limitedParallelism(n)` | `tokio::sync::Semaphore(n)` |
| `logcat` | `tracing` |
| `Instant.now().toEpochMilli()` | `SystemTime::now().duration_since(UNIX_EPOCH)` as millis |
| `DateUtils.getRelativeTimeSpanString` | `kuro-domain::format::relative_time_text` (§11) |
| `Intent` permission check `canRequestPackageInstalls` | dropped (no equivalent) |
| `FLAG_SECURE` | dropped (no equivalent, Deviation D-16, see §16) |

---

## 16. Pitfalls

1. GTK objects (`gdk::Texture`, `gtk::Widget`) are not `Send`. Keep them out of tokio tasks; pass `Arc<Decoded>` / `glib::Bytes` (both `Send`) and build textures on the main thread.
2. Blocking file `stat` (the custom-cover existence check runs on every key computation) must use `spawn_blocking`, not the GTK thread.
3. `Path::exists()` for keys is a race with a concurrent save; key mismatch only causes an extra decode, never a wrong image, because the key includes the timestamp.
4. Writing the cover file in place is not atomic in Kotlin (behavior §8.3). Rust writes `<name>.part` then renames, which removes the partial-read window (improvement, Deviation D-17).
5. Covers in `<data>` must never be deleted by a cache clear (`clear` only touches `<cache>`).
6. JPEG EXIF rotation changes the reported size; sampling uses the rotated size (apply orientation before `in_sample_size`).
7. `avif-decode` and `jxl-oxide` are pure Rust but can be slow on large images; keep them on the decode semaphore so 3 decodes never block fetches.
8. Non-ASCII file names: UTF-8 byte truncation (`take_bytes`) must not split a code point; test with a 3-byte CJK title.
9. `gdk::MemoryTexture` requires `stride >= width * 4`; passing a smaller stride panics in GTK.
10. The `Sharer` on Linux opens the file in the default viewer; under Flatpak this needs the portal (`ashpd`), not `xdg-open` directly.
11. The memory cache must not hold GTK objects; storing `Arc<Decoded>` keeps the RGBA alive, so the budget counts `rgba.len()`.
12. Keys containing `;` and URLs are never parsed back; do not use keys as file names (file names are `md5(key)`).

---

## 17. Tests

See `p1-edge-cases-and-tests.md` for the case list, golden vectors and fixtures. Each module has `#[cfg(test)]` tests; the golden table test lives in `kuro-images/tests/golden.rs` and `kuro-domain/tests/format.rs`. No network: HTTP is tested against a local `hyper`-based fixture server started in the test.
