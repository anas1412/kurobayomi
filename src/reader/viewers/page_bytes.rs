//! Where a page's bytes are read from (T-43b-014, T-44a-062). A page on disk or in an archive can be read straight
//! away. A page that comes over the network is fetched by its chapter's loader into the disk cache, and is read from
//! there once the loader says it is ready: [`Located::Fetched`].

use std::io::{self, Read};
use std::path::PathBuf;
use std::rc::Rc;

use crate::reader::model::{PageSource, ReaderPage};

/// A place the bytes of one page can be read from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ByteSource {
    File(PathBuf),
    Entry { archive: PathBuf, entry: String },
}

/// The source of a page's bytes. A shared page reads through its parent. `None` for a page that is fetched.
pub fn byte_source(source: &PageSource) -> Option<ByteSource> {
    match source {
        PageSource::Http => None,
        PageSource::File(path) => Some(ByteSource::File(path.clone())),
        PageSource::ArchiveEntry { archive, entry } => Some(ByteSource::Entry { archive: archive.clone(), entry: entry.clone() }),
        PageSource::Shared(parent) => byte_source(&parent.source),
    }
}

/// The page whose own source is read: an insert page reads through its parent.
pub fn root_page(page: &Rc<ReaderPage>) -> Rc<ReaderPage> {
    match &page.source {
        PageSource::Shared(parent) => root_page(parent),
        _ => Rc::clone(page),
    }
}

/// How a page's bytes are found.
#[derive(Clone)]
pub enum Located {
    /// A file or an archive entry: always there.
    Fixed(ByteSource),
    /// A page the chapter's loader fetches: the file exists once the loader has it ready. Holds the root page.
    Fetched(Rc<ReaderPage>),
}

impl Located {
    /// The place to read from now. `None` for a fetched page the loader has not got (or has lost: the loader then
    /// queues it again).
    pub fn resolve(&self) -> Option<ByteSource> {
        match self {
            Self::Fixed(source) => Some(source.clone()),
            Self::Fetched(page) => page.chapter()?.page_file(page.index as usize).map(ByteSource::File),
        }
    }

    pub fn is_fetched(&self) -> bool {
        matches!(self, Self::Fetched(_))
    }
}

/// Where `page`'s bytes come from. `None` for a page that has no way to get them: a fetched page whose chapter has no
/// loader stays blank (E-40).
pub fn locate(page: &Rc<ReaderPage>) -> Option<Located> {
    let root = root_page(page);
    match byte_source(&root.source) {
        Some(source) => Some(Located::Fixed(source)),
        None => root.chapter().filter(|c| c.has_loader()).map(|_| Located::Fetched(root)),
    }
}

/// Reads the page. Blocking: call it on the blocking pool.
pub fn read(source: &ByteSource) -> io::Result<Vec<u8>> {
    match source {
        ByteSource::File(path) => std::fs::read(path),
        ByteSource::Entry { archive, entry } => {
            let reader = kuro_archive::reader::ArchiveReader::open(archive).map_err(io::Error::other)?;
            let mut stream = reader
                .open_entry(entry)
                .map_err(io::Error::other)?
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("page not found in archive: {entry}")))?;
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes)?;
            Ok(bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::model::ReaderPage;

    #[test]
    fn a_file_and_an_archive_entry_have_sources_and_a_fetched_page_has_none() {
        let file = PageSource::File(PathBuf::from("/tmp/a.png"));
        assert_eq!(byte_source(&file), Some(ByteSource::File(PathBuf::from("/tmp/a.png"))));
        let entry = PageSource::ArchiveEntry { archive: PathBuf::from("/tmp/a.cbz"), entry: "1.png".into() };
        assert_eq!(byte_source(&entry), Some(ByteSource::Entry { archive: PathBuf::from("/tmp/a.cbz"), entry: "1.png".into() }));
        assert_eq!(byte_source(&PageSource::Http), None);
    }

    #[test]
    fn a_shared_page_reads_through_its_parent() {
        let parent = ReaderPage::new(0, "p0", None, PageSource::File(PathBuf::from("/tmp/p0.png")));
        let insert = ReaderPage::insert_page(&parent);
        assert_eq!(byte_source(&insert.source), Some(ByteSource::File(PathBuf::from("/tmp/p0.png"))));
    }

    struct Fake(std::cell::Cell<Option<PathBuf>>);

    impl crate::reader::model::PageLoader for Fake {
        fn recycle(&self) {}
        fn is_local(&self) -> bool {
            false
        }
        fn image_file(&self, _index: usize) -> Option<PathBuf> {
            let file = self.0.take();
            self.0.set(file.clone());
            file
        }
    }

    fn fetched_page(file: Option<PathBuf>) -> (Rc<crate::reader::model::ReaderChapter>, Rc<ReaderPage>) {
        let chapter = crate::reader::model::ReaderChapter::new(kuro_data::chapter::Chapter::create());
        chapter.set_loader(Box::new(Fake(std::cell::Cell::new(file))));
        let page = ReaderPage::new(0, "p0", None, PageSource::Http);
        chapter.finish_loading(vec![Rc::clone(&page)]).unwrap();
        (chapter, page)
    }

    #[test]
    fn a_fetched_page_is_located_through_its_loader_and_resolves_once_the_file_exists() {
        let (_chapter, waiting) = fetched_page(None);
        let located = locate(&waiting).expect("the chapter has a loader");
        assert!(located.is_fetched());
        assert_eq!(located.resolve(), None, "the loader has not got the file yet");
        let (_chapter, ready) = fetched_page(Some(PathBuf::from("/cache/p0.0")));
        assert_eq!(locate(&ready).unwrap().resolve(), Some(ByteSource::File(PathBuf::from("/cache/p0.0"))));
    }

    #[test]
    fn a_fetched_page_without_a_loader_stays_blank_and_a_file_page_is_fixed() {
        let orphan = ReaderPage::new(0, "p0", None, PageSource::Http);
        assert!(locate(&orphan).is_none(), "no chapter, so nothing can fetch it (E-40)");
        let file = ReaderPage::new(0, "p0", None, PageSource::File(PathBuf::from("/tmp/a.png")));
        assert!(matches!(locate(&file), Some(Located::Fixed(_))));
    }

    #[test]
    fn an_insert_of_a_fetched_page_is_located_through_the_parent() {
        let (_chapter, parent) = fetched_page(Some(PathBuf::from("/cache/p0.0")));
        let insert = ReaderPage::insert_page(&parent);
        assert!(Rc::ptr_eq(&root_page(&insert), &parent));
        assert_eq!(locate(&insert).unwrap().resolve(), Some(ByteSource::File(PathBuf::from("/cache/p0.0"))));
    }

    #[test]
    fn a_file_is_read_and_a_missing_one_is_an_error() {
        let dir = std::env::temp_dir().join(format!("kuro-page-bytes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.bin");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(read(&ByteSource::File(path.clone())).unwrap(), b"abc");
        assert!(read(&ByteSource::File(dir.join("missing"))).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
