//! FAB expansion on scroll (`shouldExpandFAB`, T-49b-033). The FAB expands at the top of a list, while
//! the list scrolls up, and at the bottom; it collapses while the list scrolls down.

use std::cell::Cell;

use gtk::prelude::*;

/// A scroll position: the adjustment's value, page size and upper bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub value: f64,
    pub page: f64,
    pub upper: f64,
}

/// True when the FAB should expand: at the top, scrolling up, or within half a pixel of the bottom.
pub fn should_expand_fab(now: Position, prev_value: f64) -> bool {
    now.value <= 0.0 || now.value < prev_value || now.value + now.page >= now.upper - 0.5
}

/// Calls `on_expand` on each `value-changed`, comparing with the value of the previous change.
pub fn track(adj: &gtk::Adjustment, on_expand: impl Fn(bool) + 'static) {
    let prev = Cell::new(adj.value());
    adj.connect_value_changed(move |a| {
        let now = Position {
            value: a.value(),
            page: a.page_size(),
            upper: a.upper(),
        };
        on_expand(should_expand_fab(now, prev.get()));
        prev.set(now.value);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The truth table over the three inputs: top, scrolling up, bottom. The expected value is the OR.
    #[test]
    fn expands_exactly_when_one_of_the_three_inputs_holds() {
        for bits in 0..8u8 {
            let top = bits & 1 != 0;
            let up = bits & 2 != 0;
            let bottom = bits & 4 != 0;
            let page = 10.0;
            let value = if top { 0.0 } else { 50.0 };
            let prev = if up { value + 10.0 } else { value - 10.0 };
            let upper = if bottom { value + page } else { value + page + 100.0 };
            let now = Position { value, page, upper };
            assert_eq!(should_expand_fab(now, prev), top || up || bottom, "bits {bits:03b}");
        }
    }

    #[test]
    fn the_top_expands_even_when_value_did_not_move() {
        let now = Position { value: -1.0, page: 10.0, upper: 500.0 };
        assert!(should_expand_fab(now, -1.0));
    }
}
