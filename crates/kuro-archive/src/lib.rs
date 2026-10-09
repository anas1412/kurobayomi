//! Kurobayomi archive reading and writing: CBZ, EPUB and libarchive formats.
//!
//! `reader` lists and streams archive entries through the system libarchive (`libarchive.rs`).
//! `epub` and `zip_writer` are reserved for T-13a-033 and T-13a-034.
//!
//! Layout: the T-13a-001 `kuro-core/src/archive/` module lives in this crate instead, as CRATE-MAP
//! section 2.1 says (kuro-archive owns archives; kuro-core has no `archive` module or feature).

pub mod epub;
mod libarchive;
pub mod reader;
pub mod zip_writer;

pub use reader::{ArchiveError, ArchiveReader, EntryInfo, EntryReader};
