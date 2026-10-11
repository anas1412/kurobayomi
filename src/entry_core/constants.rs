//! Shared constants of the entry screens (T-38a-045, T-38a-043). Upstream: `EntryScreenConstants.kt` (the three enums)
//! and `DotSeparatorText.kt` (the separator labels). Pure: the widgets read these values.

/// How many unread items a Download action queues (`DownloadAction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadAction {
    Next1Item,
    Next5Items,
    Next10Items,
    Next25Items,
    UnviewedItems,
}

impl DownloadAction {
    /// The number of items the action takes from the front, or `None` for every unread item.
    pub fn count(self) -> Option<usize> {
        match self {
            Self::Next1Item => Some(1),
            Self::Next5Items => Some(5),
            Self::Next10Items => Some(10),
            Self::Next25Items => Some(25),
            Self::UnviewedItems => None,
        }
    }

    #[cfg(test)]
    /// The ids the action queues. `unread` is already in reading order, so the next items come first.
    pub fn targets(self, unread: &[i64]) -> Vec<i64> {
        let take = self.count().unwrap_or(unread.len());
        unread.iter().copied().take(take).collect()
    }
}

/// The width of the left info pane in the two-pane layouts of the manga and anime entry screens.
pub const LEFT_PANE_WIDTH: i32 = 360;
/// The window width from which the entry screens show two panes (info on the left, list on the right).
pub const TWO_PANE_MIN_WIDTH: i32 = 900;
/// The window size assumed before the page has a real one: the app's default size.
pub const FALLBACK_WIDTH: i32 = 1180;
pub const FALLBACK_HEIGHT: i32 = 780;

/// True when a window of this width gets the two-pane entry layout.
pub fn two_pane(window_width: i32) -> bool {
    window_width >= TWO_PANE_MIN_WIDTH
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_panes_start_at_900_px_and_the_left_pane_is_360() {
        assert!(!two_pane(899));
        assert!(two_pane(900));
        assert_eq!(LEFT_PANE_WIDTH, 360);
        assert_eq!((FALLBACK_WIDTH, FALLBACK_HEIGHT), (1180, 780));
    }

    #[test]
    fn download_actions_take_the_next_n_unread_or_all_of_them() {
        let unread = [11, 12, 13, 14, 15, 16];
        assert_eq!(DownloadAction::Next1Item.targets(&unread), vec![11]);
        assert_eq!(DownloadAction::Next5Items.targets(&unread), vec![11, 12, 13, 14, 15]);
        assert_eq!(DownloadAction::Next10Items.targets(&unread), unread.to_vec(), "fewer than ten: all of them");
        assert_eq!(DownloadAction::Next25Items.count(), Some(25));
        assert_eq!(DownloadAction::UnviewedItems.targets(&unread), unread.to_vec());
        assert!(DownloadAction::UnviewedItems.targets(&[]).is_empty());
    }

}
