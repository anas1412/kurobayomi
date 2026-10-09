# 43-reader-core: Rust design (Kurobayomi)

Proposal document. Everything here is "Proposal:" unless it cites Kotlin (`path:line`). Behavior that must be preserved is in `p1-behavior.md`; models and keys are in `p1-data-and-config.md`.

Target stack (from the project brief): Rust stable (edition 2024), gtk4-rs 0.11 with the `v4_16` feature (already in `Cargo.toml`: `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`), no libadwaita, tokio, rusqlite, reqwest, serde.

---

## 1. Crate and module layout

Add a `reader` module tree to the existing crate (`/home/blackbox/kurobayomi/src/`). Current files: `clamp.rs entry.rs main.rs player.rs screens.rs settings.rs spec.rs ui.rs`.

```
src/reader/
  mod.rs            pub fn open(...)  -> gtk::Widget ; ReaderSession; wiring to ui::push
  model.rs          ChapterKey, ReaderChapter, ReaderPage, PageStatus, ChapterState, ViewerChapters, ChapterTransition
  orientation.rs    ReaderOrientation (flag values), ReadingMode (flag values), mask helpers
  settings.rs       ReaderSettings (typed view over the prefs table), defaults, ranges
  chapters.rs       chapter list build: skip-read/filter/dedupe/downloaded-only, sort comparators
  loader/mod.rs     enum Loader (enum dispatch), ChapterLoader::load_chapter, choose_loader
  loader/dir.rs     DirectoryLoader (natural sort, image filter)
  loader/archive.rs ArchiveLoader (zip/cbz; rar behind feature), entry index built once
  loader/epub.rs    EpubLoader (container.xml -> OPF -> spine -> xhtml img list)
  loader/download.rs DownloadLoader (dir or .cbz from the download store)
  loader/http.rs    HttpLoader: priority queue, single worker task, preload 4, retry, recycle->cache
  loader/queue.rs   PriorityQueue with seq tie-break and cancellation tokens
  image.rs          ImageType detection, decode to RGBA off-thread, downscale to viewport
  cache.rs          chapter page-list + image cache (SQLite index + files, 100 MiB LRU)
  progress.rs       ProgressWriter (DB actor): last_page_read, read, bookmark, history
  actions.rs        save image, share/open, copy, set as cover (no Android APIs)
  notify.rs         gio::Notification wrappers (saved image)
  flash.rs          full-window flash overlay state machine
  overlay.rs        navigation overlay (tap zone labels), brightness/color overlay drawing
  chrome.rs         top bar, bottom bar, chapter navigator widgets, page indicator
  sheets.rs         reading-mode / orientation / page-actions sheets; reader settings window
  transition.rs     chapter transition card widget
  strings.rs        English strings keyed by the Aniyomi message keys (p1-behavior section 20)
  error.rs          LoadError (thiserror), user_message() -> key
```

Plus `src/reader/tests/` (unit tests, section 13).

`mod reader;` is added in `src/main.rs` next to `mod player;`.

---

## 2. Threading and async model

Three execution contexts:

| Context | Owns | Runs |
|---|---|---|
| GTK main thread (`glib::MainContext::default()`) | all `gtk::*` objects, `ReaderSession`, `ChapterSet`, `ReaderPage` status cells, `gdk::Texture`s | UI events, state transitions, render requests |
| Tokio runtime (`multi_thread`, 2 workers, started once in `main`) | HTTP page loaders (`reqwest`), source calls via the source layer (area:15-source-api-manga to area:18-extensions-manga), queue worker tasks | network and async source calls |
| Blocking pool / DB actor thread | `rusqlite::Connection` (single writer, WAL), zip/rar/epub reads, image decode | blocking I/O and CPU work |

Rules:
1. Widgets and `gdk::Texture` are created and touched ONLY on the GTK main thread.
2. Background work reports to the main thread through `async_channel::Sender<ReaderEvent>` (unbounded). The main thread drains the receiver with `glib::spawn_future_local(async move { while let Ok(ev) = rx.recv().await { session.handle(ev) } })`. This replaces Kotlin's rendezvous `Channel` (`R/ReaderViewModel.kt:111`); see section 14 (pitfall P-3).
3. Per-page status is a `Cell<PageStatus>` on the main thread PLUS a `tokio::sync::watch::Sender<PageStatus>` that the loader updates. The loader never touches the cell directly; it sends `ReaderEvent::PageStatus { chapter, index, status }` and the main thread writes the cell. This keeps the Kotlin `Page.status` semantics (`Page.kt:26-30`) without shared mutability.
4. DB writes never run on the main thread. `ProgressWriter` owns a dedicated thread with one `rusqlite::Connection` and a `std::sync::mpsc` (or `async_channel`) queue; commands are coalesced per chapter before the write (section 7.3).
5. `runBlocking` on the UI thread (Kotlin `R/ReaderViewModel.kt:681-703`) is not ported. Settings writes are fire-and-forget through the DB actor, followed by a main-thread state update.

Image pipeline (per page): loader downloads bytes (tokio) -> cache file written (blocking pool) -> decoder task (blocking pool) produces `RgbaFrame { width, height, stride, bytes: glib::Bytes }` -> main thread builds `gdk::MemoryTexture` and sets it on a `gtk::Picture`. Textures are never built off the main thread.

---

## 3. Key types (Rust signatures)

Only the important signatures are given. Field names mirror Kotlin for traceability.

### 3.1 Identity and chapter model

```rust
// model.rs

/// Stable identity of a chapter, equal to Kotlin ChapterImpl.equals (class + url + id).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ChapterKey {
    pub id: i64,          // chapters._id (Kotlin Chapter.id, Long)
    pub url: String,      // chapters.url
}

/// Kotlin ReaderChapter. Shared between the chapter set and the page views on the main thread.
pub struct ReaderChapter {
    pub db: ChapterRow,                         // see 3.4
    pub state: RefCell<ChapterState>,           // Kotlin stateFlow
    pub loader: RefCell<Option<Loader>>,        // Kotlin pageLoader
    pub requested_page: Cell<u32>,              // Kotlin requestedPage
    refs: Cell<i32>,                            // Kotlin references (may go negative: see edge E-REF-1)
}

pub enum ChapterState {
    Wait,
    Loading,
    Error(Rc<LoadError>),
    Loaded(Rc<Vec<Rc<ReaderPage>>>),
}

pub struct ReaderPage {
    pub index: u32,                             // 0-based, Kotlin Page.index
    pub url: String,                            // Kotlin Page.url
    pub image_url: RefCell<Option<String>>,     // Kotlin Page.imageUrl
    pub status: Cell<PageStatus>,               // Kotlin Page.status
    pub progress: Cell<i32>,                    // Kotlin Page.progress (-1 unknown, 0..100)
    pub source: PageSource,                     // how bytes are obtained (3.3)
    pub chapter: Weak<ReaderChapter>,           // Kotlin lateinit chapter
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PageStatus { Queued, LoadPage, DownloadImage, Ready, Error }   // Kotlin Page.State order

pub struct ViewerChapters {                       // Kotlin ViewerChapters
    pub curr: Rc<ReaderChapter>,
    pub prev: Option<Rc<ReaderChapter>>,
    pub next: Option<Rc<ReaderChapter>>,
}

pub enum ChapterTransition {                      // Kotlin ChapterTransition
    Prev { from: Rc<ReaderChapter>, to: Option<Rc<ReaderChapter>> },
    Next { from: Rc<ReaderChapter>, to: Option<Rc<ReaderChapter>> },
}
```

`ChapterTransition` equality is NOT derived. Implement `PartialEq` explicitly as the Kotlin rule says (same instance, or same unordered pair with the same variant). Proposal: include the variant in equality (the Kotlin rule ignores it; see E-TR-1 in `p1-edge-cases-and-tests.md`, where the decision is recorded).

`InsertPage` (Kotlin `InsertPage`) is a `ReaderPage` with `source: PageSource::Shared(Rc<ReaderPage>)` and `status = Ready` at construction. Only the viewer creates it.

### 3.2 Reference counting and recycling (`ChapterSet`)

```rust
// model.rs
impl ReaderChapter {
    pub fn add_ref(&self) { self.refs.set(self.refs.get() + 1); }
    /// Returns true when the count reached exactly zero (Kotlin: `references == 0`).
    pub fn release(&self) -> bool {
        self.refs.set(self.refs.get() - 1);
        if self.refs.get() == 0 {
            if let Some(l) = self.loader.borrow_mut().take() { l.recycle(); }
            *self.state.borrow_mut() = ChapterState::Wait;
            true
        } else { false }
    }
}
```

Proposal: use `Drop` on `Loader` for the recycle side effect (see 3.3) and keep `release` only for the state reset. The Kotlin `== 0` check is kept on purpose so behaviour matches (E-REF-1 records the negative-count hazard; the Rust port asserts `refs >= 0` in debug builds).

`ChapterSet::replace(new: ViewerChapters)` does what Kotlin `ViewerChapters.ref()` then `unref()` do (`R/ReaderViewModel.kt:322-324`): add refs for the NEW set first, then release the OLD set. Chapters shared by both sets survive.

### 3.3 Loader: enum dispatch, no trait objects

```rust
// loader/mod.rs
pub enum Loader {
    Directory(DirectoryLoader),
    Archive(ArchiveLoader),
    Epub(EpubLoader),
    Download(DownloadLoader),
    Http(HttpLoader),
}

impl Loader {
    pub fn is_local(&self) -> bool { !matches!(self, Loader::Http(_)) }   // Kotlin isLocal
    /// Kotlin PageLoader.getPages(). Returns pages in final order, indices 0..n.
    pub async fn get_pages(&self) -> Result<Vec<Rc<ReaderPage>>, LoadError>;
    /// Kotlin loadPage(page). Returns a guard; dropping it cancels the still-queued entries it added.
    pub fn load_page(&self, page: &Rc<ReaderPage>) -> LoadGuard;
    /// Kotlin retryPage(page). Only meaningful for Http.
    pub fn retry_page(&self, page: &Rc<ReaderPage>);
    /// Opens the image bytes for a READY page (Kotlin page.stream).
    pub fn open(&self, page: &ReaderPage) -> Result<Box<dyn Read + Send>, LoadError>;
}

impl Drop for Loader {                 // Kotlin recycle(): isRecycled + cancel + cache write
    fn drop(&mut self) { /* Http: stop worker, persist page list (3.5). Archive/Epub: close handles. */ }
}
```

Why enum and not `dyn PageLoader`: `async fn` in traits is not object-safe without boxing, the variant set is closed (5 variants, all in this crate), and the compiler checks exhaustiveness. Kotlin's `abstract class PageLoader` (`RL/PageLoader.kt:10`) maps to this enum plus the `is_recycled` flag held inside each variant (`recycled: Cell<bool>`) so that `check(!isRecycled)` (`RL/ArchivePageLoader.kt:29`, `RL/EpubPageLoader.kt:24`) can be a `debug_assert!` plus a typed error.

```rust
pub enum PageSource {
    Http,                                   // resolved through the source layer
    DirFile(PathBuf),                       // DirectoryLoader
    ArchiveEntry { archive: Arc<ArchiveIndex>, entry: String },
    EpubImage { epub: Arc<EpubIndex>, path: String },
    DownloadDir(PathBuf),
    DownloadArchive { archive: Arc<ArchiveIndex>, entry: String },
    Shared(Rc<ReaderPage>),                  // InsertPage
}
```

`ArchiveIndex` stores the entry list once (name, offset or index) so that `open` is O(1) to locate and does not rescan the archive per page (Kotlin rescans, `core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt:18-22`; this is the O(n^2) point in `p1-behavior.md` section 3.3).

### 3.4 Chapter row (from the DB, plain data)

```rust
pub struct ChapterRow {                     // mirrors domain Chapter + DB flags used here
    pub id: i64,
    pub manga_id: i64,
    pub url: String,
    pub name: String,
    pub scanlator: Option<String>,
    pub read: bool,
    pub bookmark: bool,
    pub last_page_read: u32,               // Kotlin Int, stored INTEGER
    pub chapter_number: f64,               // domain Double (Kotlin Float in DB; store f64 and compare as f32 when mirroring SChapter)
    pub source_order: i64,
    pub date_upload: i64,
}
impl ChapterRow {
    pub fn is_recognized_number(&self) -> bool { self.chapter_number >= 0.0 }   // Kotlin >= 0f
}
```

Float note: Kotlin stores `chapter_number` as `Float` (`SChapter.kt:15`). The duplicate-marking check `chapter_number == readerChapter.chapter_number` (`R/ReaderViewModel.kt:569`) is a Float comparison. In Rust use `(a as f32) == (b as f32)` in that one comparison to match, documented in E-DUP-2.

### 3.5 HTTP loader (queue, worker, cache)

```rust
// loader/http.rs
pub struct HttpLoader {
    chapter: Rc<ReaderChapter>,           // main-thread owner (weak in worker)
    source: Arc<dyn SourceHttp>,          // area:15-source-api-manga trait: get_page_list, get_image_url, get_image
    cache: Arc<ChapterCache>,             // cache.rs
    data_saver: DataSaver,                // from area:12-network-stack (aniyomi/util/DataSaver.kt equivalent)
    queue: Arc<PrioQueue>,                // loader/queue.rs
    preload_size: u32,                    // = 4 (RL/HttpPageLoader.kt:48)
    worker: JoinHandle<()>,               // single consumer (tokio::spawn), aborted on drop
    recycled: Cell<bool>,
}

// loader/queue.rs
pub struct PrioQueue { /* BinaryHeap + Notify + seq counter */ }
pub struct QEntry { pub page: Weak<ReaderPage>, pub priority: u8, pub seq: u64, pub cancelled: Arc<AtomicBool> }
// Order: higher priority first, then lower seq first (FIFO on ties). Kotlin PriorityPage.compareTo (RL/HttpPageLoader.kt:224-227).
impl PrioQueue {
    pub fn push(&self, page: &Rc<ReaderPage>, priority: u8) -> Arc<AtomicBool>;
    pub async fn pop(&self) -> QEntry;               // waits on Notify; skips cancelled entries
}
```

Worker loop (one task, Kotlin RL/HttpPageLoader.kt:55-63):
```
loop {
  let e = queue.pop().await;
  if e.cancelled.load() { continue; }
  let page = e.page.upgrade() else continue;
  if page.status != Queued { continue; }       // Kotlin .filter { status == QUEUE }
  internal_load_page(page).await;               // never returns Err for non-cancel failures; sets Error
}
```

`internal_load_page` mirrors `RL/HttpPageLoader.kt:186-208` exactly: resolve image URL when missing (`LoadPage`), check `cache.has_image(original_url)`, else `DownloadImage` -> `source.get_image(page, data_saver)` -> `cache.put_image(original_url, bytes)`; then `Ready` with source = cache file. Errors set `Error` and continue; cancellation (task abort) propagates.

`LoadGuard` (returned by `load_page`) implements `Drop`: for each entry it pushed whose page is still `Queued`, set its `cancelled` flag (Kotlin `queue.remove`, `RL/HttpPageLoader.kt:116-120`). The Kotlin code suspends forever in `load_page`; in Rust the caller simply holds the guard while the page is visible (section 6).

Page-list fetch (`get_pages`): cache first (`cache.get_page_list(chapter)`), else `source.get_page_list(chapter)`, re-index 0..n (`RL/HttpPageLoader.kt:72-85`). The network result is persisted on `Drop` (equivalent of `recycle`, `:135-157`).

### 3.6 DownloadLoader, DirectoryLoader, ArchiveLoader, EpubLoader

- `DirectoryLoader::get_pages`: `std::fs::read_dir(path)`, keep regular files (not dirs, no recursion) whose name passes `image::is_image_name` OR whose magic bytes pass `image::sniff`, sort by `natord::compare_ignore_case(file_name)` with stable sort, index 0..n. Empty result -> `LoadError::PageListEmpty` (Kotlin returns an empty list; `ChapterLoader` raises, `RL/ChapterLoader.kt:53-55`).
- `ArchiveLoader::open(path)`: open with the `zip` crate for `.cbz`/`.zip`. `.cbr` through `unrar` behind feature `rar` (native libunrar; the Kotlin code uses a native archive reader, area:13-core-util-storage-archive-torrent). Build `ArchiveIndex { entries: Vec<(name, idx)> }` once; keep image entries (name passes `is_image_name` or sniff), sort naturally, index 0..n.
- `EpubLoader::open(path)`: `zip` for the container; `META-INF/container.xml` -> package path; parse OPF with `quick-xml` (spine `itemref idref` -> `manifest item href`); for each XHTML spine page, parse with `quick-xml` (or `scraper` if the Jsoup selector set from `EpubReader.kt:76-80` needs CSS selectors) and collect `img/@src` and `image/@xlink:href` resolved relative to the page directory. Output order = spine order. Exact selector set: area:13-core-util-storage-archive-torrent owns it; this design requires parity with `core/archive/src/main/kotlin/mihon/core/archive/EpubReader.kt`.
- `DownloadLoader`: resolves the download chapter path from the download store (area:23-downloads-manga and area:25-downloads-shared). If the path is a file (`.cbz`), delegate to `ArchiveLoader`; if a directory, delegate to `DirectoryLoader` semantics but read with `PageSource::DownloadDir`. Kotlin uses `context.contentResolver` for downloads (`RL/DownloadPageLoader.kt:66-68`); Rust reads the path directly.

Shared `ArchiveIndex` is `Arc`, so `DownloadLoader` and the page sources reuse the same open archive handle. Concurrency: `zip::ZipArchive` requires `&mut`; wrap in `Mutex` (decode happens on the blocking pool, so the lock is held only during one entry read).

### 3.7 ChapterLoader (the state machine)

```rust
// loader/mod.rs
pub async fn load_chapter(
    ctx: &LoadCtx,                          // manga, source handle, prefs snapshot, download store
    chapter: &Rc<ReaderChapter>,
) -> Result<(), LoadError> {
    if chapter.is_ready() { return Ok(()); }          // Kotlin chapterIsReady (RL/ChapterLoader.kt:74-76)
    *chapter.state.borrow_mut() = ChapterState::Loading;
    let res = async {
        let loader = choose_loader(ctx, &chapter.db)?;     // section 3.8
        let pages = loader.get_pages().await?;
        if pages.is_empty() { return Err(LoadError::PageListEmpty); }
        let start = if !chapter.db.read || ctx.prefs.preserve_reading_position {
            chapter.db.last_page_read
        } else { chapter.requested_page.get() };          // Kotlin keeps previous requestedPage (E-START-1)
        chapter.requested_page.set(start);
        *chapter.loader.borrow_mut() = Some(loader);      // recycles any previous loader FIRST (fixes E-LEAK-1)
        Ok(pages)
    }.await;
    match res {
        Ok(pages) => { *chapter.state.borrow_mut() = ChapterState::Loaded(Rc::new(pages)); Ok(()) }
        Err(e) => { let e = Rc::new(e); *chapter.state.borrow_mut() = ChapterState::Error(e.clone()); Err(e) }
    }
}
```

Note the Kotlin ordering difference fixed on purpose: Kotlin assigns `pageLoader` before `getPages()` (`RL/ChapterLoader.kt:48-49`) so a failure leaves a loader behind. The Rust version assigns the loader only after success, so a failed load leaves `loader = None` and nothing to leak. This changes the observable state (`Error` with `loader == None`), which is why E-LEAK-1 exists in the edge file.

Cancellation: the Kotlin `catch (Throwable)` marks `Error` even on cancellation (E-CANCEL-1). Rust treats cancellation as `Err(LoadError::Cancelled)` mapped to `Wait`, not `Error`. This is a deliberate fix.

### 3.8 Loader choice (Kotlin `getPageLoader`, `RL/ChapterLoader.kt:81-111`)

```rust
pub enum ChapterSourceKind {
    Download { path: Option<PathBuf> },   // from download store lookup (skip_cache = true)
    LocalFormat(LocalFormat),             // Directory | Archive | Epub with path
    Http(Arc<dyn SourceHttp>),
    NotInstalled { source_name: String },
    Unsupported,
}

pub fn choose_loader(kind: ChapterSourceKind, ...) -> Result<Loader, LoadError> {
    match kind {
        ChapterSourceKind::Download { .. } => DownloadLoader::new(...).map(Loader::Download),   // precedence 1
        ChapterSourceKind::LocalFormat(LocalFormat::Directory(p)) => Ok(Loader::Directory(...)),
        ChapterSourceKind::LocalFormat(LocalFormat::Archive(p)) => ArchiveLoader::open(p).map(Loader::Archive),
        ChapterSourceKind::LocalFormat(LocalFormat::Epub(p)) => EpubLoader::open(p).map(Loader::Epub),
        ChapterSourceKind::Http(s) => Ok(Loader::Http(HttpLoader::new(...))),
        ChapterSourceKind::NotInstalled { source_name } => Err(LoadError::SourceNotInstalled(source_name)),
        ChapterSourceKind::Unsupported => Err(LoadError::LoaderNotImplemented),
    }
}
```

Precedence is the Kotlin order: download first, then local format, then HTTP, then stub, then unsupported (`RL/ChapterLoader.kt:90-110`). The `Download` branch is evaluated before the source type, as in Kotlin.

### 3.9 Chapter list builder (pure function, testable)

```rust
// chapters.rs
pub struct ListPrefs { pub skip_read: bool, pub skip_filtered: bool, pub skip_dupe: bool, pub downloaded_only: bool }
pub struct MangaFilters { pub unread_raw: u32, pub downloaded_raw: u32, pub bookmarked_raw: u32, pub sorting: u32 }

pub fn build_reader_list(
    all: &[ChapterRow],
    current_id: i64,
    manga: &MangaFilters,
    prefs: &ListPrefs,
    is_downloaded: impl Fn(&ChapterRow) -> bool,   // download cache lookup (skip_cache=false)
    is_local_manga: bool,
) -> Result<Vec<ChapterRow>, ListError>
```
Steps are exactly `p1-behavior.md` section 4.6 (1 to 7): filter (three-arm predicate), append current if missing, sort ascending by `sorting` mode (section 5.1 table, `sortDescending = false`), dedupe by `chapter_number` (section 5.2) if `skip_dupe`, filter to downloaded if `downloaded_only` and not local, convert. Returns `ListError::CurrentChapterMissing` when the current id is absent after step 6 (this is the Kotlin `NoSuchElementException` case, E-INIT-2, reported with the user message in section 12.3 of behavior).

Sort comparator Rust form:
```rust
fn cmp(a: &ChapterRow, b: &ChapterRow, mode: u32) -> Ordering {
    match mode {
        0x000 => b.source_order.cmp(&a.source_order),             // SOURCE, sortDescending=false => descending sourceOrder
        0x100 => a.chapter_number.total_cmp(&b.chapter_number),   // NUMBER asc
        0x200 => a.date_upload.cmp(&b.date_upload),               // UPLOAD_DATE asc
        0x300 => collator().compare(&a.name, &b.name),            // ALPHABET asc (icu_collator)
        _ => panic!("Invalid chapter sorting method"),           // Kotlin NotImplementedError
    }
}
```
Stable sort (`sort_by`) to match `sortedWith` stability.

### 3.10 Progress writer (DB actor)

```rust
// progress.rs
pub enum DbCmd {
    Progress { chapter_id: i64, read: bool, last_page_read: u32 },    // chapters.read, last_page_read
    Bookmark { chapter_id: i64, bookmark: bool },
    MarkRead { chapter_ids: Vec<i64> },                              // duplicates (read = true)
    History { chapter_id: i64, read_at_ms: i64, session_ms: i64 },   // history upsert
    SetViewerFlags { manga_id: i64, mask: u32, value: u32 },         // mangas.viewer merged with mask
}
pub struct ProgressWriter { tx: std::sync::mpsc::Sender<DbCmd> }
```
Writer thread: owns `rusqlite::Connection` (WAL, `busy_timeout(5s)`), loops `recv`, coalesces consecutive `Progress` for the same chapter (keep last), runs each batch in one transaction. `SetViewerFlags` reads the current value inside the transaction and computes `(old & !mask) | (value & mask)`, identical to Kotlin `setFlag` (`SetMangaViewerFlags.kt:24-29`).

Incognito is applied before enqueue, exactly where Kotlin checks it (`R/ReaderViewModel.kt:537`, `:595`, `:919`).

### 3.11 Image types, decode, and texture

```rust
// image.rs
pub enum ImageType { Avif, Gif, Heif, Jpeg, Jxl, Png, Webp }
impl ImageType { pub fn extension(self) -> &'static str { /* jpg for Jpeg, else lowercase name */ } }
pub fn is_image_name(name: &str) -> bool;      // ext (text after last '.') in {avif,gif,heif,jpg,jxl,png,webp}; case-sensitive per Kotlin (E-IMG-1)
pub fn sniff(bytes: &[u8]) -> Option<ImageType>;  // magic bytes
pub struct RgbaFrame { pub width: u32, pub height: u32, pub stride: usize, pub bytes: glib::Bytes }
pub fn decode_for_viewport(path_or_bytes: ..., max_w: u32, max_h: u32) -> Result<RgbaFrame, DecodeError>;  // blocking pool
```
Decoder choice: `image` crate (`features = ["jpeg","png","webp","gif","avif"]`, avif via `avif-decode` or `image` avif feature), `jxl-oxide` for JXL, `libheif-rs` (optional feature `heif`) for HEIF. Unsupported type -> `DecodeError::Unsupported` -> page `Error`.

Texture build (main thread only):
```rust
let tex = gdk::MemoryTexture::new(
    frame.width as i32, frame.height as i32,
    gdk::MemoryFormat::R8g8b8a8,      // verify enum name in gtk4-rs 0.11 at implementation time
    &frame.bytes, frame.stride,
);
picture.set_paintable(Some(&tex));
```

Downscaling to the viewport (`max_w`, `max_h`) is an area:44-reader-viewers decision; the function exists here so that memory use is bounded (pitfall P-6).

### 3.12 Notifications and actions

```rust
// notify.rs
pub fn notify_saved(app: &gtk::Application, uri: &gio::File, thumb: Option<&gdk::Texture>) {
    let n = gio::Notification::new(&strings::PICTURE_SAVED);      // "Picture saved"
    n.set_body(None);
    n.set_default_action_and_target_value("app.open-image", Some(&uri.uri().to_variant()));
    n.add_button_with_target_value(&strings::ACTION_SHARE, "app.share-image", Some(&uri.uri().to_variant()));
    if let Some(t) = thumb { n.set_icon(&t.clone().upcast::<gio::Icon>()); } // verify: gdk::Texture is a gio::Icon in 0.11
    app.send_notification(Some("reader-save-image"), &n);     // id = Notifications.ID_DOWNLOAD_IMAGE equivalent
}
```
Replaces `SaveImageNotifier` (`R/SaveImageNotifier.kt:25-96`). Mapping: `CHANNEL_COMMON` -> no channel on Linux; `ID_DOWNLOAD_IMAGE` -> a fixed notification id string so a new save replaces the old one (same as Kotlin `onClear`). `BigPictureStyle` -> `gio::Notification` icon (desktop shells show the icon only; documented loss, section 15). The notification thumbnail uses a 720x1280 decode (`SaveImageNotifier.kt:39`).

`app.share-image` and `app.open-image` are `gio::SimpleAction`s registered on the `gtk::Application` in `main.rs`. Share on Linux: see section 9 and Android mapping table (`Share` row).

---

## 4. Settings: typed view over the preference store

```rust
// settings.rs
pub struct ReaderSettings {
    pub reader_theme: i32,                 // 0 white, 1 black (default), 2 gray, 3 automatic
    pub show_page_number: bool,            // true
    pub show_reading_mode: bool,           // true
    pub fullscreen: bool,                  // true
    pub keep_screen_on: bool,              // true
    pub always_show_chapter_transition: bool, // true
    pub page_transitions: bool,            // true
    pub flash_on_page_change: bool,        // false
    pub flash_duration_ms: u32,            // 100
    pub flash_page_interval: u32,          // 1
    pub flash_color: FlashColor,           // Black
    pub read_with_long_tap: bool,          // true
    pub preserve_reading_position: bool,   // false
    pub skip_read: bool,                   // false
    pub skip_filtered: bool,               // true
    pub skip_dupe: bool,                   // false
    pub custom_brightness: bool,           // false
    pub custom_brightness_value: i32,      // 0, range -75..=100
    pub color_filter: bool,                // false
    pub color_filter_value: u32,           // 0 (ARGB packed)
    pub color_filter_mode: u32,            // 0..=5
    pub grayscale: bool,                   // false
    pub inverted_colors: bool,             // false
    pub default_reading_mode: u32,         // 2 (RIGHT_TO_LEFT)
    pub default_orientation: u32,          // 8 (FREE), stored only (E-ORI-1)
    pub show_navigation_overlay_on_start: bool, // false
    pub dual_page_split_paged: bool,       // false
    pub dual_page_invert_paged: bool,      // false
    pub dual_page_split_webtoon: bool,     // false
    pub dual_page_invert_webtoon: bool,    // false
    pub dual_page_rotate_to_fit: bool,     // false
    pub dual_page_rotate_to_fit_invert: bool,        // false
    pub dual_page_rotate_to_fit_webtoon: bool,       // false
    pub dual_page_rotate_to_fit_invert_webtoon: bool, // false
    pub crop_borders: bool,                // false
    pub crop_borders_webtoon: bool,        // false
    pub navigate_to_pan: bool,             // true
    pub landscape_zoom: bool,              // true
    pub webtoon_side_padding: u32,         // 0 (range 0..=25)
    pub webtoon_double_tap_zoom: bool,     // true
    pub webtoon_disable_zoom_out: bool,    // false
    pub image_scale_type: u32,             // 1 (range 1..=6)
    pub zoom_start: u32,                   // 1 (range 1..=4)
    pub navigation_mode_pager: u32,        // 0 (range 0..=5)
    pub navigation_mode_webtoon: u32,      // 0 (range 0..=5)
    pub pager_nav_inverted: TappingInvertMode,   // None
    pub webtoon_nav_inverted: TappingInvertMode, // None
    pub read_with_volume_keys: bool,       // false (stored, not handled on Linux, section 15)
    pub show_navigation_overlay_new_user: bool,  // true
    pub reader_hide_threshold: ReaderHideThreshold, // Low (viewer, area:44-reader-viewers)
    pub folder_per_manga: bool,            // false
    pub download_ahead: u32,               // from download prefs, not reader prefs (auto_download_while_reading, default 0)
    pub remove_after_read_slots: i32,      // from download prefs (default -1)
}
// Every field above maps one-to-one to a key in p1-data-and-config.md section 3.1 or 3.2-3.5.
impl Default for ReaderSettings { /* exact defaults from p1-data-and-config section 3.1 */ }
```

Storage: the shared preference table (area:02-preferences-system) keyed by the Aniyomi key strings so backups map 1:1 (`p1-data-and-config.md` section 3.1). `ReaderSettings::load(&Db)` and `save_key(&Db, key, value)`; reads are cached in memory and invalidated by `ProgressWriter` acknowledgements. Missing key -> default. Invalid enum value -> default (Kotlin `getEnum` behaviour is area:02-preferences-system's; reader side uses `FlashColor::from_name(s).unwrap_or(Black)`).

Setting writes go through the DB actor (`DbCmd::Pref { key, value }`, add to the enum in section 3.10 when area:02-preferences-system lands).

---

## 5. Session and state (ViewModel replacement)

```rust
// mod.rs
pub struct ReaderSession {
    pub manga: Rc<MangaView>,                      // title, source id, filters, viewer flags (snapshot)
    pub list: Vec<Rc<ReaderChapter>>,              // chapterList
    pub viewer_chapters: Option<ViewerChapters>,   // current + neighbours
    pub current_page: i32,                         // -1 until first page (E-UI-1)
    pub menu_visible: bool,
    pub bookmarked: bool,
    pub loading_adjacent: bool,
    pub dialog: Option<Dialog>,                    // Loading | Settings | ReadingModeSelect | OrientationSelect | PageActions(index)
    pub brightness_overlay: i32,                   // -100..=100 (Kotlin IntRange)
    pub incognito: bool,                           // lazy in Kotlin (R/ReaderViewModel.kt:225)
    pub download_ahead: u32,                       // captured at open (R/ReaderViewModel.kt:226)
    pub read_started_at: Option<Instant>,          // chapterReadStartTime
    pub chapter_to_download: Option<DownloadId>,   // chapterToDownload
    pub saved: SavedRestore,                       // chapter_id, page_index (section 5.1)
}
```

State is mutated only on the main thread. Each mutation is a method that computes the next value before assigning (proposal P-4, fixes the Kotlin `update` side-effect hazard, E-STATE-1).

### 5.1 Restore

Kotlin SavedStateHandle keys `chapter_id` and `page_index` (`R/ReaderViewModel.kt:123`, `:132`) survive process death. Rust: write `(chapter_id, page_index)` to the `reader_session` table on each page change (through the DB actor, coalesced) and read it on open. On a fresh open from the library the explicit chapter id wins over the saved one, same as `if (chapterId == -1L) chapterId = initialChapterId` (`R/ReaderViewModel.kt:282`); the saved values are used only when the reader is re-opened for the same manga after a crash, governed by a `restore_after_crash` boolean (default true, documented decision E-RESTORE-1).

---

## 6. Widgets

Reader window = a page in the existing mockup stack (`ui::push`, `src/ui.rs:34-37`), keeping the name `screen:reader` (`src/main.rs:58` checks it to hide the sidebar and tab bar).

Layout (GTK 4, no libadwaita):
```
gtk::Overlay (reader_root)
 ├─ child: viewer container (gtk::Stack of pager | webtoon view; area:44-reader-viewers)
 ├─ overlay: ReaderNavigationOverlay (DrawingArea, tap-zone labels, fade 1000 ms)
 ├─ overlay: FlashOverlay (DrawingArea, full-window solid colour, section 10)
 ├─ overlay: ContentOverlay (brightness black + colour filter layers, section 11)
 ├─ overlay (top, valign start): TopBar (back, title+subtitle, bookmark toggle, overflow menu)
 ├─ overlay (bottom, valign end): BottomStack
 │     ├─ ChapterNavigator (prev button, page scale 1..N, next button)
 │     └─ BottomBar (reading mode, rotation, crop, settings)
 └─ overlay (bottom centre): PageIndicator (shown when chrome hidden and page number pref on)
```

Chrome visibility: `menu_visible` drives `TopBar` and `BottomStack` with `gtk::Revealer` (transition `SlideDown`/`SlideUp`, duration 200 ms = Kotlin `tween(200)`, `P/appbars/ReaderAppBars.kt:35`).

Fullscreen: `window.fullscreen()` when `fullscreen` pref is on and the menu is hidden; restore when shown (replaces `setMenuVisibility` immersive logic, `R/ReaderActivity.kt:516-528`). Keyboard: `Escape` leaves fullscreen first (GTK convention) then leaves the reader.

Sheets (replacing Compose `AdaptiveSheet` and `AlertDialog`): `gtk::Window` transient, modal, sized by content, or a bottom `gtk::Revealer` inside the overlay for the mode/orientation selectors (proposal: revealer, closer to the phone sheet behaviour the mockup already uses through `ui::sheet`). Confirm dialogs use `gtk::AlertDialog` (GTK 4.10+).

---

## 7. Behaviour mapping, per flow

### 7.1 Open chapter
1. `ReaderSession::open(manga_id, chapter_id)` on main thread creates a loading state (`LoadingDialog` is not needed; the page area shows a spinner).
2. Spawn on tokio: `load_init` = fetch manga + chapters (DB via `spawn_blocking`), `build_reader_list` (pure), `load_chapter(current)`.
3. Result events return to main; on success `replace(ViewerChapters)`, viewer built, `requested_page` applied.
4. On failure: show an error page with the message key (E-INIT-1), offer Retry (mockup `reader` spec lists "Failed to load pages: %s" and "Retry"). Kotlin closes the reader and toasts (`R/ReaderActivity.kt:629-633`). Proposal: stay in the reader with a retry card instead of closing, because the mockup spec already defines it; the toast message text is kept.

### 7.2 Page changed (from viewer)
`on_page_selected(page)`: exactly `p1-behavior.md` section 7.1 (steps 1-6). The progress write is `ProgressWriter::send(Progress{...})`, not on the main thread.

### 7.3 Chapter switch and preload
`load_new_chapter` and `load_adjacent` as in behaviour sections 6.2 and 6.3. The progress indicator is the mockup's spinner in the navigator, replacing the Compose loading dialog (`R/ReaderActivity.kt:441-455`). Adjacent failure: keep the current chapter and show a toast; do NOT jump to page 0 (fixes E-NAV-1).

### 7.4 Completion
As behaviour section 7.3; the database actor receives `Progress{read: true}`, `MarkRead{...}` (duplicates, only if `markDuplicateReadChapterAsRead` contains `existing`), and the track call (area:27-tracking-core) and delete-after-read enqueue (area:09-domain-source-track-download-release-misc).

---

## 8. Error handling

```rust
// error.rs
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("No pages found")] PageListEmpty,                             // page_list_empty_error
    #[error("Source not installed: {0}")] SourceNotInstalled(String),     // source_not_installed
    #[error("Source not found")] LoaderNotImplemented,                    // loader_not_implemented_error
    #[error("Cancelled")] Cancelled,                                      // not a user message
    #[error("Unknown err")] UnknownManga,                                 // literal in Kotlin (not localised)
    #[error(transparent)] Io(#[from] std::io::Error),
    #[error("archive: {0}")] Archive(String),
    #[error("source: {0}")] Source(String),                               // source layer message
    #[error("No element matches the requested chapter")] CurrentChapterMissing,  // Kotlin NoSuchElementException text
}
impl LoadError {
    pub fn user_message(&self) -> String { /* maps to the key text from strings.rs */ }
}
```
Rules:
- `anyhow` only at the top of spawned tasks, converted to `LoadError::Source` at the boundary.
- Non-cancellation failures in page downloads never escape the worker (Kotlin semantics, `RL/HttpPageLoader.kt:202-207`): they set `Error` and log at warn with `tracing`/`log` (no `logcat` on Linux; use `log` crate with `env_logger`, or `tracing`).
- User-visible strings come only from `strings.rs` (the English texts in `p1-behavior.md` section 20).

---

## 9. Android to Linux mapping (every API touched by the 32 files)

| Android / Kotlin API (file) | Linux / Rust replacement | Decision | User loss / notes |
|---|---|---|---|
| `ReaderActivity` (Activity, intent extras `manga`, `chapter`) | page in main window (`ui::push`), state in `ReaderSession` | Rewrite | none; launch is a function call |
| `ReaderViewModel` + `SavedStateHandle` | `ReaderSession` + `reader_session` table | Rewrite | crash restore kept |
| `Channel<Event>` rendezvous | `async_channel::unbounded` | Replace | events are no longer dropped (E-EVT-1) |
| `runBlocking` on UI | none (async DB actor) | Drop | no UI blocking (E-UI-2) |
| `viewModelScope`, `launchIO`, `launchNonCancellable` | tokio tasks + `glib::spawn_future_local` | Replace | cancellation is explicit (guards/handles) |
| `Dispatchers.IO` / `withUIContext` | tokio + blocking pool / main context | Replace | |
| `Injekt.get()` | constructor parameters | Replace | |
| `ChapterCache` (`DiskLruCache`) | `cache.rs`: files + SQLite index (`reader_cache`) with LRU to 100 MiB | Rewrite | existing Android cache is not migrated (out of scope) |
| `UniFile` | `std::path::PathBuf`, `gio::File` for URIs | Replace | SAF not applicable |
| `ArchiveReader` (`mmap` + native) | `zip` crate; `unrar` behind feature `rar` | Replace | RAR needs libunrar at build (feature off = `.cbr` unsupported, user loss unless enabled) |
| `EpubReader` (Jsoup) | `zip` + `quick-xml` (`scraper` only if needed) | Replace | selector parity to verify (area:13-core-util-storage-archive-torrent) |
| `ImageUtil.isImage`, magic bytes (`tachiyomi.decoder`) | `image::is_image_name`, `image::sniff` | Port | JXL/HEIF need extra crates (features) |
| `BitmapFactory`/Coil decode | `image` crate + `jxl-oxide` (+ `libheif-rs` feature) | Replace | decode on blocking pool |
| `SubsamplingScaleImageView` | area:44-reader-viewers viewer (texture tiling) | Replace | area:44-reader-viewers decides |
| `CompositionLocal` / Compose layouts | GTK 4 widgets (`gtk::Overlay`, `gtk::Revealer`, `gtk::Scale`) | Rewrite | |
| `AdaptiveSheet`, `AlertDialog` | `gtk::Window` (modal) / revealer; `gtk::AlertDialog` | Replace | |
| `Toast` (`toast(...)`) | in-window toast label, 2000 ms (Android `LENGTH_SHORT` equivalent, proposal) | Replace | the toast is in-window only |
| `Notification` + `NotificationCompat.BigPictureStyle` | `gio::Notification` with default action and button; icon = thumbnail | Replace | big picture expanded view not available in all shells (loss) |
| `NotificationReceiver` broadcasts (share, dismiss) | `gio::SimpleAction` `app.share-image`, `app.open-image` | Replace | |
| `ClipboardManager` / `ClipData.newUri` | `gdk::Display::clipboard()`: `set_texture` (image) or `set_text` (url) | Replace | |
| `Intent.createChooser` (share) | `gtk::UriLauncher` / `gio::AppInfo::launch_default_for_uri` on the file; "Show in file manager" via `org.freedesktop.FileManager1.ShowItems` over zbus | Replace (P2) | the system share target list is lost; no equivalent on Linux desktops |
| `openInBrowser` | `gtk::UriLauncher` (GTK 4.10+) | Replace | |
| `WebViewActivity` | `webkit6` crate (WebKitGTK 6.0), area:36-ui-shell-navigation-security-webview | Replace (P1) with browser fallback | |
| `FLAG_KEEP_SCREEN_ON` | `gtk::Application::inhibit(Some(&win), ApplicationInhibitFlags::IDLE, reason)` / `uninhibit(cookie)` | Replace | |
| `WindowManager.LayoutParams.screenBrightness` | software overlay for negative values (same math); positive values through `/sys/class/backlight/*/brightness` when writable, otherwise no-op | Replace, partial | loss: hardware brightness above 0 on machines without a writable backlight |
| `LAYOUT_IN_DISPLAY_CUTOUT_MODE_*` (cutout) | none | Drop | no notches on the target desktops; `cutout_short` pref kept and ignored |
| `requestedOrientation` / `ActivityInfo.SCREEN_ORIENTATION_*` | none (ReaderOrientation values stored, not applied) | Drop (P3) | loss: per-manga orientation lock/auto-rotate has no effect on desktop |
| `WindowInsetsController`, `systemBarsPadding`, `applyInsetter` | GTK fullscreen state only | Drop | no system bars on Linux desktop |
| `WindowCompat.setDecorFitsSystemWindows` | none | Drop | |
| `Window.statusBarColor` / `navigationBarColor` | none (GTK header bar theme) | Drop | Tokyo Night theme applies through GTK CSS (user preference) |
| `isNightMode()` | `gtk::Settings::default()` `gtk-application-prefer-dark-theme`, and portal `org.freedesktop.appearance color-scheme` (1 = dark) via `ashpd` | Replace | |
| `SurfaceColors.SURFACE_2` / `surfaceColorAtElevation` | GTK CSS named color (`@headerbar_bg_color`, alpha 0.9/0.95) | Replace | |
| `DisplayRefreshHost` (flash) | `flash.rs`: full-window colour for `flash_duration_ms`, half/half for WHITE_BLACK | Port | e-ink specific; kept for parity, P2 |
| `ColorMatrixColorFilter` / `Paint` layer | GSK `gsk::ColorMatrixNode` or `gtk::Snapshot::push_color_matrix` (GTK 4) for grayscale+invert; overlay `gtk::Snapshot::append_color` with blend | Port | blend modes map to `gsk::BlendMode` (Multiply, Screen, Overlay, Lighten, Darken) |
| `HapticFeedback` (`TextHandleMove`) | none | Drop (P3) | no haptics on Linux desktop |
| `LocalHapticFeedback`, `LayoutDirection` | GTK `gtk::Widget::set_direction` (`TextDirection::Rtl`) | Port | |
| `Activity` transitions (`shared_axis_x_*`) | `gtk::Stack` `transition_type = SlideLeftRight` | Replace (P3) | |
| `MaterialContainerTransform` | none | Drop | |
| Volume keys (`reader_volume_keys`) | none (P3 drop) | Drop | keys stored, not handled |
| `KEYCODE_N` / `KEYCODE_P` | `gtk::EventControllerKey`: `n` next, `p` previous | Port | same keys |
| `ViewerNavigation` regions (area:44-reader-viewers) | rectangles drawn in the overlay | Port | |
| `ContentOverlay` (Compose `Canvas`) | `gtk::DrawingArea` or `gtk::Snapshot` | Port | |
| `FileProvider`/`content://` for images | `gio::File` / `file://` | Replace | |
| `ICC display profile` (`SubsamplingScaleImageView.setDisplayProfile`) | `lcms2` crate transform (P2) | Replace (P2) | until implemented: no colour management (loss) |
| Logcat | `log` + `env_logger` | Replace | |
| `Toast.LENGTH_*` | constant 2000 ms | Replace | |

---

## 10. Flash overlay (replaces `DisplayRefreshHost`)

```rust
// flash.rs
pub struct FlashState { calls: u32, interval: u32, active: Option<Instant>, color: FlashColor, duration: Duration }
impl FlashState {
    /// Kotlin flash(): trigger when calls % interval == 0; calls += 1 (RL-free; P/DisplayRefreshHost.kt:36-41).
    pub fn on_page_changed(&mut self) -> bool;
    pub fn set_interval(&mut self, n: u32) { self.interval = n; self.calls = 0; }   // P/DisplayRefreshHost.kt:43-46
    /// Colour for the current instant: BLACK -> Black; WHITE -> White; WHITE_BLACK -> White first half, Black second half.
    pub fn colour_at(&self, now: Instant) -> Option<Rgba>;
}
```
Drawing: `gtk::DrawingArea` full size, `set_draw_func` paints the colour when `Some`; a `glib::timeout_add_local` (or `FrameClock` tick) requests redraws until `duration` elapses, then clears `active`. Only composed when `flash_on_page_change` is on; `on_page_changed` is not called otherwise (fixes E-FLASH-1: the Kotlin flag stays set when the feature is off).

---

## 11. Brightness and colour overlays

- Negative brightness `v` (`-75..=-1`): black `DrawingArea` with alpha `|v| / 100` over the viewer (`P/ReaderContentOverlay.kt:22-36`). Window brightness is not changed (Kotlin sets `0.01f`, which on Linux is replaced by the overlay alone; documented in the mapping table).
- Positive `v` (`1..=100`): backlight through sysfs if writable (`/sys/class/backlight/<dev>/brightness` scaled to `max_brightness`). Otherwise show the toast "Custom brightness not supported on this display" (new message, not in Kotlin strings; flagged E-BRIGHT-2).
- `0`: restore system brightness and hide the overlay.
- Colour filter: GSK colour matrix or blend overlay with the packed ARGB value and the mode from section 18 of `p1-behavior.md`.
- Grayscale: `gsk` saturation 0. Invert: colour matrix `[-1 0 0 0 1; 0 -1 0 0 1; 0 0 -1 0 1; 0 0 0 1 0]` in 0..1 units (Kotlin uses the 0..255 offsets `255`; the same transform after normalising).
- Order (Kotlin): grayscale first, then invert on the result (`R/ReaderActivity.kt:810-832`).

---

## 12. Strings

`strings.rs` holds `pub const` English strings keyed by the Aniyomi key names (Kotlin `MR.strings.*`), so future translation work can map 1:1. Example: `pub const PICTURE_SAVED: &str = "Picture saved";`. Format strings use Rust formatting (`format!("{} / {}", cur, total)`), with the Kotlin placeholder forms documented in `p1-behavior.md` section 20 (`%1$s`, `%2$s`, `%3$d`, `%d`).

Plural: `missing_chapters_warning` one/other (`p1-behavior.md` section 20) implemented as `fn missing_chapters(n: u32) -> String`.

---

## 13. Tests

- Pure-logic unit tests (no GTK): `chapters::build_reader_list` (all filter arms, current re-append, dedupe including unknown-number collapse, downloaded-only missing-current case), sort comparators (SOURCE descending, NUMBER with -1, UPLOAD_DATE, ALPHABET collation), `ChapterRow::is_recognized_number`, `calculate_chapter_gap` (copy of the Kotlin formula with its test vectors), `ReadingMode`/`ReaderOrientation` mask decode and encode, `set_flag` merge, `DisplayFlash` trigger counting, `PrioQueue` ordering and cancellation, `ReaderChapter::release` zero-crossing, `ChapterTransition` equality.
- Loader tests: `DirectoryLoader` and `ArchiveLoader` against fixtures (natural order `2.jpg < 10.jpg`, case-insensitive names, non-image files ignored, `.jpeg` detected by sniffing, empty archive -> `PageListEmpty`), `EpubLoader` spine order.
- Integration (mock source): HTTP loader preload ordering (current first, then 4 forward pages at priority 0), retry on Error, eviction re-queue, cancellation removes queued entries.
- Progress: coalescing keeps the last value per chapter. A page in `Error` must NOT write `last_page_read`, matching Kotlin (`R/ReaderViewModel.kt:537`). A test asserts that no `Progress` command is emitted for an `Error` page.
- Full list in `p1-edge-cases-and-tests.md`.

---

## 14. Pitfalls (Rust port specific)

- P-1 GTK objects are not `Send`. Keep `Rc`/`RefCell` on the main thread only; background tasks carry plain data (`RgbaFrame`, `PathBuf`, `ChapterRow`).
- P-2 `glib::spawn_future_local` needs the main context to be running; do not call it from tokio threads.
- P-3 Do not use rendezvous channels for UI events (Kotlin's `Channel()` drops `trySend` events, `R/ReaderViewModel.kt:111`, `:421`, `:459`).
- P-4 Do not run side effects inside state-update closures (Kotlin `mutableState.update` lambdas, `R/ReaderViewModel.kt:321-331`).
- P-5 Archive readers are not thread-safe; guard with `Mutex` and keep the lock short.
- P-6 Decode large pages on the blocking pool and downscale before texture creation; a 4000x6000 RGBA texture is 96 MB.
- P-7 Cancel the page-visibility guard when the widget is unmapped or the chapter is unloaded, otherwise queued pages accumulate.
- P-8 `gdk::Texture` creation needs a display connection; create textures only after the window is realized.
- P-9 SQLite: one writer connection, WAL, `busy_timeout`; never call `rusqlite` from the main thread.
- P-10 `natord::compare_ignore_case` and ICU collation are not identical to Java's `CaseInsensitiveSimpleNaturalComparator` and `Collator` on every input; the difference is restricted to exotic Unicode and is recorded in E-SORT-1.

---

## 15. Dropped and degraded items (summary for the user)

| Item | Decision | What the user loses |
|---|---|---|
| Screen orientation lock/auto rotate (`ReaderOrientation`) | Drop (P3). Values stored and synced, not applied. | Per-manga rotation has no effect. |
| Display cutout mode | Drop | Nothing on desktop; the setting is hidden. |
| Haptics on slider | Drop | Nothing. |
| Volume keys to turn pages | Drop (P3) | Use arrow keys, Page Up/Down, mouse wheel. |
| System share sheet | Replace (P2): open file, show in file manager | Share to arbitrary apps. |
| Hardware brightness above 0 | Partial (sysfs if writable) | On laptops without writable backlight, values above 0 do nothing. |
| Big picture notification expansion | Replace | Thumbnail shown as the icon only on most shells. |
| ICC display profile | P2 | No colour management until implemented. |
| Activity transitions | Replace (P3) | Stack slide used instead of shared-axis. |

---

## 16. Pre-implementation checklist for the implementer

- Confirm the gtk4-rs 0.11 signatures for `gdk::MemoryTexture::new`, `gdk::MemoryFormat`, `gtk::Application::inhibit`, `gtk::UriLauncher`, `gtk::AlertDialog`, `gsk` colour-matrix APIs, and `gio::Notification::set_icon` taking a texture. The signatures written in this document are proposals to verify, not verified API calls.
- Confirm `natord` and `icu_collator` licenses are compatible with the project (both permissive).
- Decide `rar` feature default (libunrar licensing).
- Confirm the mockup's `src/settings.rs` persistence model with area:02-preferences-system before implementing `ReaderSettings::load`.
