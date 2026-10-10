//! The empty states of the chapter and episode lists (T-38a-053). A loading list shows a centred spinner and no text.
//! A list with no entries reads "0 chapters" and the empty line. A filter that hides every row reads "0 chapters" and
//! the filter hint. The rows of a loaded list keep the empty line hidden.

/// What the list area shows instead of rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Empty {
    /// Rows are shown, or the list is still loading.
    Nothing,
    /// The entry has no chapters or episodes at all.
    NoEntries,
    /// The entry has rows, and the current filters hide every one of them.
    FilteredOut,
}

/// `total` is the stored list, `shown` the rows after the filters.
pub fn empty_state(loading: bool, total: usize, shown: usize) -> Empty {
    if loading || shown > 0 {
        Empty::Nothing
    } else if total == 0 {
        Empty::NoEntries
    } else {
        Empty::FilteredOut
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loading_list_shows_nothing_but_the_spinner() {
        assert_eq!(empty_state(true, 0, 0), Empty::Nothing);
    }

    #[test]
    fn zero_chapters_is_the_no_entries_state() {
        assert_eq!(empty_state(false, 0, 0), Empty::NoEntries);
    }

    #[test]
    fn a_filter_that_hides_everything_is_the_filtered_out_state() {
        assert_eq!(empty_state(false, 12, 0), Empty::FilteredOut);
    }

    #[test]
    fn visible_rows_hide_the_empty_line() {
        assert_eq!(empty_state(false, 12, 3), Empty::Nothing);
    }
}
