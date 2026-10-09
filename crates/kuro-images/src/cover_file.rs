//! Library cover file cache: writes the source to the cover file and promotes it (area:34 behavior section 8),
//! and the custom-cover write and delete paths (behavior section 18.6 and 18.7).
//!
//! Cover files are plain files under `<data>`, so they survive a cache clear. `write_atomic` is the
//! `write_source_to_cover_file` of Kotlin with a rename at the end, so a reader never sees a partial file (design section 6.4).

use std::io;
use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;
use tokio::io::{AsyncRead, AsyncWriteExt};

use crate::art::ArtRef;
use crate::cover_paths::{self, Family};
use crate::disk_image_cache::DiskImageCache;

/// Removes a temporary file on drop, unless it was disarmed after the rename. A failed or cancelled write
/// therefore leaves no `.part` file behind (behavior section 8.3 step 4, section 9.1 step 6).
struct PartFile(Option<PathBuf>);

impl PartFile {
    fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for PartFile {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Writes `bytes` to `target`: creates the parent folder, writes `<name>.part`, then renames it over
/// `target`. A reader that opened the old file keeps it, and a failure leaves `target` untouched.
pub async fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no file name"))?;
    let part = target.with_file_name(format!("{}.part", name.to_string_lossy()));
    let guard = PartFile(Some(part.clone()));
    tokio::fs::write(&part, bytes).await?;
    tokio::fs::rename(&part, target).await?;
    guard.disarm();
    Ok(())
}

/// Copies a disk-cache snapshot into the library cover file, then removes the disk entry, so a library
/// entry's image lives in one place (behavior section 6.3).
pub async fn promote_snapshot(
    disk: &DiskImageCache,
    key: &str,
    snapshot: &Path,
    cover: &Path,
) -> io::Result<()> {
    let bytes = tokio::fs::read(snapshot).await?;
    write_atomic(cover, &bytes).await?;
    disk.remove(key).await?;
    Ok(())
}

/// Copies the whole stream into the custom file, truncating an existing one. No validation of the bytes
/// (behavior section 18.6 and 24.3). Any I/O error is returned and nothing else is changed.
pub async fn set_custom<R>(
    paths: &dyn Paths,
    family: Family,
    entry_id: i64,
    mut reader: R,
) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let target = cover_paths::custom_file(paths, family, entry_id);
    let mut file = tokio::fs::File::create(&target).await?;
    tokio::io::copy(&mut reader, &mut file).await?;
    file.flush().await?;
    Ok(())
}

/// Deletes the custom file of an entry. `true` only when the file existed and was removed (behavior section 18.7).
pub fn delete_custom_cover(paths: &dyn Paths, family: Family, entry_id: i64) -> bool {
    remove_if_exists(&cover_paths::custom_file(paths, family, entry_id))
}

/// Deletes the thumbnail or background file of the image and, when asked, its custom file. Returns how many
/// files were actually deleted, so a thumbnail plus a custom cover counts 2 (behavior section 18.7).
pub fn delete_from_cache(paths: &dyn Paths, art: &ArtRef, delete_custom: bool) -> usize {
    let mut deleted = 0;
    if let Some(file) = cover_paths::cover_file_for(paths, art) {
        if remove_if_exists(&file) {
            deleted += 1;
        }
    }
    if delete_custom && delete_custom_cover(paths, art.family(), art.entry_id) {
        deleted += 1;
    }
    deleted
}

fn remove_if_exists(path: &Path) -> bool {
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "cannot delete cached image");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestPaths;

    #[tokio::test]
    async fn write_atomic_creates_parents_and_replaces_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("a/b/cover");
        write_atomic(&target, b"first").await.unwrap();
        write_atomic(&target, b"second").await.unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");
        assert!(!tmp.path().join("a/b/cover.part").exists());
    }

    #[tokio::test]
    async fn failed_write_leaves_no_part_file() {
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, b"file, not a folder").unwrap();
        let target = blocker.join("cover");
        assert!(write_atomic(&target, b"x").await.is_err());
        assert_eq!(std::fs::read(&blocker).unwrap(), b"file, not a folder");
    }

    #[tokio::test]
    async fn set_custom_truncates_an_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = TestPaths::new(tmp.path());
        set_custom(
            &paths,
            Family::MangaCover,
            3,
            &b"a much longer first body"[..],
        )
        .await
        .unwrap();
        set_custom(&paths, Family::MangaCover, 3, &b"short"[..])
            .await
            .unwrap();
        let file = cover_paths::custom_file(&paths, Family::MangaCover, 3);
        assert_eq!(std::fs::read(file).unwrap(), b"short");
    }

    #[tokio::test]
    async fn set_custom_accepts_any_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = TestPaths::new(tmp.path());
        set_custom(&paths, Family::AnimeBackground, 9, &b"not an image"[..])
            .await
            .unwrap();
        let file = cover_paths::custom_file(&paths, Family::AnimeBackground, 9);
        assert_eq!(std::fs::read(file).unwrap(), b"not an image");
    }

    #[test]
    fn delete_custom_is_true_only_when_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = TestPaths::new(tmp.path());
        assert!(!delete_custom_cover(&paths, Family::AnimeCover, 5));
        std::fs::write(
            cover_paths::custom_file(&paths, Family::AnimeCover, 5),
            b"x",
        )
        .unwrap();
        assert!(delete_custom_cover(&paths, Family::AnimeCover, 5));
        assert!(!delete_custom_cover(&paths, Family::AnimeCover, 5));
    }
}
