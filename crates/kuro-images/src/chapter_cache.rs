//! Page-list and page-image cache for chapters (area:34 section 6.3 and behavior section 20).
//!
//! Both kinds of entry live in `<cache>/chapter_disk_cache/` as `<md5(key)>.0`, the file DiskLruCache keeps for
//! value 0. A page list is keyed by `md5(manga_id + chapter_url)`, a page image by `md5(image_url)`. Writes go
//! to `<name>.0.part` and are renamed on success, the total is capped at 100 MiB, and the least recently used
//! file goes first (behavior sections 20.2 to 20.5). Kotlin's journal file is not kept: the folder is the index.

use std::io;
use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;
use serde::{Deserialize, Serialize};

use crate::cover_file::write_atomic;
use crate::disk_image_cache::DiskImageCache;
use crate::sniff::sniff;

/// The folder under `Paths::cache_dir()` (behavior section 20.1).
pub const FOLDER: &str = "chapter_disk_cache";
/// DiskLruCache parameters of `ChapterCache.kt` (data section 8.3). The journal version and the value count are
/// fixed by the file layout (`<key>.0`); they are kept as constants so the format stays documented.
pub const APP_VERSION: u32 = 1;
pub const VALUE_COUNT: u32 = 1;
pub const CACHE_SIZE: u64 = 100 * 1024 * 1024;
const VALUE_SUFFIX: &str = ".0";

/// One page of a chapter as the cache stores it. The source layer owns the full `Page`; this is the part that is
/// serialised (`index`, `url`, `imageUrl`), with unknown keys ignored and defaults left out (data section 6.2).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub index: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

pub struct ChapterCache {
    disk: DiskImageCache,
}

/// The key text of a page list: the ids concatenated with no separator (behavior section 20.2).
fn page_list_key(manga_id: i64, chapter_url: &str) -> String {
    format!("{manga_id}{chapter_url}")
}

impl ChapterCache {
    /// Opens the cache under `Paths::cache_dir()` with the 100 MiB limit.
    pub async fn open(paths: &dyn Paths) -> io::Result<Self> {
        Self::open_at(crate::platform::chapter_cache_dir(paths), CACHE_SIZE).await
    }

    pub async fn open_at(root: PathBuf, limit_bytes: u64) -> io::Result<Self> {
        Ok(Self {
            disk: DiskImageCache::open_named(root, limit_bytes, VALUE_SUFFIX).await?,
        })
    }

    /// The cached page list, or `None` for a missing, unreadable or corrupt entry. A corrupt file is deleted so
    /// the next load refetches (edge E-IMG-27).
    pub async fn get_page_list(&self, manga_id: i64, chapter_url: &str) -> Option<Vec<Page>> {
        let key = page_list_key(manga_id, chapter_url);
        let path = self.disk.get(&key).await?;
        let parsed = tokio::fs::read(&path)
            .await
            .map_err(|e| e.to_string())
            .and_then(|json| serde_json::from_slice::<Vec<Page>>(&json).map_err(|e| e.to_string()));
        match parsed {
            Ok(pages) => Some(pages),
            Err(error) => {
                tracing::warn!(%error, "Failed to read page list from cache");
                let _ = self.disk.remove(&key).await;
                None
            }
        }
    }

    /// Stores the page list. A failure is logged and ignored (behavior section 20.3).
    pub async fn put_page_list(&self, manga_id: i64, chapter_url: &str, pages: &[Page]) {
        let key = page_list_key(manga_id, chapter_url);
        let result = match serde_json::to_vec(pages) {
            Ok(json) => self.disk.put(&key, &json).await.map(|_| ()),
            Err(e) => Err(io::Error::other(e)),
        };
        if let Err(error) = result {
            tracing::warn!(%error, "Failed to put page list to cache");
        }
    }

    /// Whether the image of `image_url` is cached. Any error is `false` (behavior section 20.4).
    pub async fn is_image_cached(&self, image_url: &str) -> bool {
        self.disk.get(image_url).await.is_some()
    }

    /// Where the cached image of `image_url` lives. It does not check that the file exists, so call it only
    /// after `is_image_cached` returned true.
    pub fn image_file(&self, image_url: &str) -> PathBuf {
        self.disk.path_for(image_url)
    }

    /// Stores the bytes of a page image. Unlike `put_page_list` the error is returned, as in Kotlin, where the
    /// page then ends in the error state.
    pub async fn put_image(&self, image_url: &str, bytes: &[u8]) -> io::Result<()> {
        self.disk.put(image_url, bytes).await.map(|_| ())
    }

    /// Copies the cached page image of `image_url` into `dir` as `<stem>.<ext>`. The extension comes from sniffing
    /// the bytes, and `jpg` when nothing is recognised (behavior sections 17.4 and 20.8). `None` when the image is
    /// not cached, so the caller downloads it instead.
    pub async fn copy_image_to(
        &self,
        image_url: &str,
        dir: &Path,
        stem: &str,
    ) -> io::Result<Option<PathBuf>> {
        let Some(source) = self.disk.get(image_url).await else {
            return Ok(None);
        };
        let bytes = tokio::fs::read(&source).await?;
        let extension = sniff(&bytes).map_or("jpg", |format| format.extension());
        let target = dir.join(format!("{stem}.{extension}"));
        write_atomic(&target, &bytes).await?;
        Ok(Some(target))
    }

    /// Removes every cached page list and image and returns how many entries went (behavior section 20.6).
    pub async fn clear(&self) -> io::Result<usize> {
        self.disk.clear().await
    }

    /// Bytes held by the cache.
    pub async fn size(&self) -> u64 {
        self.disk.size().await
    }

    pub fn root(&self) -> &Path {
        self.disk.root()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::hash_key;
    use crate::testing::TestPaths;
    use std::time::Duration;

    async fn cache(root: &Path, limit: u64) -> ChapterCache {
        ChapterCache::open_at(root.join(FOLDER), limit)
            .await
            .unwrap()
    }

    fn pages() -> Vec<Page> {
        vec![
            Page {
                index: 0,
                url: "https://example.invalid/p/0".into(),
                image_url: Some("https://example.invalid/img/0.jpg".into()),
            },
            Page {
                index: 1,
                ..Page::default()
            },
        ]
    }

    #[test]
    fn constants_match_the_disk_lru_cache_parameters() {
        assert_eq!(APP_VERSION, 1);
        assert_eq!(VALUE_COUNT, 1);
        assert_eq!(CACHE_SIZE, 104_857_600);
        assert_eq!(FOLDER, "chapter_disk_cache");
    }

    #[tokio::test]
    async fn open_uses_the_cache_dir_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = ChapterCache::open(&TestPaths::new(tmp.path()))
            .await
            .unwrap();
        assert_eq!(
            cache.root(),
            tmp.path().join("cache").join("chapter_disk_cache")
        );
        assert!(cache.root().is_dir());
    }

    #[tokio::test]
    async fn miss_on_an_empty_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        assert_eq!(cache.get_page_list(7, "/chapter/1").await, None);
        assert!(!cache.is_image_cached("https://example.invalid/a.jpg").await);
    }

    #[tokio::test]
    async fn page_list_round_trip_and_file_name() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        cache.put_page_list(7, "/chapter/1", &pages()).await;
        assert_eq!(cache.get_page_list(7, "/chapter/1").await, Some(pages()));
        // md5 of the id and the url joined with no separator, plus the value index.
        let name = format!("{}.0", hash_key("7/chapter/1"));
        assert!(cache.root().join(name).is_file());
        // Another manga id with the same chapter url is another entry.
        assert_eq!(cache.get_page_list(8, "/chapter/1").await, None);
    }

    #[tokio::test]
    async fn default_values_are_left_out_of_the_json() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        cache.put_page_list(1, "u", &pages()).await;
        let file = cache.root().join(format!("{}.0", hash_key("1u")));
        assert_eq!(
            std::fs::read_to_string(file).unwrap(),
            r#"[{"index":0,"url":"https://example.invalid/p/0","imageUrl":"https://example.invalid/img/0.jpg"},{"index":1}]"#
        );
    }

    #[tokio::test]
    async fn unknown_keys_and_null_values_are_tolerated() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        let file = cache.root().join(format!("{}.0", hash_key("3c")));
        std::fs::write(
            &file,
            r#"[{"index":4,"extra":1,"imageUrl":null},{"index":5,"url":"u"}]"#,
        )
        .unwrap();
        let got = cache.get_page_list(3, "c").await.unwrap();
        assert_eq!(
            got[0],
            Page {
                index: 4,
                ..Page::default()
            }
        );
        assert_eq!(got[1].url, "u");
    }

    #[tokio::test]
    async fn corrupt_json_is_a_miss_and_the_file_is_deleted() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        let file = cache.root().join(format!("{}.0", hash_key("3c")));
        std::fs::write(&file, "{").unwrap();
        assert_eq!(cache.get_page_list(3, "c").await, None);
        assert!(!file.exists());
        // A page without `index` is corrupt too.
        std::fs::write(&file, r#"[{"url":"u"}]"#).unwrap();
        assert_eq!(cache.get_page_list(3, "c").await, None);
    }

    #[tokio::test]
    async fn put_page_list_failure_is_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        std::fs::remove_dir_all(cache.root()).unwrap();
        std::fs::write(cache.root(), b"a file where the folder was").unwrap();
        // Returns normally, with nothing stored.
        cache.put_page_list(1, "u", &pages()).await;
        assert_eq!(cache.get_page_list(1, "u").await, None);
    }

    #[tokio::test]
    async fn image_put_then_cached_then_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        let url = "https://example.invalid/img/1.jpg";
        assert!(!cache.is_image_cached(url).await);
        cache.put_image(url, b"jpeg bytes").await.unwrap();
        assert!(cache.is_image_cached(url).await);
        let file = cache.image_file(url);
        assert_eq!(file, cache.root().join(format!("{}.0", hash_key(url))));
        assert_eq!(std::fs::read(file).unwrap(), b"jpeg bytes");
        assert_eq!(cache.size().await, 10);
    }

    #[tokio::test]
    async fn image_file_does_not_check_existence() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        assert!(
            !cache
                .image_file("https://example.invalid/none.jpg")
                .exists()
        );
    }

    #[tokio::test]
    async fn a_failed_write_leaves_no_partial_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        std::fs::remove_dir_all(cache.root()).unwrap();
        std::fs::write(cache.root(), b"blocked").unwrap();
        assert!(cache.put_image("u", b"bytes").await.is_err());
        assert!(!cache.is_image_cached("u").await);
    }

    #[tokio::test]
    async fn writes_never_leave_part_files_visible() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        cache.put_image("u", &vec![1u8; 4096]).await.unwrap();
        let names: Vec<String> = std::fs::read_dir(cache.root())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![format!("{}.0", hash_key("u"))]);
    }

    #[tokio::test]
    async fn clear_removes_every_entry_and_counts_them() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        cache.put_page_list(1, "c", &pages()).await;
        cache.put_image("a", &[1u8; 30]).await.unwrap();
        cache.put_image("b", &[2u8; 20]).await.unwrap();
        assert_eq!(cache.clear().await.unwrap(), 3);
        assert_eq!(cache.size().await, 0);
        assert!(!cache.is_image_cached("a").await);
        assert_eq!(cache.get_page_list(1, "c").await, None);
        // Nothing left, so a second clear removes nothing.
        assert_eq!(cache.clear().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn clear_leaves_part_files_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        std::fs::write(cache.root().join("inflight.0.part"), b"half").unwrap();
        cache.put_image("a", &[1u8; 8]).await.unwrap();
        assert_eq!(cache.clear().await.unwrap(), 1);
        assert!(cache.root().join("inflight.0.part").exists());
    }

    #[tokio::test]
    async fn copy_of_a_cached_image_keeps_its_sniffed_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        let png = crate::testing::encode(&crate::testing::gradient(4, 4), image::ImageFormat::Png);
        cache
            .put_image("https://example.invalid/p/1", &png)
            .await
            .unwrap();
        let dest = tmp.path().join("downloads");
        let copied = cache
            .copy_image_to("https://example.invalid/p/1", &dest, "001")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(copied, dest.join("001.png"));
        assert_eq!(std::fs::read(&copied).unwrap(), png);
    }

    #[tokio::test]
    async fn copy_of_an_unknown_format_uses_jpg_and_a_miss_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), CACHE_SIZE).await;
        cache.put_image("u", b"not a known header").await.unwrap();
        let dest = tmp.path().join("out");
        let copied = cache.copy_image_to("u", &dest, "9").await.unwrap().unwrap();
        assert_eq!(copied, dest.join("9.jpg"));
        assert!(
            cache
                .copy_image_to("missing", &dest, "1")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn the_oldest_files_are_evicted_over_the_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), 250).await;
        for url in ["a", "b", "c"] {
            cache.put_image(url, &[7u8; 100]).await.unwrap();
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(cache.size().await <= 250);
        assert!(!cache.is_image_cached("a").await);
        assert!(cache.is_image_cached("b").await);
        assert!(cache.is_image_cached("c").await);
    }

    #[tokio::test]
    async fn page_lists_and_images_share_the_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = cache(tmp.path(), 200).await;
        cache.put_page_list(1, "old", &pages()).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        cache.put_image("big", &[0u8; 190]).await.unwrap();
        assert_eq!(cache.get_page_list(1, "old").await, None);
        assert!(cache.is_image_cached("big").await);
    }
}
