//! The domain's item rules that are implemented for the legacy `Chapter` and `Episode` only, over the shared `Item`: the
//! missing-item rules (`kuro_domain::items::missing::Numbered`) and the auto-download filter (`NewItem`, `ReadState`). The
//! orphan rule keeps an `impl` of a domain trait for `Item` out of this crate, so the rows go through these wrappers. They
//! go when kuro-domain implements the traits for `Item`.

use kuro_data::item::Item;
use kuro_domain::download::filter::{NewItem, ReadState};
use kuro_domain::items::missing::Numbered;

/// An item seen by the gap and separator rules.
#[derive(Clone, Copy, Debug)]
pub struct NumberedItem<'a>(pub &'a Item);

impl Numbered for NumberedItem<'_> {
    fn item_id(&self) -> i64 {
        self.0.id
    }
    fn number(&self) -> f64 {
        self.0.number
    }
}

/// An owned item for the auto-download filter (`filter_new`).
#[derive(Clone, Debug)]
pub struct AutoDownloadItem(pub Item);

impl NewItem for AutoDownloadItem {
    fn number(&self) -> f64 {
        self.0.number
    }
}

impl ReadState for AutoDownloadItem {
    fn is_read(&self) -> bool {
        self.0.done
    }
}

#[cfg(test)]
mod tests {
    use kuro_domain::items::missing::{ListRow, insert_missing_separators, item_gap};
    use kuro_platform::kind::Kind;

    use super::*;

    fn item(id: i64, number: f64) -> Item {
        Item { id, number, ..Item::create(Kind::Anime) }
    }

    #[test]
    fn a_gap_between_two_items_is_counted_and_separated_for_either_kind() {
        let (a, b) = (item(1, 5.0), item(2, 8.0));
        assert_eq!(item_gap(Some(&NumberedItem(&b)), Some(&NumberedItem(&a))), 2);
        let rows = insert_missing_separators(vec![NumberedItem(&b), NumberedItem(&a)], true);
        assert!(rows.iter().any(|r| matches!(r, ListRow::MissingCount { count: 2, .. })), "{rows:?}");
    }
}
