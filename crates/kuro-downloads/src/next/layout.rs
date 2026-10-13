//! One downloads layout per kind: folder discovery, creation, rename, prune and the startup sweep (merges `layout.rs`
//! and `anime/layout.rs`; the names are in `naming.rs`).
//!
//! Tree: `<root>/<source dir>/<entry dir>/<unit>`. Lookups never create folders. Only [`Layout::entry_dir`] creates one.
//! The root is required. A source is passed as its folder name, [`Layout::source_dir_name`], and a unit as a
//! [`UnitName`], so this file does not depend on the shared unit types of lane A1.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kuro_platform::kind::Kind;
use tracing::debug;

use super::naming::{
    self, CBZ_PART_SUFFIX, TMP_DIR_SUFFIX, UnitName, entry_dir_name, staging_dir_names, unit_dir_name, unit_dir_names,
};
use crate::error::DownloadError;
use crate::pipeline::MIN_FREE_BYTES;

/// `rename` for a download folder. A rename that would cross filesystems is named as such (T-24a-071, E-F16). There is no
/// copy fallback: the folder is left where it is and the error is shown.
pub fn rename_dir(from: &Path, to: &Path) -> Result<(), DownloadError> {
    fs::rename(from, to).map_err(|err| rename_error(err, to))
}

/// The error of a failed rename: a cross-device move is `CrossDevice` with the target, anything else is I/O.
fn rename_error(err: io::Error, to: &Path) -> DownloadError {
    if err.kind() == io::ErrorKind::CrossesDevices {
        DownloadError::CrossDevice(to.display().to_string())
    } else {
        DownloadError::Io(err)
    }
}

/// Renames a folder or file and fails when the target exists, where `rename(2)` would replace an empty folder.
fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("{} already exists", to.display())));
    }
    fs::rename(from, to)
}

/// A case-only rename goes through `<to>_tmp`, which a case-insensitive disk needs. If the second step fails, the
/// folder goes back to its old name.
fn rename_folder(from: &Path, to: &Path, case_only: bool) -> io::Result<()> {
    if !case_only {
        return rename_new(from, to);
    }
    let mut staging = to.as_os_str().to_owned();
    staging.push(TMP_DIR_SUFFIX);
    let staging = PathBuf::from(staging);
    rename_new(from, &staging)?;
    rename_new(&staging, to).inspect_err(|_| {
        let _ = fs::rename(&staging, from);
    })
}

fn children(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path).into_iter().flatten().flatten().map(|e| e.path()).collect()
}

fn is_empty_dir(path: &Path) -> bool {
    fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none())
}

/// Removes an empty folder. A folder that is already gone counts as removed.
fn remove_empty_dir(path: &Path) -> io::Result<()> {
    match fs::remove_dir(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// What [`Layout::prune_empty`] removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    pub entry_removed: bool,
    pub source_removed: bool,
}

/// Free-space precheck (T-24a-017). Below 200 MiB the unit fails with `LowSpace`. An unknown value (the `statvfs` call
/// failed) skips the check.
pub fn check_free_space(free: Option<u64>) -> Result<(), DownloadError> {
    match free {
        Some(free) if free < MIN_FREE_BYTES => Err(DownloadError::LowSpace { free, need: MIN_FREE_BYTES }),
        _ => Ok(()),
    }
}

/// The downloads root of one kind, plus the naming rules for every level.
#[derive(Clone, Debug)]
pub struct Layout {
    kind: Kind,
    root: PathBuf,
}

impl Layout {
    pub fn new(kind: Kind, root: impl Into<PathBuf>) -> Self {
        Self { kind, root: root.into() }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `"<name> (<LANG>)"` sanitised. One function for the app and for the orphan-folder scan.
    pub fn source_dir_name(name: &str, lang: &str) -> String {
        naming::source_dir_name(name, lang)
    }

    /// The sanitised title.
    pub fn entry_dir_name(title: &str) -> String {
        entry_dir_name(title)
    }

    /// The names a unit may have on disk: the current one, then the legacy one (a `.cbz` for manga).
    pub fn unit_dir_names(&self, unit: UnitName<'_>) -> [String; 2] {
        unit_dir_names(self.kind, unit)
    }

    fn source_path(&self, source_dir: &str) -> PathBuf {
        self.root.join(source_dir)
    }

    fn entry_path(&self, source_dir: &str, title: &str) -> PathBuf {
        self.source_path(source_dir).join(entry_dir_name(title))
    }

    /// `<root>/<source>/<entry>`, created when missing. A failed creation is `InvalidLocation` with the root path.
    pub fn entry_dir(&self, source_dir: &str, title: &str) -> Result<PathBuf, DownloadError> {
        let dir = self.entry_path(source_dir, title);
        fs::create_dir_all(&dir).map_err(|_| DownloadError::InvalidLocation(self.root.display().to_string()))?;
        Ok(dir)
    }

    /// The existing source folder. Does not create it.
    pub fn find_source_dir(&self, source_dir: &str) -> Option<PathBuf> {
        Some(self.source_path(source_dir)).filter(|p| p.is_dir())
    }

    /// The existing entry folder. Does not create it.
    pub fn find_entry_dir(&self, source_dir: &str, title: &str) -> Option<PathBuf> {
        Some(self.entry_path(source_dir, title)).filter(|p| p.is_dir())
    }

    /// The finished unit (folder or file) under its current or legacy name; the first that exists wins. `None` when the
    /// entry folder is missing.
    pub fn find_unit(&self, source_dir: &str, title: &str, unit: UnitName<'_>) -> Option<PathBuf> {
        let entry_dir = self.find_entry_dir(source_dir, title)?;
        self.unit_dir_names(unit).into_iter().map(|n| entry_dir.join(n)).find(|p| p.exists())
    }

    /// The staging folders a unit may use: `<name>_tmp` for each of its names. Pure path building: nothing is created
    /// and nothing is checked for existence.
    pub fn staging_dirs(&self, source_dir: &str, title: &str, unit: UnitName<'_>) -> Vec<PathBuf> {
        let entry_path = self.entry_path(source_dir, title);
        staging_dir_names(self.kind, unit).into_iter().map(|n| entry_path.join(n)).collect()
    }

    /// `(entry folder, finished units found, staging folders found)`. `(None, [], [])` when the entry folder is missing.
    /// Deleting a unit removes its partial download too (E-D01).
    pub fn find_units(
        &self,
        source_dir: &str,
        title: &str,
        units: &[UnitName<'_>],
    ) -> (Option<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
        let Some(entry_dir) = self.find_entry_dir(source_dir, title) else {
            return (None, Vec::new(), Vec::new());
        };
        let mut finished = Vec::new();
        let mut staging: Vec<PathBuf> = Vec::new();
        for unit in units {
            if let Some(path) = self.unit_dir_names(*unit).into_iter().map(|n| entry_dir.join(n)).find(|p| p.exists()) {
                finished.push(path);
            }
            for name in staging_dir_names(self.kind, *unit) {
                let path = entry_dir.join(name);
                if path.is_dir() && !staging.contains(&path) {
                    staging.push(path);
                }
            }
        }
        (Some(entry_dir), finished, staging)
    }

    /// Renames the first existing of the old unit's names to the new unit's current name, keeping a file's extension
    /// (`.cbz`, `.mkv`). Returns the new on-disk name. `Ok(None)` when the unit is not downloaded or the name already
    /// matches. A target that exists fails and keeps the old folder (EC-104). Needs no entry folder to exist.
    pub fn rename_unit(
        &self,
        source_dir: &str,
        title: &str,
        old: UnitName<'_>,
        new: UnitName<'_>,
    ) -> Result<Option<String>, DownloadError> {
        let Some(entry_dir) = self.find_entry_dir(source_dir, title) else {
            return Ok(None);
        };
        let Some(found) = self.unit_dir_names(old).into_iter().map(|n| entry_dir.join(n)).find(|p| p.exists()) else {
            debug!(target: "kuro_downloads", "no downloaded unit to rename");
            return Ok(None);
        };
        let mut new_name = unit_dir_name(self.kind, new);
        if found.is_file() {
            if let Some(ext) = found.extension().and_then(|e| e.to_str()) {
                new_name = format!("{new_name}.{ext}");
            }
        }
        if found.file_name().is_some_and(|n| n == new_name.as_str()) {
            return Ok(None);
        }
        let target = entry_dir.join(&new_name);
        rename_new(&found, &target).map_err(|e| rename_error(e, &target))?;
        Ok(Some(new_name))
    }

    /// Renames the source folder `old_dir` to `new_dir` (both from [`Layout::source_dir_name`]). A name that differs only
    /// by case goes through `<new>_tmp`, which a case-insensitive disk needs. Returns the new folder name; `Ok(None)`
    /// when there is no such folder or the name already matches. A target that exists fails (EC-102).
    pub fn rename_source(&self, old_dir: &str, new_dir: &str) -> Result<Option<String>, DownloadError> {
        let Some(old_path) = self.find_source_dir(old_dir) else {
            return Ok(None);
        };
        if old_path.file_name().is_some_and(|n| n == new_dir) {
            return Ok(None);
        }
        let target = self.source_path(new_dir);
        let case_only = old_dir.to_lowercase() == new_dir.to_lowercase();
        rename_folder(&old_path, &target, case_only).map_err(|e| rename_error(e, &target))?;
        Ok(Some(new_dir.to_owned()))
    }

    /// Removes the entry folder, then the source folder, when each one is empty. A folder that is not there is skipped.
    pub fn prune_empty(&self, source_dir: &str, title: &str) -> io::Result<Pruned> {
        let mut pruned = Pruned::default();
        if let Some(dir) = self.find_entry_dir(source_dir, title) {
            if is_empty_dir(&dir) {
                remove_empty_dir(&dir)?;
                pruned.entry_removed = true;
            }
        }
        if let Some(dir) = self.find_source_dir(source_dir) {
            if is_empty_dir(&dir) {
                remove_empty_dir(&dir)?;
                pruned.source_removed = true;
            }
        }
        Ok(pruned)
    }

    /// Startup sweep (T-23a-044), after the queue was restored. Under the root, every staging folder (`<name>_tmp`) and
    /// every `.cbz.part` file is removed, except the folders in `keep`, which belong to queued units and wait for a
    /// resume. Only the `<source>/<entry>` levels are walked. Returns how many entries were removed.
    pub fn sweep_staging(&self, keep: &[PathBuf]) -> usize {
        let mut removed = 0;
        for source in children(&self.root).into_iter().filter(|p| p.is_dir()) {
            for entry_dir in children(&source).into_iter().filter(|p| p.is_dir()) {
                for path in children(&entry_dir) {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let stray_tmp = path.is_dir() && name.ends_with(TMP_DIR_SUFFIX) && !keep.contains(&path);
                    let stray_part = path.is_file() && name.ends_with(CBZ_PART_SUFFIX);
                    if (stray_tmp || stray_part) && Self::remove_path(&path).is_ok() {
                        removed += 1;
                    }
                }
            }
        }
        removed
    }

    /// Deletes a folder or a file. A path that is already gone is `Ok` (B10): a delete never fails because the user
    /// removed the folder by hand.
    pub fn remove_path(path: &Path) -> io::Result<()> {
        let result = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
        match result {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn un<'a>(name: &'a str, scanlator: Option<&'a str>) -> UnitName<'a> {
        UnitName::new(name, scanlator)
    }

    const SRC: &str = "Src (EN)";

    #[test]
    fn source_and_entry_dir_names_are_sanitised() {
        assert_eq!(Layout::source_dir_name("MangaDex", "en"), "MangaDex (EN)");
        assert_eq!(Layout::entry_dir_name("Mangá/Ż"), "Mangá_Ż");
    }

    #[test]
    fn entry_dir_creates_and_lookups_do_not() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let layout = Layout::new(kind, tmp.path());
            assert_eq!(layout.kind(), kind);
            assert_eq!(layout.root(), tmp.path());
            assert!(layout.find_entry_dir(SRC, "M").is_none());
            assert!(layout.find_source_dir(SRC).is_none());
            assert!(!tmp.path().join(SRC).exists(), "find never creates");
            let dir = layout.entry_dir(SRC, "M").unwrap();
            assert!(dir.is_dir());
            assert_eq!(layout.find_entry_dir(SRC, "M"), Some(dir));
            assert_eq!(layout.find_source_dir(SRC), Some(tmp.path().join(SRC)));
        }
    }

    #[test]
    fn a_root_that_is_a_file_is_invalid_location_with_its_path() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let file = tmp.path().join("not-a-dir");
            fs::write(&file, b"x").unwrap();
            let layout = Layout::new(kind, file.clone());
            match layout.entry_dir(SRC, "M") {
                Err(DownloadError::InvalidLocation(path)) => assert_eq!(path, file.display().to_string()),
                other => panic!("expected InvalidLocation, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_manga_unit_prefers_the_folder_and_falls_back_to_the_cbz() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::new(Kind::Manga, tmp.path());
        let dir = layout.entry_dir(SRC, "M").unwrap();
        fs::write(dir.join("Ch 1.cbz"), b"x").unwrap();
        assert_eq!(layout.find_unit(SRC, "M", un("Ch 1", None)), Some(dir.join("Ch 1.cbz")));
        fs::create_dir(dir.join("Ch 1")).unwrap();
        assert_eq!(layout.find_unit(SRC, "M", un("Ch 1", None)), Some(dir.join("Ch 1")));
        assert_eq!(layout.find_unit(SRC, "Other", un("Ch 1", None)), None);
    }

    #[test]
    fn an_anime_unit_finds_the_legacy_name_and_prefers_the_new_one() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::new(Kind::Anime, tmp.path());
        let dir = layout.entry_dir(SRC, "Show").unwrap();
        fs::create_dir(dir.join("_Ep 1")).unwrap();
        let ep = un("Ep 1", Some(" "));
        assert_eq!(layout.find_unit(SRC, "Show", ep), Some(dir.join("_Ep 1")), "legacy name found");
        fs::create_dir(dir.join("Ep 1")).unwrap();
        assert_eq!(layout.find_unit(SRC, "Show", ep), Some(dir.join("Ep 1")), "new name wins");
    }

    #[test]
    fn find_units_is_none_without_an_entry_folder_and_lists_finished_and_staging() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let layout = Layout::new(kind, tmp.path());
            let units = [un("Ch 1", None), un("Ch 2", None)];
            assert_eq!(layout.find_units(SRC, "M", &units), (None, Vec::new(), Vec::new()));
            let dir = layout.entry_dir(SRC, "M").unwrap();
            fs::create_dir(dir.join("Ch 1")).unwrap();
            fs::create_dir(dir.join("Ch 2_tmp")).unwrap();
            let (found_dir, finished, staging) = layout.find_units(SRC, "M", &units);
            assert_eq!(found_dir, Some(dir.clone()));
            assert_eq!(finished, vec![dir.join("Ch 1")]);
            assert_eq!(staging, vec![dir.join("Ch 2_tmp")], "a partial download is found too");
        }
    }

    #[test]
    fn staging_candidates_follow_the_unit_names_and_create_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let anime = Layout::new(Kind::Anime, tmp.path());
        let dir = tmp.path().join(SRC).join("Show");
        assert_eq!(anime.staging_dirs(SRC, "Show", un("Ep 1", Some(" "))), vec![dir.join("Ep 1_tmp"), dir.join("_Ep 1_tmp")]);
        let manga = Layout::new(Kind::Manga, tmp.path());
        assert_eq!(manga.staging_dirs(SRC, "Show", un("Ch 1", Some("Scan"))), vec![dir.join("Scan_Ch 1_tmp")]);
        assert!(!tmp.path().join(SRC).exists(), "nothing is created");
    }

    #[test]
    fn the_startup_sweep_removes_stray_staging_folders_and_part_files_only() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let layout = Layout::new(kind, tmp.path());
            let dir = layout.entry_dir(SRC, "M").unwrap();
            fs::create_dir(dir.join("Ch 1_tmp")).unwrap();
            fs::write(dir.join("Ch 1_tmp").join("0001.jpg.tmp"), b"x").unwrap();
            fs::create_dir(dir.join("Ch 2_tmp")).unwrap();
            fs::write(dir.join("Ch 3.cbz.part"), b"x").unwrap();
            fs::write(dir.join("Ch 4.cbz"), b"x").unwrap();
            fs::create_dir(dir.join("Ch 5")).unwrap();
            let kept = dir.join("Ch 2_tmp");
            assert_eq!(layout.sweep_staging(std::slice::from_ref(&kept)), 2, "Ch 1_tmp and Ch 3.cbz.part");
            assert!(!dir.join("Ch 1_tmp").exists());
            assert!(kept.is_dir(), "a queued unit's staging folder waits for the resume (B20)");
            assert!(!dir.join("Ch 3.cbz.part").exists());
            assert!(dir.join("Ch 4.cbz").is_file() && dir.join("Ch 5").is_dir(), "finished units are never touched");
            assert_eq!(layout.sweep_staging(&[]), 1, "the kept folder goes once it is no longer queued");
        }
    }

    #[test]
    fn rename_unit_moves_the_legacy_anime_folder_to_the_new_name() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::new(Kind::Anime, tmp.path());
        let dir = layout.entry_dir(SRC, "Show").unwrap();
        fs::create_dir(dir.join("_Ep 1")).unwrap();
        let renamed = layout.rename_unit(SRC, "Show", un("Ep 1", Some(" ")), un("Ep 1 renamed", None)).unwrap();
        assert_eq!(renamed, Some("Ep 1 renamed".to_owned()));
        assert!(!dir.join("_Ep 1").exists());
        assert!(dir.join("Ep 1 renamed").is_dir());
    }

    #[test]
    fn rename_unit_keeps_the_extension_of_a_file_unit() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::new(Kind::Manga, tmp.path());
        let dir = layout.entry_dir(SRC, "M").unwrap();
        fs::write(dir.join("Ch 1.cbz"), b"x").unwrap();
        let renamed = layout.rename_unit(SRC, "M", un("Ch 1", None), un("Ch 1b", Some("S"))).unwrap();
        assert_eq!(renamed, Some("S_Ch 1b.cbz".to_owned()));
        assert!(dir.join("S_Ch 1b.cbz").is_file());
        assert!(!dir.join("Ch 1.cbz").exists());
    }

    #[test]
    fn rename_unit_is_a_no_op_when_names_match_or_nothing_is_downloaded() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let layout = Layout::new(kind, tmp.path());
            let ep = un("Ep 1", None);
            assert_eq!(layout.rename_unit(SRC, "Show", ep, un("Ep 2", None)).unwrap(), None, "no entry folder");
            let dir = layout.entry_dir(SRC, "Show").unwrap();
            assert_eq!(layout.rename_unit(SRC, "Show", ep, un("Ep 2", None)).unwrap(), None, "not downloaded");
            assert!(!dir.join("Ep 2").exists(), "nothing is created");
            fs::create_dir(dir.join("Ep 1")).unwrap();
            assert_eq!(layout.rename_unit(SRC, "Show", ep, un("Ep 1", Some(" "))).unwrap(), None, "same name");
            assert!(dir.join("Ep 1").is_dir());
        }
    }

    #[test]
    fn rename_unit_fails_on_an_existing_target_and_keeps_the_old_folder() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let layout = Layout::new(kind, tmp.path());
            let dir = layout.entry_dir(SRC, "Show").unwrap();
            fs::create_dir(dir.join("Ep 1")).unwrap();
            fs::create_dir(dir.join("Ep 2")).unwrap();
            let result = layout.rename_unit(SRC, "Show", un("Ep 1", None), un("Ep 2", None));
            assert!(matches!(result, Err(DownloadError::Io(_))), "got {result:?}");
            assert!(dir.join("Ep 1").is_dir() && dir.join("Ep 2").is_dir());
        }
    }

    #[test]
    fn rename_source_renames_and_handles_a_case_only_change() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let layout = Layout::new(kind, tmp.path());
            fs::create_dir_all(tmp.path().join("Old (EN)")).unwrap();
            assert_eq!(layout.rename_source("Old (EN)", "New (EN)").unwrap(), Some("New (EN)".to_owned()));
            assert!(tmp.path().join("New (EN)").is_dir());
            assert!(!tmp.path().join("Old (EN)").exists());
            assert_eq!(layout.rename_source("New (EN)", "new (EN)").unwrap(), Some("new (EN)".to_owned()), "case-only");
            assert!(tmp.path().join("new (EN)").is_dir());
            assert!(!tmp.path().join("new (EN)_tmp").exists(), "staging name is gone");
            assert_eq!(layout.rename_source("Missing (EN)", "X (EN)").unwrap(), None);
            assert_eq!(layout.rename_source("new (EN)", "new (EN)").unwrap(), None, "same name");
        }
    }

    #[test]
    fn rename_source_fails_on_an_existing_target_and_keeps_the_old_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = Layout::new(Kind::Manga, tmp.path());
        fs::create_dir_all(tmp.path().join("A (EN)")).unwrap();
        fs::create_dir_all(tmp.path().join("B (EN)")).unwrap();
        assert!(matches!(layout.rename_source("A (EN)", "B (EN)"), Err(DownloadError::Io(_))));
        assert!(tmp.path().join("A (EN)").is_dir() && tmp.path().join("B (EN)").is_dir());
    }

    #[test]
    fn a_cross_device_rename_is_named_and_other_failures_stay_io() {
        let to = Path::new("/mnt/other/Ep 1");
        let cross = rename_error(io::Error::new(io::ErrorKind::CrossesDevices, "EXDEV"), to);
        assert_eq!(cross.to_string(), "Cannot move the download to another disk: /mnt/other/Ep 1");
        assert!(matches!(rename_error(io::Error::other("busy"), to), DownloadError::Io(_)));
    }

    #[test]
    fn free_space_precheck_uses_a_strict_200_mib_floor() {
        assert!(check_free_space(Some(MIN_FREE_BYTES)).is_ok());
        assert!(matches!(check_free_space(Some(MIN_FREE_BYTES - 1)), Err(DownloadError::LowSpace { .. })));
        assert!(check_free_space(None).is_ok(), "a statvfs error skips the check");
        assert_eq!(MIN_FREE_BYTES, 209_715_200);
    }

    #[test]
    fn remove_path_tolerates_a_missing_path_for_a_folder_and_a_file() {
        let tmp = tempfile::tempdir().unwrap();
        let folder = tmp.path().join("folder");
        let file = tmp.path().join("file.cbz");
        fs::create_dir_all(folder.join("inner")).unwrap();
        fs::write(&file, b"x").unwrap();
        assert!(Layout::remove_path(&folder).is_ok());
        assert!(Layout::remove_path(&file).is_ok());
        assert!(!folder.exists() && !file.exists());
        assert!(Layout::remove_path(&folder).is_ok(), "already gone: NotFound is Ok");
        assert!(Layout::remove_path(&file).is_ok());
        assert!(Layout::remove_path(&tmp.path().join("never-existed")).is_ok());
    }

    #[test]
    fn prune_empty_removes_the_entry_folder_then_the_source_folder() {
        for kind in Kind::ALL {
            let tmp = tempfile::tempdir().unwrap();
            let layout = Layout::new(kind, tmp.path());
            let a = layout.entry_dir(SRC, "A").unwrap();
            let b = layout.entry_dir(SRC, "B").unwrap();
            fs::write(b.join("keep"), b"x").unwrap();
            assert_eq!(layout.prune_empty(SRC, "A").unwrap(), Pruned { entry_removed: true, source_removed: false });
            assert!(!a.exists() && b.is_dir());
            assert_eq!(layout.prune_empty(SRC, "B").unwrap(), Pruned::default(), "a non-empty folder stays");
            Layout::remove_path(&b).unwrap();
            assert_eq!(layout.prune_empty(SRC, "B").unwrap(), Pruned { entry_removed: false, source_removed: true });
            assert!(!tmp.path().join(SRC).exists());
            assert_eq!(layout.prune_empty(SRC, "B").unwrap(), Pruned::default(), "nothing left: nothing to do");
        }
    }

    #[test]
    fn the_new_and_the_legacy_fixture_trees_are_found_by_their_folder_names() {
        let new = crate::testing::dl_fixture("tree_new");
        let layout = Layout::new(Kind::Anime, new.path());
        let found = layout.find_unit("Example (EN)", "Show", un("Episode 1", Some("Scan")));
        assert_eq!(found, Some(new.path().join("Example (EN)/Show/Scan_Episode 1")));
        assert!(new.path().join("Example (EN)/Show/Scan_Episode 1/.nomedia").is_file());

        let legacy = crate::testing::dl_fixture("tree_legacy");
        let layout = Layout::new(Kind::Anime, legacy.path());
        let found = layout.find_unit("Example (EN)", "Show", un("Episode 2", Some("Scan")));
        assert_eq!(found, Some(legacy.path().join("Example (EN)/Show/Scan_Episode 2")));
        assert_eq!(layout.find_unit("Example (EN)", "Show", un("Episode 9", None)), None);
    }

    #[test]
    fn a_case_only_source_rename_keeps_the_fixture_tree_content() {
        let tree = crate::testing::dl_fixture("tree_new");
        let layout = Layout::new(Kind::Anime, tree.path());
        assert_eq!(layout.rename_source("Example (EN)", "EXAMPLE (EN)").unwrap(), Some("EXAMPLE (EN)".to_owned()));
        let names: Vec<String> =
            fs::read_dir(tree.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["EXAMPLE (EN)".to_owned()], "the directory ends with the new spelling");
        assert!(tree.path().join("EXAMPLE (EN)/Show/Scan_Episode 1/Episode 1.mkv").is_file());
    }
}
