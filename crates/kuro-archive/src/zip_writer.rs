//! `CbzWriter`: writes chapter pages as a stored (uncompressed) CBZ, via a temp file renamed on finish.
//!
//! Port of `core/archive/.../ZipWriter.kt` and the caller in `MangaDownloader.archiveChapter`
//! (T-13a-034). Entries are file basenames, stored without compression, with the source's mtime
//! (clamped to 1980 because ZIP cannot store earlier times) and unix mode.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime as ChronoDateTime, Datelike, Local};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

/// Suffix of the file being written; the final name is the target without it (`_tmp`).
pub const TMP_SUFFIX: &str = "_tmp";

/// Writes one CBZ. Entries go to `<target>_tmp`, which `finish` renames to `<target>`. A writer
/// dropped before `finish` removes its `_tmp` file.
pub struct CbzWriter {
    zip: Option<ZipWriter<File>>,
    tmp: PathBuf,
    target: PathBuf,
}

impl CbzWriter {
    /// Creates `<target>_tmp`, truncating an existing one.
    pub fn create(target: &Path) -> io::Result<Self> {
        let mut tmp = target.as_os_str().to_owned();
        tmp.push(TMP_SUFFIX);
        let tmp = PathBuf::from(tmp);
        let file = File::create(&tmp)?;
        Ok(Self {
            zip: Some(ZipWriter::new(file)),
            tmp,
            target: target.to_owned(),
        })
    }

    /// Adds one file as a stored entry named by its basename. Entries are written in the order
    /// they are added; the caller chooses the order.
    pub fn add_file(&mut self, source: &Path) -> io::Result<()> {
        let name = source.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "source has no UTF-8 file name")
        })?;
        let meta = fs::metadata(source)?;
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(dos_time(&meta))
            .unix_permissions(unix_mode(&meta) & 0o7777)
            .large_file(true);
        let zip = self.zip.as_mut().ok_or_else(finished)?;
        zip.start_file(name, options).map_err(to_io)?;
        io::copy(&mut File::open(source)?, zip)?;
        Ok(())
    }

    /// Closes the archive, syncs it and renames `<target>_tmp` to `<target>`. On failure the
    /// `_tmp` file is removed.
    pub fn finish(mut self) -> io::Result<PathBuf> {
        let zip = self.zip.take().ok_or_else(finished)?;
        let result = zip
            .finish()
            .map_err(to_io)
            .and_then(|file| file.sync_all())
            .and_then(|()| fs::rename(&self.tmp, &self.target));
        if result.is_err() {
            let _ = fs::remove_file(&self.tmp);
        }
        result.map(|()| self.target.clone())
    }
}

impl Drop for CbzWriter {
    fn drop(&mut self) {
        if self.zip.is_some() {
            let _ = fs::remove_file(&self.tmp);
        }
    }
}

fn finished() -> io::Error {
    io::Error::other("cbz writer already finished")
}

fn to_io(e: zip::result::ZipError) -> io::Error {
    io::Error::other(e)
}

/// Source mtime as a ZIP (MS-DOS) time. Earlier than 1980 gives 1980-01-01 00:00:00; later than
/// 2107 gives 2107-12-31 23:59:58, the last representable value.
fn dos_time(meta: &fs::Metadata) -> DateTime {
    let Ok(modified) = meta.modified() else {
        return DateTime::DEFAULT;
    };
    let local = ChronoDateTime::<Local>::from(modified).naive_local();
    DateTime::try_from(local).unwrap_or_else(|_| {
        if local.year() < 1980 {
            DateTime::DEFAULT
        } else {
            DateTime::try_from_msdos(0xFF9F, 0xBF7D).unwrap_or(DateTime::DEFAULT)
        }
    })
}

#[cfg(unix)]
fn unix_mode(meta: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode()
}

#[cfg(not(unix))]
fn unix_mode(_meta: &fs::Metadata) -> u32 {
    0o644
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::time::{Duration, UNIX_EPOCH};

    use crate::reader::ArchiveReader;

    /// Writes a source file with `body` and returns its path.
    fn source(dir: &Path, name: &str, body: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, body).unwrap();
        path
    }

    fn zip_entries(path: &Path) -> Vec<(String, CompressionMethod, Option<u32>, DateTime)> {
        let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .map(|i| {
                let f = archive.by_index(i).unwrap();
                (
                    f.name().to_owned(),
                    f.compression(),
                    f.unix_mode(),
                    f.last_modified().unwrap_or_default(),
                )
            })
            .collect()
    }

    #[test]
    fn writes_stored_basename_entries_and_renames_on_finish() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = source(dir.path(), "001.jpg", b"one");
        let p2 = source(dir.path(), "002.png", b"two two");
        let target = dir.path().join("Chapter 1.cbz");

        let mut writer = CbzWriter::create(&target).unwrap();
        assert!(dir.path().join("Chapter 1.cbz_tmp").exists());
        writer.add_file(&p1).unwrap();
        writer.add_file(&p2).unwrap();
        let out = writer.finish().unwrap();

        assert_eq!(out, target);
        assert!(!dir.path().join("Chapter 1.cbz_tmp").exists());
        let entries = zip_entries(&target);
        let names: Vec<_> = entries.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(names, ["001.jpg", "002.png"]);
        assert!(entries.iter().all(|e| e.1 == CompressionMethod::Stored));

        let listed = ArchiveReader::open(&target).unwrap().entries().unwrap();
        assert_eq!(listed.len(), 2);
    }

    #[test]
    fn entry_bytes_match_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let body: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
        let p = source(dir.path(), "page.jpg", &body);
        let target = dir.path().join("c.cbz");
        let mut writer = CbzWriter::create(&target).unwrap();
        writer.add_file(&p).unwrap();
        writer.finish().unwrap();

        let reader = ArchiveReader::open(&target).unwrap();
        let mut entry = reader.open_entry("page.jpg").unwrap().unwrap();
        let mut got = Vec::new();
        entry.read_to_end(&mut got).unwrap();
        assert_eq!(got, body);
    }

    #[test]
    fn mode_is_copied_and_old_mtime_is_clamped_to_1980() {
        let dir = tempfile::tempdir().unwrap();
        let p = source(dir.path(), "1.jpg", b"x");
        fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o640)).unwrap();
        File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1000))
            .unwrap();
        let target = dir.path().join("c.cbz");
        let mut writer = CbzWriter::create(&target).unwrap();
        writer.add_file(&p).unwrap();
        writer.finish().unwrap();

        let entries = zip_entries(&target);
        assert_eq!(entries[0].2.map(|m| m & 0o7777), Some(0o640));
        assert_eq!(entries[0].3, DateTime::DEFAULT);
    }

    #[test]
    fn recent_mtime_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let p = source(dir.path(), "1.jpg", b"x");
        let target = dir.path().join("c.cbz");
        let mut writer = CbzWriter::create(&target).unwrap();
        writer.add_file(&p).unwrap();
        writer.finish().unwrap();

        let entries = zip_entries(&target);
        let expected = dos_time(&fs::metadata(&p).unwrap());
        assert_eq!(entries[0].3, expected);
        assert!(entries[0].3.year() >= 2020);
    }

    #[test]
    fn create_truncates_an_existing_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("c.cbz");
        fs::write(dir.path().join("c.cbz_tmp"), vec![b'z'; 10_000]).unwrap();
        let writer = CbzWriter::create(&target).unwrap();
        writer.finish().unwrap();
        let meta = fs::metadata(&target).unwrap();
        assert!(
            meta.len() < 10_000,
            "old bytes must be gone, got {}",
            meta.len()
        );
    }

    #[test]
    fn dropping_without_finish_removes_the_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = source(dir.path(), "1.jpg", b"x");
        let target = dir.path().join("c.cbz");
        {
            let mut writer = CbzWriter::create(&target).unwrap();
            writer.add_file(&p).unwrap();
        }
        assert!(!dir.path().join("c.cbz_tmp").exists());
        assert!(!target.exists());
    }

    #[test]
    fn add_file_without_a_name_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("c.cbz");
        let mut writer = CbzWriter::create(&target).unwrap();
        let err = writer.add_file(Path::new("/")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn written_archive_passes_unzip_test_when_unzip_is_installed() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = source(dir.path(), "1.jpg", &[0xff, 0xd8, 0xff, 0xe0, 1, 2, 3]);
        let p2 = source(dir.path(), "2.jpg", b"abc");
        let target = dir.path().join("c.cbz");
        let mut writer = CbzWriter::create(&target).unwrap();
        writer.add_file(&p1).unwrap();
        writer.add_file(&p2).unwrap();
        writer.finish().unwrap();

        match std::process::Command::new("unzip")
            .arg("-tq")
            .arg(&target)
            .output()
        {
            Ok(out) => assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stdout)
            ),
            Err(_) => eprintln!("unzip not installed; skipping the external check"),
        }
    }
}
