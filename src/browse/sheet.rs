//! The filter sheet's buttons, widget values and tri-state display (T-40b-050, T-40b-051, T-40b-053). The state rules
//! are in `kuro_source::filter_sheet`; this file holds the decisions the sheet widgets read from them. Pure.

/// The two buttons of the filter sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetButton {
    Reset,
    Filter,
}

/// What a sheet button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetEffect {
    pub reset_state: bool,
    pub search: bool,
    pub close: bool,
}

/// Reset replaces the state with the defaults and keeps the sheet open without searching (E-FL-06). Filter searches
/// with the current values and closes the sheet (E-FL-07).
pub fn on_button(button: SheetButton) -> SheetEffect {
    match button {
        SheetButton::Reset => SheetEffect { reset_state: true, search: false, close: false },
        SheetButton::Filter => SheetEffect { reset_state: false, search: true, close: true },
    }
}

/// The value a select shows: `values[state]`, and index 0 when the stored index is out of range (B-EC-21).
pub fn select_label(values: &[String], state: i32) -> Option<&str> {
    usize::try_from(state).ok().and_then(|i| values.get(i)).or(values.first()).map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_keeps_the_sheet_open_and_does_not_search() {
        assert_eq!(on_button(SheetButton::Reset), SheetEffect { reset_state: true, search: false, close: false });
    }

    #[test]
    fn filter_searches_with_the_current_values_and_closes() {
        assert_eq!(on_button(SheetButton::Filter), SheetEffect { reset_state: false, search: true, close: true });
    }

    #[test]
    fn a_select_shows_the_indexed_value_and_index_zero_when_out_of_range() {
        let values = vec!["Any".to_owned(), "Action".to_owned()];
        assert_eq!(select_label(&values, 1), Some("Action"));
        assert_eq!(select_label(&values, 9), Some("Any"));
        assert_eq!(select_label(&values, -1), Some("Any"));
        assert_eq!(select_label(&[], 0), None);
    }

}
