//! Units waiting for the reader or player to close before their download is deleted (merges `pending.rs` and
//! `anime/pending.rs`). Rows live in `download_pending_delete` for both kinds, deduplicated by unit id. A flush takes the
//! rows whose source can be named and leaves the rest (E-P01); a row that cannot be decoded moves to
//! `download_pending_corrupt` (E-P04, B28).

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use kuro_db::tables::downloads::{self, PendingRow};
use kuro_db::Db;
use kuro_platform::kind::Kind;

use super::store::StoreError;
use super::unit::{EntryRef, UnitRef};

/// The units pending for one entry, in insertion order. Only the fields the delete needs are stored; the rest of each
/// [`UnitRef`] is left at its default.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingGroup {
    pub kind: Kind,
    pub entry: EntryRef,
    pub units: Vec<UnitRef>,
}

pub struct PendingStore {
    db: Arc<Db>,
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

fn row_of(kind: Kind, entry: &EntryRef, unit: &UnitRef, seq: i64, now: i64) -> PendingRow {
    PendingRow {
        unit_id: unit.id,
        kind,
        entry_id: entry.id,
        entry_url: entry.url.clone(),
        entry_title: entry.title.clone(),
        source: entry.source,
        unit_url: unit.url.clone(),
        unit_name: unit.name.clone(),
        scanlator: unit.scanlator.clone(),
        done: unit.done,
        bookmark: unit.bookmark,
        seq,
        enqueued_at: now,
    }
}

fn unit_of(row: &PendingRow) -> UnitRef {
    UnitRef {
        id: row.unit_id,
        entry_id: row.entry_id,
        url: row.unit_url.clone(),
        name: row.unit_name.clone(),
        scanlator: row.scanlator.clone(),
        done: row.done,
        bookmark: row.bookmark,
        fillermark: false,
        source_order: 0,
        date_upload: 0,
        number: 0.0,
    }
}

impl PendingStore {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Adds the units, deduplicated by unit id within the batch and against the stored rows (EC-90). The caller has
    /// already applied the delete rules. Returns how many were new.
    pub fn add(&self, kind: Kind, entry: &EntryRef, units: &[UnitRef]) -> Result<usize, StoreError> {
        let now = now_ms();
        let rows: Vec<PendingRow> = units.iter().map(|u| row_of(kind, entry, u, 0, now)).collect();
        Ok(self.db.write(|tx| Ok(downloads::pending_add(tx, &rows)?))?)
    }

    /// Takes the rows whose `(kind, source)` `known` accepts, grouped by entry in insertion order, and removes exactly
    /// those rows. The rest stay (E-P01). Rows that cannot be decoded go to `download_pending_corrupt` and are removed
    /// (B28).
    pub fn take(&self, known: &dyn Fn(Kind, i64) -> bool) -> Result<Vec<PendingGroup>, StoreError> {
        let now_secs = now_ms() / 1000;
        let rows = self.db.write(|tx| Ok(downloads::pending_take(tx, now_secs, known)?))?;
        let mut groups: Vec<PendingGroup> = Vec::new();
        for row in &rows {
            match groups.last_mut() {
                Some(g) if g.kind == row.kind && g.entry.id == row.entry_id => g.units.push(unit_of(row)),
                _ => groups.push(PendingGroup {
                    kind: row.kind,
                    entry: EntryRef {
                        id: row.entry_id,
                        kind: row.kind,
                        source: row.source,
                        url: row.entry_url.clone(),
                        title: row.entry_title.clone(),
                    },
                    units: vec![unit_of(row)],
                }),
            }
        }
        Ok(groups)
    }

    /// Puts a group that could not be flushed back as it was (B29): same flags, same order. A unit that is pending
    /// again is kept as it is. Returns how many rows were restored.
    // ponytail: `seq` restarts at 0 for the group, so units added meanwhile may interleave; the flush does not depend on order.
    pub fn put_back(&self, group: &PendingGroup) -> Result<usize, StoreError> {
        let now = now_ms();
        let rows: Vec<PendingRow> =
            group.units.iter().enumerate().map(|(i, u)| row_of(group.kind, &group.entry, u, i as i64, now)).collect();
        Ok(self.db.write(|tx| Ok(downloads::pending_put_back(tx, &rows)?))?)
    }

    /// Number of stored rows, both kinds.
    pub fn len(&self) -> Result<usize, StoreError> {
        Ok(self.db.read(|c| {
            let mut total = 0;
            for kind in Kind::ALL {
                total += downloads::pending_len(c, kind)?;
            }
            Ok(total)
        })?)
    }

    pub fn is_empty(&self) -> Result<bool, StoreError> {
        Ok(self.len()? == 0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::store::testing::{count, test_db};
    use super::*;

    fn store() -> (tempfile::TempDir, Arc<Db>, PendingStore) {
        let (dir, db) = test_db();
        let s = PendingStore::new(db.clone());
        (dir, db, s)
    }

    fn entry(id: i64, kind: Kind, source: i64) -> EntryRef {
        EntryRef { id, kind, source, url: format!("/e/{id}"), title: format!("Entry {id}") }
    }

    fn unit(id: i64, entry_id: i64) -> UnitRef {
        UnitRef {
            id,
            entry_id,
            url: format!("/u/{id}"),
            name: format!("Unit {id}"),
            scanlator: Some("grp".into()),
            done: true,
            bookmark: id % 2 == 0,
            fillermark: false,
            source_order: 0,
            date_upload: 0,
            number: 0.0,
        }
    }

    fn ids(group: &PendingGroup) -> Vec<i64> {
        group.units.iter().map(|u| u.id).collect()
    }

    #[test]
    fn adding_the_same_unit_twice_keeps_one_row() {
        for kind in Kind::ALL {
            let (_dir, _db, s) = store();
            let e = entry(1, kind, 10);
            assert_eq!(s.add(kind, &e, &[unit(1, 1), unit(1, 1), unit(2, 1)]).unwrap(), 2, "dedupe inside the batch");
            assert_eq!(s.add(kind, &e, &[unit(2, 1)]).unwrap(), 0, "EC-90: already stored");
            assert_eq!(s.len().unwrap(), 2);
        }
    }

    #[test]
    fn take_returns_groups_in_insertion_order_and_empties_the_table() {
        for kind in Kind::ALL {
            let (_dir, _db, s) = store();
            s.add(kind, &entry(2, kind, 20), &[unit(5, 2)]).unwrap();
            s.add(kind, &entry(1, kind, 10), &[unit(3, 1), unit(4, 1)]).unwrap();
            s.add(kind, &entry(1, kind, 10), &[unit(1, 1)]).unwrap();
            let groups = s.take(&|_, _| true).unwrap();
            assert_eq!(groups.len(), 2);
            assert_eq!(groups[0].entry, entry(1, kind, 10));
            assert_eq!(ids(&groups[0]), vec![3, 4, 1], "EC-91");
            assert_eq!(ids(&groups[1]), vec![5]);
            assert!(groups[0].units[0].done && groups[0].units[1].bookmark, "the live flags are kept (B30)");
            assert_eq!(groups[0].units[0].scanlator.as_deref(), Some("grp"));
            assert!(s.is_empty().unwrap(), "EC-92");
        }
    }

    #[test]
    fn take_keeps_rows_of_unknown_sources_and_scopes_by_kind() {
        let (_dir, _db, s) = store();
        s.add(Kind::Anime, &entry(1, Kind::Anime, 10), &[unit(1, 1)]).unwrap();
        s.add(Kind::Anime, &entry(2, Kind::Anime, 20), &[unit(2, 2)]).unwrap();
        s.add(Kind::Manga, &entry(3, Kind::Manga, 10), &[unit(3, 3)]).unwrap();
        let groups = s.take(&|kind, source| kind == Kind::Anime && source == 10).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!((groups[0].kind, ids(&groups[0])), (Kind::Anime, vec![1]));
        assert_eq!(s.len().unwrap(), 2, "E-P01: the other rows wait for their source");
        let rest = s.take(&|_, _| true).unwrap();
        assert_eq!(rest.iter().map(|g| (g.kind, g.entry.id)).collect::<Vec<_>>(), vec![(Kind::Anime, 2), (Kind::Manga, 3)]);
    }

    #[test]
    fn a_failed_group_is_put_back_as_it_was() {
        for kind in Kind::ALL {
            let (_dir, _db, s) = store();
            s.add(kind, &entry(1, kind, 10), &[unit(1, 1), unit(2, 1)]).unwrap();
            let taken = s.take(&|_, _| true).unwrap();
            assert_eq!(s.len().unwrap(), 0);
            assert_eq!(s.put_back(&taken[0]).unwrap(), 2);
            assert_eq!(s.put_back(&taken[0]).unwrap(), 0, "already pending");
            let again = s.take(&|_, _| true).unwrap();
            assert_eq!(again[0].entry, taken[0].entry);
            assert_eq!(again[0].units, taken[0].units, "same flags, same order (B29)");
        }
    }

    #[test]
    fn a_corrupt_row_is_isolated_in_its_own_table() {
        for kind in Kind::ALL {
            let (_dir, db, s) = store();
            s.add(kind, &entry(1, kind, 10), &[unit(1, 1), unit(2, 1)]).unwrap();
            db.write(|tx| Ok(tx.execute("UPDATE download_pending_delete SET done = 'yes' WHERE unit_id = 1", [])?)).unwrap();
            let groups = s.take(&|_, _| true).unwrap();
            assert_eq!(ids(&groups[0]), vec![2], "the good row is taken");
            assert!(s.is_empty().unwrap(), "the corrupt row leaves the pending list");
            assert_eq!(count(&db, "download_pending_corrupt"), 1, "E-P04: it is kept in its own table");
        }
    }
}
