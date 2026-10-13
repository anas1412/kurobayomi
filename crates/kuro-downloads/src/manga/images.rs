//! Image file naming, extension resolution, resume lookup, the integrity count, the reader cache hit and the tall-image
//! split (port of ImageUtil and the page helpers of MangaDownloader.kt).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use image::GenericImageView;
use kuro_core::image::{decode_rgba, dimensions, encode_jpeg_q100, is_animated_supported, kind_from_mime, sniff};
use tracing::debug;

use crate::naming::TMP_DIR_SUFFIX;
use super::source::{Page, PageState};

/// Page file names are at least this wide.
pub const MIN_PAGE_DIGITS: usize = 3;
/// Name of the ComicInfo file written into a chapter (ignored by the integrity count).
pub const COMIC_INFO_FILE: &str = "ComicInfo.xml";
pub const NOMEDIA_FILE: &str = ".nomedia";
const TMP_FILE_SUFFIX: &str = ".tmp";
/// Bytes read to decide the image type (`ImageUtil.getImageType`).
const HEAD_LEN: usize = 32;
/// A page is tall when `height / width` (integer division) is greater than this (EC-50, EC-51).
pub const TALL_RATIO: u32 = 3;
/// Optimal part height when no display size is known (`2 * 2048`). The GUI overrides it with `2 * max(monitor)`.
pub const DEFAULT_OPTIMAL_HEIGHT: u32 = 4096;
/// Decoded images above this size are not split (memory guard, pitfall 10).
pub const MAX_SPLIT_DECODED_BYTES: u64 = 256 * 1024 * 1024;

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

/// The reader page cache (area 34, `ChapterCache`). A hit is copied into the chapter, then the cache entry is removed.
pub trait PageImageCache: Send + Sync {
    /// The cached file for an image URL, if the cache holds one.
    fn cached_file(&self, image_url: &str) -> Option<PathBuf>;
    /// Removes the cache entry after its copy was renamed into the chapter (EC-47).
    fn remove(&self, image_url: &str);
}

/// Copies a cached image into the chapter as `stem.ext`, the extension taken from the bytes (EC-48). `Ok(None)` when
/// the type is not recognised: the temporary copy is removed and the caller downloads the page instead.
pub fn copy_from_cache(cached: &Path, tmp: &Path, stem: &str) -> io::Result<Option<PathBuf>> {
    let partial = tmp.join(format!("{stem}{TMP_FILE_SUFFIX}"));
    fs::copy(cached, &partial)?;
    let mut head = vec![0u8; HEAD_LEN];
    let read = fs::File::open(&partial)?.read(&mut head)?;
    head.truncate(read);
    match sniff(&head) {
        Some(found) => {
            let target = tmp.join(format!("{stem}.{}", found.kind.extension()));
            fs::rename(&partial, &target)?;
            Ok(Some(target))
        }
        None => {
            fs::remove_file(&partial)?;
            Ok(None)
        }
    }
}

/// `true` when `height / width > TALL_RATIO` with integer division, so `height >= 4 * width` (EC-50, EC-51).
pub fn is_tall(width: u32, height: u32) -> bool {
    width > 0 && height / width > TALL_RATIO
}

/// `true` when the decoded RGBA size is over `MAX_SPLIT_DECODED_BYTES`.
pub fn exceeds_split_memory(width: u32, height: u32) -> bool {
    u64::from(width) * u64::from(height) * 4 > MAX_SPLIT_DECODED_BYTES
}

/// One horizontal slice of a tall page: rows `top..bottom`, 0-based and `bottom` exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitPart {
    pub top: u32,
    pub bottom: u32,
}

/// Row ranges of a split (`ImageUtil.splitData`). The last part absorbs the rounding remainder.
pub fn split_geometry(height: u32, optimal: u32) -> Vec<SplitPart> {
    if height == 0 || optimal == 0 {
        return Vec::new();
    }
    let part_count = (height - 1) / optimal + 1;
    let optimal_split = height / part_count;
    let mut parts: Vec<SplitPart> = Vec::new();
    for index in 0..part_count {
        if parts.last().is_some_and(|last| height <= last.bottom) {
            break;
        }
        let top = index * optimal_split;
        let mut split_height = optimal_split.min(height - top);
        if index == part_count - 1 {
            split_height += height - (top + split_height);
        }
        parts.push(SplitPart { top, bottom: top + split_height });
    }
    parts
}

/// Name of one split part: `prefix__NNN.jpg`, 1-based and three digits (`%03d`).
pub fn split_part_name(prefix: &str, number: usize) -> String {
    format!("{prefix}__{number:03}.jpg")
}

/// Splits a tall page into `prefix__NNN.jpg` parts at JPEG quality 100, then removes the original (TST-28). Returns
/// `Ok(false)` when nothing was split: not tall, animated, too large to decode in memory, or the bytes did not decode.
/// On a failure midway the parts already written are removed and the original is kept.
pub fn split_tall_image(tmp: &Path, file: &Path, prefix: &str, optimal: u32) -> io::Result<bool> {
    let bytes = fs::read(file)?;
    let head = &bytes[..bytes.len().min(HEAD_LEN)];
    if is_animated_supported(sniff(head)) {
        return Ok(false);
    }
    let (width, height) = dimensions(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    if !is_tall(width, height) {
        return Ok(false);
    }
    if exceeds_split_memory(width, height) {
        debug!(target: "kuro_downloads", page = %prefix, "tall page not split: decoded size over 256 MiB");
        return Ok(false);
    }
    let img = decode_rgba(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let mut written: Vec<PathBuf> = Vec::new();
    let result: io::Result<()> = (|| {
        for (index, part) in split_geometry(height, optimal).into_iter().enumerate() {
            let target = tmp.join(split_part_name(prefix, index + 1));
            let _ = fs::remove_file(&target);
            written.push(target.clone());
            let slice = img.view(0, part.top, width, part.bottom - part.top).to_image();
            let jpeg = encode_jpeg_q100(&slice).map_err(|e| io::Error::other(e.to_string()))?;
            fs::write(&target, jpeg)?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            fs::remove_file(file)?;
            Ok(true)
        }
        Err(err) => {
            for part in &written {
                let _ = fs::remove_file(part);
            }
            Err(err)
        }
    }
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
    use crate::manga::source::Page;

    #[test]
    fn width_is_at_least_three_and_grows_with_the_page_count() {
        assert_eq!(page_digits(9), 3);
        assert_eq!(page_digits(999), 3);
        assert_eq!(page_digits(1000), 4);
        assert_eq!(page_digits(1234), 4);
        assert_eq!(page_stem(1, page_digits(1234)), "0001");
        assert_eq!(page_stem(9, page_digits(9)), "009");
    }

    // TST-21
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

#[cfg(test)]
mod split_tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(width, height, Rgba([200, 30, 30, 255]));
        let mut out = io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    // TST-22
    #[test]
    fn split_geometry_matches_kotlin() {
        // Behaviour 10.7: 5000 px at 2000 px gives three parts, the last one absorbs the remainder.
        assert_eq!(
            split_geometry(5000, 2000),
            vec![SplitPart { top: 0, bottom: 1666 }, SplitPart { top: 1666, bottom: 3332 }, SplitPart { top: 3332, bottom: 5000 }]
        );
        assert_eq!(split_geometry(4000, 2000), vec![SplitPart { top: 0, bottom: 2000 }, SplitPart { top: 2000, bottom: 4000 }]);
        assert_eq!(
            split_geometry(4001, 2000),
            vec![SplitPart { top: 0, bottom: 1333 }, SplitPart { top: 1333, bottom: 2666 }, SplitPart { top: 2666, bottom: 4001 }]
        );
        assert_eq!(split_geometry(2001, 2000), vec![SplitPart { top: 0, bottom: 1000 }, SplitPart { top: 1000, bottom: 2001 }]);
        // EC-52: exactly the optimal height is one part, so no split.
        assert_eq!(split_geometry(2000, 2000), vec![SplitPart { top: 0, bottom: 2000 }]);
        assert_eq!(split_geometry(1, 2000), vec![SplitPart { top: 0, bottom: 1 }]);
        assert!(split_geometry(0, 2000).is_empty());
    }

    #[test]
    fn parts_always_cover_the_page_without_gaps() {
        for height in 1..200u32 {
            for optimal in 1..50u32 {
                let parts = split_geometry(height, optimal);
                assert_eq!(parts.first().map(|p| p.top), Some(0), "h={height} o={optimal}");
                assert_eq!(parts.last().map(|p| p.bottom), Some(height), "h={height} o={optimal}");
                assert!(parts.windows(2).all(|w| w[0].bottom == w[1].top), "h={height} o={optimal}");
                assert_eq!(parts.len() as u32, (height - 1) / optimal + 1, "h={height} o={optimal}");
            }
        }
    }

    #[test]
    fn tall_rule_uses_integer_division() {
        assert!(is_tall(10, 40), "EC-50: exactly 4x the width");
        assert!(!is_tall(10, 39), "EC-51: 3.9x is 3 with integer division");
        assert!(!is_tall(0, 4000));
        assert!(!exceeds_split_memory(8192, 8192), "exactly 256 MiB is allowed");
        assert!(exceeds_split_memory(8193, 8193));
    }

    #[test]
    fn part_names_use_three_digit_part_numbers() {
        assert_eq!(split_part_name("001", 1), "001__001.jpg");
        assert_eq!(split_part_name("0001", 12), "0001__012.jpg");
    }

    #[test]
    fn tall_split_writes_jpeg_parts_and_removes_the_original() {
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("001.png");
        fs::write(&original, png(800, 4000)).unwrap();
        assert!(split_tall_image(tmp.path(), &original, "001", 2000).unwrap());
        assert!(!original.exists(), "original removed after all parts");
        for name in ["001__001.jpg", "001__002.jpg"] {
            let bytes = fs::read(tmp.path().join(name)).unwrap();
            assert_eq!(sniff(&bytes).map(|s| s.kind), Some(kuro_core::image::ImageKind::Jpeg));
            assert_eq!(dimensions(&bytes).unwrap().0, 800);
        }
        assert_eq!(dimensions(&fs::read(tmp.path().join("001__001.jpg")).unwrap()).unwrap().1, 2000);
        assert_eq!(dimensions(&fs::read(tmp.path().join("001__002.jpg")).unwrap()).unwrap().1, 2000);
    }

    #[test]
    fn a_page_that_is_not_tall_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("001.png");
        fs::write(&file, png(800, 1200)).unwrap();
        assert!(!split_tall_image(tmp.path(), &file, "001", 2000).unwrap());
        assert!(file.exists());
    }

    // TST-29
    #[test]
    fn animated_gif_is_not_split() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("001.gif");
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(10, 400, Rgba([0, 0, 0, 255])));
        let mut out = io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Gif).unwrap();
        fs::write(&file, out.into_inner()).unwrap();
        assert!(!split_tall_image(tmp.path(), &file, "001", 50).unwrap(), "EC-54");
        assert!(file.exists());
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }

    #[test]
    fn undecodable_bytes_are_an_error_and_keep_the_original() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("001.jpg");
        fs::write(&file, b"not an image at all").unwrap();
        assert!(split_tall_image(tmp.path(), &file, "001", 2000).is_err());
        assert!(file.exists());
    }

    #[test]
    fn cache_copy_takes_the_extension_from_the_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let cached = tmp.path().join("cache.bin");
        fs::write(&cached, png(4, 4)).unwrap();
        let chapter = tmp.path().join("chapter_tmp");
        fs::create_dir(&chapter).unwrap();
        let copied = copy_from_cache(&cached, &chapter, "001").unwrap();
        assert_eq!(copied, Some(chapter.join("001.png")));
        assert!(!chapter.join("001.tmp").exists());
    }

    #[test]
    fn cache_copy_of_an_unknown_type_leaves_no_tmp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cached = tmp.path().join("cache.bin");
        fs::write(&cached, b"junk").unwrap();
        let chapter = tmp.path().join("chapter_tmp");
        fs::create_dir(&chapter).unwrap();
        assert_eq!(copy_from_cache(&cached, &chapter, "001").unwrap(), None);
        assert_eq!(fs::read_dir(&chapter).unwrap().count(), 0, "EC-48: nothing left behind");
    }
}
