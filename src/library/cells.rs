//! What the grid shows: the entries of the active page, preceded by the global search row while a search is on
//! (B-6.4, T-37a-022). Pure; the view turns each cell into a widget.

use kuro_domain::library::page::LibraryItem;
use kuro_i18n::{Arg, MsgKey, trf};

/// One cell of the cover grid.
#[derive(Clone, Debug)]
pub enum Cell {
    /// `Search for "<query>" globally`; opens global search with the query.
    Global(String),
    Item(LibraryItem),
}

/// The label of the global search row.
pub fn global_label(query: &str) -> String {
    trf(MsgKey::action_global_search_query, &[("arg1", Arg::Str(query))])
}

/// The grid cells: the global row first when `query` is non-empty, then every item in order. A page with no items
/// has no grid; its empty state carries the same button, so the row is not added there.
pub fn cells(items: &[LibraryItem], query: Option<&str>) -> Vec<Cell> {
    let global = query.filter(|q| !q.is_empty() && !items.is_empty()).map(|q| Cell::Global(q.to_owned()));
    global.into_iter().chain(items.iter().cloned().map(Cell::Item)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::fixtures::item as fixture;

    fn item(id: i64) -> LibraryItem {
        fixture(id, 0)
    }

    #[test]
    fn a_query_puts_the_global_row_first_even_with_matches() {
        let all = cells(&[item(1), item(2)], Some("ber"));
        assert_eq!(all.len(), 3);
        assert!(matches!(&all[0], Cell::Global(q) if q == "ber"));
        assert!(matches!(&all[1], Cell::Item(i) if i.row.id == 1));
    }

    #[test]
    fn an_empty_page_leaves_the_global_button_to_the_empty_state() {
        assert!(cells(&[], Some("ber")).is_empty());
    }

    #[test]
    fn no_query_or_an_empty_query_has_no_global_row() {
        assert_eq!(cells(&[item(1)], None).len(), 1);
        assert_eq!(cells(&[item(1)], Some("")).len(), 1);
    }

    #[test]
    fn the_label_quotes_the_query() {
        let label = global_label("berserk");
        assert!(label.contains("Search for") && label.contains("berserk") && label.contains("globally"), "{label}");
    }
}
