//! Generic track list sheet skeleton (area 46 `GenericTracksSheet.kt`, T-46b-010). Header, a
//! scrollable list, and a footer slot that is always the last list item. Pure: the GTK sheet builds
//! its rows from `rows` and uses the sizes below. Lengths are logical pixels.

/// Title row padding: start, end, top, bottom.
pub const TITLE_PADDING: (i32, i32, i32, i32) = (16, 16, 8, 4);
/// Height of the add-track row.
pub const ADD_ROW_HEIGHT: i32 = 48;

/// One row of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row<'a, T> {
    Item(&'a T),
    /// The footer slot. Present even when the footer has no content (behaviour 1.1).
    Footer,
}

/// The list rows: every item in order, then the footer slot.
pub fn rows<T>(items: &[T]) -> Vec<Row<'_, T>> {
    items
        .iter()
        .map(Row::Item)
        .chain(std::iter::once(Row::Footer))
        .collect()
}

/// Whether a click at `(x, y)` in a row `width` wide hits the add row. The whole row is the target,
/// not only its label.
pub fn add_row_hit(x: f32, y: f32, width: f32) -> bool {
    (0.0..width).contains(&x) && (0.0..ADD_ROW_HEIGHT as f32).contains(&y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_footer_is_the_last_row_even_for_an_empty_list() {
        assert_eq!(rows::<i32>(&[]), vec![Row::Footer]);
        let r = rows(&[1, 2]);
        assert_eq!(r, vec![Row::Item(&1), Row::Item(&2), Row::Footer]);
    }

    #[test]
    fn sizes_match_the_spec() {
        assert_eq!(TITLE_PADDING, (16, 16, 8, 4));
        assert_eq!(ADD_ROW_HEIGHT, 48);
    }

    #[test]
    fn the_whole_add_row_is_clickable() {
        assert!(add_row_hit(0.0, 0.0, 420.0));
        assert!(add_row_hit(419.0, 47.0, 420.0));
        assert!(!add_row_hit(420.0, 10.0, 420.0));
        assert!(!add_row_hit(10.0, 48.0, 420.0));
    }
}
