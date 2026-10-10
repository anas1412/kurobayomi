//! T2-BULK-01, T2-BULK-04, T2-DEL-02 and T2-DEL-04: the download amount rule and the remove dialog's plan.

use crate::library::actions::{DeleteChoice, DownloadAmount, delete_plan, downloads_checkbox_offered, plan_download};
use crate::library::fixtures::row;

#[test]
fn t2_bulk_01_next_n_skips_queued_and_downloaded_items_before_taking_n() {
    // items 1 and 2 are on disk, 3 is queued: "next 2" is 4 and 5, not 1 and 2
    let unseen = [1, 2, 3, 4, 5, 6];
    let on_disk_or_queued = |id: i64| matches!(id, 1..=3);
    assert_eq!(plan_download(&unseen, DownloadAmount::Next(2), on_disk_or_queued), vec![4, 5]);
    assert_eq!(plan_download(&unseen, DownloadAmount::Next(25), on_disk_or_queued), vec![4, 5, 6], "fewer than N left");
    assert!(plan_download(&unseen, DownloadAmount::Next(0), |_| false).is_empty());
}

#[test]
fn t2_bulk_04_unviewed_takes_every_unseen_item_that_is_not_downloaded_or_queued() {
    let unseen = [10, 11, 12, 13];
    assert_eq!(plan_download(&unseen, DownloadAmount::Unviewed, |id| id == 11), vec![10, 12, 13]);
    assert_eq!(plan_download(&unseen, DownloadAmount::Unviewed, |_| true), Vec::<i64>::new());
}

#[test]
fn t2_del_02_deleting_downloads_only_keeps_the_favorite_and_skips_nothing_for_http_entries() {
    let plan = delete_plan(&[(1, false), (2, false)], DeleteChoice { from_library: false, downloads: true });
    assert!(plan.unfavorite.is_empty(), "the entries stay in the library");
    assert_eq!(plan.delete_downloads, vec![1, 2]);
    let both = delete_plan(&[(1, false)], DeleteChoice { from_library: true, downloads: true });
    assert_eq!((both.unfavorite, both.delete_downloads), (vec![1], vec![1]));
    assert!(!DeleteChoice::default().confirmable(), "OK needs a box checked");
}

#[test]
fn t2_del_04_local_entries_never_lose_their_files_and_the_checkbox_is_not_offered() {
    let plan = delete_plan(&[(1, true), (2, false)], DeleteChoice { from_library: true, downloads: true });
    assert_eq!(plan.unfavorite, vec![1, 2], "a local entry still leaves the library");
    assert_eq!(plan.delete_downloads, vec![2], "its folder is user data");
    let mut local = row(1, 0);
    local.source = 0;
    let http = row(2, 0);
    assert!(!downloads_checkbox_offered([&local, &http].into_iter()));
    assert!(downloads_checkbox_offered([&http].into_iter()));
}

#[test]
fn t2_bulk_menu_offers_1_5_10_25_and_all_unviewed_with_per_kind_labels() {
    // T-37a-028: the menu labels are per kind; "all" is the unviewed amount.
    use crate::library::actions::download_menu;
    use kuro_domain::kind::EntryKind;
    let labels = |kind| download_menu(kind).into_iter().map(|(_, label)| label).collect::<Vec<_>>();
    assert_eq!(
        labels(EntryKind::Manga),
        ["Next chapter", "Next 5 chapters", "Next 10 chapters", "Next 25 chapters", "Unread"]
    );
    assert_eq!(
        labels(EntryKind::Anime),
        ["Next episode", "Next 5 episodes", "Next 10 episodes", "Next 25 episodes", "Unwatched"]
    );
    let amounts: Vec<DownloadAmount> = download_menu(EntryKind::Manga).into_iter().map(|(amount, _)| amount).collect();
    assert_eq!(
        amounts,
        [DownloadAmount::Next(1), DownloadAmount::Next(5), DownloadAmount::Next(10), DownloadAmount::Next(25), DownloadAmount::Unviewed]
    );
}
