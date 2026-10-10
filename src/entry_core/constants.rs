//! Shared constants of the entry screens (T-38a-045, T-38a-043). Upstream: `EntryScreenConstants.kt` (the three enums)
//! and `DotSeparatorText.kt` (the separator labels). Pure: the widgets read these values.

/// The separator between two words on one line: a space, U+2022 and a space.
pub const DOT_SEPARATOR: &str = " \u{2022} ";

/// The separator without the spaces.
pub const DOT: &str = "\u{2022}";

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

    /// The ids the action queues. `unread` is already in reading order, so the next items come first.
    pub fn targets(self, unread: &[i64]) -> Vec<i64> {
        let take = self.count().unwrap_or(unread.len());
        unread.iter().copied().take(take).collect()
    }
}

/// The two actions of the cover edit menu (`EditCoverAction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditCoverAction {
    Edit,
    Delete,
}

/// The rows of the entry screen (`EntryScreenItem`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryScreenItem {
    InfoBox,
    ActionRow,
    DescriptionWithTag,
    ItemHeader,
    Item,
    AiringTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_are_the_exact_code_points() {
        assert_eq!(DOT_SEPARATOR, " \u{2022} ");
        assert_eq!(DOT_SEPARATOR.chars().count(), 3);
        assert_eq!(DOT, "\u{2022}");
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

    #[test]
    fn the_enum_variants_are_distinct() {
        assert_ne!(EditCoverAction::Edit, EditCoverAction::Delete);
        assert_ne!(EntryScreenItem::InfoBox, EntryScreenItem::AiringTime);
    }
}
