//! `ArchiveReader`: opens a memory-mapped archive, lists its entries and streams one entry by name.
//!
//! Listing opens its own libarchive handle over the shared mapping. A zip archive also gets its
//! central directory parsed once at `open` (T-43a-009), so `open_entry` finds an entry by name
//! without scanning headers. Port of `core/archive/.../ArchiveReader.kt` and the page filter of
//! `ArchivePageLoader.kt` (T-13a-031, T-13a-032, T-43a-009).

use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use kuro_core::image::is_image_name;
use kuro_core::text::natural_cmp;
use memmap2::Mmap;
use zip::ZipArchive;

use crate::libarchive::{Handle, Header};

/// Bytes read from an entry for the sniff of a name without an image extension (`HEAD_LEN`).
const SNIFF_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// The archive cannot be mapped or listed (missing, empty, truncated or not an archive).
    #[error("cannot open archive {}: {reason}", path.display())]
    Open { path: PathBuf, reason: String },
    /// libarchive failed while an entry was being scanned.
    #[error("libarchive: {0}")]
    Backend(String),
    /// An EPUB package document named by the container (or the `OEBPS/content.opf` fallback) is
    /// not in the archive.
    #[error("epub package missing: {0}")]
    PackageMissing(String),
}

/// One entry of an archive listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    pub name: String,
    pub is_file: bool,
}

/// A read-only memory-mapped archive.
pub struct ArchiveReader {
    map: Arc<Mmap>,
    path: PathBuf,
    /// Zip central directory, parsed once at `open`; `None` when the archive is not a zip that the
    /// `zip` crate reads. Its name index gives the position of an entry without a header scan.
    zip: Option<Mutex<ZipArchive<Cursor<MapBytes>>>>,
}

/// The mapping as bytes for the `zip` crate. Shares the `Arc`, so the reader holds one mapping.
struct MapBytes(Arc<Mmap>);

impl AsRef<[u8]> for MapBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Streams the bytes of one entry. A zip entry is decoded into memory once; any other entry is
/// streamed from its own libarchive handle, which is freed on drop.
pub struct EntryReader {
    bytes: EntryBytes,
}

enum EntryBytes {
    Buffered(Cursor<Vec<u8>>),
    Stream(Handle),
}

impl Read for EntryReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.bytes {
            EntryBytes::Buffered(cursor) => cursor.read(buf),
            EntryBytes::Stream(handle) => handle.read_data(buf),
        }
    }
}

impl ArchiveReader {
    /// Maps `path` read-only and, for a zip, parses its central directory. A missing or empty file
    /// is an `Open` error.
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        let map = map_file(path).map_err(|e| ArchiveError::Open {
            path: path.to_owned(),
            reason: e.to_string(),
        })?;
        let map = Arc::new(map);
        let zip = ZipArchive::new(Cursor::new(MapBytes(Arc::clone(&map))))
            .ok()
            .map(Mutex::new);
        Ok(Self {
            map,
            path: path.to_owned(),
            zip,
        })
    }

    /// Lists every entry in archive order. Nameless entries and names that are not UTF-8 are
    /// skipped. Any libarchive failure while listing is an `Open` error (for example a
    /// truncated zip without its central directory).
    pub fn entries(&self) -> Result<Vec<EntryInfo>, ArchiveError> {
        let mut handle = self.open_listing()?;
        let mut out = Vec::new();
        while let Some(header) = self.next_listed(&mut handle)? {
            if let Some(name) = header.name {
                out.push(EntryInfo {
                    name,
                    is_file: header.is_file,
                });
            }
        }
        Ok(out)
    }

    /// Image pages in reading order: file entries whose name has an image extension (exact,
    /// case-sensitive) or whose first 32 bytes sniff as an image, sorted by the natural
    /// comparator. The sniff reads the entry from the same handle as the listing, so the archive
    /// is scanned once.
    pub fn page_entries(&self) -> Result<Vec<EntryInfo>, ArchiveError> {
        let mut handle = self.open_listing()?;
        let mut pages = Vec::new();
        while let Some(header) = self.next_listed(&mut handle)? {
            let Some(name) = header.name else { continue };
            if !header.is_file {
                continue;
            }
            if is_image_name(&name, || head(&mut handle, SNIFF_LEN)) {
                pages.push(EntryInfo {
                    name,
                    is_file: true,
                });
            }
        }
        pages.sort_by(|a, b| natural_cmp(&a.name, &b.name));
        Ok(pages)
    }

    /// Opens the entry whose name equals `name` exactly (case-sensitive, no normalisation).
    /// `Ok(None)` when no entry matches.
    ///
    /// A zip entry found in the central-directory index is decoded directly, so a page costs one
    /// lookup and one decode, not a header scan. Anything else (other formats, a name the index
    /// does not hold, an entry the index cannot read) is answered by a scan from the first header,
    /// so libarchive's own name rules still decide the result.
    ///
    // Decision (E-ARC-1): the once-built name index covers zip/cbz only. libarchive has no seek
    // for rar, 7z or tar (solid or streamed formats), so those open a fresh handle and scan
    // headers per page; a chapter is O(n^2) headers for them. Accepted: the mapping is shared and
    // read-only and chapter archives are small.
    // ponytail: index keys are the names the zip crate reports; libarchive normalises some names
    // differently (backslash separators), so those misses scan too. Upgrade path for non-zip:
    // cache the decoded bytes of the most recent entry, or extract the chapter once.
    pub fn open_entry(&self, name: &str) -> Result<Option<EntryReader>, ArchiveError> {
        if let Some(bytes) = self.zip.as_ref().and_then(|zip| read_zip_entry(zip, name)) {
            return Ok(Some(EntryReader {
                bytes: EntryBytes::Buffered(Cursor::new(bytes)),
            }));
        }
        self.scan_entry(name)
    }

    /// Scans headers from the first one until `name` matches. The handle is freed when no match
    /// is found.
    fn scan_entry(&self, name: &str) -> Result<Option<EntryReader>, ArchiveError> {
        let mut handle = Handle::open(Arc::clone(&self.map)).map_err(ArchiveError::Backend)?;
        while let Some(header) = handle.next_header().map_err(ArchiveError::Backend)? {
            if header.name.as_deref() == Some(name) {
                return Ok(Some(EntryReader {
                    bytes: EntryBytes::Stream(handle),
                }));
            }
        }
        Ok(None)
    }

    fn open_listing(&self) -> Result<Handle, ArchiveError> {
        Handle::open(Arc::clone(&self.map)).map_err(|reason| self.open_error(reason))
    }

    fn next_listed(&self, handle: &mut Handle) -> Result<Option<Header>, ArchiveError> {
        handle
            .next_header()
            .map_err(|reason| self.open_error(reason))
    }

    fn open_error(&self, reason: String) -> ArchiveError {
        ArchiveError::Open {
            path: self.path.clone(),
            reason,
        }
    }
}

/// Decodes the zip entry `name` through the central-directory index. `None` when the index does
/// not hold the name or cannot read it. The lock is held only for this call.
fn read_zip_entry(zip: &Mutex<ZipArchive<Cursor<MapBytes>>>, name: &str) -> Option<Vec<u8>> {
    let mut archive = zip.lock().unwrap_or_else(PoisonError::into_inner);
    let index = archive.index_for_name(name)?;
    let mut file = archive.by_index(index).ok()?;
    // No capacity hint: the size comes from the archive and is not trusted for an allocation.
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// Reads up to `len` bytes of the current entry. `None` on a read error, or when no byte is read.
fn head(handle: &mut Handle, len: usize) -> Option<Vec<u8>> {
    let mut buf = vec![0u8; len];
    let mut filled = 0;
    while filled < len {
        match handle.read_data(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    buf.truncate(filled);
    (filled > 0).then_some(buf)
}

/// Maps a regular file read-only. `memmap2` marks the call unsafe because the file must not be
/// truncated while mapped; archives are only read here, never written in place.
#[allow(unsafe_code)]
fn map_file(path: &Path) -> io::Result<Mmap> {
    let file = File::open(path)?;
    // memmap2 maps a zero-length file without error on Linux, so reject it here.
    if file.metadata()?.len() == 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file is empty"));
    }
    // SAFETY: the mapping is read-only and Kurobayomi never writes to an archive it is reading.
    // Truncating the file underneath a live reader would fault on access, which is the usual
    // memmap2 caveat and is accepted for local archives.
    unsafe { Mmap::map(&file) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn open(name: &str) -> ArchiveReader {
        ArchiveReader::open(&fixture(name)).unwrap()
    }

    fn names(entries: &[EntryInfo]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// The bytes the fixture generator writes for an entry of `total` bytes with `magic` first.
    fn expected(magic: &[u8], total: usize) -> Vec<u8> {
        let mut v = magic.to_vec();
        v.extend((0..total - magic.len()).map(|i| ((i * 31) % 251) as u8));
        v
    }

    const JPEG: &[u8] = b"\xff\xd8\xff\xe0";

    fn read_all(mut r: EntryReader) -> Vec<u8> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 7];
        loop {
            let n = r.read(&mut chunk).unwrap();
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
        }
        out
    }

    #[test]
    fn stored_zip_lists_entries_in_archive_order() {
        let entries = open("stored.zip").entries().unwrap();
        assert_eq!(
            names(&entries),
            ["10.jpg", "2.jpg", "1.jpg", "002.PNG", "notes.txt", "dir/"]
        );
        assert!(entries[0].is_file);
        assert!(
            !entries.last().unwrap().is_file,
            "directory entry is not a file"
        );
    }

    #[test]
    fn deflate_cbz_lists_entries() {
        let entries = open("deflate.cbz").entries().unwrap();
        assert_eq!(names(&entries), ["1.jpg", "2.jpg", "notes.txt"]);
        assert!(entries.iter().all(|e| e.is_file));
    }

    #[test]
    fn rar_cbr_lists_entries() {
        let entries = open("sample.cbr").entries().unwrap();
        assert_eq!(names(&entries), ["1.jpg", "2.jpg"]);
        assert!(entries.iter().all(|e| e.is_file));
    }

    #[test]
    fn seven_zip_and_tar_list_entries() {
        for name in ["sample.7z", "sample.tar", "sample.tar.gz"] {
            let mut listed: Vec<String> = open(name)
                .entries()
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            listed.sort();
            assert_eq!(listed, ["1.jpg", "10.jpg", "2.jpg"], "{name}");
        }
    }

    #[test]
    fn page_filter_sniffs_uppercase_png_and_drops_text_and_directories() {
        let pages = open("stored.zip").page_entries().unwrap();
        // Natural order: 1 < 2 < 002.PNG (value 2, then "j" < "p") < 10.
        assert_eq!(names(&pages), ["1.jpg", "2.jpg", "002.PNG", "10.jpg"]);
    }

    #[test]
    fn page_filter_gives_natural_order_for_numbered_pages() {
        let pages = open("sample.7z").page_entries().unwrap();
        assert_eq!(names(&pages), ["1.jpg", "2.jpg", "10.jpg"]);
        let pages = open("sample.tar").page_entries().unwrap();
        assert_eq!(names(&pages), ["1.jpg", "2.jpg", "10.jpg"]);
    }

    #[test]
    fn open_entry_returns_the_exact_entry_bytes() {
        let reader = open("deflate.cbz");
        let bytes = read_all(reader.open_entry("1.jpg").unwrap().expect("1.jpg present"));
        assert_eq!(bytes, expected(JPEG, 20_000));
        let bytes = read_all(reader.open_entry("2.jpg").unwrap().expect("2.jpg present"));
        assert_eq!(bytes, expected(JPEG, 96));
    }

    #[test]
    fn open_entry_reads_stored_and_rar_entries() {
        let bytes = read_all(open("stored.zip").open_entry("10.jpg").unwrap().unwrap());
        assert_eq!(bytes, expected(JPEG, 50));
        let bytes = read_all(open("sample.cbr").open_entry("2.jpg").unwrap().unwrap());
        assert_eq!(bytes, expected(JPEG, 120));
    }

    #[test]
    fn open_entry_is_exact_and_case_sensitive() {
        let reader = open("stored.zip");
        assert!(reader.open_entry("1.JPG").unwrap().is_none());
        assert!(reader.open_entry("1.jp").unwrap().is_none());
        assert!(reader.open_entry("missing.jpg").unwrap().is_none());
    }

    #[test]
    fn open_entry_does_not_fold_case_for_a_single_lowercase_entry() {
        let reader = open("case.tar");
        assert!(reader.open_entry("A.jpg").unwrap().is_none());
        let bytes = read_all(reader.open_entry("a.jpg").unwrap().expect("a.jpg present"));
        assert_eq!(bytes, expected(JPEG, 64));
    }

    #[test]
    fn a_thousand_missing_lookups_return_none() {
        let reader = open("stored.zip");
        for _ in 0..1000 {
            assert!(reader.open_entry("missing.jpg").unwrap().is_none());
        }
    }

    #[test]
    fn missing_archive_is_an_open_error() {
        let err = ArchiveReader::open(&fixture("does-not-exist.cbz"))
            .err()
            .unwrap();
        assert!(matches!(err, ArchiveError::Open { .. }), "{err}");
    }

    #[test]
    fn empty_file_is_an_open_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.cbz");
        std::fs::write(&path, b"").unwrap();
        let err = ArchiveReader::open(&path).err().unwrap();
        assert!(matches!(err, ArchiveError::Open { .. }), "{err}");
    }

    #[test]
    fn truncated_zip_is_an_open_error_not_a_panic() {
        let reader = open("truncated.zip");
        assert!(matches!(reader.entries(), Err(ArchiveError::Open { .. })));
        assert!(matches!(
            reader.page_entries(),
            Err(ArchiveError::Open { .. })
        ));
    }

    #[test]
    fn eight_threads_list_the_same_archive_identically() {
        let reader = open("deflate.cbz");
        let expected = reader.entries().unwrap();
        thread::scope(|s| {
            let handles: Vec<_> = (0..8)
                .map(|_| s.spawn(|| reader.entries().unwrap()))
                .collect();
            for h in handles {
                assert_eq!(h.join().unwrap(), expected);
            }
        });
    }

    #[test]
    fn only_zip_archives_get_a_central_directory_index() {
        assert!(open("deflate.cbz").zip.is_some());
        assert!(open("stored.zip").zip.is_some());
        assert!(open("sample.cbr").zip.is_none());
        assert!(open("sample.7z").zip.is_none());
        assert!(open("sample.tar").zip.is_none());
    }

    #[test]
    fn indexed_zip_lookup_matches_the_header_scan_for_every_entry() {
        for name in ["stored.zip", "deflate.cbz"] {
            let reader = open(name);
            for entry in reader.entries().unwrap() {
                let indexed = reader.open_entry(&entry.name).unwrap();
                let scanned = reader.scan_entry(&entry.name).unwrap();
                match (indexed, scanned) {
                    (Some(a), Some(b)) => {
                        assert_eq!(read_all(a), read_all(b), "{name}: {}", entry.name);
                    }
                    (None, None) => {}
                    _ => panic!("{name}: index and scan disagree on {}", entry.name),
                }
            }
        }
    }

    #[test]
    fn reader_is_send_sync_and_entry_stream_is_send() {
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_send<T: Send>() {}
        assert_send_sync::<ArchiveReader>();
        assert_send::<EntryReader>();
    }
}
