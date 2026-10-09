//! T2-SORT-01 to T2-SORT-10: the sort engine and its writes.

use std::collections::BTreeMap;
use std::sync::Arc;

use kuro_domain::category::service::CategoryService;
use kuro_domain::kind::EntryKind;
use kuro_domain::library::order::{SortContext, sort_page, tracker_mean};
use kuro_domain::library::page::LibraryItem;
use kuro_domain::library::sort::{Direction, LibrarySort, SortType};
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::store::PrefStore;

use crate::library::fixtures::item;

fn sorted(items: Vec<LibraryItem>, ty: SortType, dir: Direction, scores: &BTreeMap<i64, Vec<f64>>, seed: i32) -> Vec<i64> {
    let ctx = SortContext { tracker_scores: scores, random_seed: seed };
    sort_page(items, LibrarySort { ty, dir }, &ctx).iter().map(|i| i.row.id).collect()
}

fn plain(items: Vec<LibraryItem>, ty: SortType, dir: Direction) -> Vec<i64> {
    sorted(items, ty, dir, &BTreeMap::new(), 0)
}

fn titled(id: i64, title: &str) -> LibraryItem {
    let mut i = item(id, 0);
    i.row.title = title.into();
    i
}

#[test]
fn t2_sort_01_alphabetical_ignores_case_and_descending_reverses() {
    let items = || vec![titled(1, "Banana"), titled(2, "apple"), titled(3, "cherry")];
    assert_eq!(plain(items(), SortType::Alphabetical, Direction::Ascending), vec![2, 1, 3]);
    assert_eq!(plain(items(), SortType::Alphabetical, Direction::Descending), vec![3, 1, 2]);
    let ties = || vec![titled(1, "b"), titled(2, "a"), titled(3, "b")];
    assert_eq!(plain(ties(), SortType::Alphabetical, Direction::Ascending), vec![2, 1, 3], "equal titles keep their order");
}

#[test]
fn t2_sort_02_numeric_types_compare_plainly() {
    type Set = fn(&mut LibraryItem, i64);
    let cases: [(SortType, Set); 6] = [
        (SortType::LastActivity, |i, v| i.row.last_activity = v),
        (SortType::LastUpdate, |i, v| i.row.last_update = v),
        (SortType::TotalItems, |i, v| i.row.total = v),
        (SortType::LatestItem, |i, v| i.row.latest_upload = v),
        (SortType::ItemFetchDate, |i, v| i.row.item_fetched_at = v),
        (SortType::DateAdded, |i, v| i.row.date_added = v),
    ];
    for (ty, set) in cases {
        let items = || {
            [(1, 30), (2, 10), (3, 20)]
                .into_iter()
                .map(|(id, v)| {
                    let mut i = item(id, 0);
                    set(&mut i, v);
                    i
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(plain(items(), ty, Direction::Ascending), vec![2, 3, 1], "{ty:?} ascending");
        assert_eq!(plain(items(), ty, Direction::Descending), vec![1, 3, 2], "{ty:?} descending");
    }
}

fn pending(id: i64, pending: i64) -> LibraryItem {
    let mut i = item(id, 0);
    i.row.total = pending;
    i.row.done = 0;
    i
}

#[test]
fn t2_sort_03_unseen_count_puts_zero_last_in_both_directions() {
    let items = || [0, 3, 1, 0, 2].iter().enumerate().map(|(n, p)| pending(n as i64 + 1, *p)).collect::<Vec<_>>();
    let values = |ids: Vec<i64>| ids.iter().map(|id| [0, 3, 1, 0, 2][*id as usize - 1]).collect::<Vec<i64>>();
    assert_eq!(values(plain(items(), SortType::UnreadCount, Direction::Ascending)), vec![1, 2, 3, 0, 0]);
    assert_eq!(values(plain(items(), SortType::UnreadCount, Direction::Descending)), vec![3, 2, 1, 0, 0]);
}

#[test]
fn t2_sort_04_airing_time_orders_by_pending_then_airing_with_zero_last() {
    let airing = |id: i64, at: i64| {
        let mut i = pending(id, 2);
        i.row.viewer_flags = at << 24;
        i
    };
    let items = || vec![airing(1, 0), airing(2, 500), airing(3, 100)];
    assert_eq!(plain(items(), SortType::AiringTime, Direction::Ascending), vec![3, 2, 1]);
    assert_eq!(plain(items(), SortType::AiringTime, Direction::Descending), vec![2, 3, 1]);
    let mut fewer = pending(4, 1);
    fewer.row.viewer_flags = 900 << 24;
    let mixed = vec![airing(1, 100), fewer];
    assert_eq!(plain(mixed, SortType::AiringTime, Direction::Ascending), vec![4, 1], "the pending count is the primary key");
}

#[test]
fn t2_sort_05_random_is_seeded_and_does_not_tie_break() {
    let items = || (1..=12).map(|id| item(id, 0)).collect::<Vec<_>>();
    let scores = BTreeMap::new();
    let a = sorted(items(), SortType::Random, Direction::Ascending, &scores, 7);
    assert_eq!(a, sorted(items(), SortType::Random, Direction::Ascending, &scores, 7));
    assert_ne!(a, sorted(items(), SortType::Random, Direction::Ascending, &scores, 8));
    assert_ne!(a, (1..=12).collect::<Vec<i64>>(), "no alphabetical tiebreak is applied");
}

#[test]
fn t2_sort_06_tracker_mean_uses_minus_one_for_unscored() {
    let scores = BTreeMap::from([(1, vec![7.5]), (2, vec![]), (3, vec![9.0])]);
    let items = vec![item(1, 0), item(2, 0), item(3, 0)];
    assert_eq!(sorted(items, SortType::TrackerMean, Direction::Ascending, &scores, 0), vec![2, 1, 3]);
    assert_eq!(tracker_mean(&[]), -1.0);
    assert_eq!(tracker_mean(&[f64::NAN]), -1.0, "a NaN mean counts as unscored (E-14)");
}

fn service(kind: EntryKind) -> (tempfile::TempDir, CategoryService, Arc<LibraryPrefs>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let (path, db) = match kind {
        EntryKind::Manga => {
            let path = dir.path().join("manga.db");
            let writer = kuro_db::schema_manga::open(&path).expect("manga db");
            (path.clone(), Arc::new(kuro_db::handle::Db::new(&path, writer).expect("handle")))
        }
        EntryKind::Anime => {
            let path = dir.path().join("anime.db");
            let writer = kuro_db::schema_anime::open(&path).expect("anime db");
            (path.clone(), Arc::new(kuro_db::handle::Db::new(&path, writer).expect("handle")))
        }
    };
    let _ = path;
    let prefs = Arc::new(LibraryPrefs::new(PrefStore::in_memory()));
    (dir, CategoryService::new(db, kind, prefs.clone()), prefs)
}

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime").block_on(future)
}

#[test]
fn t2_sort_07_the_random_seed_is_stored_per_kind() {
    let (_d, manga, manga_prefs) = service(EntryKind::Manga);
    let random = LibrarySort { ty: SortType::Random, dir: Direction::Ascending };
    block_on(manga.set_sort_mode(None, random, || 4242)).expect("set the manga sort");
    assert_eq!(manga_prefs.random_manga_sort_seed.get(), 4242);
    assert_eq!(manga_prefs.random_anime_sort_seed.get(), 0, "the anime seed is a different key");
}

#[test]
fn t2_sort_08_a_missing_last_update_sorts_first_ascending() {
    let mut never = item(1, 0);
    never.row.last_update = 0; // NULL reads as 0
    let mut later = item(2, 0);
    later.row.last_update = 5;
    assert_eq!(plain(vec![later.clone(), never.clone()], SortType::LastUpdate, Direction::Ascending), vec![1, 2]);
    assert_eq!(plain(vec![later, never], SortType::LastUpdate, Direction::Descending), vec![2, 1]);
}

#[test]
fn t2_sort_09_equal_keys_break_by_title_ascending_even_when_descending() {
    let mut a = titled(1, "b");
    let mut b = titled(2, "a");
    a.row.total = 5;
    b.row.total = 5;
    assert_eq!(plain(vec![a, b], SortType::TotalItems, Direction::Descending), vec![2, 1]);
}

#[test]
fn t2_sort_10_categorized_display_decides_which_categories_take_the_sort() {
    let (_d, svc, prefs) = service(EntryKind::Manga);
    let a = block_on(svc.create("A")).expect("a");
    let b = block_on(svc.create("B")).expect("b");
    let date_added = LibrarySort { ty: SortType::DateAdded, dir: Direction::Descending };
    let flags_of = |id: i64| svc.repo().get(id).expect("read").expect("row").flags;
    let before = (flags_of(a.id), flags_of(b.id));

    prefs.categorized_display_settings.set(true).expect("pref");
    block_on(svc.set_sort_mode(Some(a.id), date_added, || 0)).expect("categorized write");
    assert_eq!(flags_of(a.id), date_added.apply_to(before.0), "the target category changes");
    assert_eq!(flags_of(b.id), before.1, "the others do not");

    prefs.categorized_display_settings.set(false).expect("pref");
    block_on(svc.set_sort_mode(Some(a.id), date_added, || 0)).expect("global write");
    assert_eq!(flags_of(a.id), flags_of(b.id), "the same flags reach every category");
    assert_eq!(svc.global_sort(), date_added);
}
