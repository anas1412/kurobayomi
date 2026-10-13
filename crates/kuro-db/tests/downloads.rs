//! `kuro_db::tables::downloads` against the unified baseline (docs/design/unified-downloads.md section 2.7).

use kuro_db::schema_unified::open_in_memory;
use kuro_db::tables::downloads::{self, PendingRow, QueueRow};
use kuro_db::tables::{entries, items};
use kuro_platform::kind::Kind;
use rusqlite::Connection;

mod common;
use common::{entry, item_row};

/// An entry of `kind` (source 7) with `n` items; returns the entry id and the item ids.
fn seed(c: &Connection, kind: Kind, url: &str, n: usize) -> (i64, Vec<i64>) {
    let entry_id = entries::insert(c, &entry(kind, 7, url, url)).unwrap();
    let ids = (0..n).map(|i| items::insert(c, &item_row(entry_id, kind, &format!("{url}/{i}"), i as f64)).unwrap()).collect();
    (entry_id, ids)
}

fn queued(unit_id: i64, entry_id: i64) -> QueueRow {
    QueueRow { unit_id, entry_id, source: 7, position: 99, state: 1, enqueued_at: 5 }
}

fn pending(kind: Kind, unit_id: i64, entry_id: i64) -> PendingRow {
    PendingRow {
        unit_id,
        kind,
        entry_id,
        entry_url: "/e".into(),
        entry_title: "Title".into(),
        source: 7,
        unit_url: format!("/u{unit_id}"),
        unit_name: format!("Unit {unit_id}"),
        scanlator: Some("grp".into()),
        done: true,
        bookmark: false,
        seq: 0,
        enqueued_at: 3,
    }
}

fn count(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
}

#[test]
fn the_queue_round_trips_for_each_kind_in_slice_order() {
    let c = open_in_memory().unwrap();
    for kind in Kind::ALL {
        let (e, ids) = seed(&c, kind, "/a", 3);
        let rows = vec![queued(ids[2], e), queued(ids[0], e), queued(ids[1], e)];
        assert_eq!(downloads::queue_replace(&c, kind, &rows).unwrap(), 3);
        let back = downloads::queue_load(&c, kind).unwrap();
        assert_eq!(back.iter().map(|r| r.unit_id).collect::<Vec<_>>(), vec![ids[2], ids[0], ids[1]]);
        assert_eq!(back.iter().map(|r| r.position).collect::<Vec<_>>(), vec![0, 1, 2], "positions are dense");
        assert_eq!(back[0], QueueRow { position: 0, ..queued(ids[2], e) });
    }
}

#[test]
fn replace_touches_only_its_own_kind() {
    let c = open_in_memory().unwrap();
    let (me, mids) = seed(&c, Kind::Manga, "/m", 2);
    let (ae, aids) = seed(&c, Kind::Anime, "/a", 2);
    downloads::queue_replace(&c, Kind::Manga, &[queued(mids[0], me), queued(mids[1], me)]).unwrap();
    downloads::queue_replace(&c, Kind::Anime, &[queued(aids[0], ae)]).unwrap();
    downloads::queue_replace(&c, Kind::Manga, &[queued(mids[1], me)]).unwrap();
    assert_eq!(downloads::queue_load(&c, Kind::Manga).unwrap().len(), 1);
    assert_eq!(downloads::queue_load(&c, Kind::Anime).unwrap().len(), 1);
    // a unit of the other kind is not stored under this kind
    assert_eq!(downloads::queue_replace(&c, Kind::Manga, &[queued(aids[1], ae)]).unwrap(), 0);
    assert!(downloads::queue_load(&c, Kind::Manga).unwrap().is_empty());
    assert_eq!(downloads::queue_load(&c, Kind::Anime).unwrap().len(), 1);
}

#[test]
fn replace_with_nothing_clears_and_a_missing_item_is_skipped_not_fatal() {
    let c = open_in_memory().unwrap();
    let (e, ids) = seed(&c, Kind::Manga, "/m", 2);
    downloads::queue_replace(&c, Kind::Manga, &[queued(ids[0], e)]).unwrap();
    let stored = downloads::queue_replace(&c, Kind::Manga, &[queued(9_999, e), queued(ids[1], e)]).unwrap();
    assert_eq!(stored, 1, "the unit without an item is dropped");
    let back = downloads::queue_load(&c, Kind::Manga).unwrap();
    assert_eq!((back.len(), back[0].unit_id, back[0].position), (1, ids[1], 0));
    downloads::queue_replace(&c, Kind::Manga, &[]).unwrap();
    assert!(downloads::queue_load(&c, Kind::Manga).unwrap().is_empty());
}

#[test]
fn a_queued_unit_follows_its_item_when_the_item_is_deleted() {
    let c = open_in_memory().unwrap();
    let (e, ids) = seed(&c, Kind::Anime, "/a", 1);
    downloads::queue_replace(&c, Kind::Anime, &[queued(ids[0], e)]).unwrap();
    downloads::pending_add(&c, &[pending(Kind::Anime, ids[0], e)]).unwrap();
    c.execute("DELETE FROM items WHERE _id = ?1", [ids[0]]).unwrap();
    assert_eq!(count(&c, "download_queue"), 0);
    assert_eq!(count(&c, "download_pending_delete"), 1, "pending rows have no foreign keys");
}

#[test]
fn pending_add_dedupes_and_numbers_per_entry() {
    let c = open_in_memory().unwrap();
    assert_eq!(downloads::pending_add(&c, &[pending(Kind::Manga, 1, 10), pending(Kind::Manga, 1, 10), pending(Kind::Manga, 2, 10)]).unwrap(), 2);
    assert_eq!(downloads::pending_add(&c, &[pending(Kind::Manga, 2, 10), pending(Kind::Manga, 3, 11)]).unwrap(), 1);
    let taken = downloads::pending_take(&c, 0, &|_, _| true).unwrap();
    let seqs: Vec<(i64, i64)> = taken.iter().map(|r| (r.entry_id, r.seq)).collect();
    assert_eq!(seqs, vec![(10, 0), (10, 1), (11, 0)]);
}

#[test]
fn take_keeps_unknown_sources_and_scopes_by_kind() {
    let c = open_in_memory().unwrap();
    let mut other = pending(Kind::Manga, 2, 10);
    other.source = 8;
    let anime = pending(Kind::Anime, 3, 20);
    downloads::pending_add(&c, &[pending(Kind::Manga, 1, 10), other, anime]).unwrap();
    let taken = downloads::pending_take(&c, 0, &|kind, source| kind == Kind::Manga && source == 7).unwrap();
    assert_eq!(taken.iter().map(|r| r.unit_id).collect::<Vec<_>>(), vec![1]);
    assert_eq!(taken[0].scanlator.as_deref(), Some("grp"));
    assert!(taken[0].done && !taken[0].bookmark);
    assert_eq!(downloads::pending_len(&c, Kind::Manga).unwrap(), 1, "the unknown source stays");
    assert_eq!(downloads::pending_len(&c, Kind::Anime).unwrap(), 1);
}

#[test]
fn put_back_restores_the_rows_unchanged() {
    let c = open_in_memory().unwrap();
    downloads::pending_add(&c, &[pending(Kind::Anime, 1, 10), pending(Kind::Anime, 2, 10)]).unwrap();
    let taken = downloads::pending_take(&c, 0, &|_, _| true).unwrap();
    assert_eq!(downloads::pending_len(&c, Kind::Anime).unwrap(), 0);
    assert_eq!(downloads::pending_put_back(&c, &taken).unwrap(), 2);
    assert_eq!(downloads::pending_put_back(&c, &taken).unwrap(), 0, "already pending");
    assert_eq!(downloads::pending_take(&c, 0, &|_, _| true).unwrap(), taken);
}

#[test]
fn a_corrupt_row_moves_to_the_corrupt_table() {
    let c = open_in_memory().unwrap();
    downloads::pending_add(&c, &[pending(Kind::Anime, 1, 10), pending(Kind::Anime, 2, 10)]).unwrap();
    c.execute("UPDATE download_pending_delete SET done = 'yes' WHERE unit_id = 1", []).unwrap();
    let taken = downloads::pending_take(&c, 42, &|_, _| true).unwrap();
    assert_eq!(taken.iter().map(|r| r.unit_id).collect::<Vec<_>>(), vec![2], "the good row is taken");
    assert_eq!(count(&c, "download_pending_delete"), 0, "the corrupt row leaves the list");
    let (rowid, recorded): (i64, i64) =
        c.query_row("SELECT source_rowid, recorded_at FROM download_pending_corrupt", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!((rowid, recorded), (1, 42));
}

#[test]
fn pending_corrupt_records_one_row() {
    let c = open_in_memory().unwrap();
    downloads::pending_corrupt(&c, 5, "bad", 9).unwrap();
    assert_eq!(count(&c, "download_pending_corrupt"), 1);
}
