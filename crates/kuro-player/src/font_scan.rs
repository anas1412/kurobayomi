//! Font families for the subtitle font dropdown (area 46 `SubtitleSettingsTypographyCard.kt`,
//! T-46b-006). Reads the family name (name record 1) of each `.ttf` and `.otf` file in the fonts
//! directory. `.ttc` is skipped. Unreadable or corrupt files are skipped silently. The scan itself
//! runs on a worker thread at the GTK layer, and its result is dropped if the panel has closed.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Largest font file the scan reads (E32). CJK fonts are 20-40 MB, so this leaves headroom while
/// stopping a runaway read.
pub const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

/// The default family. The scan never repeats it (E30).
pub const DEFAULT_FAMILY: &str = "Sans Serif";

/// True for names ending in `.ttf` or `.otf`, in any case.
pub fn is_font_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".ttf") || lower.ends_with(".otf")
}

fn be16(data: &[u8], at: usize) -> Option<usize> {
    let b = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]) as usize)
}

fn be32(data: &[u8], at: usize) -> Option<usize> {
    let b = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

/// Family name from the `name` table, record ID 1. Unicode records (platform 0, or Windows
/// platform 3) are read as UTF-16BE and preferred. Macintosh Roman (platform 1) is the fallback,
/// read as Latin-1. `None` for a file that is not a readable font.
pub fn family_name(data: &[u8]) -> Option<String> {
    let tables = be16(data, 4)?;
    let mut table = None;
    for i in 0..tables {
        let record = 12 + i * 16;
        if data.get(record..record + 4)? == b"name" {
            table = Some(be32(data, record + 8)?);
            break;
        }
    }
    let t = table?;
    let count = be16(data, t + 2)?;
    let string_base = t + be16(data, t + 4)?;
    let mut unicode = None;
    let mut mac = None;
    for i in 0..count {
        let record = t + 6 + i * 12;
        let platform = be16(data, record)?;
        let encoding = be16(data, record + 2)?;
        if be16(data, record + 6)? != 1 {
            continue;
        }
        let len = be16(data, record + 8)?;
        let start = string_base.checked_add(be16(data, record + 10)?)?;
        let bytes = data.get(start..start.checked_add(len)?)?;
        match (platform, encoding) {
            (0, _) | (3, 1 | 10) if unicode.is_none() => {
                let units: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|b| u16::from_be_bytes([b[0], b[1]]))
                    .collect();
                unicode = Some(String::from_utf16_lossy(&units));
            }
            (1, 0) if mac.is_none() => mac = Some(bytes.iter().map(|&b| b as char).collect()),
            _ => {}
        }
    }
    unicode.or(mac).filter(|name| !name.trim().is_empty())
}

/// The user fonts directory under the mpv config, created once at startup (E29). Later calls
/// reuse it.
pub fn ensure_fonts_dir(mpv_config: &Path) -> io::Result<PathBuf> {
    let dir = mpv_config.join("fonts");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Family names of the font files in `dir`, sorted by file name, without duplicates and without
/// `DEFAULT_FAMILY`. An unreadable directory gives an empty list (F4).
pub fn scan_fonts_dir(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(is_font_file)
        })
        .collect();
    paths.sort();
    let mut families: Vec<String> = Vec::new();
    for path in paths {
        if fs::metadata(&path).map_or(true, |m| m.len() > MAX_FONT_BYTES) {
            continue;
        }
        let Ok(data) = fs::read(&path) else {
            continue;
        };
        let Some(name) = family_name(&data) else {
            continue;
        };
        if name != DEFAULT_FAMILY && !families.contains(&name) {
            families.push(name);
        }
    }
    families
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal sfnt with one `name` table holding record 1 in the given platform and encoding.
    fn sample_font(family: &str, platform: u16, encoding: u16) -> Vec<u8> {
        let units: Vec<u8> = if platform == 1 {
            family.bytes().collect()
        } else {
            family.encode_utf16().flat_map(u16::to_be_bytes).collect()
        };
        let mut name = Vec::new();
        name.extend(0u16.to_be_bytes());
        name.extend(1u16.to_be_bytes());
        name.extend(18u16.to_be_bytes());
        for field in [platform, encoding, 0, 1, units.len() as u16, 0] {
            name.extend(field.to_be_bytes());
        }
        name.extend(units);
        let mut font = Vec::new();
        font.extend(0x0001_0000u32.to_be_bytes());
        font.extend(1u16.to_be_bytes());
        font.extend([0u8; 6]);
        font.extend(b"name");
        font.extend(0u32.to_be_bytes());
        font.extend(28u32.to_be_bytes());
        font.extend((name.len() as u32).to_be_bytes());
        font.extend(name);
        font
    }

    #[test]
    fn only_ttf_and_otf_names_are_font_files() {
        assert!(is_font_file("A.TTF"));
        assert!(is_font_file("a.otf"));
        assert!(!is_font_file("a.ttc"));
        assert!(!is_font_file("a.ttf.bak"));
        assert!(!is_font_file("ttf"));
    }

    #[test]
    fn family_comes_from_the_unicode_or_mac_record() {
        assert_eq!(
            family_name(&sample_font("Kurobayomi Sample", 3, 1)).as_deref(),
            Some("Kurobayomi Sample")
        );
        assert_eq!(
            family_name(&sample_font("Kurobayomi Sample", 1, 0)).as_deref(),
            Some("Kurobayomi Sample")
        );
    }

    #[test]
    fn garbage_has_no_family() {
        assert_eq!(family_name(&[0u8; 16]), None);
        assert_eq!(family_name(b"not a font at all"), None);
    }

    #[test]
    fn scan_skips_bad_files_duplicates_and_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, data: &[u8]| fs::write(dir.path().join(name), data).unwrap();
        write("a.TTF", &sample_font("Alpha", 3, 1));
        write("b.otf", &sample_font("Beta", 3, 1));
        write("bad.ttf", b"junk");
        write("c.ttf", &sample_font("Alpha", 1, 0));
        write("d.ttc", &sample_font("Ignored", 3, 1));
        write("e.ttf", &sample_font(DEFAULT_FAMILY, 3, 1));
        assert_eq!(scan_fonts_dir(dir.path()), vec!["Alpha", "Beta"]);
    }

    #[test]
    fn fonts_directory_is_created_once_and_reused() {
        let dir = tempfile::tempdir().unwrap();
        let first = ensure_fonts_dir(dir.path()).unwrap();
        assert!(first.is_dir());
        assert_eq!(ensure_fonts_dir(dir.path()).unwrap(), first);
    }

    #[test]
    fn missing_directory_gives_an_empty_list() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan_fonts_dir(&dir.path().join("absent")).is_empty());
    }

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fonts")
    }

    #[test]
    fn the_fixture_font_has_the_sample_family() {
        let data = fs::read(fixture_dir().join("Sample-Regular.ttf")).unwrap();
        assert_eq!(family_name(&data).as_deref(), Some("Kurobayomi Sample"));
        let broken = fs::read(fixture_dir().join("Broken.ttf")).unwrap();
        assert_eq!(family_name(&broken), None);
    }

    #[test]
    fn a_renamed_copy_of_the_fixture_is_listed_once() {
        let dir = tempfile::tempdir().unwrap();
        let sample = fixture_dir().join("Sample-Regular.ttf");
        fs::copy(&sample, dir.path().join("Sample-Regular.ttf")).unwrap();
        fs::copy(&sample, dir.path().join("Sample-Copy.ttf")).unwrap();
        fs::copy(
            fixture_dir().join("Broken.ttf"),
            dir.path().join("Broken.ttf"),
        )
        .unwrap();
        assert_eq!(scan_fonts_dir(dir.path()), vec!["Kurobayomi Sample"]);
    }

    #[test]
    fn a_file_over_the_size_cap_is_skipped_without_being_read() {
        let dir = tempfile::tempdir().unwrap();
        let huge = fs::File::create(dir.path().join("a-huge.ttf")).unwrap();
        huge.set_len(MAX_FONT_BYTES + 1024 * 1024).unwrap();
        fs::copy(
            fixture_dir().join("Sample-Regular.ttf"),
            dir.path().join("b.ttf"),
        )
        .unwrap();
        assert_eq!(scan_fonts_dir(dir.path()), vec!["Kurobayomi Sample"]);
    }
}
