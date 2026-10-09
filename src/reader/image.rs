//! Image file rule and natural name order for page lists (`ImageUtil.kt`, `StringExtensions.kt`). T-43a-012.
//!
//! The extension list is the upstream one (`avif gif heif jpg jxl png webp`, case-sensitive). Any other name,
//! including `.jpeg` and upper-case extensions, is accepted when its first bytes sniff as a known image. Both
//! checks live in `kuro_core::image`, shared with the archive reader.

use std::cmp::Ordering;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use kuro_core::image::{ImageKind, is_image_name, sniff};
use kuro_core::text::natural_cmp;

/// Bytes read from a file to sniff its type (`HEAD_LEN` in the archive reader).
pub const HEAD_LEN: usize = 32;

/// `ImageUtil.isImage(name) { openStream }`: extension first, then magic bytes from `head`.
pub fn is_image_entry(name: &str, head: impl FnOnce() -> Option<Vec<u8>>) -> bool {
    is_image_name(name, head)
}

/// The first [`HEAD_LEN`] bytes of a file, `None` when it cannot be read or is empty.
pub fn read_head(path: &Path) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; HEAD_LEN];
    let mut filled = 0;
    while filled < HEAD_LEN {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(_) => return None,
        }
    }
    buf.truncate(filled);
    (filled > 0).then_some(buf)
}

/// Whether a file on disk is a page: its name passes the rule, reading the head only when needed.
pub fn is_image_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    is_image_entry(name, || read_head(path))
}

/// The image format of a byte head, found by magic bytes alone.
pub fn detect_kind(head: &[u8]) -> Option<ImageKind> {
    sniff(head).map(|s| s.kind)
}

/// Natural, case-insensitive order (`2.jpg < 10.jpg`, `PAGE 1 < Page 2 < page 10`).
pub fn natural_name_cmp(a: &str, b: &str) -> Ordering {
    natural_cmp(a, b)
}

/// Sorts page names into reading order. Stable, so equal names keep their input order.
pub fn sort_names(names: &mut [String]) {
    names.sort_by(|a, b| natural_name_cmp(a, b));
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    const JPEG: &[u8] = b"\xff\xd8\xff\xe0\x00\x10JFIF";
    const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00";
    const WEBP: &[u8] = b"RIFF\x24\x00\x00\x00WEBPVP8 ";
    const AVIF: &[u8] = b"\x00\x00\x00\x1cftypavif\x00\x00\x00\x00";
    const HEIF: &[u8] = b"\x00\x00\x00\x18ftypheic\x00\x00\x00\x00";
    const JXL: &[u8] = b"\0\0\0\x0cJXL \r\n\x87\n\0\0\0\x1f";

    #[test]
    fn extension_list_is_accepted_without_reading_the_head() {
        for name in ["a.avif", "a.gif", "a.heif", "a.jpg", "a.jxl", "a.png", "a.webp"] {
            assert!(is_image_entry(name, || panic!("head must not be read for {name}")), "{name}");
        }
    }

    #[test]
    fn jpeg_and_upper_case_need_the_magic_bytes() {
        assert!(!is_image_entry("page.jpeg", || None));
        assert!(is_image_entry("page.jpeg", || Some(JPEG.to_vec())));
        assert!(!is_image_entry("PAGE.JPG", || Some(b"not an image".to_vec())));
        assert!(is_image_entry("PAGE.JPG", || Some(PNG.to_vec())));
        assert!(!is_image_entry("notes.txt", || Some(b"hello".to_vec())));
    }

    #[test]
    fn magic_bytes_identify_each_supported_format() {
        assert_eq!(detect_kind(PNG), Some(ImageKind::Png));
        assert_eq!(detect_kind(JPEG), Some(ImageKind::Jpeg));
        assert_eq!(detect_kind(WEBP), Some(ImageKind::Webp));
        assert_eq!(detect_kind(GIF), Some(ImageKind::Gif));
        assert_eq!(detect_kind(AVIF), Some(ImageKind::Avif));
        assert_eq!(detect_kind(HEIF), Some(ImageKind::Heif));
        assert_eq!(detect_kind(JXL), Some(ImageKind::Jxl));
        assert_eq!(detect_kind(b"plain text"), None);
    }

    #[test]
    fn natural_order_ignores_case_and_compares_digit_runs_numerically() {
        let mut names: Vec<String> = ["10.jpg", "2.jpg", "PAGE 1", "page 10", "Page 2"].map(String::from).to_vec();
        sort_names(&mut names);
        assert_eq!(names, ["2.jpg", "10.jpg", "PAGE 1", "Page 2", "page 10"]);
        assert_eq!(natural_name_cmp("2.jpg", "10.jpg"), Ordering::Less);
        assert_eq!(natural_name_cmp("PAGE 1", "Page 2"), Ordering::Less);
        assert_eq!(natural_name_cmp("Page 2", "page 10"), Ordering::Less);
    }
}
