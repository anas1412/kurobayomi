//! Next item to continue with: the first unread chapter (`ChapterGetNextUnread`) and the first unseen episode
//! (`EpsiodeGetNextUnseen`). A descending list yields the last matching item, as in the Kotlin `rev().find`.

/// The item "continue reading" opens, or `None` when everything is read.
pub fn next_unread<T>(items: &[T], descending: bool, is_read: impl Fn(&T) -> bool) -> Option<&T> {
    if descending {
        items.iter().rev().find(|item| !is_read(item))
    } else {
        items.iter().find(|item| !is_read(item))
    }
}

/// The item "continue watching" opens, or `None` when everything is seen.
pub fn next_unseen<T>(items: &[T], descending: bool, is_seen: impl Fn(&T) -> bool) -> Option<&T> {
    if descending {
        items.iter().rev().find(|item| !is_seen(item))
    } else {
        items.iter().find(|item| !is_seen(item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (number, read)
    fn chapters() -> Vec<(u32, bool)> {
        vec![(1, true), (2, true), (3, false), (4, false), (5, true)]
    }

    #[test]
    fn ascending_returns_the_first_unread() {
        let list = chapters();
        assert_eq!(next_unread(&list, false, |c| c.1).map(|c| c.0), Some(3));
    }

    #[test]
    fn descending_returns_the_last_unread() {
        let list = chapters();
        assert_eq!(next_unread(&list, true, |c| c.1).map(|c| c.0), Some(4));
    }

    #[test]
    fn none_when_all_are_read() {
        let list = vec![(1, true), (2, true)];
        assert_eq!(next_unread(&list, false, |c| c.1), None);
        assert_eq!(next_unread(&list, true, |c| c.1), None);
    }

    #[test]
    fn unseen_follows_the_same_order_rules() {
        let episodes = vec![(1, true), (2, false), (3, false), (4, true)];
        assert_eq!(next_unseen(&episodes, false, |e| e.1).map(|e| e.0), Some(2));
        assert_eq!(next_unseen(&episodes, true, |e| e.1).map(|e| e.0), Some(3));
        let all_seen = vec![(1, true)];
        assert_eq!(next_unseen(&all_seen, false, |e| e.1), None);
    }
}
