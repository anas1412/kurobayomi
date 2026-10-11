//! Storage helpers (port of `DiskUtil.kt`, `OkioExtensions.kt` and `ContextExtensions.kt`).
//!
//! Free and total disk space are not here: they need statvfs, which lives in
//! `kuro_platform::linux::storage`.

use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

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

/// `UniFile.extension`: the text after the last `.`, or the whole name when there is no `.`
/// (`"README"` gives `"README"`, `".hidden"` gives `"hidden"`). Kept only for ported UI text; new code uses
/// `Path::extension`.
pub fn extension_quirk(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// `UniFile.nameWithoutExtension`: the text before the last `.`, or the whole name when there is no `.`
/// (`".hidden"` gives `""`). Kept only for ported UI text.
pub fn name_without_extension_quirk(name: &str) -> &str {
    name.rfind('.').map_or(name, |i| &name[..i])
}

/// An open temporary file created by [`TempFiles::create_from`]. The handle closes on drop; the file stays on
/// disk until [`TempFiles::clear`] (like `File.createTempFile`).
#[derive(Debug)]
pub struct TempFile {
    pub path: PathBuf,
    pub file: File,
}

/// The temporary file manager (`UniFileTempFileManager`): files live in `<cache>/tmp`.
#[derive(Debug, Clone)]
pub struct TempFiles {
    dir: PathBuf,
}

impl TempFiles {
    pub fn new(cache_dir: &Path) -> Self {
        Self { dir: cache_dir.join("tmp") }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Copies `source` into a new `<prefix><unique>.tmp` file, where the prefix is the source name without
    /// its extension padded with spaces to at least three bytes.
    pub fn create_from(&self, source: &Path) -> io::Result<TempFile> {
        fs::create_dir_all(&self.dir)?;
        let mut input = File::open(source)?;
        let file_name = source.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
        let mut prefix = name_without_extension_quirk(&file_name).to_owned();
        while prefix.len() < 3 {
            prefix.push(' ');
        }
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        loop {
            let unique = (nanos as u64) ^ COUNTER.fetch_add(1, Ordering::Relaxed).rotate_left(40);
            let path = self.dir.join(format!("{prefix}{unique}.tmp"));
            match File::options().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    if let Err(e) = io::copy(&mut input, &mut file) {
                        drop(file);
                        let _ = fs::remove_file(&path);
                        return Err(e);
                    }
                    return Ok(TempFile { path, file });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Removes the temporary folder and everything in it. A missing folder is fine.
    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Error)]
pub enum CopyError {
    #[error("the source file doesn't exist: {}", .0.display())]
    SourceMissing(PathBuf),
    #[error("the destination file already exists: {}", .0.display())]
    DestinationExists(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[cfg(unix)]
fn set_read_only(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o444))
}

#[cfg(not(unix))]
fn set_read_only(path: &Path) -> io::Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(path, perms)
}

/// `File.copyAndSetReadOnlyTo`. A file is copied to `<dst>.tmp`, made read-only (mode `0o444`, deviation D-5)
/// and renamed to `dst`, so a failure never leaves a read-only partial file. A directory source creates the
/// destination directory and copies no content.
pub fn copy_read_only(src: &Path, dst: &Path, overwrite: bool) -> Result<PathBuf, CopyError> {
    let meta = fs::metadata(src).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => CopyError::SourceMissing(src.to_owned()),
        _ => CopyError::Io(e),
    })?;
    if let Ok(existing) = fs::symlink_metadata(dst) {
        if !overwrite {
            return Err(CopyError::DestinationExists(dst.to_owned()));
        }
        if existing.is_dir() {
            fs::remove_dir(dst)?;
        } else {
            fs::remove_file(dst)?;
        }
    }
    if meta.is_dir() {
        fs::create_dir_all(dst)?;
        return Ok(dst.to_owned());
    }
    if let Some(parent) = dst.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let mut part_name = dst.as_os_str().to_owned();
    part_name.push(".tmp");
    let part = PathBuf::from(part_name);
    let result = File::open(src)
        .and_then(|mut input| {
            let mut out = File::create(&part)?;
            io::copy(&mut input, &mut out)?;
            out.sync_all()
        })
        .and_then(|()| set_read_only(&part))
        .and_then(|()| fs::rename(&part, dst));
    if let Err(e) = result {
        let _ = fs::remove_file(&part);
        return Err(e.into());
    }
    Ok(dst.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn name_quirks_match_unifile() {
        assert_eq!(extension_quirk("README"), "README");
        assert_eq!(extension_quirk("a.tar.gz"), "gz");
        assert_eq!(extension_quirk(".hidden"), "hidden");
        assert_eq!(name_without_extension_quirk(".hidden"), "");
        assert_eq!(name_without_extension_quirk("README"), "README");
        assert_eq!(name_without_extension_quirk("a.tar.gz"), "a.tar");
    }

    #[test]
    fn temp_file_copies_source_under_cache_tmp_with_padded_prefix() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("ab.cbz");
        fs::write(&src, b"payload").unwrap();
        let temp = TempFiles::new(root.path());
        let made = temp.create_from(&src).unwrap();
        assert_eq!(made.path.parent().unwrap(), root.path().join("tmp"));
        let name = made.path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("ab "), "{name}");
        assert!(name.ends_with(".tmp"), "{name}");
        assert_eq!(fs::read(&made.path).unwrap(), b"payload");
        let long = root.path().join("chapter.cbz");
        fs::write(&long, b"x").unwrap();
        let name = temp.create_from(&long).unwrap().path;
        assert!(name.file_name().unwrap().to_str().unwrap().starts_with("chapter"));
    }

    #[test]
    fn temp_clear_removes_the_folder_and_is_repeatable() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("a.bin");
        fs::write(&src, b"x").unwrap();
        let temp = TempFiles::new(root.path());
        assert!(temp.clear().is_ok(), "nothing created yet");
        temp.create_from(&src).unwrap();
        assert!(temp.dir().is_dir());
        temp.clear().unwrap();
        assert!(!temp.dir().exists());
        temp.clear().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn temp_handles_close_on_drop() {
        if !Path::new("/proc/self/fd").is_dir() {
            return;
        }
        let open_fds = || fs::read_dir("/proc/self/fd").unwrap().count();
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("a.bin");
        fs::write(&src, b"x").unwrap();
        let temp = TempFiles::new(root.path());
        let before = open_fds();
        for _ in 0..100 {
            drop(temp.create_from(&src).unwrap());
        }
        // Other tests may open descriptors at the same time; 100 leaks would show as far more than that.
        assert!(open_fds() < before + 50, "descriptors leaked");
        assert_eq!(fs::read_dir(temp.dir()).unwrap().count(), 100, "names are unique");
    }

    #[test]
    fn copy_read_only_refuses_existing_destination_without_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let (src, dst) = (root.path().join("s"), root.path().join("d"));
        fs::write(&src, b"new").unwrap();
        fs::write(&dst, b"old").unwrap();
        assert!(matches!(copy_read_only(&src, &dst, false), Err(CopyError::DestinationExists(_))));
        assert_eq!(fs::read(&dst).unwrap(), b"old");
        copy_read_only(&src, &dst, true).unwrap();
        assert_eq!(fs::read(&dst).unwrap(), b"new");
    }

    #[test]
    fn copy_read_only_reports_missing_source() {
        let root = tempfile::tempdir().unwrap();
        let err = copy_read_only(&root.path().join("nope"), &root.path().join("d"), false);
        assert!(matches!(err, Err(CopyError::SourceMissing(_))));
    }

    #[cfg(unix)]
    #[test]
    fn copy_read_only_sets_mode_0o444_and_leaves_no_partial() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let (src, dst) = (root.path().join("s"), root.path().join("sub").join("d"));
        fs::write(&src, b"bytes").unwrap();
        copy_read_only(&src, &dst, false).unwrap();
        assert_eq!(fs::metadata(&dst).unwrap().permissions().mode() & 0o777, 0o444);
        assert_eq!(fs::read(&dst).unwrap(), b"bytes");
        assert!(!dst.with_file_name("d.tmp").exists());
        // Overwriting a read-only destination works.
        copy_read_only(&src, &dst, true).unwrap();
    }

    #[test]
    fn copy_read_only_failure_leaves_no_destination_or_partial() {
        let root = tempfile::tempdir().unwrap();
        // A file where the destination folder should be makes create_dir_all fail.
        let blocker = root.path().join("blocker");
        fs::write(&blocker, b"x").unwrap();
        let src = root.path().join("s");
        fs::write(&src, b"x").unwrap();
        let dst = blocker.join("d");
        assert!(matches!(copy_read_only(&src, &dst, false), Err(CopyError::Io(_))));
        assert!(!dst.exists());
    }

    #[test]
    fn copy_read_only_directory_source_creates_directory_without_content() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("srcdir");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("inner"), b"x").unwrap();
        let dst = root.path().join("dstdir");
        copy_read_only(&src, &dst, false).unwrap();
        assert!(dst.is_dir());
        assert_eq!(fs::read_dir(&dst).unwrap().count(), 0);
    }

    #[test]
    fn copy_read_only_failure_while_writing_leaves_no_destination() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("s");
        fs::write(&src, b"x").unwrap();
        // A folder in the place of the part file makes the write fail after the checks passed.
        let dst = root.path().join("d");
        fs::create_dir(root.path().join("d.tmp")).unwrap();
        assert!(matches!(copy_read_only(&src, &dst, false), Err(CopyError::Io(_))));
        assert!(!dst.exists());
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
