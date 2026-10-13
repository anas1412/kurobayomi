//! `kuro_db::tables` against the unified baseline (docs/design/unified-model.md section 5.2, item 2).

use kuro_db::flags::{item, season, set_flag};
use kuro_db::codec::FetchType;
use kuro_db::schema_unified::open_in_memory;
use kuro_db::tables::entries::FlagColumn;
use kuro_db::tables::{self, *};
use kuro_platform::kind::Kind;
use rusqlite::Connection;

mod common;
use common::{entry, item_row};

fn db() -> Connection {
    open_in_memory().unwrap()
}

fn add(c: &Connection, kind: Kind, url: &str) -> i64 {
    entries::insert(c, &entry(kind, 1, url, url)).unwrap()
}

#[test]
fn an_entry_round_trips_for_each_kind() {
    let c = db();
    for kind in Kind::ALL {
        let row = entry(kind, 4, "/x", "Title");
        let id = entries::insert(&c, &row).unwrap();
        let back = entries::by_id(&c, id).unwrap();
        assert_eq!(back, EntryRow { id, ..row.clone() });
        assert_eq!(entries::by_url_and_source(&c, kind, "/x", 4).unwrap().unwrap().id, id);
        assert_eq!(entries::favorites(&c, kind).unwrap().len(), 1);
    }
    // the same (source, url) in the other kind is a different row
    let ids: Vec<i64> = Kind::ALL.iter().map(|k| entries::by_url_and_source(&c, *k, "/x", 4).unwrap().unwrap().id).collect();
    assert_ne!(ids[0], ids[1]);
    assert!(matches!(entries::by_id(&c, 999), Err(kuro_db::DbError::NotFound)));
}

#[test]
fn an_anime_only_field_on_a_manga_entry_fails_the_check() {
    let c = db();
    let id = add(&c, Kind::Manga, "/m");
    let patch = EntryPatch { id, season_flags: Some(1), ..Default::default() };
    assert!(entries::update(&c, &patch).is_err());
    let ok = EntryPatch { id, title: Some("new".into()), ..Default::default() };
    assert_eq!(entries::update(&c, &ok).unwrap(), 1);
    assert_eq!(entries::by_id(&c, id).unwrap().title, "new");
}

#[test]
fn a_masked_flag_write_on_a_manga_row_ignores_anime_only_bits_and_bumps_version() {
    let c = db();
    let m = add(&c, Kind::Manga, "/m");
    let a = add(&c, Kind::Anime, "/a");
    let before = entries::by_id(&c, m).unwrap();
    // the fillermark group (anime only) and the bookmarked group in one mask
    let mask = item::FILLERMARKED_MASK | item::BOOKMARKED_MASK;
    let flag = item::SHOW_FILLERMARKED | item::SHOW_BOOKMARKED;
    assert_eq!(entries::set_flags_masked(&c, FlagColumn::ItemFlags, m, flag, mask).unwrap(), 1);
    let after = entries::by_id(&c, m).unwrap();
    assert_eq!(after.item_flags, set_flag(before.item_flags, item::SHOW_BOOKMARKED, item::BOOKMARKED_MASK));
    assert_eq!(after.item_flags & item::FILLERMARKED_MASK, 0);
    assert_eq!(after.version, before.version + 1);
    // anime keeps both groups
    entries::set_flags_masked(&c, FlagColumn::ItemFlags, a, flag, mask).unwrap();
    let anime = entries::by_id(&c, a).unwrap();
    assert_eq!(anime.item_flags & mask, flag);
    assert_eq!(anime.version, 1);
    // a whole word keeps only the manga groups on manga
    entries::write_flags(&c, FlagColumn::ItemFlags, m, 0xFFFF_FFFF).unwrap();
    assert_eq!(entries::read_flags(&c, FlagColumn::ItemFlags, m).unwrap(), Some(item::MANGA_MASK));
    entries::write_flags(&c, FlagColumn::ItemFlags, a, 0x1801).unwrap();
    assert_eq!(entries::read_flags(&c, FlagColumn::ItemFlags, a).unwrap(), Some(0x1801));
    // season flags: anime only
    entries::set_flags_masked(&c, FlagColumn::SeasonFlags, a, season::SHOW_UNSEEN, season::UNSEEN_MASK).unwrap();
    assert_eq!(entries::read_flags(&c, FlagColumn::SeasonFlags, a).unwrap(), Some(0x11 & !season::UNSEEN_MASK | season::SHOW_UNSEEN));
    assert_eq!(entries::set_flags_masked(&c, FlagColumn::SeasonFlags, m, 1, 1).unwrap(), 0);
    assert_eq!(entries::read_flags(&c, FlagColumn::SeasonFlags, m).unwrap(), None);
    assert_eq!(entries::read_flags(&c, FlagColumn::ItemFlags, 999).unwrap(), None);
}

#[test]
fn viewer_bits_are_masked_and_reset_is_per_kind() {
    let c = db();
    let m = add(&c, Kind::Manga, "/m");
    let a = add(&c, Kind::Anime, "/a");
    assert_eq!(entries::set_viewer_bits(&c, m, 0x8, 0x38).unwrap(), 1);
    let row = entries::by_id(&c, m).unwrap();
    assert_eq!((row.viewer, row.version), (3 | 0x8, 1));
    entries::reset_viewer(&c, Kind::Manga).unwrap();
    assert_eq!(entries::by_id(&c, m).unwrap().viewer, 0);
    assert_eq!(entries::by_id(&c, a).unwrap().viewer, 3, "the other kind is untouched");
}

#[test]
fn clear_non_library_orphans_a_favourite_season_and_removes_reset_history_for_anime() {
    let mut c = db();
    let mut parent = entry(Kind::Anime, 9, "/p", "parent");
    parent.favorite = false;
    parent.fetch_type = Some(FetchType::Seasons);
    let p = entries::insert(&c, &parent).unwrap();
    let mut season_row = entry(Kind::Anime, 9, "/s", "season");
    season_row.parent_id = Some(p);
    let s = entries::insert(&c, &season_row).unwrap(); // favourite child
    let mut plain = entry(Kind::Anime, 9, "/n", "plain");
    plain.favorite = false;
    let n = entries::insert(&c, &plain).unwrap();
    let i = items::insert(&c, &item_row(n, Kind::Anime, "e1", 1.0)).unwrap();
    history::upsert(&c, i, 0, 0).unwrap(); // a reset row (last_at 0)
    let keep = add(&c, Kind::Anime, "/keep");
    let ik = items::insert(&c, &item_row(keep, Kind::Anime, "e2", 1.0)).unwrap();
    history::upsert(&c, ik, 0, 5).unwrap();
    // a manga on the same source is not touched by an anime clear
    let mut m = entry(Kind::Manga, 9, "/m", "m");
    m.favorite = false;
    let mid = entries::insert(&c, &m).unwrap();

    let tx = c.transaction().unwrap();
    assert!(entries::clear_non_library(&tx, Kind::Anime, &[]).unwrap() == 0);
    let removed = entries::clear_non_library(&tx, Kind::Anime, &[9]).unwrap();
    tx.commit().unwrap();

    assert_eq!(removed, 1, "the reset row of the kept entry is removed; the deleted entry's row went by cascade");
    assert!(entries::by_id(&c, p).is_err() && entries::by_id(&c, n).is_err());
    assert_eq!(entries::by_id(&c, s).unwrap().parent_id, None, "the favourite season is orphaned, not deleted");
    assert!(entries::by_id(&c, mid).is_ok());
    assert!(history::by_entry(&c, keep).unwrap().is_empty());
}

#[test]
fn clear_non_library_by_ids_deletes_orphans_and_drops_reset_history() {
    let mut c = db();
    let mut a = entry(Kind::Anime, 1, "/a", "a");
    a.favorite = false;
    let id = entries::insert(&c, &a).unwrap();
    let fav = add(&c, Kind::Anime, "/fav");
    entries::update(&c, &EntryPatch { id: fav, parent_id: Some(id), ..Default::default() }).unwrap();
    let it = items::insert(&c, &item_row(id, Kind::Anime, "e", 1.0)).unwrap();
    history::upsert(&c, it, 0, 0).unwrap();
    let keep = add(&c, Kind::Anime, "/keep");
    let ik = items::insert(&c, &item_row(keep, Kind::Anime, "k", 1.0)).unwrap();
    history::upsert(&c, ik, 0, 3).unwrap(); // a reset row on an entry that stays
    let (ids, orphaned) = entries::clearable_tree(&c, id).unwrap();
    assert_eq!((ids.clone(), orphaned.clone()), (vec![id], vec![fav]));
    let tx = c.transaction().unwrap();
    entries::clear_ids(&tx, &ids, &orphaned).unwrap();
    tx.commit().unwrap();
    assert!(entries::by_id(&c, id).is_err());
    assert!(history::by_entry(&c, keep).unwrap().is_empty(), "reset history is dropped");
    assert_eq!(entries::by_id(&c, fav).unwrap().parent_id, None);
    let counts = entries::non_library_counts(&c, Kind::Anime).unwrap();
    assert!(counts.is_empty());
    assert_eq!(entries::favorite_counts(&c, Kind::Anime).unwrap(), vec![(1, 2)]);
}

#[test]
fn duplicates_upcoming_and_children_are_kind_scoped() {
    let c = db();
    let m = add(&c, Kind::Manga, "/m");
    let a = entries::insert(&c, &entry(Kind::Anime, 1, "/a", "/m")).unwrap(); // same title, other kind
    assert!(entries::duplicates(&c, Kind::Manga, m, "/M").unwrap().is_empty());
    assert_eq!(entries::duplicates(&c, Kind::Anime, 0, "/m").unwrap().len(), 1);
    assert_eq!(entries::duplicates(&c, Kind::Manga, a, "/m").unwrap().len(), 1);
    assert_eq!(entries::upcoming(&c, Kind::Manga, 0, Kind::Manga.upcoming_statuses()).unwrap().len(), 1);
    assert!(entries::upcoming(&c, Kind::Manga, 10, &[1]).unwrap().is_empty());
    assert!(entries::upcoming(&c, Kind::Anime, 0, &[]).unwrap().is_empty());
    assert_eq!(entries::all_source_and_url(&c, Kind::Manga).unwrap(), vec![(1, "/m".to_owned())]);
    let ids = entries::deletable_parents(&c).unwrap();
    assert!(ids.is_empty(), "both are favourites");
}

#[test]
fn items_round_trip_with_the_joined_kind_and_the_lowest_id_wins_a_duplicate_url() {
    let c = db();
    for kind in Kind::ALL {
        let e = add(&c, kind, "/e");
        let first = items::insert(&c, &item_row(e, kind, "u", 1.0)).unwrap();
        let second = items::insert(&c, &item_row(e, kind, "u", 2.0)).unwrap();
        assert!(first < second);
        let row = items::by_id(&c, first).unwrap();
        assert_eq!(row, ItemRow { id: first, ..item_row(e, kind, "u", 1.0) });
        assert_eq!(items::by_url_and_entry(&c, "u", e).unwrap().first().unwrap().id, first);
        assert_eq!(items::by_url(&c, kind, "u").unwrap().len(), 2);
        assert_eq!(items::bookmarked(&c, e).unwrap().len(), 2);
        assert_eq!(items::scanlators(&c, e).unwrap(), vec!["grp", "grp"]);
        let patch = ItemPatch { id: first, done: Some(true), ..Default::default() };
        items::update(&c, &patch).unwrap();
        assert!(items::by_id(&c, first).unwrap().done);
        assert_eq!(items::remove_with_ids(&c, &[first, second]).unwrap(), 2);
    }
    assert!(items::remove_with_ids(&c, &[]).unwrap() == 0);
}

#[test]
fn insert_all_returns_ids_in_order_and_by_entry_applies_the_scanlator_filter() {
    let mut c = db();
    let e = add(&c, Kind::Manga, "/m");
    let rows: Vec<ItemRow> = (1..=3).map(|n| item_row(e, Kind::Manga, &format!("c{n}"), f64::from(n))).collect();
    let tx = c.transaction().unwrap();
    let ids = items::insert_all(&tx, &rows).unwrap();
    tx.commit().unwrap();
    assert_eq!(ids.len(), 3);
    c.execute("INSERT INTO excluded_scanlators(entry_id, scanlator) VALUES (?1, 'grp')", [e]).unwrap();
    assert_eq!(items::by_entry(&c, e, false).unwrap().len(), 3);
    assert!(items::by_entry(&c, e, true).unwrap().is_empty());
}

#[test]
fn history_upsert_adds_the_delta_and_lists_per_kind_with_literal_wildcards() {
    let c = db();
    let m = entries::insert(&c, &entry(Kind::Manga, 1, "/m", "50% Ärger_x")).unwrap();
    let a = entries::insert(&c, &entry(Kind::Anime, 1, "/a", "Ärger anime")).unwrap();
    let im = items::insert(&c, &item_row(m, Kind::Manga, "c", 1.0)).unwrap();
    let ia = items::insert(&c, &item_row(a, Kind::Anime, "e", 1.0)).unwrap();
    history::upsert(&c, im, 100, 10).unwrap();
    history::upsert(&c, im, 200, 5).unwrap();
    history::upsert(&c, ia, 300, 0).unwrap();
    let row = &history::by_entry(&c, m).unwrap()[0];
    assert_eq!((row.last_at, row.duration_ms), (200, 15));
    assert_eq!(history::total_duration(&c, Kind::Manga).unwrap(), 15);
    assert_eq!(history::total_duration(&c, Kind::Anime).unwrap(), 0);
    let list = history::list(&c, Kind::Manga, "").unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].kind, list[0].entry_id, list[0].item_id, list[0].last_at), (Kind::Manga, m, im, 200));
    assert_eq!(history::list(&c, Kind::Manga, "ÄRGER").unwrap().len(), 1, "Unicode fold");
    assert_eq!(history::list(&c, Kind::Manga, "0%").unwrap().len(), 1);
    assert!(history::list(&c, Kind::Manga, "%").unwrap().len() == 1);
    assert!(history::list(&c, Kind::Manga, "zz%").unwrap().is_empty());
    assert_eq!(history::list(&c, Kind::Anime, "").unwrap().len(), 1);
    assert_eq!(history::latest(&c, Kind::Anime).unwrap().unwrap().item_id, ia);
    assert_eq!(history::by_item_url(&c, Kind::Anime, "e").unwrap().unwrap().item_id, ia);
    assert!(history::by_item_url(&c, Kind::Manga, "e").unwrap().is_none());
    // reset keeps the row and its duration; the list hides it; remove_resetted drops only that kind's reset rows
    assert_eq!(history::reset_by_entry(&c, m).unwrap(), 1);
    assert_eq!(history::by_entry(&c, m).unwrap()[0].duration_ms, 15);
    assert!(history::list(&c, Kind::Manga, "").unwrap().is_empty());
    assert_eq!(history::remove_resetted(&c, Kind::Anime).unwrap(), 0);
    assert_eq!(history::remove_resetted(&c, Kind::Manga).unwrap(), 1);
    assert_eq!(history::remove_all(&c, Kind::Anime).unwrap(), 1);
    history::upsert(&c, ia, 5, 0).unwrap();
    let h = history::by_entry(&c, a).unwrap()[0].id;
    history::reset_by_id(&c, h).unwrap();
    assert_eq!(history::by_entry(&c, a).unwrap()[0].last_at, 0);
}

#[test]
fn tracks_upsert_keeps_the_row_id_and_all_is_kind_scoped() {
    let c = db();
    let m = add(&c, Kind::Manga, "/m");
    let a = add(&c, Kind::Anime, "/a");
    let track = |entry_id, kind| TrackRow {
        id: 0,
        entry_id,
        kind,
        tracker_id: 2,
        remote_id: 77,
        library_id: Some(5),
        title: "t".into(),
        last_item: 3.5,
        total_items: 12,
        status: 1,
        score: 8.0,
        remote_url: "u".into(),
        start_date: 1,
        finish_date: 2,
        private: true,
    };
    let id = tracks::insert(&c, &track(m, Kind::Manga)).unwrap();
    let mut again = track(m, Kind::Manga);
    again.last_item = 9.0;
    assert_eq!(tracks::insert(&c, &again).unwrap(), id);
    tracks::insert(&c, &track(a, Kind::Anime)).unwrap();
    assert_eq!(tracks::by_id(&c, id).unwrap(), TrackRow { id, ..again });
    assert_eq!(tracks::all(&c, Kind::Manga).unwrap().len(), 1);
    assert_eq!(tracks::by_entry(&c, a).unwrap()[0].kind, Kind::Anime);
    let patch = TrackPatch { id, score: Some(1.0), ..Default::default() };
    assert_eq!(tracks::update_coalesced(&c, &patch).unwrap(), 1);
    assert_eq!(tracks::by_id(&c, id).unwrap().score, 1.0);
    assert_eq!(tracks::delete(&c, m, 2).unwrap(), 1);
    assert!(matches!(tracks::by_id(&c, id), Err(kuro_db::DbError::NotFound)));
}

#[test]
fn categories_are_per_kind_and_membership_follows_the_entry_kind() {
    let mut c = db();
    let m = add(&c, Kind::Manga, "/m");
    let a = add(&c, Kind::Anime, "/a");
    let cm = categories::insert(&c, Kind::Manga, "Reading", 1, 64).unwrap();
    let ca = categories::insert(&c, Kind::Anime, "Reading", 1, 64).unwrap();
    assert_eq!((cm, ca), (1, 1), "ids are per kind");
    assert_eq!(categories::all(&c, Kind::Manga).unwrap().iter().map(|r| r.id).collect::<Vec<_>>(), vec![0, 1]);
    let tx = c.transaction().unwrap();
    entries::set_categories(&tx, m, &[cm, cm]).unwrap();
    entries::set_categories(&tx, a, &[ca]).unwrap();
    tx.commit().unwrap();
    assert_eq!(categories::by_entry(&c, Kind::Manga, m).unwrap().len(), 1);
    assert!(categories::by_entry(&c, Kind::Anime, m).unwrap().is_empty());
    assert_eq!(library::library(&c, Kind::Manga).unwrap()[0].category, cm);
    let patch = CategoryPatch { id: cm, hidden: Some(true), name: Some("Done".into()), ..Default::default() };
    categories::update(&c, Kind::Manga, &patch).unwrap();
    assert_eq!(categories::visible(&c, Kind::Manga).unwrap().len(), 1, "only the system row is visible");
    assert!(categories::visible_by_entry(&c, Kind::Manga, m).unwrap().is_empty());
    assert_eq!(categories::get(&c, Kind::Manga, cm).unwrap().unwrap().name, "Done");
    categories::update_all_flags(&c, Kind::Anime, Some(0x44)).unwrap();
    assert_eq!(categories::get(&c, Kind::Anime, 0).unwrap().unwrap().flags, 0x44);
    assert_eq!(categories::get(&c, Kind::Manga, 0).unwrap().unwrap().flags, 64);
    assert!(categories::delete(&c, Kind::Manga, 0).is_err(), "the system category stays");
    let tx = c.transaction().unwrap();
    categories::delete(&tx, Kind::Manga, cm).unwrap();
    categories::renumber_all(&tx, Kind::Manga).unwrap();
    tx.commit().unwrap();
    assert_eq!(categories::all(&c, Kind::Manga).unwrap().len(), 1);
    assert_eq!(categories::all(&c, Kind::Anime).unwrap().len(), 2);
}

#[test]
fn stub_sources_are_per_kind() {
    let c = db();
    stub_sources::upsert(&c, Kind::Manga, 5, "en", "One").unwrap();
    stub_sources::upsert(&c, Kind::Manga, 5, "en", "Two").unwrap();
    stub_sources::upsert(&c, Kind::Anime, 5, "ja", "Other").unwrap();
    assert_eq!(stub_sources::find_one(&c, Kind::Manga, 5).unwrap().unwrap().name, "Two");
    assert_eq!(stub_sources::find_all(&c, Kind::Anime).unwrap().len(), 1);
    assert!(stub_sources::find_one(&c, Kind::Anime, 6).unwrap().is_none());
}

#[test]
fn the_library_and_seasons_read_aggregates_per_kind() {
    let c = db();
    let m = add(&c, Kind::Manga, "/m");
    for n in 1..=3 {
        let mut r = item_row(m, Kind::Manga, &format!("c{n}"), f64::from(n));
        r.done = n == 1;
        items::insert(&c, &r).unwrap();
    }
    let mut parent = entry(Kind::Anime, 1, "/p", "parent");
    parent.fetch_type = Some(FetchType::Seasons);
    let p = entries::insert(&c, &parent).unwrap();
    let mut child = entry(Kind::Anime, 1, "/s1", "s1");
    child.parent_id = Some(p);
    child.favorite = false;
    let s = entries::insert(&c, &child).unwrap();
    items::insert(&c, &item_row(s, Kind::Anime, "e1", 1.0)).unwrap();

    let lib = library::library(&c, Kind::Manga).unwrap();
    assert_eq!(lib.len(), 1);
    assert_eq!((lib[0].total, lib[0].done, lib[0].bookmarks, lib[0].category), (3, 1, 3, 0));
    assert_eq!(lib[0].entry.id, m);
    let anime = library::library(&c, Kind::Anime).unwrap();
    assert_eq!(anime.iter().find(|r| r.entry.id == p).unwrap().total, 1, "the parent counts its seasons");

    let seasons = entries::seasons_of(&c, p).unwrap();
    assert_eq!(seasons.len(), 1);
    assert_eq!((seasons[0].entry.id, seasons[0].total, seasons[0].fillermarks), (s, 1, 1));
    assert_eq!(entries::children_of(&c, p).unwrap().len(), 1);
}

#[test]
fn updates_order_by_fetch_date_then_item_id_and_filter_by_kind_and_done() {
    let c = db();
    let m = add(&c, Kind::Manga, "/m");
    let a = add(&c, Kind::Anime, "/a");
    let mut ids = Vec::new();
    for n in 1..=3 {
        let mut r = item_row(m, Kind::Manga, &format!("c{n}"), f64::from(n));
        r.date_fetch = 5_000; // all tie
        r.date_upload = 4_000 + i64::from(n);
        ids.push(items::insert(&c, &r).unwrap());
    }
    let mut r = item_row(a, Kind::Anime, "e1", 1.0);
    r.date_fetch = 9_000;
    items::insert(&c, &r).unwrap();
    let rows = updates::recent(&c, Kind::Manga, 0, 10).unwrap();
    assert_eq!(rows.iter().map(|r| r.item_id).collect::<Vec<_>>(), vec![ids[2], ids[1], ids[0]]);
    assert_eq!(updates::recent(&c, Kind::Manga, 0, 2).unwrap().len(), 2, "the limit is in SQL");
    assert_eq!(updates::recent(&c, Kind::Anime, 0, 10).unwrap()[0].total_seconds, Some(1400));
    assert_eq!(updates::recent(&c, Kind::Manga, 0, 10).unwrap()[0].total_seconds, None);
    items::update(&c, &ItemPatch { id: ids[0], done: Some(true), ..Default::default() }).unwrap();
    assert_eq!(updates::by_done(&c, Kind::Manga, true, 0, 10).unwrap().len(), 1);
    assert_eq!(updates::by_done(&c, Kind::Manga, false, 0, 10).unwrap().len(), 2);
    assert!(updates::recent(&c, Kind::Manga, 4_003, 10).unwrap().is_empty());
}

#[test]
fn every_flag_write_is_one_statement_and_two_groups_never_overwrite_each_other() {
    let c = db();
    let a = add(&c, Kind::Anime, "/a");
    entries::write_flags(&c, FlagColumn::ItemFlags, a, 0).unwrap();
    entries::set_flags_masked(&c, FlagColumn::ItemFlags, a, item::SHOW_DONE, item::UNDONE_MASK).unwrap();
    entries::set_flags_masked(&c, FlagColumn::ItemFlags, a, item::SORTING_NUMBER, item::SORTING_MASK).unwrap();
    assert_eq!(entries::read_flags(&c, FlagColumn::ItemFlags, a).unwrap(), Some(item::SHOW_DONE | item::SORTING_NUMBER));
    let _ = tables::entries::by_id(&c, a).unwrap();
}
