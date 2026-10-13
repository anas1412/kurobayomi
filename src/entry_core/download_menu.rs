//! The Download menu, one for the manga page, the anime page and the Library's selection bar: the entries (the next 1, 5,
//! 10 or 25 unread items, or every unread one), their labels, which items an entry queues, and the popover.
//!
//! The rule: from the displayed (filtered, sorted) list, keep the items that are unread and not queued or on disk;
//! they queue in reading order, oldest first, and an entry takes the first N or all of them.

use kuro_downloads::DownloadState;
use kuro_i18n::{MsgKey, tr, tr_plural};
use kuro_platform::kind::Kind;

use crate::entry_core::page_shell::plain_row;

/// How many items a Download menu entry queues.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadAmount {
    Next(usize),
    AllUnread,
}

/// The counts of the "Next N" entries, in menu order.
pub const NEXT_COUNTS: [usize; 4] = [1, 5, 10, 25];

/// The entries of the menu, in order.
pub fn amounts() -> impl Iterator<Item = DownloadAmount> {
    NEXT_COUNTS.into_iter().map(DownloadAmount::Next).chain([DownloadAmount::AllUnread])
}

/// The label of an entry: "Next 5 chapters", "Next episode", "Unread", "Unseen".
pub fn label(kind: Kind, amount: DownloadAmount) -> String {
    match (kind, amount) {
        (Kind::Manga, DownloadAmount::Next(n)) => tr_plural(MsgKey::download_amount, n as i64, &[]),
        (Kind::Anime, DownloadAmount::Next(n)) => tr_plural(MsgKey::download_amount_anime, n as i64, &[]),
        (Kind::Manga, DownloadAmount::AllUnread) => tr(MsgKey::download_unread),
        (Kind::Anime, DownloadAmount::AllUnread) => tr(MsgKey::download_unseen),
    }
}

/// The menu is not offered for a local entry (its files are already on disk) or without a download handler.
pub fn menu_visible(local: bool, has_download_handler: bool) -> bool {
    !local && has_download_handler
}

/// One row of the displayed list, as the menu reads it.
#[derive(Clone, Copy, Debug)]
pub struct Candidate {
    pub id: i64,
    /// Read or seen.
    pub done: bool,
    pub state: DownloadState,
}

/// The first N or all of `pending`, which is in reading order (oldest first).
pub fn pick(pending: impl IntoIterator<Item = i64>, amount: DownloadAmount) -> Vec<i64> {
    match amount {
        DownloadAmount::Next(n) => pending.into_iter().take(n).collect(),
        DownloadAmount::AllUnread => pending.into_iter().collect(),
    }
}

/// The ids an entry queues. `rows` is the displayed list and `descending` its direction, so the oldest item is last
/// in a descending list.
pub fn targets(rows: &[Candidate], descending: bool, amount: DownloadAmount) -> Vec<i64> {
    let mut pending: Vec<i64> = rows.iter().filter(|r| !r.done && r.state == DownloadState::NotDownloaded).map(|r| r.id).collect();
    if descending {
        pending.reverse();
    }
    pick(pending, amount)
}

/// The Download menu as a popover of flat rows. `on_pick` runs with the chosen entry.
pub fn popover(kind: Kind, on_pick: impl Fn(DownloadAmount) + Clone + 'static) -> gtk::Popover {
    crate::ui::popover_menu(|popover| {
        amounts()
            .into_iter()
            .map(|amount| {
                let on_pick = on_pick.clone();
                plain_row(&label(kind, amount), popover, move |_| on_pick(amount))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, done: bool, state: DownloadState) -> Candidate {
        Candidate { id, done, state }
    }

    fn open(id: i64) -> Candidate {
        row(id, false, DownloadState::NotDownloaded)
    }

    #[test]
    fn the_menu_lists_the_next_1_5_10_25_and_every_unread_one() {
        assert_eq!(
            amounts().collect::<Vec<_>>(),
            vec![
                DownloadAmount::Next(1),
                DownloadAmount::Next(5),
                DownloadAmount::Next(10),
                DownloadAmount::Next(25),
                DownloadAmount::AllUnread
            ]
        );
    }

    #[test]
    fn read_queued_and_downloaded_items_are_not_offered_again() {
        let list = [
            open(1),
            row(2, true, DownloadState::NotDownloaded),
            row(3, false, DownloadState::Queue),
            row(4, false, DownloadState::Downloading),
            row(5, false, DownloadState::Downloaded),
            row(6, false, DownloadState::Error),
            open(7),
        ];
        assert_eq!(targets(&list, false, DownloadAmount::AllUnread), vec![1, 7], "an item that failed is not offered by the menu either");
    }

    #[test]
    fn items_queue_in_reading_order_for_either_display_direction() {
        let descending = [open(4), open(3), open(2), open(1)];
        assert_eq!(targets(&descending, true, DownloadAmount::Next(2)), vec![1, 2]);
        assert_eq!(targets(&descending, true, DownloadAmount::AllUnread), vec![1, 2, 3, 4]);
        let ascending = [open(1), open(2), open(3), open(4)];
        assert_eq!(targets(&ascending, false, DownloadAmount::Next(2)), vec![1, 2]);
        assert_eq!(targets(&ascending, false, DownloadAmount::AllUnread), vec![1, 2, 3, 4]);
    }

    #[test]
    fn next_n_stops_at_what_exists_and_next_zero_takes_nothing() {
        let list = [open(1), row(2, false, DownloadState::Downloaded), open(3)];
        assert_eq!(targets(&list, false, DownloadAmount::Next(25)), vec![1, 3]);
        assert!(targets(&list, false, DownloadAmount::Next(0)).is_empty());
        assert!(targets(&[], false, DownloadAmount::AllUnread).is_empty());
    }

    #[test]
    fn pick_takes_from_a_list_already_in_reading_order() {
        assert_eq!(pick([10, 11, 12], DownloadAmount::Next(2)), vec![10, 11]);
        assert_eq!(pick([10, 11, 12], DownloadAmount::AllUnread), vec![10, 11, 12]);
    }

    #[test]
    fn the_menu_is_hidden_for_a_local_entry_or_without_a_download_handler() {
        assert!(menu_visible(false, true));
        assert!(!menu_visible(true, true));
        assert!(!menu_visible(false, false));
    }

    #[test]
    fn labels_follow_the_kind() {
        assert_eq!(label(Kind::Manga, DownloadAmount::Next(1)), "Next chapter");
        assert_eq!(label(Kind::Manga, DownloadAmount::Next(5)), "Next 5 chapters");
        assert_eq!(label(Kind::Anime, DownloadAmount::Next(1)), "Next episode");
        assert_eq!(label(Kind::Anime, DownloadAmount::Next(25)), "Next 25 episodes");
        assert_eq!(label(Kind::Manga, DownloadAmount::AllUnread), "Unread");
        assert_eq!(label(Kind::Anime, DownloadAmount::AllUnread), "Unseen");
    }
}
