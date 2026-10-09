//! Storage helpers (port of `DiskUtil.kt`, `OkioExtensions.kt` and `ContextExtensions.kt`).
//!
//! Free and total disk space are not here: they need statvfs, which lives in
//! `kuro_platform::linux::storage`.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::text::{take_bytes, take_utf16};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("cannot create folder {}: {source}", path.display())]
    CreateDir {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    /// The text is the first-run storage step error; the path and cause stay in the fields for logs.
    #[error("Selected folder is not writable")]
    NotWritable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Creates `dir` if it is missing, then proves it is writable by creating and removing a probe file.
pub fn ensure_writable_dir(dir: &Path) -> Result<(), StorageError> {
    fs::create_dir_all(dir).map_err(|source| StorageError::CreateDir {
        path: dir.to_owned(),
        source,
    })?;
    let probe = dir.join(".kuro-write-probe");
    let not_writable = |source| StorageError::NotWritable {
        path: dir.to_owned(),
        source,
    };
    match File::create(&probe).and_then(|mut f| f.write_all(b"probe")) {
        Ok(()) => fs::remove_file(&probe).map_err(not_writable),
        Err(source) => {
            let _ = fs::remove_file(&probe);
            Err(not_writable(source))
        }
    }
}

/// Total size of regular files under `path`. Symlinks are neither followed nor counted, so a
/// symlink loop terminates.
pub fn dir_size(path: &Path) -> io::Result<u64> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        let mut total = 0;
        for entry in fs::read_dir(path)? {
            total += dir_size(&entry?.path())?;
        }
        Ok(total)
    } else if meta.is_file() {
        Ok(meta.len())
    } else {
        Ok(0)
    }
}

/// Copies `src` to `dest` through a buffer. Missing parent folders are created. The bytes go
/// to `<dest>.part` first and are renamed over `dest` only when complete. Any failure removes
/// the `.part` file and leaves `dest` as it was.
pub fn save_stream(src: &mut impl Read, dest: &Path) -> io::Result<u64> {
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut part_name = dest.as_os_str().to_owned();
    part_name.push(".part");
    let part = PathBuf::from(part_name);
    let result = write_part(src, &part).and_then(|written| {
        fs::rename(&part, dest)?;
        Ok(written)
    });
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn write_part(src: &mut impl Read, part: &Path) -> io::Result<u64> {
    let mut out = BufWriter::new(File::create(part)?);
    let written = io::copy(src, &mut out)?;
    let file = out.into_inner().map_err(io::IntoInnerError::into_error)?;
    file.sync_all()?;
    Ok(written)
}

/// Creates an empty file `name` under `cache_dir`, replacing any existing file of that name.
/// Returns the path of the new file.
pub fn create_file_in_cache_dir(cache_dir: &Path, name: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(cache_dir)?;
    let path = cache_dir.join(name);
    match fs::remove_file(&path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    File::create(&path)?;
    Ok(path)
}

const MAX_COMPONENT_UTF16: usize = 240;
/// Byte budget for a name before the page suffix is added (safe for 255-byte file names).
pub const MAX_FILE_NAME_BYTES: usize = 250;

const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn is_forbidden(c: char) -> bool {
    c <= '\u{1f}'
        || c == '\u{7f}'
        || matches!(c, '"' | '*' | '/' | ':' | '<' | '>' | '?' | '\\' | '|')
}

fn is_trim(c: char) -> bool {
    c == '.' || c == ' '
}

/// Makes one path component valid on FAT, NTFS and ext4: trims dots and spaces at both
/// ends, replaces forbidden and control characters with `_`, prefixes `_` to Windows reserved
/// names (with or without extension), and keeps at most 240 UTF-16 units. Empty results
/// become `(invalid)`.
pub fn valid_filename(orig: &str) -> String {
    let name = orig.trim_matches(is_trim);
    if name.is_empty() {
        return "(invalid)".to_owned();
    }
    let mut out: String = name
        .chars()
        .map(|c| if is_forbidden(c) { '_' } else { c })
        .collect();
    let stem = out
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    if RESERVED_NAMES.iter().any(|r| stem.eq_ignore_ascii_case(r)) {
        out.insert(0, '_');
    }
    take_utf16(&out, MAX_COMPONENT_UTF16)
        .trim_end_matches(is_trim)
        .to_owned()
}

/// File name for one page: `"<title> - <chapter>"` cut to the byte budget that leaves room
/// for `" - <page>"`, sanitised, then the suffix is appended.
pub fn page_filename(title: &str, chapter: &str, page: u32) -> String {
    let suffix = format!(" - {page}");
    let joined = format!("{title} - {chapter}");
    let base = take_bytes(&joined, MAX_FILE_NAME_BYTES.saturating_sub(suffix.len()));
    format!("{}{suffix}", valid_filename(base))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn filename_trims_dots_and_spaces() {
        assert_eq!(valid_filename("  .hidden. "), "hidden");
        assert_eq!(valid_filename("trail. "), "trail");
        assert_eq!(valid_filename("ＡＢＣ. "), "ＡＢＣ");
    }

    #[test]
    fn filename_empty_after_trim_is_invalid() {
        assert_eq!(valid_filename("..."), "(invalid)");
        assert_eq!(valid_filename(""), "(invalid)");
    }

    #[test]
    fn filename_replaces_forbidden_and_control_characters() {
        assert_eq!(valid_filename("a/b:c*d?"), "a_b_c_d_");
        assert_eq!(valid_filename("name\u{1}x"), "name_x");
        assert_eq!(valid_filename("q\"<>|\\\u{7f}"), "q______");
    }

    #[test]
    fn reserved_names_get_a_prefix_with_or_without_extension() {
        assert_eq!(valid_filename("CON"), "_CON");
        assert_eq!(valid_filename("CON.txt"), "_CON.txt");
        assert_eq!(valid_filename("con"), "_con");
        assert_eq!(valid_filename("lpt9.tar.gz"), "_lpt9.tar.gz");
        assert_eq!(valid_filename("COM1 ."), "_COM1");
        assert_eq!(valid_filename("NULL"), "NULL");
        assert_eq!(valid_filename("COM10"), "COM10");
        assert_eq!(valid_filename("my CON"), "my CON");
    }

    #[test]
    fn component_is_capped_at_240_utf16_units() {
        assert_eq!(valid_filename(&"a".repeat(300)).len(), 240);
        let emoji_130 = "😀".repeat(130);
        assert_eq!(valid_filename(&emoji_130), "😀".repeat(120));
        let emoji_120_then_a = format!("{}a", "😀".repeat(120));
        assert_eq!(valid_filename(&emoji_120_then_a), "😀".repeat(120));
    }

    #[test]
    fn cap_never_leaves_a_split_surrogate_pair() {
        let input = format!("{}😀", "a".repeat(239));
        assert_eq!(valid_filename(&input), "a".repeat(239));
    }

    #[test]
    fn page_filename_matches_spec_example() {
        assert_eq!(page_filename("Naruto", "Ch 1", 7), "Naruto - Ch 1 - 7");
    }

    #[test]
    fn page_filename_cuts_base_before_suffix() {
        let long = "a".repeat(400);
        let name = page_filename(&long, "Ch", 7);
        // Base cut to 250 - 4 bytes, then the 240-unit component cap, then " - 7".
        assert_eq!(name, format!("{} - 7", "a".repeat(240)));
    }

    #[test]
    fn ensure_writable_dir_creates_missing_folders() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("one").join("two");
        ensure_writable_dir(&dir).unwrap();
        assert!(dir.is_dir());
        assert!(!dir.join(".kuro-write-probe").exists());
    }

    #[test]
    fn ensure_writable_dir_reports_a_file_in_the_way() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file");
        fs::write(&file, b"x").unwrap();
        assert!(matches!(
            ensure_writable_dir(&file),
            Err(StorageError::CreateDir { .. })
        ));
    }

    #[test]
    fn ensure_writable_dir_reports_read_only_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().to_path_buf();
        let original = fs::metadata(&dir).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&dir, readonly).unwrap();
        let result = ensure_writable_dir(&dir);
        fs::set_permissions(&dir, original).unwrap();
        if result.is_ok() {
            // Running as root ignores the read-only bit, so the write succeeds and there is nothing to assert.
            eprintln!(
                "skipped ensure_writable_dir_reports_read_only_folder: a write succeeded despite the read-only mode (running as root?)"
            );
            return;
        }
        let err = result.unwrap_err();
        assert!(matches!(err, StorageError::NotWritable { .. }));
        assert_eq!(err.to_string(), "Selected folder is not writable");
    }

    #[test]
    fn not_writable_text_is_the_storage_step_message() {
        let err = StorageError::NotWritable {
            path: PathBuf::from("/x"),
            source: io::Error::from(io::ErrorKind::PermissionDenied),
        };
        assert_eq!(err.to_string(), "Selected folder is not writable");
    }

    #[test]
    fn dir_size_sums_nested_regular_files() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a"), b"abc").unwrap();
        fs::create_dir(root.path().join("sub")).unwrap();
        fs::write(root.path().join("sub").join("b"), b"defgh").unwrap();
        assert_eq!(dir_size(root.path()).unwrap(), 8);
    }

    #[cfg(unix)]
    #[test]
    fn dir_size_terminates_on_symlink_loop_and_does_not_follow() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a"), b"abc").unwrap();
        std::os::unix::fs::symlink(root.path(), root.path().join("loop")).unwrap();
        assert_eq!(dir_size(root.path()).unwrap(), 3);
    }

    #[test]
    fn save_stream_creates_parents_and_writes_bytes() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("x").join("y").join("cover.jpg");
        let written = save_stream(&mut Cursor::new(b"image-bytes"), &dest).unwrap();
        assert_eq!(written, 11);
        assert_eq!(fs::read(&dest).unwrap(), b"image-bytes");
        assert!(!dest.with_file_name("cover.jpg.part").exists());
    }

    #[test]
    fn save_stream_replaces_existing_destination() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("f.bin");
        fs::write(&dest, b"old").unwrap();
        save_stream(&mut Cursor::new(b"new"), &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new");
    }

    /// Yields some bytes, then fails, to simulate a broken download.
    struct FailAfter(&'static [u8]);

    impl Read for FailAfter {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.0.is_empty() {
                return Err(io::Error::other("connection reset"));
            }
            let n = self.0.len().min(buf.len());
            buf[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    #[test]
    fn failed_save_removes_part_and_leaves_no_destination() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ch.cbz");
        assert!(save_stream(&mut FailAfter(b"partial"), &dest).is_err());
        assert!(!dest.exists());
        assert!(!dest.with_file_name("ch.cbz.part").exists());
    }

    #[test]
    fn failed_save_keeps_the_old_destination() {
        let root = tempfile::tempdir().unwrap();
        let dest = root.path().join("ch.cbz");
        fs::write(&dest, b"old").unwrap();
        assert!(save_stream(&mut FailAfter(b"partial"), &dest).is_err());
        assert_eq!(fs::read(&dest).unwrap(), b"old");
        assert!(!dest.with_file_name("ch.cbz.part").exists());
    }

    #[test]
    fn cache_file_is_created_empty_and_replaces_existing() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("kurobayomi");
        let first = create_file_in_cache_dir(&cache, "logs.txt").unwrap();
        assert_eq!(first, cache.join("logs.txt"));
        fs::write(&first, b"stale").unwrap();
        let second = create_file_in_cache_dir(&cache, "logs.txt").unwrap();
        assert_eq!(fs::read(&second).unwrap(), b"");
    }

    #[test]
    fn cache_dir_that_is_a_file_returns_an_error() {
        let root = tempfile::tempdir().unwrap();
        let blocker = root.path().join("blocked");
        fs::write(&blocker, b"x").unwrap();
        assert!(create_file_in_cache_dir(&blocker, "logs.txt").is_err());
    }
}
