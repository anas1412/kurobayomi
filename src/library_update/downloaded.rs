//! The "downloaded only" filter of the update passes (T-26a-060 chapters, T-26a-063 episodes). A local entry returns
//! its input unchanged, because its items are the files themselves and nothing is downloaded from a source. Any other
//! entry keeps the items the download cache holds, in their original order.

/// The items to act on. `is_downloaded` answers from the download cache of the entry.
pub fn only_downloaded<T: Clone>(local: bool, items: &[T], is_downloaded: impl Fn(&T) -> bool) -> Vec<T> {
    if local {
        return items.to_vec();
    }
    items.iter().filter(|item| is_downloaded(item)).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // (number, present in the download cache)
    fn items() -> Vec<(f64, bool)> {
        vec![(1.0, true), (2.0, false), (3.0, true), (4.0, false)]
    }

    #[test]
    fn a_remote_entry_keeps_only_the_downloaded_items_in_order() {
        // T-26a-060, T-26a-063
        let kept = only_downloaded(false, &items(), |item| item.1);
        assert_eq!(kept, vec![(1.0, true), (3.0, true)]);
    }

    #[test]
    fn a_local_entry_returns_its_input_unchanged() {
        let kept = only_downloaded(true, &items(), |item| item.1);
        assert_eq!(kept, items());
    }

    #[test]
    fn an_empty_cache_leaves_a_remote_entry_with_nothing_to_do() {
        assert!(only_downloaded(false, &items(), |_| false).is_empty());
    }
}
