//! The library read: `libraryView` filtered by kind, one row per category link.

use kuro_platform::kind::Kind;
use rusqlite::{Connection, Result};

use super::{entry_cols, query_all, rows::LibraryRow};

const LIBRARY: &str = concat!(
    "SELECT ", entry_cols!(""), ", totalCount, doneCount, bookmarkCount, fillermarkCount, latestUpload, itemFetchedAt, lastActivity, category FROM libraryView WHERE kind = :kind"
);

/// Every library entry of the kind with its aggregates, read by column name.
pub fn library(c: &Connection, kind: Kind) -> Result<Vec<LibraryRow>> {
    query_all(c, LIBRARY, &[(":kind", &kind.as_str())], LibraryRow::from_row)
}
