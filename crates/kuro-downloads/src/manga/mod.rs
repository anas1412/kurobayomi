//! Manga downloads: page fetch, tall-image split, ComicInfo and `.cbz` packing, run as one `UnitRunner`
//! (docs/design/unified-downloads.md 2.3). The files are copies of the old top-level modules; phase D deletes the old
//! copies.

pub mod comicinfo;
pub mod images;
pub mod local;
pub mod pages;
pub mod pipeline;
pub mod runner;
pub mod source;
