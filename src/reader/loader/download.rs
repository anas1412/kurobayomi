//! Downloaded chapter loader (`DownloadPageLoader.kt`): a `.cbz` download is read through the archive loader,
//! a folder download through the directory loader. T-43a-011 (P1), needed by the precedence rule.

use std::path::PathBuf;
use std::rc::Rc;

use super::LoadFailure;
use super::archive::ArchiveLoader;
use super::dir::DirectoryLoader;
use crate::reader::model::{LoadError, ReaderPage};

pub enum DownloadLoader {
    Archive(ArchiveLoader),
    Directory(DirectoryLoader),
}

impl DownloadLoader {
    /// A file is a `.cbz` download, anything else is a chapter folder.
    pub fn open(path: PathBuf) -> Result<Self, LoadError> {
        if path.is_file() {
            ArchiveLoader::open(path).map(Self::Archive)
        } else {
            Ok(Self::Directory(DirectoryLoader::new(path)))
        }
    }

    #[cfg(test)]
    pub fn is_recycled(&self) -> bool {
        match self {
            Self::Archive(l) => l.is_recycled(),
            Self::Directory(l) => l.is_recycled(),
        }
    }

    pub fn recycle(&self) {
        match self {
            Self::Archive(l) => l.recycle(),
            Self::Directory(l) => l.recycle(),
        }
    }

    #[cfg(test)]
    pub fn check_open(&self) -> Result<(), LoadError> {
        match self {
            Self::Archive(l) => l.check_open(),
            Self::Directory(_) => Ok(()),
        }
    }

    pub fn get_pages(&self) -> Result<Vec<Rc<ReaderPage>>, LoadFailure> {
        match self {
            Self::Archive(l) => l.get_pages(),
            Self::Directory(l) => l.get_pages(),
        }
    }
}
