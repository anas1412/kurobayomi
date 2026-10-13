//! The delete filter, the same for both kinds. The category exclusions are cut (docs/design/unified-downloads.md D3):
//! delete-after-read keeps bookmarked units, an explicit delete removes them.

use crate::unit::UnitRef;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeleteRules {
    /// Allow deleting bookmarked units.
    pub remove_bookmarked: bool,
}

impl DeleteRules {
    /// Every explicit delete (a button the user pressed).
    pub const EXPLICIT: Self = Self { remove_bookmarked: true };
    /// Delete after read or watched.
    pub const AFTER_READ: Self = Self { remove_bookmarked: false };
}

/// The units a delete may remove: bookmarked ones are kept unless `rules.remove_bookmarked`. Empty in, empty out.
pub fn select_for_delete(units: Vec<UnitRef>, rules: DeleteRules) -> Vec<UnitRef> {
    units.into_iter().filter(|u| rules.remove_bookmarked || !u.bookmark).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: i64, bookmark: bool) -> UnitRef {
        UnitRef {
            id,
            entry_id: 1,
            url: String::new(),
            name: format!("Unit {id}"),
            scanlator: None,
            done: true,
            bookmark,
            fillermark: false,
            source_order: id,
            date_upload: 0,
            number: id as f64,
        }
    }

    fn ids(units: Vec<UnitRef>) -> Vec<i64> {
        units.iter().map(|u| u.id).collect()
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(select_for_delete(Vec::new(), DeleteRules::EXPLICIT).is_empty());
    }

    #[test]
    fn after_read_keeps_bookmarked_units() {
        let all = vec![unit(1, false), unit(2, true), unit(3, false)];
        assert_eq!(ids(select_for_delete(all, DeleteRules::AFTER_READ)), vec![1, 3]);
    }

    #[test]
    fn an_explicit_delete_removes_bookmarked_units_too() {
        let all = vec![unit(1, false), unit(2, true)];
        assert_eq!(ids(select_for_delete(all, DeleteRules::EXPLICIT)), vec![1, 2]);
    }
}
