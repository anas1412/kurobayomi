//! Category definitions and entry membership (area 22, T-22a-008 and T-22a-017). One code path serves manga and anime;
//! only the kind differs.

use kuro_platform::kind::Kind;
use std::collections::{HashMap, HashSet};

use kuro_db::tables::categories;
use kuro_db::DbResult;
use rusqlite::Connection;

use crate::schema::BackupCategory;

/// What the categories restore hands to the entry restore and the preference restore.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryRestore {
    /// Backup category `order` to database id. Entry membership reads this map.
    pub by_order: HashMap<i64, i64>,
    /// `Some` when the flag was computed. `None` when the backup had no categories, so no preference write follows.
    pub categorized_display: Option<bool>,
}

struct Row {
    id: i64,
    name: String,
    order: i64,
    flags: i64,
}

/// The categories of the table. The system category (id 0, seeded by the schema) is not a user category and is left out.
fn existing(c: &Connection, kind: Kind) -> DbResult<Vec<Row>> {
    Ok(categories::all(c, kind)?
        .into_iter()
        .map(|r| Row { id: r.id, name: r.name, order: r.order, flags: r.flags })
        .filter(|r| r.id != 0)
        .collect())
}

/// Matches backup categories to database categories by exact name and creates the missing ones (T-22a-008).
///
/// Existing rows keep their flags. New rows take `max(order) + 1` upward in backup order, with the backup flags. A
/// name repeated in the backup is created once (PROPOSAL, E-16). `categorized_display` is true when the existing and
/// the created rows carry more than one distinct flag value (D23, kept).
pub fn restore_categories(c: &Connection, kind: Kind, backup: &[BackupCategory]) -> DbResult<CategoryRestore> {
    if backup.is_empty() {
        return Ok(CategoryRestore::default());
    }
    let db = existing(c, kind)?;
    let mut next_order = db.iter().map(|r| r.order).max().map_or(0, |m| m + 1);
    let mut by_name: HashMap<String, i64> = db.iter().map(|r| (r.name.clone(), r.id)).collect();
    let mut flags: Vec<i64> = db.iter().map(|r| r.flags).collect();

    let mut sorted: Vec<&BackupCategory> = backup.iter().collect();
    sorted.sort_by_key(|b| b.order);
    let mut by_order = HashMap::new();
    for b in sorted {
        let id = match by_name.get(&b.name) {
            Some(&id) => id,
            None => {
                let id = categories::insert(c, kind, &b.name, next_order, b.flags)?;
                next_order += 1;
                by_name.insert(b.name.clone(), id);
                flags.push(b.flags);
                id
            }
        };
        by_order.insert(b.order, id);
    }
    let distinct: HashSet<i64> = flags.into_iter().collect();
    Ok(CategoryRestore { by_order, categorized_display: Some(distinct.len() > 1) })
}

/// The database ids of one entry's categories, from the backup `order` values (T-22a-017). `None` when no order maps:
/// the caller then leaves the memberships untouched. Unknown orders are dropped.
pub fn membership_ids(by_order: &HashMap<i64, i64>, orders: &[i64]) -> Option<Vec<i64>> {
    let ids: Vec<i64> = orders.iter().filter_map(|o| by_order.get(o).copied()).collect();
    (!ids.is_empty()).then_some(ids)
}

/// True when the stored memberships differ from `ids`. Order and duplicates do not count, so a repeat restore that
/// writes the same set reports no change and the caller skips the rewrite.
pub fn differs_from_stored(mut stored: Vec<i64>, ids: &[i64]) -> bool {
    let mut wanted = ids.to_vec();
    stored.sort_unstable();
    stored.dedup();
    wanted.sort_unstable();
    wanted.dedup();
    stored != wanted
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_db::schema_unified;

    #[test]
    fn membership_change_ignores_order_and_duplicates() {
        assert!(!differs_from_stored(vec![3, 1], &[1, 3, 3]));
        assert!(differs_from_stored(vec![1], &[1, 2]));
        assert!(differs_from_stored(vec![], &[1]));
    }

    fn cat(name: &str, order: i64, id: i64, flags: i64) -> BackupCategory {
        BackupCategory { name: name.into(), order, id, flags }
    }

    fn test_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_unified::open_file(&dir.path().join("kurobayomi.db")).unwrap();
        (dir, c)
    }

    fn names(c: &Connection) -> Vec<(String, i64, i64)> {
        categories::all(c, Kind::Manga)
            .unwrap()
            .into_iter()
            .filter(|r| r.id != 0)
            .map(|r| (r.name, r.order, r.flags))
            .collect()
    }

    #[test]
    fn existing_name_keeps_db_flags_and_new_names_follow_max_order() {
        let (_d, c) = test_db();
        categories::insert(&c, Kind::Manga, "Read", 0, 1).unwrap();
        categories::insert(&c, Kind::Manga, "Later", 4, 2).unwrap();
        let backup = vec![cat("Later", 0, 10, 9), cat("Fresh", 1, 11, 3)];
        let out = restore_categories(&c, Kind::Manga, &backup).unwrap();
        assert_eq!(names(&c), vec![
            ("Read".into(), 0, 1),
            ("Later".into(), 4, 2),
            ("Fresh".into(), 5, 3),
        ]);
        assert_eq!(out.by_order[&0], 2, "Later keeps its database id (second row inserted)");
        assert_eq!(out.by_order.len(), 2);
    }

    #[test]
    fn new_categories_start_at_zero_when_the_db_is_empty_and_in_backup_order() {
        let (_d, c) = test_db();
        let backup = vec![cat("B", 2, 1, 0), cat("A", 1, 2, 0)];
        restore_categories(&c, Kind::Manga, &backup).unwrap();
        assert_eq!(names(&c), vec![("A".into(), 0, 0), ("B".into(), 1, 0)]);
    }

    #[test]
    fn duplicate_backup_names_create_one_row_and_map_to_it() {
        let (_d, c) = test_db();
        let backup = vec![cat("Same", 0, 1, 0), cat("Same", 1, 2, 0)];
        let out = restore_categories(&c, Kind::Manga, &backup).unwrap();
        assert_eq!(names(&c).len(), 1);
        assert_eq!(out.by_order[&0], out.by_order[&1]);
    }

    #[test]
    fn categorized_display_is_true_only_with_two_distinct_flags() {
        let (_d, c) = test_db();
        let same = restore_categories(&c, Kind::Manga, &[cat("X", 0, 1, 0)]).unwrap();
        assert_eq!(same.categorized_display, Some(false));
        let differ = restore_categories(&c, Kind::Manga, &[cat("Y", 1, 2, 5)]).unwrap();
        assert_eq!(differ.categorized_display, Some(true));
    }

    #[test]
    fn empty_backup_writes_nothing_and_reports_no_flag() {
        let (_d, c) = test_db();
        let out = restore_categories(&c, Kind::Manga, &[]).unwrap();
        assert_eq!(out.categorized_display, None);
        assert!(names(&c).is_empty());
    }

    #[test]
    fn membership_needs_at_least_one_mapped_order() {
        let map = HashMap::from([(1, 10), (2, 20)]);
        assert_eq!(membership_ids(&map, &[1, 9, 2]), Some(vec![10, 20]));
        assert_eq!(membership_ids(&map, &[7]), None);
        assert_eq!(membership_ids(&HashMap::new(), &[1]), None);
    }
}
