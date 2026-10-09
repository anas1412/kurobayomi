//! Image file naming, extension resolution, resume lookup and the integrity count (port of ImageUtil and the page
//! helpers of MangaDownloader.kt). The tall-image split (T-23a-018) is not here yet.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kuro_core::image::{kind_from_mime, sniff};

use crate::layout::TMP_DIR_SUFFIX;
use crate::model::{Page, PageState};

/// Page file names are at least this wide.
pub const MIN_PAGE_DIGITS: usize = 3;
/// Name of the ComicInfo file written into a chapter (ignored by the integrity count).
pub const COMIC_INFO_FILE: &str = "ComicInfo.xml";
pub const NOMEDIA_FILE: &str = ".nomedia";
const TMP_FILE_SUFFIX: &str = ".tmp";

/// Width of the page number in file names: `max(3, digits of the page count)`, so page 1 of a 1000-page chapter
/// never shares a prefix with page 10.
pub fn page_digits(page_count: usize) -> usize {
    MIN_PAGE_DIGITS.max(page_count.to_string().len())
}

/// The file stem for a 1-based page number, zero-padded to `width`.
pub fn page_stem(number: u32, width: usize) -> String {
    format!("{number:0width$}")
}

/// Extension of a fetched image. Sniffed bytes win over the Content-Type (EC-42). Then the Content-Type. Then `jpg`.
pub fn resolve_extension(content_type: Option<&str>, head: &[u8]) -> &'static str {
    if let Some(sniffed) = sniff(head) {
        return sniffed.kind.extension();
    }
    let mime = content_type.map(|c| c.split(';').next().unwrap_or_default().trim().to_ascii_lowercase());
    mime.as_deref().and_then(kind_from_mime).map_or("jpg", |k| k.extension())
}

/// The page number a file name stands for: `NNN.ext`, or `NNN__001.ext` (the first split part). `None` for every
/// other name, including temporary `.tmp` files, `ComicInfo.xml` and `.nomedia`.
pub fn page_key_of(file_name: &str) -> Option<u32> {
    let (base, ext) = file_name.rsplit_once('.')?;
    if ext.is_empty() || ext.eq_ignore_ascii_case("tmp") {
        return None;
    }
    let (number, part) = match base.split_once("__") {
        Some((number, part)) => (number, Some(part)),
        None => (base, None),
    };
    if part.is_some_and(|p| p != "001") {
        return None;
    }
    if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    number.parse().ok()
}

/// A finished, non-empty file for the page stem: `stem.ext` or `stem__001.ext`. Zero-length files are treated as
/// missing (EC-46).
pub fn find_finished(tmp: &Path, stem: &str) -> Option<PathBuf> {
    let wanted: u32 = stem.parse().ok()?;
    let entries = fs::read_dir(tmp).ok()?;
    entries.flatten().map(|e| e.path()).find(|path| {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return false };
        let prefixed = name.starts_with(&format!("{stem}.")) || name.starts_with(&format!("{stem}__001."));
        prefixed
            && page_key_of(name) == Some(wanted)
            && path.metadata().is_ok_and(|m| m.is_file() && m.len() > 0)
    })
}

/// Counts the finished page files in `tmp`, one entry per page number in `1..=pages`. Temporary files, ComicInfo,
/// `.nomedia`, zero-length files and stray names are not counted (Proposal, EC-60, EC-61).
pub fn page_key_counts(tmp: &Path) -> io::Result<BTreeMap<u32, usize>> {
    let mut counts = BTreeMap::new();
    for entry in fs::read_dir(tmp)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == COMIC_INFO_FILE || name == NOMEDIA_FILE || name.ends_with(TMP_FILE_SUFFIX) {
            continue;
        }
        let Some(number) = page_key_of(&name) else { continue };
        if entry.metadata()?.len() == 0 {
            continue;
        }
        *counts.entry(number).or_insert(0) += 1;
    }
    Ok(counts)
}

/// `true` when every page is `Ready` and the folder holds exactly one page key for every page number `1..=n`
/// (behaviour 10.8, with keys instead of a raw file count).
pub fn is_complete(tmp: &Path, pages: &[Page]) -> io::Result<bool> {
    if pages.is_empty() || pages.iter().any(|p| p.state != PageState::Ready) {
        return Ok(false);
    }
    let counts = page_key_counts(tmp)?;
    let exact = (1..=pages.len() as u32).all(|n| counts.get(&n) == Some(&1));
    Ok(exact)
}

/// The `.tmp` folder name for a chapter folder name.
pub fn tmp_dir_name(chapter_dir: &str) -> String {
    format!("{chapter_dir}{TMP_DIR_SUFFIX}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Page;

    #[test]
    fn width_is_at_least_three_and_grows_with_the_page_count() {
        assert_eq!(page_digits(9), 3);
        assert_eq!(page_digits(999), 3);
        assert_eq!(page_digits(1000), 4);
        assert_eq!(page_digits(1234), 4);
        assert_eq!(page_stem(1, page_digits(1234)), "0001");
        assert_eq!(page_stem(9, page_digits(9)), "009");
    }

    #[test]
    fn extension_follows_sniffing_then_content_type_then_jpg() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0];
        let png = [0x89, b'P', b'N', b'G', 0, 0, 0, 0];
        // EC-42: the header says webp but the body is a JPEG: the bytes win.
        assert_eq!(resolve_extension(Some("image/webp"), &jpeg), "jpg");
        // EC-43: no Content-Type, PNG body.
        assert_eq!(resolve_extension(None, &png), "png");
        // EC-44: unrecognised body and no header: jpg fallback.
        assert_eq!(resolve_extension(None, b"junk-bytes"), "jpg");
        // Header decides when the body is not recognisable.
        assert_eq!(resolve_extension(Some("image/gif; charset=x"), b"junk"), "gif");
        assert_eq!(resolve_extension(Some("image/avif"), b""), "avif");
    }

    #[test]
    fn page_keys_accept_plain_and_first_part_names_only() {
        assert_eq!(page_key_of("001.jpg"), Some(1));
        assert_eq!(page_key_of("0010.jpg"), Some(10));
        assert_eq!(page_key_of("001__001.jpg"), Some(1));
        assert_eq!(page_key_of("001__002.jpg"), None);
        assert_eq!(page_key_of("001.tmp"), None);
        assert_eq!(page_key_of("ComicInfo.xml"), None);
        assert_eq!(page_key_of(".nomedia"), None);
        assert_eq!(page_key_of("abc.jpg"), None);
    }

    #[test]
    fn resume_finds_finished_files_and_skips_empty_and_tmp_ones() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("001.jpg"), b"img").unwrap();
        fs::write(tmp.path().join("002.png"), b"").unwrap();
        fs::write(tmp.path().join("003.tmp"), b"partial").unwrap();
        fs::write(tmp.path().join("0010.jpg"), b"other page").unwrap();
        assert_eq!(find_finished(tmp.path(), "001"), Some(tmp.path().join("001.jpg")));
        assert_eq!(find_finished(tmp.path(), "002"), None, "zero length is missing");
        assert_eq!(find_finished(tmp.path(), "003"), None);
        assert_eq!(find_finished(tmp.path(), "004"), None);
        fs::write(tmp.path().join("005__001.webp"), b"split").unwrap();
        assert_eq!(find_finished(tmp.path(), "005"), Some(tmp.path().join("005__001.webp")));
    }

    #[test]
    fn integrity_counts_page_keys_not_files() {
        let tmp = tempfile::tempdir().unwrap();
        let pages: Vec<Page> = (0..3)
            .map(|i| {
                let mut p = Page::new(i, "u", None);
                p.state = PageState::Ready;
                p
            })
            .collect();
        for n in ["001.jpg", "002.jpg", "003.png", "ComicInfo.xml", ".nomedia", "003.tmp"] {
            fs::write(tmp.path().join(n), b"x").unwrap();
        }
        assert!(is_complete(tmp.path(), &pages).unwrap(), "ignored names do not count (TST-23)");
        fs::write(tmp.path().join("notes.txt"), b"stray").unwrap();
        assert!(is_complete(tmp.path(), &pages).unwrap(), "stray names do not break the count (EC-60)");
        fs::remove_file(tmp.path().join("002.jpg")).unwrap();
        assert!(!is_complete(tmp.path(), &pages).unwrap(), "a missing page fails");
    }

    #[test]
    fn integrity_fails_when_a_page_is_not_ready() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("001.jpg"), b"x").unwrap();
        let mut page = Page::new(0, "u", None);
        page.state = PageState::Error;
        assert!(!is_complete(tmp.path(), &[page]).unwrap());
    }
}
