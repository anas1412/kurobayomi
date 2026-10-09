# Area 14 — Local source and metadata: Rust design

Targets: crate `kuro-core` (module `metadata`) and crate `kuro-source-local` (area:01-foundation-app-bootstrap names). Behavior contract: `p1-behavior.md` (section numbers below refer to it when marked "behavior §"). Data shapes: `p1-data-and-config.md`.

Cross-platform rules (addendum `addenda/requirements-cross-platform.md`) apply: no `cfg(target_os)` here; file paths only through `Path`/`PathBuf`; generated names pass the NTFS-safe sanitizer; external tools are found through `kuro-platform`.

---

## 1. Crate and module layout

```
kuro-core/
  src/metadata/mod.rs          pub mod comicinfo; pub mod details;
  src/metadata/comicinfo.rs    ComicInfo model, ComicInfoPublishingStatus, to_xml(), from_xml()
  src/metadata/details.rs      MangaDetails, AnimeDetails, ChapterDetails, EpisodeDetails (serde)

kuro-source-local/
  Cargo.toml
  src/lib.rs                   pub use of the two sources and the public types
  src/ids.rs                   LOCAL_SOURCE_ID = 0; is_local_source(id)
  src/roots.rs                 manga_root(base) / anime_root(base): join "local" / "localanime", create on first access (behavior §2)
  src/fs.rs                    list_dir, find_child (nested), entry metadata, safe_join, create_nomedia
  src/names.rs                 extension(), stem(), is_hidden()        (behavior §4.1)
  src/format.rs                Format, MANGA_ARCHIVE_EXTENSIONS, ANIME_EPISODE_EXTENSIONS, format_of()  (§4.2)
  src/images.rs                ImageType, detect_image_type(), is_image(), find_image(), write_image()  (§4.4, §9.2)
  src/natsort.rs               natural_cmp_ci(), natural_cmp()          (§4.5)
  src/sort.rs                  SortSelection, BrowseMode, sort_entries()  (§6)
  src/recognition.rs           thin wrappers over kuro_domain::recognition (§5)
  src/manga/mod.rs             LocalMangaSource (public API)
  src/manga/browse.rs          search/popular/latest      (§6)
  src/manga/chapters.rs        chapter list + cover fallback (§8, §9.3)
  src/manga/details.rs         getMangaDetails branches a/b/c (§7)
  src/manga/reader.rs          format_of_chapter()        (§10.1)
  src/anime/mod.rs             LocalAnimeSource (public API)
  src/anime/browse.rs          browse entries             (§11.2)
  src/anime/fetch_type.rs      fetch_type_of()            (§11.1)
  src/anime/seasons.rs         season list + season update (§11.3)
  src/anime/episodes.rs        episode list, thumbnails, cover/background fallback (§11.5)
  src/anime/playback.rs        hoster for local episode   (§11.7)
  src/media.rs                 ffprobe / ffmpeg via tokio::process (§14)
  src/epub.rs                  fill_metadata() (§21) using kuro_core::archive::epub (area:13-core-util-storage-archive-torrent)
  src/bridge.rs                manga_to_comic_info(), apply_comic_info(), status helpers (§5.6)
  src/error.rs                 LocalSourceError
  src/json_side.rs             chapters.json / episodes.json / details.json readers (§20)
```

Ponytail rule applied: the three image managers (cover, background, episode thumbnail) collapse into one `images.rs` implementation with two parameters (stem, default file name). The two source file-systems collapse into `fs.rs` (one set of functions over a root). The two browse implementations share `browse.rs` logic through `sort.rs` and `fs.rs`; the anime and manga files differ only in the per-entry mapping. Upgrade path noted in §12.

### 1.1 Cargo.toml dependencies (exact crates)

| Crate | Version line | Used by | Why |
|---|---|---|---|
| `kuro-core` | path | everything | archive reader, metadata, logging |
| `kuro-domain` | path (area:07-domain-entries-items) | `recognition.rs` | `parse_chapter_number`, `parse_episode_number` |
| `kuro-platform` | path (area:01-foundation-app-bootstrap) | `roots.rs`, `media.rs` | `Paths`, external-tool lookup |
| `kuro-i18n` | path (the i18n area (51, no plan folder in this set), named by area:01-foundation-app-bootstrap and area:03-db-manga-schema-migrations; no plan folder in this set) | `error.rs` | user-visible strings; keys in behavior §23.1 |
| `kuro-source` | path (area:15-source-api-manga and area:16-source-api-anime) | `manga/`, `anime/` | `SManga`, `SChapter`, `SAnime`, `SEpisode`, `FilterList`, `ItemsPage` |
| `serde` | 1 (`derive`) | `details.rs` | JSON models |
| `serde_json` | 1 | `json_side.rs`, `details.rs` | `Json` config parity (§3.6 data) |
| `quick-xml` | 0.37+ (`serialize` not required) | `comicinfo.rs` | event-based XML writer and reader |
| `time` | 0.3 (`formatting`, `parsing`, `local-offset`) | `json_side.rs`, `epub.rs` | per area:01-foundation-app-bootstrap convention |
| `url` | 2 | `roots.rs`, `chapters.rs`, `episodes.rs` | `file://` URLs for thumbnails |
| `scraper` | 0.20+ | `epub.rs` | HTML tolerant parse of XHTML pages (replaces Jsoup) |
| `tempfile` | 3 | `media.rs`, `images.rs` | temp files for frame extraction |
| `tokio` | 1 (`process`, `rt`, `fs`, `time`) | `media.rs`, all `spawn_blocking` | per area:01-foundation-app-bootstrap |
| `thiserror` | 2 | `error.rs` | error enum |
| `tracing` | 0.1 | everywhere | replaces `logcat` |

Not added (ponytail): `infer` (image magic is a 20-line table, §4.4), `natord` (Kotlin's exact comparator is ~40 lines, §4.5), `chrono` (area:01-foundation-app-bootstrap uses `time`), `jsoup`-equivalents beyond `scraper`, `async-trait` (concrete types, no trait objects in this crate).

---

## 2. Public API (signatures)

### 2.1 Shared primitives
```rust
// kuro-source-local/src/ids.rs
pub const LOCAL_SOURCE_ID: i64 = 0;                       // behavior §1
pub fn is_local_source(source_id: i64) -> bool { source_id == LOCAL_SOURCE_ID }

// kuro-source-local/src/sort.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortSelection { pub index: u8, pub ascending: bool }   // 0 = Title, 1 = Date
pub const POPULAR_SORT: SortSelection = SortSelection { index: 0, ascending: true };
pub const LATEST_SORT: SortSelection = SortSelection { index: 1, ascending: false };

#[derive(Debug, Clone)]
pub enum BrowseMode {
    /// Kotlin getPopularManga: query "", POPULAR_SORT (or the selection from the UI filter list)
    Popular(SortSelection),
    /// Kotlin getLatestUpdates: LATEST_SORT, 7-day window, query ignored
    Latest,
    /// Kotlin getSearchManga with a user filter list
    Search { query: String, sort: Option<SortSelection> },
}
```
`BrowseMode::Latest` is the Rust form of the Kotlin identity check `filters === LatestFilters` (behavior §6.1 step 2). Kotlin's `Popular` mode also honours `query` when a user searches through the same entry point; the Rust API keeps the three modes explicit.

### 2.2 Manga
```rust
// kuro-source-local/src/manga/mod.rs
#[derive(Clone)]
pub struct LocalMangaSource { root: Option<PathBuf>, name: String }   // root = <storage_dir>/local, None when the storage folder is missing; name = kuro_i18n::t("local_manga_source")

impl LocalMangaSource {
    pub fn new(root: Option<PathBuf>) -> Self;
    pub const ID: i64 = LOCAL_SOURCE_ID;
    pub fn lang(&self) -> &'static str { "other" }
    pub fn supports_latest(&self) -> bool { true }
    pub fn is_http(&self) -> bool { false }
    pub fn is_unmetered(&self) -> bool { true }
    pub fn filter_list(&self) -> Vec<SortFilter>;                       // [Sort "Order by" [Title, Date] default Title asc]

    pub async fn browse(&self, page: u32, mode: BrowseMode) -> Result<ItemsPage<SManga>, LocalSourceError>;  // page ignored, has_next_page = false
    pub async fn details(&self, manga: &mut SManga) -> Result<(), LocalSourceError>;  // Ok(()) even on internal error (logged, §7.6)
    pub async fn chapters(&self, manga: &mut SManga) -> Result<Vec<SChapter>, LocalSourceError>;
    pub async fn format_of_chapter(&self, chapter_url: &str) -> Result<Format, LocalSourceError>;

    // Unused in Kotlin: absent in Rust (behavior §23.2)
}
```
`ItemsPage<T> { items: Vec<T>, has_next_page: bool }` comes from the source API area (area:06-data-repos-history-track-source-misc topic). `details` takes `&mut` because Kotlin mutates the object in place; the caller persists the result.

### 2.3 Anime
```rust
// kuro-source-local/src/anime/mod.rs
#[derive(Clone)]
pub struct LocalAnimeSource { root: Option<PathBuf>, name: String }   // root = <storage_dir>/localanime

impl LocalAnimeSource {
    pub fn new(root: Option<PathBuf>) -> Self;
    pub const ID: i64 = LOCAL_SOURCE_ID;
    pub fn lang(&self) -> &'static str { "other" }
    pub fn supports_latest(&self) -> bool { true }
    pub fn supports_related_anime(&self) -> bool { false }
    pub fn is_http(&self) -> bool { false }
    pub async fn browse(&self, page: u32, mode: BrowseMode) -> Result<ItemsPage<SAnime>, LocalSourceError>;
    pub async fn season_update(&self, anime: &mut SAnime, seasons: &mut Vec<SAnime>, fetch_details: bool, fetch_seasons: bool) -> Result<(), LocalSourceError>;
    pub async fn episode_update(&self, anime: &mut SAnime, episodes: &mut Vec<SEpisode>, fetch_details: bool, fetch_episodes: bool) -> Result<(), LocalSourceError>;
    pub async fn related(&self, _anime: &SAnime) -> Vec<AnimeRelation> { Vec::new() }   // Kotlin emptyList()
    pub async fn hosters_for(&self, episode_url: &str) -> Result<Vec<Video>, LocalSourceError>;   // §11.7; empty on failure
}
```
`season_update` / `episode_update` mirror Kotlin `SAnimeSeasonUpdate` / `SAnimeEpisodeUpdate`: when a flag is false the input is kept (the caller's `seasons` / `episodes` vector is returned unchanged). Rust takes `&mut` and returns `Result<(), _>` instead of a wrapper struct.

### 2.4 ComicInfo
```rust
// kuro-core/src/metadata/comicinfo.rs
pub const COMIC_INFO_FILE: &str = "ComicInfo.xml";
pub const NS_XSD: &str = "http://www.w3.org/2001/XMLSchema";
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

impl ComicInfo {
    pub fn to_xml(&self) -> String;                                  // §5.1
    pub fn from_xml(text: &str) -> Result<ComicInfo, ComicInfoError>; // §5.2
}
pub enum ComicInfoPublishingStatus { Ongoing, Completed, Licensed, PublishingFinished, Cancelled, OnHiatus, Unknown }
impl ComicInfoPublishingStatus {
    pub fn to_comic_info_value(s_manga_status: i64) -> &'static str;  // Kotlin toComicInfoValue(Long)
    pub fn to_s_manga_value(text: Option<&str>) -> i32;               // Kotlin toSMangaValue(String?)
}
```
`to_comic_info_value` takes `i64` because the domain status is `Long` (`COMIC:179`), and the match uses `value as i32`-equivalent truncation semantics: `value.to_int()` in Kotlin truncates to 32 bits. Rust: `s_manga_status as i32` (wrapping cast, same as Kotlin `Long.toInt()`).

### 2.5 Side files
```rust
// kuro-source-local/src/json_side.rs
pub fn read_chapters_json(path: &Path) -> Option<Vec<ChapterDetails>>;     // runCatching: any error -> None (behavior §20.3)
pub fn read_episodes_json(path: &Path) -> Result<Vec<EpisodeDetails>, LocalSourceError>;  // propagates (§20.4)
pub fn read_details_json<T: DeserializeOwned>(path: &Path) -> Result<T, LocalSourceError>;
pub fn parse_local_date(text: &str) -> Result<i64, LocalSourceError>;      // "yyyy-MM-dd'T'HH:mm:ss" in local offset, §4.6
pub fn parse_epub_date(text: &str) -> Option<i64>;                          // "yyyy-MM-dd'T'HH:mm:ssZ"-style, None on failure
```

### 2.6 Error
Behavior §23 and data §2.8. Two variants carry exact user-visible text (`ChapterNotFound`, `InvalidFormat`). The UI shows `Display` of the error.

---

## 3. Filesystem layer (`fs.rs`) — replaces UniFile

```rust
pub struct FsEntry { pub name: String, pub path: PathBuf, pub is_dir: bool, pub is_file: bool, pub modified_ms: i64 }

/// Kotlin listFiles(): unsorted, all entries. Rust returns them sorted by name (deterministic; Kotlin order is undefined).
pub fn list_dir(dir: &Path) -> io::Result<Vec<FsEntry>>;

/// Kotlin findFile(name) on a directory. Accepts a relative path with '/' separators (season URLs).
/// Returns None when missing. Rejects unsafe components (behavior §3.3).
pub fn find_child(dir: &Path, rel: &str) -> Option<PathBuf>;

/// Path safety: every '/'-separated component must be non-empty, not "." or "..", contain no NUL, no backslash, and not start with a drive/root prefix.
pub fn safe_join(base: &Path, rel: &str) -> Option<PathBuf>;

/// Kotlin: UniFile.isDirectory uses File.isDirectory (follows symlinks). Rust: std::fs::metadata (follows symlinks).
pub fn metadata_of(path: &Path) -> Option<fs::Metadata>;

/// Kotlin DiskUtil.createNoMediaFile: create ".nomedia" in dir when dir exists and file absent (empty file).
pub fn create_nomedia(dir: &Path) -> io::Result<()>;

/// Kotlin lastModified(): milliseconds since epoch; 0 when unknown.
pub fn modified_ms(meta: &fs::Metadata) -> i64;
```
Notes:
- `list_dir` never follows a dangling symlink for `is_dir`/`is_file` (both false), and still lists the name (Kotlin does the same, behavior §8.1 step 2 on a broken link: name present, isDirectory false, not an archive -> excluded from chapters unless its extension is a manga archive, then `format_of` fails at read time).
- `find_child` uses `safe_join` then `symlink_metadata`-free `exists()` (follows symlinks like Java).
- Hidden-file filtering is done by callers (`!name.starts_with('.')`), not by `list_dir`, because Kotlin filters differently per listing (behavior §4.3).

---

## 4. Names, formats and images

### 4.1 `names.rs`
```rust
/// Kotlin UniFile.extension: substringAfterLast('.') (no dot => whole name). None when name is None.
pub fn extension(name: &str) -> &str;
/// Kotlin nameWithoutExtension: substringBeforeLast('.') (no dot => whole name).
pub fn stem(name: &str) -> &str;
```
Rust implementation must mirror the Kotlin semantics exactly: `name.rsplit_once('.').map(|(_, e)| e).unwrap_or(name)` for extension; `name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)` for stem. (`rsplit_once` splits at the last dot, the same as `substringAfterLast`/`substringBeforeLast`.)

### 4.2 `format.rs`
```rust
pub const MANGA_ARCHIVE_EXTENSIONS: [&str; 8] = ["zip", "cbz", "rar", "cbr", "7z", "cb7", "tar", "cbt"];
pub const ANIME_EPISODE_EXTENSIONS: [&str; 8] = ["avi", "flv", "mkv", "mov", "mp4", "webm", "wmv", "torrent"];

pub fn is_manga_archive(name: &str) -> bool { MANGA_ARCHIVE_EXTENSIONS.contains(&extension(name).to_lowercase().as_str()) }
pub fn is_anime_episode(name: &str) -> bool { ANIME_EPISODE_EXTENSIONS.contains(&extension(name)) }   // case-sensitive
pub fn is_epub(name: &str) -> bool { extension(name).eq_ignore_ascii_case("epub") }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format { Directory(PathBuf), Archive(PathBuf), Epub(PathBuf) }

/// Kotlin Format.valueOf: directory first, epub next, manga archive last, otherwise Err(Unknown).
pub fn format_of(path: &Path, is_dir: bool) -> Result<Format, FormatError>;
```
Note `is_manga_archive` lowercases with `to_lowercase()` (Kotlin `lowercase()` is locale-invariant; for ASCII extensions identical).

### 4.3 Chapter entry predicate
```rust
/// Kotlin getChapterList filter: !startsWith('.') && (isDirectory || manga archive || epub)
pub fn is_chapter_entry(e: &FsEntry) -> bool;
/// Kotlin getOldEpisodeList filter: !startsWith('.') && ArchiveAnime.isSupported
pub fn is_episode_entry(e: &FsEntry) -> bool;
```

### 4.4 `images.rs` (ImageUtil replacement)
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType { Avif, Gif, Heif, Jpeg, Jxl, Png, Webp }
impl ImageType {
    pub fn mime(self) -> &'static str;      // data §2.5
    pub fn extension(self) -> &'static str; // "jpg" for Jpeg
    pub const ALL: [ImageType; 7] = [Avif, Gif, Heif, Jpeg, Jxl, Png, Webp];
}

/// Magic-byte table over the first 32 bytes (behavior §4.4).
pub fn sniff(head: &[u8]) -> Option<ImageType> {
    if head.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) { return Some(Png); }
    if head.starts_with(&[0xFF, 0xD8, 0xFF]) { return Some(Jpeg); }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") { return Some(Gif); }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" { return Some(Webp); }
    if head.len() >= 12 && &head[4..12] == b"ftypavif" { return Some(Avif); }
    if head.len() >= 12 && &head[4..8] == b"ftyp" && HEIF_BRANDS.contains(&&head[8..12]) { return Some(Heif); }
    if head.starts_with(&[0xFF, 0x0A]) { return Some(Jxl); }
    if head.starts_with(&[0, 0, 0, 0x0C, b'J', b'X', b'L', b' ', 0x0D, 0x0A, 0x87, 0x0A]) { return Some(Jxl); }
    None
}
const HEIF_BRANDS: [&[u8]; 8] = [b"heic", b"heix", b"hevc", b"heim", b"heis", b"hevx", b"mif1", b"msf1"];

/// Kotlin isImage(name, openStream): extension test (case-sensitive, "jpg" spelling) OR sniff.
pub fn is_image_named(name: &str, head: impl FnOnce() -> Option<Vec<u8>>) -> bool;
```
Callers in this crate always pass a head reader (Kotlin always supplies `openStream`), so the extension test is evaluated first and the sniff only when it fails. The reader reads at most 32 bytes and re-opens the file (Kotlin `findImageType` opens a new stream).

Writes and finds:
```rust
/// Kotlin LocalMangaCoverManager.find / LocalAnimeCoverManager.find etc.
/// Candidates: files (is_file) whose stem equals `stem` ignoring ASCII case, that pass is_image. Sorted by name; first wins.
pub fn find_image(dir: &Path, stem: &str) -> Option<PathBuf>;

/// Kotlin update(): reuse find_image(dir, stem) when present, else create `default_name`.
/// Overwrites bytes in place; creates .nomedia in dir afterwards (behavior §9.2 step 4).
pub fn write_image(dir: &Path, stem: &str, default_name: &str, bytes: &mut dyn Read) -> io::Result<PathBuf>;
```
Episode thumbnails use a different lookup (exact file name), see §10 of this file and behavior §13.2 quirk:
```rust
pub fn thumbnail_path(dir: &Path, episode_name: &str) -> PathBuf {  // dir/<sanitized episode name>-thumbnail.jpg
    dir.join(format!("{}-thumbnail.jpg", sanitize_file_stem(episode_name)))
}
```

### 4.5 `natsort.rs` (exact port of `net.greypanther.natsort`)
```rust
/// CaseInsensitiveSimpleNaturalComparator.compare over UTF-16 code units.
pub fn natural_cmp_ci(a: &str, b: &str) -> std::cmp::Ordering;
/// SimpleNaturalComparator (case-sensitive), kept for completeness; not used by the source.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering;
```
Implementation contract (copy of the Java algorithm; behavior §4.5):
```rust
fn cmp_units(s1: &[u16], s2: &[u16], ci: bool) -> Ordering {
    let (len1, len2) = (s1.len(), s2.len());
    let (mut i1, mut i2) = (0usize, 0usize);
    while i1 < len1 && i2 < len2 {
        let c1 = s1[i1]; i1 += 1;
        let c2 = s2[i2]; i2 += 1;
        let d1 = (b'0' as u16..=b'9' as u16).contains(&c1);
        let d2 = (b'0' as u16..=b'9' as u16).contains(&c2);
        if d1 && !d2 { return Ordering::Less; }
        if !d1 && d2 { return Ordering::Greater; }
        if !d1 && !d2 {
            let (x, y) = if ci { (java_lower(c1), java_lower(c2)) } else { (c1, c2) };
            if x != y { return x.cmp(&y); }            // Java: int difference, sign only matters
        } else {
            let mut n1: u64 = (c1 - b'0' as u16) as u64;
            while i1 < len1 && is_digit(s1[i1]) { n1 = n1.wrapping_mul(10).wrapping_add((s1[i1] - b'0' as u16) as u64); i1 += 1; }
            let mut n2: u64 = (c2 - b'0' as u16) as u64;
            while i2 < len2 && is_digit(s2[i2]) { n2 = n2.wrapping_mul(10).wrapping_add((s2[i2] - b'0' as u16) as u64); i2 += 1; }
            if n1 != n2 { return n1.cmp(&n2); }       // unsigned compare, matches compareUnsigned
        }
    }
    if i1 < len1 { Ordering::Greater } else if i2 < len2 { Ordering::Less } else { Ordering::Equal }
}
/// Character.toLowerCase(char): simple (one-to-one) mapping; anything that maps to several chars or to a surrogate is unchanged.
fn java_lower(u: u16) -> u16 {
    let Some(c) = char::from_u32(u as u32) else { return u };
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(l), None) if (l as u32) <= 0xFFFF => l as u16,
        _ => u,
    }
}
```
Java's `compareChars` returns a difference; only its sign is observable through `compare`, and the Kotlin callers use only `compareTo` results `< 0` / `> 0` / `== 0`, so `Ordering` is equivalent.

Important: the Java code uses `idx1--` when a non-digit follows a digit run; the Rust loop above avoids the decrement by peeking first (same effect). Surrogate pairs are compared as separate units (Java behavior).

### 4.6 Dates (`json_side.rs`)
```rust
const CHAPTER_DATE: &[FormatItem<'_>] = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
pub fn parse_local_date(text: &str) -> Result<i64, LocalSourceError> {
    let p = PrimitiveDateTime::parse(text, CHAPTER_DATE).map_err(|e| LocalSourceError::Date(e.to_string()))?;
    Ok(p.assume_offset(local_offset()).unix_timestamp() * 1000)
}
```
Behavior difference (documented, E-DATE-1): Java `SimpleDateFormat` is lenient and ignores trailing text; `time` is strict. Consequence: chapters.json dates that Kotlin rolls over or truncates are rejected here; the caller treats rejection as "no date override" (deviation from E-DATE-2, which aborted the whole list).

EPUB date (`parse_epub_date`): `format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][offset_hour sign:mandatory][offset_minute]")`; on error `None`.

---

## 5. ComicInfo (`kuro-core::metadata::comicinfo`)

### 5.1 Writer
Emission order and attributes are exactly data §5.3-5.4. Algorithm:
```rust
pub fn to_xml(&self) -> String {
    let mut w = quick_xml::Writer::new_with_indent(Vec::new(), b' ', 2);
    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None))).unwrap();
    let mut root = BytesStart::new("ComicInfo");
    root.push_attribute(("xmlns:xsd", NS_XSD));
    root.push_attribute(("xmlns:xsi", NS_XSI));
    w.write_event(Event::Start(root)).unwrap();
    write_text(&mut w, "Title", self.title.as_deref(), None);
    write_text(&mut w, "Series", self.series.as_deref(), None);
    write_text(&mut w, "Number", self.number.as_deref(), None);
    write_text(&mut w, "Summary", self.summary.as_deref(), None);
    write_text(&mut w, "Writer", self.writer.as_deref(), None);
    write_text(&mut w, "Penciller", self.penciller.as_deref(), None);
    write_text(&mut w, "Inker", self.inker.as_deref(), None);
    write_text(&mut w, "Colorist", self.colorist.as_deref(), None);
    write_text(&mut w, "Letterer", self.letterer.as_deref(), None);
    write_text(&mut w, "CoverArtist", self.cover_artist.as_deref(), None);
    write_text(&mut w, "Translator", self.translator.as_deref(), None);
    write_text(&mut w, "Genre", self.genre.as_deref(), None);
    write_text(&mut w, "Tags", self.tags.as_deref(), None);
    write_text(&mut w, "Web", self.web.as_deref(), None);
    write_text(&mut w, "PublishingStatusTachiyomi", self.publishing_status.as_deref(), Some(("ty", NS_XSD)));
    write_text(&mut w, "Categories", self.categories.as_deref(), Some(("ty", NS_XSD)));
    write_text(&mut w, "SourceAniyomi", self.source.as_deref(), Some(("ay", NS_XSD)));
    w.write_event(Event::End(BytesEnd::new("ComicInfo"))).unwrap();
    String::from_utf8(w.into_inner()).unwrap()
}
```
```rust
fn write_text(w: &mut quick_xml::Writer<Vec<u8>>, name: &str, value: Option<&str>, ns: Option<(&str, &str)>) {
    let Some(value) = value else { return };                 // null -> element omitted (data §5.3)
    let mut start = match ns {
        Some((prefix, uri)) => { let mut b = BytesStart::new(format!("{prefix}:{name}")); b.push_attribute((format!("xmlns:{prefix}").as_str(), uri)); b }
        None => BytesStart::new(name),
    };
    let qname = String::from_utf8(start.name().as_ref().to_vec()).unwrap();
    w.write_event(Event::Start(start)).unwrap();
    w.write_event(Event::Text(BytesText::new(value))).unwrap();   // escapes & < > ' "
    w.write_event(Event::End(BytesEnd::new(qname))).unwrap();
}
```
`write_text` writes `<name>` (or `<prefix:name xmlns:prefix="uri">` for namespaced) only when `Some`, with escaped text (`BytesText::new` escapes `&`, `<`, `>`, `'`, `"`). The element name uses the **element name** of data §3.1 (`PublishingStatusTachiyomi`, `Categories`, `SourceAniyomi`), not the Rust field name.

Trailing newline: `quick-xml` does not append one; Kotlin's serializer behavior at end of document is not established. Proposal: append `\n` (text files should end with newline); the golden comparison trims trailing whitespace on both sides.

### 5.2 Reader
Event loop over `quick_xml::Reader`, depth-tracked:
- Root `ComicInfo` required (local name compare); anything else -> `ComicInfoError::Root`.
- Depth 1 children: match `local_name()` against the 17 element local names (`PublishingStatusTachiyomi`, `Categories`, `SourceAniyomi` included). Unknown elements are skipped (`read_to_end`, matching `ignoreUnknownChildren`).
- Text: collect `Event::Text` and `Event::CData`, unescape; an `Event::Empty` (self-closing) element yields `Some("")`.
- Duplicate element: the last occurrence wins (xmlutil would reject duplicates; Proposal: last wins to be lenient; E-CI-4).
- Namespace prefix ignored (Proposal, E-CI-6).

### 5.3 Errors
`ComicInfoError::{Xml(quick_xml::Error), Root(String)}` mapped into `LocalSourceError::Xml`.

### 5.4 Status mapping
Exact tables in data §3.2. `to_s_manga_value(None)` returns 0.

### 5.5 Field-order rule
`ComicInfo` fields in Rust are declared in the Kotlin order (data §3.1) so `Debug` output and any future `serde` derive stay aligned. Writing never uses reflection; the explicit call order in `to_xml` is the contract.

### 5.6 Bridge (`kuro-source-local/src/bridge.rs`)
```rust
pub fn manga_to_comic_info(m: &SManga) -> ComicInfo {    // Kotlin SManga.getComicInfo()
    ComicInfo {
        series: Some(m.title.clone()),
        summary: m.description.clone(),
        writer: m.author.clone(),
        penciller: m.artist.clone(),
        genre: m.genre.clone(),
        publishing_status: Some(ComicInfoPublishingStatus::to_comic_info_value(m.status as i64).to_string()),
        ..Default::default()
    }
}

pub fn apply_comic_info(m: &mut SManga, ci: &ComicInfo) {  // Kotlin SManga.copyFromComicInfo
    if let Some(v) = &ci.series { m.title = v.clone(); }
    if let Some(v) = &ci.writer { m.author = Some(v.clone()); }
    if let Some(v) = &ci.summary { m.description = Some(v.clone()); }

    let genre = join_distinct([&ci.genre, &ci.tags, &ci.categories].into_iter().flatten().map(String::as_str), ", ");
    if !genre.is_empty() { m.genre = Some(genre); }

    let artist_parts = [&ci.penciller, &ci.inker, &ci.colorist, &ci.letterer, &ci.cover_artist]
        .into_iter().flatten()
        .flat_map(|s| s.split(", ").map(str::to_string).collect::<Vec<_>>());
    let artist = join_distinct_owned(artist_parts, ", ");
    if !artist.is_empty() { m.artist = Some(artist); }

    m.status = ComicInfoPublishingStatus::to_s_manga_value(ci.publishing_status.as_deref());
}

/// Kotlin: distinct() on the list, then joinToString(sep) { it.trim() }
fn join_distinct<'a>(items: impl Iterator<Item = &'a str>, sep: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    items.filter(|s| seen.insert(*s)).map(str::trim).collect::<Vec<_>>().join(sep)
}
```
Ordering subtlety preserved: `distinct()` runs **before** `trim()` in the Kotlin pipeline for the genre path (`COMIC:38-44`) and **after** `flatMap(split)` for the artist path. `join_distinct` must dedupe by exact string, then trim each element while joining. Empty elements are kept (Kotlin keeps them; `"a, , b"` is observable).

Important: `manga_to_comic_info` sets `series` from `m.title` (always `Some`), mirroring `ComicInfo.Series(title)` (`COMIC:12`).

---

## 6. Browse implementation (`manga/browse.rs`, `anime/browse.rs`)

Pseudocode (both media; differences noted):
```rust
pub async fn browse_manga(root: Option<PathBuf>, mode: BrowseMode) -> Result<ItemsPage<SManga>, LocalSourceError> {
    let Some(root) = root else {                                  // storage folder missing: empty page (behavior §2)
        return Ok(ItemsPage { items: Vec::new(), has_next_page: false });
    };
    tokio::task::spawn_blocking(move || -> Result<_, LocalSourceError> {
        let limit = match mode { BrowseMode::Latest => now_ms() - LATEST_WINDOW_MS, _ => 0 };
        let query: &str = match &mode { BrowseMode::Search { query, .. } => query.as_str(), _ => "" };
        let mut seen = std::collections::HashSet::new();
        let mut dirs: Vec<FsEntry> = list_dir(&root)?
            .into_iter()
            .filter(|e| e.is_dir && !e.name.starts_with('.'))
            .filter(|e| seen.insert(e.name.clone()))               // Kotlin distinctBy { name }: first occurrence wins
            .filter(|e| {
                if limit != 0 { e.modified_ms >= limit }            // Latest: query ignored
                else if query.trim().is_empty() { true }            // Kotlin isBlank()
                else { e.name.to_lowercase().contains(&query.to_lowercase()) }
            })
            .collect();
        let sort = match &mode {
            BrowseMode::Latest => Some(LATEST_SORT),
            BrowseMode::Popular(s) => Some(*s),
            BrowseMode::Search { sort, .. } => *sort,
        };
        if let Some(s) = sort { sort_entries(&mut dirs, s); }
        let items = dirs.into_iter().map(|e| SManga {
            url: e.name.clone(),
            title: e.name.clone(),
            thumbnail_url: find_image(&e.path, "cover").map(file_url),   // Kotlin coverManager.find(name)
            ..SManga::create()
        }).collect();
        Ok(ItemsPage { items, has_next_page: false })
    }).await?
}
```
Details that must match Kotlin:
- `query.isBlank()` (whitespace-only counts as blank) for the keep-all branch; the contains-branch uses the raw query: `name.contains(query, ignoreCase = true)` (no trim). Rust: `query.trim().is_empty()` for the first test, and `name.to_lowercase().contains(&query.to_lowercase())` for the second (ASCII-exact; Unicode case folding differs only for exotic letters — noted in edge file E-BROWSE-3).
- Kotlin applies the query filter only when `lastModifiedLimit == 0`; the Latest branch ignores the query (behavior §6.1 step 6).
- `sort_entries` for `SortSelection { index: 0, ascending }` -> compare by `case_insensitive_order(name)` (Java `String.CASE_INSENSITIVE_ORDER`, per-char `to_uppercase` then `to_lowercase`; Rust approximates with per-char `to_lowercase` of the single-char mapping; E-BROWSE-4). `index: 1` -> sort by `modified_ms` (stable) ascending or descending.
- Sort is stable (Kotlin `sortedWith`/`sortedBy` are stable; Rust `sort_by` is stable). Ties keep the `list_dir` order, which is name-sorted in Rust (deterministic; Kotlin is filesystem order — the deviation is listed in behavior §31).
- `thumbnail_url` is `find_image(dir, "cover")` converted with `file_url` (data §6.5).

Anime browse (`anime/browse.rs`) is the same function with: entries filter identical; mapping produces `SAnime { url: name, title: name.rsplit('/').next(), fetch_type: fetch_type_of(path), thumbnail_url: cover, background_url: find_image(dir, "background") }`. The `fetch_type_of` call makes a second listing of the same directory (Kotlin behavior; a Proposal may pass the already-listed children to avoid double listing, §12).

`LATEST_WINDOW_MS = 7 * 24 * 60 * 60 * 1000` (`604_800_000`).

---

## 7. Manga details (`manga/details.rs`)

```rust
pub async fn details(root: Option<PathBuf>, manga: &mut SManga) -> Result<(), LocalSourceError> {
    let Some(dir) = root.and_then(|r| find_child(&r, &manga.url)).filter(|p| p.is_dir()) else {
        tracing::error!(url = %manga.url, "not a valid directory");   // Kotlin error(...) is caught and logged (behavior §7.1)
        return Ok(());
    };
    if let Some(cover) = find_image(&dir, "cover") { manga.thumbnail_url = Some(file_url(cover)); }   // step 1 (behavior §7.1)
    // Decision: move the SManga into the blocking task and take it back, so partial writes survive an error (behavior §7.6).
    let title = manga.title.clone();
    let taken = std::mem::replace(manga, SManga::create());
    let (back, result) = tokio::task::spawn_blocking(move || {
        let mut m = taken;
        let r = metadata_branch(&dir, &mut m);
        (m, r)
    }).await?;
    *manga = back;
    if let Err(e) = result { tracing::error!(error = %e, title = %title, "Error setting manga details from local metadata"); }   // never returned (behavior §7.6)
    Ok(())
}
```
The Rust version works on a clone, moves the result back on success, and keeps the original on error. This reproduces the Kotlin "partial updates survive" rule only for the cover (step 1), which is applied to `manga` directly. Rationale: the Kotlin branch writes fields directly on the object before failing; for ComicInfo parse failure (branch a) the object is unchanged in Kotlin too, because `copyFromComicInfo` runs only after a successful decode. For the legacy JSON branch (b), fields are assigned before the XML write; a write failure in Kotlin leaves the assigned fields. Proposal: keep that by passing `&mut manga` through the blocking task (not a clone) — use `Arc<Mutex<SManga>>` moved into the task and unlocked after join. (Decision: pass the mutable reference by value into the blocking closure and return it, which avoids the mutex.)

Metadata branch in blocking code:
```rust
fn metadata_branch(dir: &Path, manga: &mut SManga) -> Result<(), LocalSourceError> {
    let entries = list_dir(dir)?;
    let comic = entries.iter().find(|e| e.name == COMIC_INFO_FILE);
    let noxml = entries.iter().find(|e| e.name == ".noxml");
    let legacy = entries.iter().find(|e| extension(&e.name) == "json" && stem(&e.name) == "details");
    if let Some(c) = comic {
        if let Some(n) = noxml { let _ = fs::remove_file(&n.path); }
        apply_comic_info(manga, &ComicInfo::from_xml(&fs::read_to_string(&c.path)?)?);
    } else if let Some(l) = legacy {
        let d: MangaDetails = read_details_json(&l.path)?;
        apply_manga_details(manga, &d);                         // title/author/artist/description/genre/status when Some
        let xml = bridge::manga_to_comic_info(manga).to_xml();
        write_file(dir.join(COMIC_INFO_FILE), xml.as_bytes())?;  // createFile + overwrite
        fs::remove_file(&l.path)?;                               // only after successful write
    } else if noxml.is_none() {
        let archives: Vec<&FsEntry> = entries.iter().filter(|e| !e.is_dir && is_manga_archive(&e.name)).collect();
        if let Some(bytes) = copy_comic_info_from_archives(&archives)? { // first archive that has root ComicInfo.xml
            write_file(dir.join(COMIC_INFO_FILE), &bytes)?;
            apply_comic_info(manga, &ComicInfo::from_xml(std::str::from_utf8(&bytes)?)?);
        } else {
            write_file(dir.join(".noxml"), b"")?;
        }
    }
    Ok(())
}
```
Notes:
- Kotlin `ArchiveManga.isSupported` is applied to `mangaDirFiles` without the `isDirectory` check (directory named `x.zip` would be opened as archive, fail, and the error aborts the method). Rust: `!e.is_dir` (documented deviation E-MANGA-DETAILS-2).
- `archives` order: Kotlin unsorted; Rust name-sorted (deterministic).
- For archive copy, reading uses `kuro_core::archive::ArchiveReader::open(path)` and `read_entry("ComicInfo.xml")` (area:13-core-util-storage-archive-torrent). The entry name is the exact root-relative name.
- A top-level existing ComicInfo.xml is never overwritten by the copy path (branch a precludes it).
- `write_file` = `fs::write` (create or truncate). Kotlin `createFile` on an existing name is not reached in branch b (no ComicInfo exists). In branch c it is not reached either.

Anime details (`anime/mod.rs`): `details_anime(dir, anime)`: cover/background `find_image` as above; `details.json` via `read_details_json::<AnimeDetails>` with **error propagation** (E-ANIME-2).

---

## 8. Chapter list (`manga/chapters.rs`)

```rust
pub async fn chapters(root: Option<PathBuf>, manga: &mut SManga) -> Result<Vec<SChapter>, LocalSourceError> {
    let needs_cover = manga.thumbnail_url.as_deref().map_or(true, |s| s.trim().is_empty());   // Kotlin isNullOrBlank()
    let (url, title) = (manga.url.clone(), manga.title.clone());
    let (cover_url, chapters) = tokio::task::spawn_blocking(move || chapters_blocking(root, &url, &title, needs_cover)).await??;
    if let Some(c) = cover_url { manga.thumbnail_url = Some(c); }
    Ok(chapters)
}
```
Blocking body (order matches Kotlin):
1. `chapters_json = find_child(dir, ..)`: the first entry with `extension == "json" && stem == "chapters"` in `list_dir(dir)` order (Rust: name-sorted, first). Parse with `read_chapters_json` (silent None on any error, E-CHAPTERS-1).
2. `entries = list_dir(dir)?.filter(is_chapter_entry)`. Kotlin does not sort; Rust name-sorted before mapping (only affects the tie-break of identical numbers and names, which the final sort fixes anyway).
3. For each entry build `SChapter { url: format!("{}/{}", manga.url, e.name), name: if e.is_dir { e.name } else { stem(&e.name) }, date_upload: e.modified_ms, chapter_number: -1.0 as f32, scanlator: None, ..}`.
4. `chapter_number = recognition::chapter_number(&title, &name, None) as f32` (behavior §5). The known-number argument is `None` (Kotlin passes -1.0 which is "unknown").
5. If the entry is an EPUB: `epub::fill_metadata(manga, &mut chapter, &path)?` — errors propagate (E-MANGA-3).
6. If `chapters_json` contains an element with `|chapter_number - n| < 1e-4` (first wins): apply `name`, `date_upload` (parse_local_date; on Err treat as absent, deviation E-DATE-1), and `scanlator = data.scanlator.clone()` (unconditional).
7. Sort: `chapters.sort_by(|a, b| b.chapter_number.total_cmp(&a.chapter_number)` then `natural_cmp_ci(&b.name, &a.name)`)`. Use `partial_cmp`-equivalent ordering: Kotlin `Float.compareTo` is total (NaN greatest, -0.0 < 0.0); `f32::total_cmp` is the same total order. [Equivalent to Kotlin compareTo.]
8. If `manga.thumbnail_url` is None or blank: take the **last** element (lowest number) and run `cover_from_chapter` (behavior §9.3); swallow errors (logged).
9. Return.

`cover_from_chapter`: `Format::Directory` -> `list_dir(dir)` sorted by `natural_cmp_ci(name)`, `find(!is_dir && is_image)`, copy into `write_image(manga_dir, "cover", "cover.jpg", ...)`. `Format::Archive` -> entries via `kuro_core::archive` in natural-CI name order, first `is_file && is_image`, `read_entry(name)`. `Format::Epub` -> `epub::first_image(path)`. Then `manga.thumbnail_url = Some(file_url(cover_path))`.

---

## 9. Images (`images.rs`) — writes

`write_image` algorithm:
1. `dir` must exist (`is_dir`); else return `Err(NotFound)` (Kotlin: closes the input and returns null; the Rust function returns an error the caller maps to `None`).
2. `target = find_image(dir, stem).unwrap_or_else(|| dir.join(default_name))`.
3. Copy all bytes (`io::copy`) into `target` with `File::create` (truncate). No validation.
4. `create_nomedia(dir)`.
5. Return `target`.

Thumbnail writes use `write_image` with `stem = <episode name>-thumbnail` and `default_name = <episode name>-thumbnail.jpg` **after** the sanitizer (§4.4 `thumbnail_path`); the find then matches the sanitized name exactly. This is the documented fix of the Kotlin `find` quirk (behavior §13.2): Kotlin never matches, always creates; Rust matches the same file, so the observable result (one file per episode, overwritten) is identical.

---

## 10. Episode list (`anime/episodes.rs`)

Blocking body mirrors behavior §11.5:
1. `episodes_json = read_episodes_json(&path)` — `Option` only when the file exists; errors propagate (E-ANIME-3).
2. `entries = list_dir(anime_dir)?.filter(is_episode_entry)`; each -> `SEpisode { url: format!("{}/{}", anime.url, name), name: stem(name), date_upload: modified_ms, episode_number: -1.0, ..}`.
3. `episode_number = recognition::episode_number(&anime.title, &name, None) as f32`.
4. `episodes_json` override (first match within `1e-4`): `name`, `date_upload` (lenient-fail = absent), `scanlator` (unconditional), `summary` (unconditional).
5. Thumbnail per episode (only when `preview_url` is None — always, on a fresh object; behavior E-ANIME-6): `media::thumbnail(video_path, out = thumbnail_path(dir, name), at = frame_position)`, errors logged; if the output exists and is non-empty -> `preview_url = Some(file_url(out))`. Proposal (documented deviation): skip extraction when `thumbnail_path` already exists and is non-empty, unless `force = true`.
6. Sort: `episode_number` desc (`total_cmp`), then `natural_cmp_ci(b.name, a.name)` desc.
7. Cover fallback (condition: `anime.thumbnail_url` None or `find_image(dir, "cover")` None): last episode; `media::frame_to(file, out_tmp)` then `write_image(dir, "cover", "cover.jpg", tmp)`; set `thumbnail_url`; errors logged (`"Couldn't extract cover from video"`).
8. Background fallback: same with `background` (`background_url`).
9. Return.

Kotlin's `anime.thumbnail_url == null` check happens on the **object passed in**, so the cover fallback runs when the caller's object has no cover URL even if a file exists. Preserved.

---

## 11. Anime seasons (`anime/seasons.rs`)
```rust
pub async fn season_update(anime: &mut SAnime, seasons: &mut Vec<SAnime>, fetch_details: bool, fetch_seasons: bool) -> Result<(), LocalSourceError>   // see §2.3; both branches run concurrently with tokio::join!
```
- `details` branch (anime details.json) and `seasons` branch run with `tokio::join!` (Kotlin `supervisorScope` + `async`). Each returns its own `Result`; `season_update` returns the first `Err` after both complete (Kotlin rethrows an exception of an awaited child; `supervisorScope` lets the sibling finish). Behavior §11.3.
- Season list: `list_dir(anime_dir)?` filtered to `is_dir && !name.starts_with('.')`, then first-occurrence dedupe by name (same rule as browse); for each: `url = format!("{}/{}", anime.url, name)`, `SAnime` via the browse mapping, `fetch_type = fetch_type_of(season_path)`.

---

## 12. Fetch type and playback

`anime/fetch_type.rs`:
```rust
pub fn fetch_type_of(children: &[FsEntry]) -> FetchType {
    if children.iter().any(|e| is_anime_episode(&e.name)) { FetchType::Episodes }   // includes dir named *.mkv
    else if children.iter().any(|e| e.is_dir) { FetchType::Seasons }                 // includes hidden dirs (E-ANIME-8)
    else { FetchType::Episodes }
}
```
`children` is the listing of the anime folder (`list_dir` result). Proposal: pass the listing down so browse does one listing per folder instead of two (ceiling: browse cost O(folders × 3) in Kotlin).

`anime/playback.rs`:
```rust
pub async fn hosters_for(root: Option<PathBuf>, episode_url: &str) -> Result<Vec<Video>, LocalSourceError> {
    // split at first '/', safe_join(root, episode_url) -> path; exists() ? vec![Video{url: file_url(path), title: format!("Local source: {episode_url}")}] : vec![]
}
```
Kotlin catches every exception and returns an empty list (E-PLAY-1). Rust: `Ok(vec![])` for "missing", `Err` only for unsafe URL? Proposal: both map to empty list plus a `tracing::warn!`.

`Video` type comes from area:16-source-api-anime (anime source API). Title string is exactly `"Local source: <episode.url>"`.

---

## 13. Media (`media.rs`) — ffmpeg replacement

```rust
pub struct FrameJob<'a> { pub input: &'a Path, pub output: &'a Path }

pub async fn probe_duration_secs(input: &Path) -> Result<f64, MediaError> {
    let out = Command::new(tool::ffprobe()?)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=noprint_wrappers=1:nokey=1"])
        .arg(input)
        .kill_on_drop(true)
        .output().await?;                                  // stdout trimmed, parsed
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    text.parse::<f64>().map_err(|_| MediaError::BadDuration(text))
}

pub async fn extract_frame(input: &Path, second: i64, output: &Path) -> Result<bool, MediaError> {
    let status = Command::new(tool::ffmpeg()?)
        .arg("-ss").arg(second.to_string())
        .arg("-i").arg(input)
        .args(["-frames:v", "1", "-update", "true"])
        .arg(output)
        .arg("-y")
        .kill_on_drop(true)
        .output().await?;
    Ok(std::fs::metadata(output).map(|m| m.len() > 0).unwrap_or(false))   // Kotlin: tempFile.length() > 0; exit code ignored
}

pub fn frame_position(duration_secs: f64) -> i64 { (duration_secs as i64) / 2 }   // Kotlin duration.toInt() / 2
```
Rules:
- The Kotlin command string is replaced by an argument vector (no shell, no quoting). The path is passed as one argument; escaping `"` is unnecessary (Kotlin escapes only for FFmpegKit's tokenizer).
- Temp output: `tempfile::Builder::new().prefix("tmp_").suffix(&suffix).tempfile_in(paths.temp_dir())`, then `keep()` or read and drop to delete. Proposal: delete after the image is written (E-ANIME-7 fix). The suffix is `format!("{title}cover.jpg")` etc. — but a sanitized suffix is used (title can contain characters invalid on Windows).
- Probe and extract run sequentially. Kotlin does not use a timeout. Proposal: `tokio::time::timeout(Duration::from_secs(20), probe)` and `timeout(Duration::from_secs(120), extract)`; on timeout kill the child (`kill_on_drop`) and return `MediaError::Timeout`.
- Tool lookup: `tool::ffmpeg()` / `tool::ffprobe()` come from `kuro-platform` (`ExternalTools` trait; Linux: search `PATH`; Windows later: `ffmpeg.exe` next to the app). If absent: `MediaError::ToolMissing`; the episode listing continues without thumbnails and logs one `warn` per session (E-MEDIA-1).
- ffmpeg's stdout/stderr are captured and discarded (`output()`), never shown.

---

## 14. EPUB (`epub.rs`)

Consumers: `fill_metadata` (§21 behavior) and `first_image` (cover fallback). Both operate on an `EpubReader` from `kuro_core::archive::epub` (area:13-core-util-storage-archive-torrent). Required API from area:13-core-util-storage-archive-torrent: `EpubReader::open(path)`, `package_href() -> Result<String>`, `package_document(href) -> Result<String>` (XML text), `images_from_pages() -> Result<Vec<String>>`, `read_entry(name) -> Result<Option<Vec<u8>>>`.

`fill_metadata(path, manga, chapter)`:
```rust
let href = reader.package_href()?;
let opf = scraper::Html::parse_document(&reader.package_document(&href)?);   // Jsoup parity: HTML mode
let sel = |tag: &str| opf.select(&Selector::parse(&escape_css(tag)).unwrap()).next();   // "dc:title" -> escaped "dc\\:title"
// fn escape_css(tag: &str) -> String { tag.replace(':', "\\:") }   // only ':' occurs in the tags used here
let title = sel("dc:title"); let publisher = sel("dc:publisher"); let creator = sel("dc:creator");
let description = sel("dc:description");
let date = sel("dc:date").or_else(|| opf.select(&Selector::parse("meta[property=\"dcterms:modified\"]").unwrap()).next());
if let Some(c) = creator { manga.author = Some(text(c)); }
if let Some(d) = description { manga.description = Some(text(d)); }
if let Some(t) = title { chapter.name = text(t); }
if let Some(p) = publisher { chapter.scanlator = Some(text(p)); } else if let Some(c) = creator { chapter.scanlator = Some(text(c)); }
if let Some(d) = date { if let Some(ts) = parse_epub_date(&text(d)) { chapter.date_upload = ts; } }
```
Important parity points:
- Kotlin `getElementsByTag("dc:title")` matches the tag name `dc:title`; `scraper` selectors with a colon need escaping (`dc\:title`). The OPF is parsed as HTML (tag names lowercased) — same as Jsoup.
- `creator?.text()?.let { manga.author = it }`: Kotlin `text()` returns the element's text (whitespace normalized by Jsoup: runs of whitespace collapsed, trimmed). Rust helper `text(el)` must collapse whitespace the same way (`split_whitespace().join(" ")`).
- Date uses the **date text** only; no trim differences beyond whitespace normalization.

`first_image(path) -> Result<Option<Vec<u8>>>`: `images_from_pages()?.first()` then `read_entry`.

---

## 15. Set-as-art and mutations

These are Kotlin extension functions that call the managers. Rust: `kuro-source-local::covers::set_manga_cover(root, manga, bytes)`, `set_anime_cover`, `set_anime_background`, `set_episode_thumbnail` — each one line calling `write_image` (or `write_image` with the thumbnail stem), guarded by `is_local` (the caller checks `is_local_source(entry.source)`), returning `Ok(updated_url)`. The UI result enum (`Success | AddToLibraryFirst | Error`) belongs to the app area (behavior §16).

---

## 16. Threading model

| Actor | Thread | Notes |
|---|---|---|
| GTK main loop | main thread | never calls this crate's blocking functions |
| Source calls (`browse`, `details`, `chapters`, `season_update`, `episode_update`, `hosters_for`) | tokio multi-thread runtime | each public async fn does `spawn_blocking` around one closure |
| ffprobe / ffmpeg | tokio child process (`kill_on_drop`) | awaited from the async wrapper, not the blocking pool |
| Preference reads | any | `Pref<T>` is `Send + Sync` (area:01-foundation-app-bootstrap) |
| Results to UI | `glib::MainContext` channel | the caller forwards `Result` to the GTK thread (area:01-foundation-app-bootstrap pattern) |

- `spawn_blocking` is used only for filesystem work. The ffmpeg await must not run inside `spawn_blocking` (it would block a blocking-pool thread for the child's lifetime; the async form is kept).
- Episode thumbnail generation is inside the episode-list blocking task and therefore holds one blocking-pool thread for its duration. Ceiling: a folder with hundreds of episodes and no thumbnails takes minutes. Proposal: a per-process `tokio::sync::Semaphore(1)` around frame extraction so two folders never run ffmpeg concurrently.
- No `Mutex` is needed. The only shared state is the immutable root path held by each source (`Option<PathBuf>`).

---

## 17. Logging

`tracing` events (level, target `kuro_source_local`):
- `error!` for every swallowed error listed in behavior §23.3 with the same English text plus the path.
- `warn!` for tool missing (once per process via `OnceLock<()>`), unsafe URL rejection, malformed chapters.json (once per file per call).
- `debug!` for each skipped thumbnail (existing output).

---

## 18. Kotlin -> Rust mapping of Android APIs touched

| Kotlin / Android (file) | Rust / Linux replacement | Decision |
|---|---|---|
| `UniFile` (hippo library; `com.hippo.unifile.UniFile`) | `std::fs` + `fs.rs` (`FsEntry`, `list_dir`, `find_child`) | Replace |
| `UniFile.uri.toString()` | `url::Url::from_file_path(path)` -> `file:///<absolute path>` | Replace |
| `UniFile.createFile` / `openOutputStream` | `File::create` (truncate) | Replace |
| `Context.stringResource(MR/AYMR)` | `kuro_i18n::t(key)` | Replace (the i18n area (51, no plan folder in this set) contract; keys in data §1) |
| `DiskUtil.createNoMediaFile` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:87`) | `fs::create_nomedia` | Keep behavior, drop media scan |
| `MediaScannerConnection.scanFile` (`DiskUtil.kt:97-100`) | nothing | Drop (no MediaStore on Linux; tracker-miner indexes files without a hint) |
| `com.arthenica.ffmpegkit.FFmpegKit` / `FFprobeKit` | `tokio::process` running `ffmpeg` / `ffprobe` from PATH (`kuro-platform`) | Replace (external tool, Arch package `ffmpeg`) |
| `FFmpegKitConfig.getSafParameter` (SAF) | nothing | Drop (no content URIs on Linux) |
| `File.createTempFile("tmp_", suffix)` | `tempfile::Builder` in `Paths::temp_dir()` | Replace |
| `java.io.File.separator` | `/` joins via `safe_join` | Replace |
| `mihon.core.archive.archiveReader / epubReader` (libarchive via `ParcelFileDescriptor` + mmap) | `kuro_core::archive` (area:13-core-util-storage-archive-torrent) | Replace (area:13-core-util-storage-archive-torrent) |
| `Jsoup` (EPUB OPF/XHTML parse) | `scraper` (html5ever) | Replace |
| `kotlinx.serialization.json.Json` | `serde_json` | Replace |
| `nl.adaptivity.xmlutil` XML | `quick-xml` | Replace (exact output in data §5) |
| `SimpleDateFormat` + `Locale.getDefault()` | `time` format descriptions + captured local offset (§19) | Replace |
| `net.greypanther.natsort` | `natsort.rs` port | Port (exact) |
| `withIOContext` / `async` / `awaitAll` / `supervisorScope` | `spawn_blocking` / `tokio::join!` | Replace |
| `logcat` | `tracing` | Replace |
| `Injekt.get()` | constructor injection of the root paths and sources from area:01-foundation-app-bootstrap `AppContext` | Replace |
| `tachiyomi.decoder.ImageDecoder` (native, external) | `images::sniff` table | Replace |
| `TimeUnit.MILLISECONDS.convert(7, DAYS)` | `const LATEST_WINDOW_MS: i64 = 604_800_000` | Replace |
| `DiskUtil.buildValidFilename` not used here | `kuro-core::fs_names::sanitize_file_stem` (area:01-foundation-app-bootstrap addendum) | Replace |
| `StorageManager.getLocalMangaSourceDirectory` / `getLocalAnimeSourceDirectory` | `roots::manga_root` / `roots::anime_root` over the base folder from area:09-domain-source-track-download-release-misc `StorageManager` | Replace |

---

## 19. Local time zone (`time` crate pitfall)

`time::UtcOffset::current_local_offset()` returns `Err` on Linux once the process is multi-threaded (time 0.3 soundness rule), so calling it inside `spawn_blocking` yields an error. Design:
```rust
// kuro-core::time (area:01-foundation-app-bootstrap owns startup)
static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();
pub fn capture_local_offset() { let _ = LOCAL_OFFSET.set(UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC)); }  // call first thing in main
pub fn local_offset() -> UtcOffset { *LOCAL_OFFSET.get().unwrap_or(&UtcOffset::UTC) }
```
Kotlin uses the device default time zone; a DST change while the app runs is not reflected (identical to the captured offset). Ceiling noted; Proposal: `local_offset()` re-reads when `OnceLock` is unset.

---

## 20. Pitfalls

1. **Path separator in URLs.** Season URLs contain `/`. Never call `Path::join(url)` with an unvalidated string; always `safe_join` (§3).
2. **`split('/', limit = 2)`** on chapter/episode URLs: use `splitn(2, '/')`, not `split`, otherwise season episode URLs lose their season segment.
3. **Case rules differ by media**: manga archive check is case-insensitive, anime episode check is case-sensitive, image extension check is case-sensitive. Use the functions in §4, never ad-hoc comparisons.
4. **Unconditional overwrites** (`scanlator`, `summary`, `status`) are intended (behavior §31). Do not "fix" them into `if let Some`.
5. **Natural order ties.** `natural_cmp_ci` returns `Equal` for `"z010"` and `"z10"`; sorts with stable algorithms keep input order; the final list order for equal numbers and equal names is therefore input order (name-sorted in Rust). Kotlin's input order is filesystem order; document.
6. **Float ties.** Chapter numbers `4.0` and `4.00001` are different `f32`s but equal under the `1e-4` matcher; the matcher is used only for JSON overrides, never for sorting.
7. **ffmpeg argument order.** `-ss` before `-i` (fast seek) matters for speed; keep the order in §13.
8. **`file://` URL round trip.** Paths with `%` or non-UTF-8 bytes: `Url::from_file_path` percent-encodes; reading back requires `to_file_path()`; a non-`file` scheme is treated as absent (data §6.5).
9. **`.nomedia` must not appear in listings as content.** It is a dot file; `is_chapter_entry` and `is_episode_entry` reject it. Keep the dot-prefix filter first.
10. **Concurrent cover writes.** Two browses of the same folder never write; only explicit set-as-cover writes. No locking needed.
11. **Case-insensitive file systems (Windows later).** `ComicInfo.xml` matching is case-sensitive in Kotlin and in this design; on a case-insensitive file system a file named `comicinfo.xml` would be found by `fs::read` but not matched by the name filter. Keep the exact-name rule to keep behavior identical across platforms; document in the platform notes.
12. **Broken symlinks**: `is_dir` false, `is_file` false; the entry is listed but never opened unless its extension is an archive, then `format_of` returns Unknown -> `InvalidFormat` at read time (Kotlin-identical).
13. **Huge archives**: `kuro_core::archive` should not load whole archives into memory for `read_entry` (area:13-core-util-storage-archive-torrent contract).

---

## 21. Mockup integration (`/home/blackbox/kurobayomi/src`)

The current mockup has no data layer. Where the local source appears:
- `src/entry.rs:283-301` builds the source list with a hard-coded `local = if anime { "Local anime source" } else { "Local source" }` in section `Other`. The manga label must become `Local manga source` (AYMR, base value) — fix in the real screen, not in the mockup (mockup text is a stand-in).
- `src/entry.rs:360-380` (`migrate_sources`) and `src/entry.rs:380-420` (`browse_source`) use `Source A/B/C` placeholders; the local source shows the Help action (behavior §17.3) only when the source is local.
- `src/screens.rs:359` lists `Source A/B` counts; not local-related.
- `spec/entries.json` and `spec/app.json` carry the screens; `spec/SCHEMA.md` defines the item types. No spec entry describes the local source's rows; this area adds no spec file (the spec is a design artifact per area:01-foundation-app-bootstrap §7).

Integration plan (for the implementation task list): the source list view receives `Vec<SourceRow>` from area:17-source-app-layer's `SourceManager`; the local row is a normal `SourceRow` with `id = 0`, `name = LocalMangaSource::name()`, `lang = "other"`, flagged `is_local = true` (no disable action; help action visible). The mockup's `Other` section already matches the placement of `lang == "other"` rows (behavior §1).

---

## 22. Test seams

- All filesystem functions take `&Path`, so tests run against `tempfile::TempDir` trees (kuro-testkit temp-dir `Paths`, area:01-foundation-app-bootstrap).
- `media` functions accept the tool path through `kuro-platform`'s `ExternalTools` so tests can point at a fake script or skip when `ffmpeg` is absent (`#[ignore]` with a clear reason when tools are missing).
- `time` helpers: `local_offset()` is injectable through a `fn(&self) -> UtcOffset` in tests (or the test sets the `OnceLock` once before running; document).
- Clock: `now_ms()` is passed as a parameter to `browse` internals so the 7-day window is testable without sleeping (`browse_inner(root, mode, now_ms)`).

---

## 23. Non-goals (explicit)
- No write support for manga chapter folders or archives (only covers, backgrounds, thumbnails, ComicInfo.xml, `.noxml`, `.nomedia`).
- No re-scan scheduler and no file-system watcher (Kotlin has none; the library refresh triggers listing).
- No recursion beyond one season level.
- No support for the `ParsedHttpSource` style helpers (area:15-source-api-manga concern).

---

## 24. Ponytail notes (simplifications with ceilings)
- Image magic table is hand-written (20 lines). Ceiling: HEIF brand list best-effort. Upgrade path: `infer` crate when brand coverage matters.
- Natural comparator is a line-by-line port. Ceiling: Java `Character.toLowerCase(char)` is approximated for non-BMP and special cases. Upgrade path: none needed for parity.
- Browse lists each folder once per call; anime fetch type re-lists the folder. Ceiling: O(folders x 2) listings per browse. Upgrade path: pass the child listing into `fetch_type_of` (already planned in §12).
- Frame extraction is sequential. Ceiling: long first-time thumbnail runs. Upgrade path: semaphore (§16) and a background queue.
- No cache. Ceiling: repeated browses re-read directories. Upgrade path: cache keyed by directory `modified_ms`.
</content>
