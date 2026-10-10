//! The pick behind "Open random entry" (T-37a-034; `AnimeLibraryScreenModel.kt` 596-604). The pick is taken over the
//! filtered and searched entries of the page on screen, so the index always refers to a row that exists.

/// Shown when the page has no entry to open (`AnimeLibraryTab.kt` 148-158).
pub const EMPTY_TOAST: &str = "No entries found in this category";

/// The index of the pick over `len` entries, or `None` for an empty page. The index is `nonce` reduced modulo `len`,
/// so it is always inside the page (E-17).
pub fn pick_index(len: usize, nonce: u64) -> Option<usize> {
    (len > 0).then(|| (nonce % len as u64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_page_has_no_pick() {
        assert_eq!(pick_index(0, 0), None);
        assert_eq!(pick_index(0, u64::MAX), None);
    }

    #[test]
    fn a_one_entry_page_always_picks_that_entry() {
        for nonce in [0_u64, 1, 7, u64::MAX] {
            assert_eq!(pick_index(1, nonce), Some(0));
        }
    }

    #[test]
    fn every_index_is_inside_the_page_and_every_entry_can_be_reached() {
        let len = 5;
        let picks: Vec<usize> = (0..200_u64).filter_map(|n| pick_index(len, n.wrapping_mul(2_654_435_761))).collect();
        assert!(picks.iter().all(|&i| i < len), "no index runs past the page");
        for slot in 0..len {
            assert!(picks.contains(&slot), "entry {slot} can be picked");
        }
    }

    #[test]
    fn the_same_nonce_gives_the_same_pick() {
        assert_eq!(pick_index(10, 42), pick_index(10, 42));
        assert_eq!(pick_index(10, 42), Some(2));
    }

    #[test]
    fn the_empty_message_is_the_upstream_text() {
        assert_eq!(EMPTY_TOAST, "No entries found in this category");
    }
}
