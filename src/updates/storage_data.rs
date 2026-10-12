//! The data of the Storage page (T-42a-048, T-42a-054): the library entries of one kind with their first category, the
//! rows that have downloads, the categories of the dropdown, and the ids that are visible. Reads only; the sizes come
//! from the download backend, which the caller runs off the GTK thread.

use std::collections::HashMap;
use std::sync::Arc;

use kuro_db::error::DbResult;
use kuro_db::handle::Db;
use kuro_domain::category::repo::CategoryRepo;
use kuro_platform::kind::Kind;

use crate::updates::download_host::DownloadBackend;
use crate::updates::storage::StorageRow;
use crate::updates::storage_screen::StorageInput;

/// One library entry as the Storage page needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryInfo {
    pub id: i64,
    pub source_id: i64,
    pub title: String,
    pub thumbnail_url: Option<String>,
    /// The entry's first category, or 0 (Default) when it has none.
    pub category_id: i64,
}

/// The library entries of `kind`, each with its lowest category id (0 when uncategorized).
pub fn library_entries(db: &Arc<Db>, kind: Kind) -> DbResult<Vec<EntryInfo>> {
    db.read(|c| {
        let mut stmt = c.prepare(
            "SELECT e._id, e.source, e.title, e.thumbnail_url, \
             COALESCE((SELECT MIN(l.category_id) FROM entry_categories l WHERE l.entry_id = e._id), 0) \
             FROM entries e WHERE e.favorite = 1 AND e.kind = ?1",
        )?;
        let rows = stmt
            .query_map([kind.as_str()], |r| {
                Ok(EntryInfo { id: r.get(0)?, source_id: r.get(1)?, title: r.get(2)?, thumbnail_url: r.get(3)?, category_id: r.get(4)? })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// The rows of the entries that have downloads. `usage` answers `(bytes, unit count)` for an entry; an entry with no
/// downloaded unit has no row.
pub fn rows(entries: &[EntryInfo], usage: impl Fn(&EntryInfo) -> (u64, usize)) -> Vec<StorageRow> {
    entries
        .iter()
        .filter_map(|entry| {
            let (bytes, count) = usage(entry);
            (count > 0).then(|| {
                StorageRow::new(
                    entry.id,
                    entry.title.clone(),
                    i64::try_from(bytes).unwrap_or(i64::MAX),
                    entry.thumbnail_url.clone(),
                    i64::try_from(count).unwrap_or(i64::MAX),
                    entry.category_id,
                )
            })
        })
        .collect()
}

/// The dropdown categories and the visible ids. With `hide_hidden` on, a hidden category is out of the "All" view;
/// the Default category (0) is always visible.
pub fn categories(db: &Arc<Db>, kind: Kind, hide_hidden: bool) -> Result<(Vec<(i64, String)>, Vec<i64>), String> {
    let all = CategoryRepo::new(db.clone(), kind).all().map_err(|e| e.to_string())?;
    let mut visible: Vec<i64> = all.iter().filter(|c| !(hide_hidden && c.hidden)).map(|c| c.id).collect();
    if !visible.contains(&0) {
        visible.push(0);
    }
    Ok((all.into_iter().map(|c| (c.id, c.name)).collect(), visible))
}

/// The whole input of one push: entries, usage from the backend, categories. `initializing` is the cache's flag.
pub fn input(db: &Arc<Db>, kind: Kind, backend: &dyn DownloadBackend, hide_hidden: bool, initializing: bool) -> Result<(StorageInput, HashMap<i64, i64>), String> {
    let entries = library_entries(db, kind).map_err(|e| e.to_string())?;
    let sources: HashMap<i64, i64> = entries.iter().map(|e| (e.id, e.source_id)).collect();
    let rows = rows(&entries, |entry| backend.entry_usage(kind, entry.source_id, &entry.title));
    let (categories, visible) = categories(db, kind, hide_hidden)?;
    Ok((StorageInput { rows, categories, visible, initializing }, sources))
}

#[cfg(test)]
mod tests {
    use kuro_data::manga::{Manga, MangaRepo};

    use super::*;
    use crate::updates::download_host::fake::Fake;

    fn test_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
        (dir, Arc::new(Db::new(&path, writer).expect("manga db handle")))
    }

    fn manga(db: &Arc<Db>, title: &str, favorite: bool) -> i64 {
        MangaRepo::new(db.clone())
            .insert(&Manga { favorite, source: 7, url: title.into(), title: title.into(), ..Manga::create() })
            .expect("insert manga")
    }

    #[test]
    fn storage_entries_are_the_library_with_the_default_category_when_uncategorized() {
        let (_dir, db) = test_db();
        let kept = manga(&db, "Kept", true);
        manga(&db, "Left", false);
        let entries = library_entries(&db, Kind::Manga).expect("entries");
        assert_eq!(entries.len(), 1);
        assert_eq!((entries[0].id, entries[0].category_id, entries[0].source_id), (kept, 0, 7));
    }

    #[test]
    fn storage_rows_only_for_entries_with_downloads() {
        let entry = |id: i64| EntryInfo { id, source_id: 7, title: format!("t{id}"), thumbnail_url: None, category_id: 0 };
        let entries = [entry(1), entry(2), entry(3)];
        let usage = |e: &EntryInfo| match e.id {
            1 => (2_000_u64, 4_usize),
            2 => (0, 0),
            _ => (0, 1),
        };
        let rows = rows(&entries, usage);
        let ids: Vec<i64> = rows.iter().map(|r| r.entry_id).collect();
        assert_eq!(ids, [1, 3], "an entry with no downloaded unit has no row; a zero size with units still does");
        assert_eq!((rows[0].size_bytes, rows[0].entries_count), (2_000, 4));
    }

    #[test]
    fn storage_categories_list_default_and_hide_hidden_ones_from_all() {
        let (_dir, db) = test_db();
        let (categories, visible) = categories(&db, Kind::Manga, true).expect("categories");
        assert!(categories.iter().any(|(id, _)| *id == 0), "Default is listed");
        assert!(visible.contains(&0));
    }

    #[test]
    fn storage_input_carries_rows_sources_and_the_cache_flag() {
        let (_dir, db) = test_db();
        let id = manga(&db, "Kept", true);
        let backend = Fake::default();
        let (input, sources) = input(&db, Kind::Manga, &backend, false, true).expect("input");
        assert!(input.initializing);
        assert!(input.rows.is_empty(), "the fake reports no downloads");
        assert_eq!(sources.get(&id), Some(&7));
    }
}
