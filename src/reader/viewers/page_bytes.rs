//! Where a page's bytes are read from (T-43b-014). A page on disk or in an archive can be read straight away; a page
//! that comes over the network is not a file the viewer knows, so it has no source here and keeps its status label.

use std::io::{self, Read};
use std::path::PathBuf;

use crate::reader::model::PageSource;

/// A place the bytes of one page can be read from.
#[derive(Clone, Debug, PartialEq, Eq)]
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
