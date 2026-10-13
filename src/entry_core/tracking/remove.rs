//! Tracker removal from an entry (T-38b-036, T-39b-046; E-TRK-12, D-5, D-11). The local unlink always runs. The remote
//! delete runs only for a deletable tracker whose checkbox is set, and the checkbox is unchecked by default. The order
//! is the tracker's (`Tracker::removes_local_first`), the same for a manga and an anime: `kuro_track::remove` runs it.

/// The checkbox shows only for a deletable tracker.
pub fn checkbox_shown(deletable: bool) -> bool {
    deletable
}

/// The checkbox starts unchecked.
pub const CHECKBOX_DEFAULT: bool = false;

/// The remote delete runs only when the tracker can delete and the checkbox is set (E-TRK-12).
pub fn remote_delete(deletable: bool, checkbox_on: bool) -> bool {
    deletable && checkbox_on
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_delete_needs_a_deletable_tracker_and_the_checkbox() {
        assert!(!CHECKBOX_DEFAULT, "unchecked by default");
        assert!(!checkbox_shown(false));
        assert!(remote_delete(true, true));
        assert!(!remote_delete(true, false));
        assert!(!remote_delete(false, true));
    }
}
