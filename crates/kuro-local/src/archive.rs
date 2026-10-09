//! Archive entry access for the cover fallback and the EPUB contract, over `kuro-archive` (area:13).

use std::io::{self, Read};
use std::path::Path;

use kuro_archive::ArchiveReader;

use crate::natsort::natural_cmp_ci;

/// One entry's bytes; `None` when the archive has no entry with that exact name (`getInputStream(name)`).
pub fn read_entry(archive: &ArchiveReader, name: &str) -> io::Result<Option<Vec<u8>>> {
    let Some(mut entry) = archive.open_entry(name).map_err(io::Error::other)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// The first image file of a manga archive in natural, case-insensitive name order (behavior 9.3). Hidden names are
/// not skipped. `None` when the archive has no image.
pub fn first_page_bytes(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let archive = ArchiveReader::open(path).map_err(io::Error::other)?;
    let mut names: Vec<String> = archive
        .page_entries()
        .map_err(io::Error::other)?
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.sort_by(|a, b| natural_cmp_ci(a, b));
    match names.first() {
        Some(name) => read_entry(&archive, name),
        None => Ok(None),
    }
}
