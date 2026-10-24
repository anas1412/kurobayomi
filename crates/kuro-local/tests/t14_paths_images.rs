//! Area 14 behavioural tests (edge-case file section 5.1, 5.2, 5.11, 5.13): names, formats, paths and images.

use std::fs;
use std::path::Path;

use kuro_local::format::{Format, format_of, is_anime_episode, is_manga_archive};
use kuro_local::fs::{find_child, list_dir, safe_join};
use kuro_local::images::{
    ImageType, atomic_write, existing_thumbnail, find_image, set_cover, sniff, thumbnail_file_name, write_image,
    write_thumbnail,
};
use kuro_local::names::{extension, stem};

fn dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("temp dir")
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\0\x10JFIF";

#[test]
fn t_14_names_extension_no_dot() {
    assert_eq!(extension("cover"), "cover");
    assert_eq!(stem("cover"), "cover");
    assert_eq!(extension(".nomedia"), "nomedia");
    assert_eq!(stem("a.b.cbz"), "a.b");
    assert_eq!(extension("a.b.cbz"), "cbz");
}

#[test]
fn t_14_format_manga_archive_case_insensitive() {
    assert!(is_manga_archive("Vol.CBZ"));
    assert!(is_manga_archive("vol.cbz"));
    assert!(is_manga_archive("Vol.Cb7"));
    assert!(!is_manga_archive("Vol.mkv"));
}

#[test]
fn t_14_format_anime_case_insensitive() {
    assert!(is_anime_episode("Ep.mkv"));
    assert!(is_anime_episode("Ep.MKV"));
    assert!(!is_anime_episode("Ep.torrent"));
}

#[test]
fn t_14_format_epub_case_insensitive() {
    let d = dir();
    let book = d.path().join("Book.EPUB");
    fs::write(&book, b"x").unwrap();
    assert_eq!(format_of(&book, false), Some(Format::Epub(book)));
}

#[test]
fn t_14_format_dir_wins() {
    let d = dir();
    let book = d.path().join("Book.epub");
    fs::create_dir(&book).unwrap();
    assert_eq!(format_of(&book, true), Some(Format::Directory(book)));
}

#[test]
fn t_14_format_order() {
    let d = dir();
    let file = d.path().join("archive.epub");
    fs::write(&file, b"x").unwrap();
    assert!(matches!(format_of(&file, false), Some(Format::Epub(_))), "an .epub file is an EPUB, not an archive");
}

#[test]
fn t_14_format_unknown_type_invalid() {
    let d = dir();
    let file = d.path().join("notes.txt");
    fs::write(&file, b"x").unwrap();
    assert_eq!(format_of(&file, false), None);
}

#[test]
fn t_14_fs_safe_join_accepts_nested() {
    let base = Path::new("/root");
    assert_eq!(safe_join(base, "Show/Season 1/ep.mkv"), Some(base.join("Show").join("Season 1").join("ep.mkv")));
}

#[test]
fn t_14_path_rejects_traversal() {
    let base = Path::new("/root");
    assert_eq!(safe_join(base, "../x"), None);
    assert_eq!(safe_join(base, "Show/../../x"), None);
    assert_eq!(safe_join(base, "./x"), None);
}

#[test]
fn t_14_path_rejects_absolute() {
    assert_eq!(safe_join(Path::new("/root"), "/etc/passwd"), None);
}

#[test]
fn t_14_path_rejects_empty_component() {
    let base = Path::new("/root");
    assert_eq!(safe_join(base, ""), None);
    assert_eq!(safe_join(base, "a//b"), None);
    assert_eq!(safe_join(base, "a/"), None);
}

#[test]
fn t_14_path_rejects_nul() {
    assert_eq!(safe_join(Path::new("/root"), "a\0b"), None);
}

// a backslash is a path separator on Windows, not part of a name
#[cfg(unix)]
#[test]
fn t_14_path_backslash_name_listed_not_addressable() {
    let d = dir();
    fs::write(d.path().join("a\\b.cbz"), b"x").unwrap();
    let names: Vec<String> = list_dir(d.path()).unwrap().into_iter().map(|e| e.name).collect();
    assert!(names.iter().any(|n| n == "a\\b.cbz"), "the file is listed");
    assert_eq!(safe_join(d.path(), "a\\b.cbz"), None, "a backslash name is never addressed by URL");
}

#[cfg(unix)]
#[test]
fn t_14_path_non_utf8_listed_lossy() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    let d = dir();
    fs::write(d.path().join(OsStr::from_bytes(b"bad\xFFname.cbz")), b"x").unwrap();
    let names: Vec<String> = list_dir(d.path()).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names.len(), 1);
    assert!(names[0].contains('\u{FFFD}'), "the name is shown with replacement characters: {:?}", names[0]);
}

#[cfg(unix)]
#[test]
fn t_14_path_symlink_inside_root_listed() {
    let d = dir();
    let target = d.path().join("Real");
    fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, d.path().join("Link")).unwrap();
    let link = list_dir(d.path()).unwrap().into_iter().find(|e| e.name == "Link").expect("the link is listed");
    assert!(link.is_dir, "a link to a folder counts as a folder");
    assert_eq!(find_child(d.path(), "Link"), Some(d.path().join("Link")));
}

#[test]
fn t_14_images_sniff_table() {
    let webp = b"RIFF\x00\x00\x00\x00WEBPVP8 ";
    let avif = b"\x00\x00\x00\x20ftypavif\x00\x00\x00\x00";
    let heic = b"\x00\x00\x00\x18ftypheic\x00\x00\x00\x00";
    let jxl_codestream = b"\xFF\x0A\x00\x00\x00\x00\x00\x00";
    let jxl_container = b"\x00\x00\x00\x0C\x4A\x58\x4C\x20\x0D\x0A\x87\x0A\x00\x00\x00\x00";
    let cases: [(&[u8], ImageType); 8] = [
        (PNG, ImageType::Png),
        (JPEG, ImageType::Jpeg),
        (b"GIF87a\x01\x00", ImageType::Gif),
        (b"GIF89a\x01\x00", ImageType::Gif),
        (webp, ImageType::Webp),
        (avif, ImageType::Avif),
        (heic, ImageType::Heif),
        (jxl_codestream, ImageType::Jxl),
    ];
    for (bytes, kind) in cases {
        assert_eq!(sniff(bytes).map(|s| s.kind), Some(kind), "{bytes:?}");
    }
    assert_eq!(sniff(jxl_container).map(|s| s.kind), Some(ImageType::Jxl));
    assert_eq!(sniff(b"random bytes that are no image"), None);
}

#[test]
fn t_14_images_is_image_named_ext_first() {
    // The closure must not be called for a name whose extension already says image.
    assert!(kuro_local::images::is_image_name("cover.jpg", || panic!("the bytes are not read")));
}

#[test]
fn t_14_images_find_first_sorted() {
    let d = dir();
    fs::write(d.path().join("cover.png"), PNG).unwrap();
    fs::write(d.path().join("COVER.jpg"), JPEG).unwrap();
    // Name order decides the tie: "COVER.jpg" sorts before "cover.png".
    assert_eq!(find_image(d.path(), "cover"), Some(d.path().join("COVER.jpg")));
}

#[test]
fn t_14_images_overwrite_keeps_name() {
    let d = dir();
    fs::write(d.path().join("cover.png"), PNG).unwrap();
    let written = write_image(d.path(), "cover", "cover.jpg", JPEG).unwrap();
    assert_eq!(written, d.path().join("cover.png"), "the existing name and extension are kept");
    assert_eq!(fs::read(d.path().join("cover.png")).unwrap(), JPEG);
}

#[test]
fn t_14_images_nomedia_once() {
    let d = dir();
    write_image(d.path(), "cover", "cover.jpg", JPEG).unwrap();
    fs::write(d.path().join(".nomedia"), b"kept").unwrap();
    write_image(d.path(), "cover", "cover.jpg", JPEG).unwrap();
    assert_eq!(fs::read(d.path().join(".nomedia")).unwrap(), b"kept", "an existing .nomedia is not touched");
}

#[test]
fn t_14_images_write_atomic_rename() {
    let d = dir();
    let target = d.path().join("cover.jpg");
    atomic_write(&target, JPEG).unwrap();
    assert_eq!(fs::read(&target).unwrap(), JPEG);
    let names: Vec<String> = list_dir(d.path()).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["cover.jpg"], "no temporary file is left behind");
}

#[test]
fn t_14_images_background_case_insensitive() {
    let d = dir();
    fs::write(d.path().join("BACKGROUND.PNG"), PNG).unwrap();
    assert_eq!(find_image(d.path(), "background"), Some(d.path().join("BACKGROUND.PNG")));
}

#[test]
fn t_14_images_uppercase_ext_via_sniff() {
    let d = dir();
    fs::write(d.path().join("cover.JPG"), JPEG).unwrap();
    assert_eq!(find_image(d.path(), "cover"), Some(d.path().join("cover.JPG")));
}

#[test]
fn t_14_images_jpeg_extension_sniffed() {
    let d = dir();
    fs::write(d.path().join("cover.jpeg"), JPEG).unwrap();
    assert_eq!(find_image(d.path(), "cover"), Some(d.path().join("cover.jpeg")));
}

#[test]
fn t_14_images_no_validation() {
    let d = dir();
    let path = write_image(d.path(), "cover", "cover.jpg", b"not an image").unwrap();
    assert_eq!(fs::read(path).unwrap(), b"not an image", "bytes are written without a check");
}

#[test]
fn t_14_thumbnail_write_overwrites_same_name() {
    let d = dir();
    write_thumbnail(d.path(), "EP 01", b"first").unwrap();
    let url = write_thumbnail(d.path(), "EP 01", b"second").unwrap();
    assert!(url.starts_with("file://") && url.ends_with("-thumbnail.jpg"), "{url}");
    assert_eq!(fs::read(d.path().join("EP 01-thumbnail.jpg")).unwrap(), b"second");
}

#[test]
fn t_14_thumbnail_name_sanitized() {
    assert_eq!(thumbnail_file_name("EP: 1?"), "EP_ 1_-thumbnail.jpg");
    assert_eq!(thumbnail_file_name("EP 01"), "EP 01-thumbnail.jpg", "ordinary names are unchanged");
}

#[test]
fn t_14_thumbnail_name_sanitized_for_windows() {
    let sanitize = |name: &str| kuro_core::text::build_valid_filename(name, true);
    assert_eq!(sanitize("CON"), "CON_");
    assert_eq!(sanitize("lpt1.txt"), "lpt1_.txt");
    assert_eq!(sanitize("Episode 1. "), "Episode 1");
    assert_eq!(sanitize("CONTINUE"), "CONTINUE", "only the whole device name is reserved");
}

#[test]
fn t_14_thumbnail_skip_existing() {
    let d = dir();
    assert_eq!(existing_thumbnail(d.path(), "EP 01"), None);
    write_thumbnail(d.path(), "EP 01", b"x").unwrap();
    assert_eq!(existing_thumbnail(d.path(), "EP 01"), Some(d.path().join("EP 01-thumbnail.jpg")));
}

#[test]
fn t_14_set_cover_success_returns_file_url() {
    let d = dir();
    let written = set_cover(d.path(), JPEG).unwrap();
    assert!(written.thumbnail_url.starts_with("file://"), "{}", written.thumbnail_url);
    assert!(written.thumbnail_url.ends_with("/cover.jpg"));
    assert!(written.cover_last_modified > 0);
    assert!(d.path().join(".nomedia").is_file());
}
