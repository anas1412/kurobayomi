//! Filesystem layer over `std::fs`: listing, nested lookup, safe join, `.nomedia` and `file://` URLs (area:14 section 3).

use std::fs::{self, Metadata};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

/// One entry of a folder. `is_dir` and `is_file` follow symlinks, as Java `File` does. A dangling link is neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_file: bool,
    /// Milliseconds since the epoch, `0` when unknown.
    pub modified_ms: i64,
}

/// The entries of `dir`, sorted by name. Kotlin's `listFiles()` order is undefined, so Rust fixes it.
pub fn list_dir(dir: &Path) -> io::Result<Vec<FsEntry>> {
    let mut entries = Vec::new();
    for item in fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        let meta = fs::metadata(&path).ok();
        entries.push(FsEntry {
            name: item.file_name().to_string_lossy().into_owned(),
            is_dir: meta.as_ref().is_some_and(Metadata::is_dir),
            is_file: meta.as_ref().is_some_and(Metadata::is_file),
            modified_ms: meta.as_ref().map_or(0, modified_ms),
            path,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Milliseconds since the epoch of the last modification, `0` when the platform does not give one.
pub fn modified_ms(meta: &Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// `base` joined with a `/`-separated relative path. Each segment must be a plain name: no empty segment, no `.`,
/// no `..`, no NUL, no backslash, no drive (`C:x` would replace `base` on Windows). An absolute path has an empty first
/// segment, so it is refused too.
pub fn safe_join(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut out = base.to_path_buf();
    for part in rel.split('/') {
        let drive = Path::new(part).components().any(|c| matches!(c, Component::Prefix(_)));
        if part.is_empty() || part == "." || part == ".." || part.contains(['\0', '\\']) || drive {
            return None;
        }
        out.push(part);
    }
    Some(out)
}

/// The path a local url names. A relative url is a title (or a file of one) inside the Local folder `root`; an absolute
/// url is a linked folder outside it (or a file of one): the system's root (`/`, or a drive such as `C:/`), then parts
/// checked the same way: no `.`, `..`, empty part, NUL or backslash. The one place a local url becomes a path, for the
/// source and the app alike.
pub fn resolve(root: &Path, url: &str) -> Option<PathBuf> {
    if !Path::new(url).is_absolute() {
        return safe_join(root, url);
    }
    let (mut base, mut skip) = (PathBuf::new(), 0);
    for part in Path::new(url).components() {
        match part {
            Component::Prefix(drive) => skip += drive.as_os_str().len(),
            Component::RootDir => skip += 1,
            _ => break,
        }
        base.push(part);
    }
    safe_join(&base, url.get(skip..)?)
}

/// The local url of an absolute path (a linked folder): its parts joined with `/`, as every local url is, so
/// `C:\Manga\Berserk` is `C:/Manga/Berserk`. `None` for a relative path, a `.` or `..` part, or a name that is not
/// valid text.
// ponytail: a drive letter or `/` root only; a network share (`\\server\share`) is refused, add it when asked for
// (no root text to skip in `resolve`)
pub fn absolute_url(path: &Path) -> Option<String> {
    if !path.is_absolute() {
        return None;
    }
    let mut url = String::new();
    for part in path.components() {
        match part {
            Component::Prefix(drive) if matches!(drive.kind(), std::path::Prefix::Disk(_)) => url.push_str(drive.as_os_str().to_str()?),
            Component::RootDir => url.push('/'),
            Component::Normal(name) => {
                if !url.ends_with('/') {
                    url.push('/');
                }
                url.push_str(name.to_str()?);
            }
            _ => return None,
        }
    }
    Some(url)
}

/// Kotlin `findFile(name)` on a folder, for a nested `a/b` path or a linked folder's absolute path ([`resolve`]). `None`
/// when missing or unsafe.
pub fn find_child(dir: &Path, rel: &str) -> Option<PathBuf> {
    resolve(dir, rel).filter(|path| path.exists())
}

/// Kotlin `DiskUtil.createNoMediaFile`: an empty `.nomedia` in `dir`, created only when `dir` exists and has none.
pub fn create_nomedia(dir: &Path) -> io::Result<()> {
    let marker = dir.join(".nomedia");
    if dir.is_dir() && !marker.exists() {
        fs::File::create(marker)?;
    }
    Ok(())
}

/// The `file://` URL of a path, as Kotlin's `uri.toString()` gives it for a raw file. `None` for a relative path.
pub fn file_url(path: &Path) -> Option<String> {
    url::Url::from_file_path(path).ok().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_is_sorted_and_reports_kinds() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join("b"))?;
        fs::write(dir.path().join("a.cbz"), b"x")?;
        let entries = list_dir(dir.path())?;
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["a.cbz", "b"]);
        assert!(entries[0].is_file && !entries[0].is_dir);
        assert!(entries[1].is_dir && !entries[1].is_file);
        assert!(entries[0].modified_ms > 0);
        Ok(())
    }

    #[test]
    fn a_missing_folder_is_an_error_for_the_caller_to_map() {
        let dir = tempfile::tempdir().expect("temp dir");
        let err = list_dir(&dir.path().join("nope")).expect_err("missing folder");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn safe_join_accepts_nested_relative_names() {
        let base = Path::new("/root");
        assert_eq!(
            safe_join(base, "Show/Season 1/ep.mkv"),
            Some(base.join("Show").join("Season 1").join("ep.mkv"))
        );
    }

    #[test]
    fn safe_join_rejects_traversal() {
        let base = Path::new("/root");
        assert_eq!(safe_join(base, "../x"), None);
        assert_eq!(safe_join(base, "a/../x"), None);
        assert_eq!(safe_join(base, "./x"), None);
    }

    #[test]
    fn safe_join_rejects_absolute_empty_nul_and_backslash() {
        let base = Path::new("/root");
        assert_eq!(safe_join(base, "/etc/passwd"), None);
        assert_eq!(safe_join(base, "a//b"), None);
        assert_eq!(safe_join(base, ""), None);
        assert_eq!(safe_join(base, "a\0b"), None);
        assert_eq!(safe_join(base, "a\\b"), None);
    }

    #[test]
    fn a_linked_folder_resolves_by_its_absolute_path_with_the_same_checks() -> io::Result<()> {
        let local = tempfile::tempdir()?;
        let linked = tempfile::tempdir()?;
        fs::create_dir(linked.path().join("Show"))?;
        let show = linked.path().join("Show");
        let url = absolute_url(&show).expect("utf-8 temp path");
        assert_eq!(find_child(local.path(), &url), Some(show.clone()), "an absolute url is the linked folder itself");
        assert_eq!(find_child(local.path(), &format!("{url}/missing.mkv")), None);
        assert_eq!(resolve(local.path(), "Title/ep.mkv"), Some(local.path().join("Title/ep.mkv")), "a relative url stays under Local");
        assert_eq!(resolve(local.path(), "/tmp/../etc"), None, "traversal is refused in both forms");
        assert_eq!(resolve(local.path(), "/a//b"), None);
        Ok(())
    }

    #[test]
    fn find_child_follows_nested_paths_and_misses_cleanly() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir_all(dir.path().join("Show").join("S1"))?;
        fs::write(dir.path().join("Show").join("S1").join("ep.mkv"), b"x")?;
        assert_eq!(
            find_child(dir.path(), "Show/S1/ep.mkv"),
            Some(dir.path().join("Show/S1/ep.mkv"))
        );
        assert_eq!(find_child(dir.path(), "Show/S1/missing.mkv"), None);
        assert_eq!(find_child(dir.path(), "../escape"), None);
        Ok(())
    }

    #[test]
    fn nomedia_is_created_once_and_only_in_an_existing_folder() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        create_nomedia(dir.path())?;
        let marker = dir.path().join(".nomedia");
        assert!(marker.is_file());
        fs::write(&marker, b"kept")?;
        create_nomedia(dir.path())?;
        assert_eq!(fs::read(&marker)?, b"kept");
        create_nomedia(&dir.path().join("absent"))?;
        assert!(!dir.path().join("absent").exists());
        Ok(())
    }

    #[test]
    fn file_urls_need_an_absolute_path() {
        let url = file_url(&std::env::temp_dir().join("local/a b/cover.jpg")).expect("the temp folder is absolute");
        assert!(url.starts_with("file:///") && url.ends_with("/local/a%20b/cover.jpg"), "{url}");
        assert_eq!(file_url(Path::new("relative/cover.jpg")), None);
    }

    #[test]
    fn an_absolute_path_and_its_url_round_trip_with_slashes() {
        let dir = tempfile::tempdir().unwrap();
        let show = dir.path().join("Shelf").join("A Show");
        let url = absolute_url(&show).expect("an absolute path");
        assert!(!url.contains('\\') && url.ends_with("/Shelf/A Show"), "{url}");
        assert_eq!(resolve(Path::new("unused"), &url), Some(show.clone()));
        assert_eq!(resolve(Path::new("unused"), &format!("{url}/Ep 1.mkv")), Some(show.join("Ep 1.mkv")));
        assert_eq!(absolute_url(Path::new("relative/A Show")), None);
        // on Windows a drive part would replace the base; elsewhere `C:x` is a plain name
        assert_eq!(safe_join(dir.path(), "C:x").is_none(), cfg!(windows));
    }
}
