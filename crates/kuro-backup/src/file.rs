//! Output file (area 21, T-21a-022), post-write self-check (T-21a-023), automatic folder, retention and startup
//! cleanup (T-21a-025), and file names (T-21a-036).
//!
//! Write order: `<name>.part` (gzip, fsync), self-check decode, rename to the final name, then retention for
//! automatic runs (E-04). A failure at any step removes the `.part` file (E-30).

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Local};
use flate2::{Compression, write::GzEncoder};

use crate::creator::{CreateInputs, build_backup};
use crate::error::BackupError;
use crate::format;
use crate::options::BackupOptions;
use crate::schema::Backup;
use crate::wire::Message;

/// Automatic files kept after a run (B-07, `MAX_AUTO_BACKUPS`).
pub const MAX_AUTO_BACKUPS: usize = 4;

const EXTENSION: &str = ".tachibk";
const PART_SUFFIX: &str = ".part";

/// Where the file goes.
pub enum Target {
    /// The path the user chose in the save dialog. An existing file is replaced.
    Manual(PathBuf),
    /// A new file in the automatic folder, named from `now` (local time).
    Automatic { dir: PathBuf, now: DateTime<Local> },
}

/// Builds the backup from the library and writes it.
pub fn create_file(inputs: &CreateInputs<'_>, opts: &BackupOptions, target: &Target) -> Result<PathBuf, BackupError> {
    let backup = build_backup(inputs, opts)?;
    write_backup(&backup, target)
}

/// Encodes, writes, self-checks and publishes `backup`. Returns the final path.
pub fn write_backup(backup: &Backup, target: &Target) -> Result<PathBuf, BackupError> {
    let payload = backup.to_bytes().map_err(BackupError::Encode)?;
    let (final_path, auto_dir) = match target {
        Target::Manual(path) => (path.clone(), None),
        Target::Automatic { dir, now } => {
            ensure_auto_dir(dir)?;
            (unique_automatic_path(dir, *now), Some(dir))
        }
    };
    let part = part_path(&final_path);
    write_part(&payload, &part)?;
    publish(&part, &final_path)?;
    if let Some(dir) = auto_dir {
        // The new file is already in place; a failed prune leaves an extra file, which is not a failed backup.
        if let Err(e) = prune_automatic(dir, MAX_AUTO_BACKUPS) {
            tracing::warn!(error = %e, "automatic backup retention failed");
        }
    }
    Ok(final_path)
}

/// `<final>.part`, in the same directory so the rename is atomic.
pub fn part_path(final_path: &Path) -> PathBuf {
    let mut s = final_path.as_os_str().to_owned();
    s.push(PART_SUFFIX);
    PathBuf::from(s)
}

/// Writes the gzip stream to `part` and fsyncs it. Removes `part` on any failure.
pub fn write_part(payload: &[u8], part: &Path) -> Result<(), BackupError> {
    let result = (|| -> io::Result<()> {
        let file = File::create(part)?;
        let mut enc = GzEncoder::new(BufWriter::new(file), Compression::default());
        enc.write_all(payload)?;
        let buf = enc.finish()?;
        let file = buf.into_inner().map_err(|e| e.into_error())?;
        file.sync_all()
    })();
    result.map_err(|e| {
        let _ = fs::remove_file(part);
        BackupError::Io(e)
    })
}

/// Self-check (T-21a-023) then rename `part` to `final_path`. A decode error or a failed rename removes `part`.
pub fn publish(part: &Path, final_path: &Path) -> Result<(), BackupError> {
    if let Err(e) = format::decode_file(part) {
        let _ = fs::remove_file(part);
        return Err(e);
    }
    fs::rename(part, final_path).map_err(|e| {
        let _ = fs::remove_file(part);
        BackupError::Io(e)
    })
}

/// Creates the automatic folder when its base exists. A missing base fails with `Io` (E-21).
pub fn ensure_auto_dir(dir: &Path) -> Result<(), BackupError> {
    if dir.is_dir() {
        return Ok(());
    }
    let base_ok = dir.parent().is_some_and(|p| p.is_dir());
    if !base_ok {
        return Err(BackupError::Io(io::Error::new(io::ErrorKind::NotFound, "base storage folder is unavailable")));
    }
    fs::create_dir(dir)?;
    Ok(())
}

/// `kurobayomi_<yyyy-MM-dd_HH-mm-ss>.tachibk` in local time (T-21a-036).
pub fn automatic_file_name(now: DateTime<Local>) -> String {
    format!("kurobayomi_{}{}", now.format("%Y-%m-%d_%H-%M-%S"), EXTENSION)
}

/// The first free name: the plain name, then `-2`, `-3` before the extension (E-03).
pub fn unique_automatic_path(dir: &Path, now: DateTime<Local>) -> PathBuf {
    let plain = automatic_file_name(now);
    let first = dir.join(&plain);
    if !first.exists() {
        return first;
    }
    let stem = plain.strip_suffix(EXTENSION).unwrap_or(&plain).to_owned();
    let mut n: u32 = 2;
    loop {
        let candidate = dir.join(format!("{stem}-{n}{EXTENSION}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// The name the manual save dialog suggests: `kurobayomi_yyyy-MM-dd_HH-mm.tachibk`, no seconds, like Aniyomi's
/// `BackupCreator.getFilename` (T-21a-036).
pub fn manual_suggested_name(now: DateTime<Local>) -> String {
    format!("kurobayomi_{}{}", now.format("%Y-%m-%d_%H-%M"), EXTENSION)
}

/// True for `kurobayomi_YYYY-MM-DD_HH-MM[-SS][-N].tachibk`. Hand-written: no regex (T-21a-036).
pub fn is_automatic_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("kurobayomi_") else { return false };
    let Some(core) = rest.strip_suffix(EXTENSION) else { return false };
    let shape = b"dddd-dd-dd_dd-dd";
    let bytes = core.as_bytes();
    if bytes.len() < shape.len() {
        return false;
    }
    let date_ok = bytes[..shape.len()]
        .iter()
        .zip(shape.iter())
        .all(|(c, p)| if *p == b'd' { c.is_ascii_digit() } else { c == p });
    if !date_ok {
        return false;
    }
    let tail = &core[shape.len()..];
    if tail.is_empty() {
        return true;
    }
    // Tail is `(-SS)?(-N)?`: split on '-' gives ["", SS?, N?].
    let parts: Vec<&str> = tail.split('-').collect();
    if !parts[0].is_empty() {
        return false;
    }
    match &parts[1..] {
        [] => true,
        [a] => is_digits(a),
        [a, b] => a.len() == 2 && is_digits(a) && is_digits(b),
        _ => false,
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// Keeps the newest `keep` automatic files in `dir` and deletes the rest. Other files are never touched.
/// Names sort chronologically because they are zero-padded local times (B-07).
pub fn prune_automatic(dir: &Path, keep: usize) -> Result<usize, BackupError> {
    let mut matching: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| e.file_name().to_str().is_some_and(is_automatic_name))
        .map(|e| e.path())
        .collect();
    matching.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    let mut removed = 0;
    for path in matching.into_iter().skip(keep) {
        fs::remove_file(&path)?;
        removed += 1;
    }
    Ok(removed)
}

/// Removes leftover `*.tachibk.part` files from a crashed run (E-30). A missing folder is not an error.
pub fn remove_stale_parts(dir: &Path) -> Result<usize, BackupError> {
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let is_part = entry.file_name().to_str().is_some_and(|n| n.ends_with(".tachibk.part"));
        if is_part && entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::BackupAnime;
    use chrono::TimeZone;

    fn at(sec: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 8, 19, 31, sec).single().unwrap()
    }

    fn sample() -> Backup {
        Backup {
            anime: vec![BackupAnime { source: 1, url: "a".into(), ..Default::default() }],
            ..Default::default()
        }
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn automatic_name_uses_local_time_seconds() {
        assert_eq!(automatic_file_name(at(5)), "kurobayomi_2026-10-08_19-31-05.tachibk");
    }

    /// The name uses the local hour. `TZ` is process-wide, so the check runs in a child process with
    /// `TZ=Europe/Berlin` (CEST, UTC+2 on 2026-10-08).
    #[test]
    fn name_format_local_time() {
        if std::env::var_os("KURO_TZ_CHILD").is_some() {
            let utc = chrono::Utc.with_ymd_and_hms(2026, 10, 8, 17, 31, 5).unwrap();
            assert_eq!(automatic_file_name(utc.with_timezone(&Local)), "kurobayomi_2026-10-08_19-31-05.tachibk");
            return;
        }
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "file::tests::name_format_local_time", "--test-threads=1"])
            .env("TZ", "Europe/Berlin")
            .env("KURO_TZ_CHILD", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    }

    #[test]
    fn manual_suggested_name_has_no_seconds() {
        let now = chrono::Local.with_ymd_and_hms(2026, 10, 9, 14, 3, 7).unwrap();
        assert_eq!(manual_suggested_name(now), "kurobayomi_2026-10-09_14-03.tachibk");
        assert!(is_automatic_name(&manual_suggested_name(now)));
    }

    #[test]
    fn name_collision_suffix() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("kurobayomi_2026-10-08_19-31-05.tachibk"), b"x").unwrap();
        let p = unique_automatic_path(dir.path(), at(5));
        assert_eq!(p.file_name().unwrap(), "kurobayomi_2026-10-08_19-31-05-2.tachibk");
        fs::write(&p, b"y").unwrap();
        let q = unique_automatic_path(dir.path(), at(5));
        assert_eq!(q.file_name().unwrap(), "kurobayomi_2026-10-08_19-31-05-3.tachibk");
    }

    #[test]
    fn regex_dot_escaped_and_forms_accepted() {
        assert!(!is_automatic_name("kurobayomiX2026-10-08_19-31.tachibk"));
        assert!(is_automatic_name("kurobayomi_2026-10-08_19-31.tachibk"));
        assert!(is_automatic_name("kurobayomi_2026-10-08_19-31-05.tachibk"));
        assert!(is_automatic_name("kurobayomi_2026-10-08_19-31-05-2.tachibk"));
        assert!(is_automatic_name("kurobayomi_2026-10-08_19-31-2.tachibk"));
        assert!(!is_automatic_name("kurobayomi_2026-10-08_19-31-05-2-3.tachibk"));
        assert!(!is_automatic_name("kurobayomi_2026-10-08_19-31-05.tachibk.part"));
        assert!(!is_automatic_name("kurobayomi_x.tachibk"));
        assert!(!is_automatic_name("xyz.jmir.tachiyomi.mi_2026-10-08_19-31.tachibk"));
    }

    #[test]
    fn write_uses_part_then_rename() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("out.tachibk");
        let part = part_path(&final_path);
        write_part(&sample().to_bytes().unwrap(), &part).unwrap();
        assert!(part.exists());
        assert!(!final_path.exists(), "no final file before the rename");
        publish(&part, &final_path).unwrap();
        assert!(final_path.exists());
        assert!(!part.exists());
        assert_eq!(format::decode_file(&final_path).unwrap().anime.len(), 1);
    }

    #[test]
    fn failure_removes_part() {
        // A final path that is a directory makes the rename fail; the part file must not survive.
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("taken.tachibk");
        fs::create_dir(&final_path).unwrap();
        let err = write_backup(&sample(), &Target::Manual(final_path.clone())).unwrap_err();
        assert!(matches!(err, BackupError::Io(_)));
        assert!(!part_path(&final_path).exists());
    }

    #[test]
    fn write_failure_into_missing_folder_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("missing").join("out.tachibk");
        assert!(write_backup(&sample(), &Target::Manual(final_path)).is_err());
    }

    #[test]
    fn manual_overwrite_truncates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manual.tachibk");
        fs::write(&path, vec![7u8; 100_000]).unwrap();
        write_backup(&sample(), &Target::Manual(path.clone())).unwrap();
        let len = fs::metadata(&path).unwrap().len();
        assert!(len < 100_000, "old longer content must not remain");
        assert_eq!(format::decode_file(&path).unwrap().anime.len(), 1);
    }

    #[test]
    fn self_check_failure_deletes_part() {
        let dir = tempfile::tempdir().unwrap();
        let final_path = dir.path().join("bad.tachibk");
        let part = part_path(&final_path);
        fs::write(&part, b"\x0A\xFF\xFF definitely not a backup").unwrap();
        let err = publish(&part, &final_path).unwrap_err();
        assert!(matches!(err, BackupError::Corrupt(_)));
        assert!(!part.exists());
        assert!(!final_path.exists());
    }

    #[test]
    fn prune_keeps_four_newest() {
        let dir = tempfile::tempdir().unwrap();
        for s in 0..6u32 {
            fs::write(dir.path().join(format!("kurobayomi_2026-10-08_18-3{s}-00.tachibk")), b"x").unwrap();
        }
        fs::write(dir.path().join("notes.txt"), b"n").unwrap();
        fs::write(dir.path().join("kurobayomi_x.tachibk"), b"n").unwrap();
        write_backup(&sample(), &Target::Automatic { dir: dir.path().to_path_buf(), now: at(59) }).unwrap();
        let left = names(dir.path());
        let matching: Vec<&String> = left.iter().filter(|n| is_automatic_name(n)).collect();
        assert_eq!(matching.len(), 4);
        assert!(matching.contains(&&"kurobayomi_2026-10-08_19-31-59.tachibk".to_string()), "new file kept");
        assert!(matching.contains(&&"kurobayomi_2026-10-08_18-35-00.tachibk".to_string()), "newest old kept");
        assert!(!left.contains(&"kurobayomi_2026-10-08_18-30-00.tachibk".to_string()));
        assert!(left.contains(&"notes.txt".to_string()));
        assert!(left.contains(&"kurobayomi_x.tachibk".to_string()));
    }

    #[test]
    fn prune_after_failed_write_keeps_existing_files() {
        // E-04: a write that fails must not delete older backups. The part path is blocked by a directory.
        let dir = tempfile::tempdir().unwrap();
        for s in 0..6u32 {
            fs::write(dir.path().join(format!("kurobayomi_2026-10-08_19-3{s}-00.tachibk")), b"x").unwrap();
        }
        let blocked = dir.path().join("kurobayomi_2026-10-08_19-31-59.tachibk.part");
        fs::create_dir(&blocked).unwrap();
        let err = write_backup(&sample(), &Target::Automatic { dir: dir.path().to_path_buf(), now: at(59) });
        assert!(err.is_err());
        let matching = names(dir.path()).into_iter().filter(|n| is_automatic_name(n)).count();
        assert_eq!(matching, 6);
    }

    #[test]
    fn prune_ignores_non_matching() {
        let dir = tempfile::tempdir().unwrap();
        for n in ["kurobayomi_x.tachibk", "notes.txt", "xyz.jmir.tachiyomi.mi_2026-10-08_19-31.tachibk"] {
            fs::write(dir.path().join(n), b"x").unwrap();
        }
        assert_eq!(prune_automatic(dir.path(), 0).unwrap(), 0);
        assert_eq!(names(dir.path()).len(), 3);
    }

    #[test]
    fn stale_part_removed_on_start_real_backups_kept() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("kurobayomi_2026-10-08_19-31-00.tachibk.part"), b"x").unwrap();
        fs::write(dir.path().join("kurobayomi_2026-10-08_19-31-00.tachibk"), b"x").unwrap();
        assert_eq!(remove_stale_parts(dir.path()).unwrap(), 1);
        assert_eq!(names(dir.path()), vec!["kurobayomi_2026-10-08_19-31-00.tachibk".to_string()]);
        assert_eq!(remove_stale_parts(&dir.path().join("absent")).unwrap(), 0);
    }

    #[test]
    fn automatic_dir_created_and_missing_base_fails() {
        let base = tempfile::tempdir().unwrap();
        let auto = base.path().join("autobackup");
        ensure_auto_dir(&auto).unwrap();
        assert!(auto.is_dir());
        let gone = base.path().join("no-such-base").join("autobackup");
        assert!(matches!(ensure_auto_dir(&gone), Err(BackupError::Io(_))));
    }

    #[test]
    fn end_to_end_auto_retention_six_runs() {
        let dir = tempfile::tempdir().unwrap();
        for s in 0..6u32 {
            write_backup(&sample(), &Target::Automatic { dir: dir.path().to_path_buf(), now: at(s) }).unwrap();
        }
        let matching: Vec<String> = names(dir.path()).into_iter().filter(|n| is_automatic_name(n)).collect();
        assert_eq!(matching.len(), 4);
        assert_eq!(matching.first().unwrap(), "kurobayomi_2026-10-08_19-31-02.tachibk");
        assert_eq!(matching.last().unwrap(), "kurobayomi_2026-10-08_19-31-05.tachibk");
    }
}
