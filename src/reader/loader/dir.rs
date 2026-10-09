//! Directory page loader (`DirectoryPageLoader.kt`). T-43a-008.
//!
//! One level only: sub-directories are ignored. Files pass the image rule, sorted by name in natural,
//! case-insensitive order. A missing or unreadable directory is an empty list, which becomes "No pages found".

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use super::LoadFailure;
use crate::reader::image::{is_image_file, natural_name_cmp};
use crate::reader::model::{PageSource, ReaderPage};

pub struct DirectoryLoader {
    dir: PathBuf,
    recycled: Cell<bool>,
}

impl DirectoryLoader {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, recycled: Cell::new(false) }
    }

    pub fn is_recycled(&self) -> bool {
        self.recycled.get()
    }

    /// Idempotent: the first call marks the loader recycled, later calls do nothing.
    pub fn recycle(&self) {
        self.recycled.set(true);
    }

    pub fn get_pages(&self) -> Result<Vec<Rc<ReaderPage>>, LoadFailure> {
        if self.is_recycled() {
            return Err(LoadFailure::Cancelled);
        }
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Ok(Vec::new());
        };
        let mut files: Vec<(String, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
            .filter(|(_, path)| is_image_file(path))
            .collect();
        files.sort_by(|a, b| natural_name_cmp(&a.0, &b.0));
        Ok(files
            .into_iter()
            .enumerate()
            .map(|(i, (name, path))| ReaderPage::new(i as u32, name, None, PageSource::File(path)))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn dir(tag: &str, names: &[&str]) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/reader-dir-tests").join(tag);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested")).unwrap();
        for name in names {
            fs::write(dir.join(name), b"\x89PNG\r\n\x1a\n").unwrap();
        }
        fs::write(dir.join("nested").join("9.png"), b"\x89PNG\r\n\x1a\n").unwrap();
        fs::write(dir.join("readme.txt"), b"text").unwrap();
        dir
    }

    fn names(pages: &[Rc<ReaderPage>]) -> Vec<String> {
        pages.iter().map(|p| p.url.clone()).collect()
    }

    #[test]
    fn lists_images_one_level_in_natural_order_with_contiguous_indices() {
        let loader = DirectoryLoader::new(dir("natural", &["10.jpg", "2.png", "Page 1.webp"]));
        let pages = loader.get_pages().ok().unwrap();
        assert_eq!(names(&pages), ["2.png", "10.jpg", "Page 1.webp"]);
        let indices: Vec<u32> = pages.iter().map(|p| p.index).collect();
        assert_eq!(indices, [0, 1, 2]);
    }

    #[test]
    fn a_missing_directory_gives_an_empty_list() {
        let loader = DirectoryLoader::new(PathBuf::from("/definitely/not/here/kuro"));
        assert!(loader.get_pages().ok().unwrap().is_empty());
    }

    #[test]
    fn a_recycled_loader_is_cancelled_and_recycle_is_idempotent() {
        let loader = DirectoryLoader::new(dir("recycle", &["1.png"]));
        loader.recycle();
        loader.recycle();
        assert!(loader.is_recycled());
        assert_eq!(loader.get_pages().err(), Some(LoadFailure::Cancelled));
    }
}
