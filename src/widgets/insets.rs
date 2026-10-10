//! Insets in px (T-49b-034, `PaddingValues` plus). `Add` sums the edges component-wise, so
//! `a + b` is the padding of two stacked paddings. Start and end are left and right: LTR only.

use std::ops::Add;

use gtk::prelude::*;

/// Padding on the four edges, in px. Start is the left edge in LTR.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub start: f32,
    pub end: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Insets {
    /// No inset on any edge: the identity of `Add`.
    pub const ZERO: Insets = Insets { start: 0.0, end: 0.0, top: 0.0, bottom: 0.0 };

    /// The same inset on every edge.
    pub const fn all(px: f32) -> Insets {
        Insets { start: px, end: px, top: px, bottom: px }
    }

    /// Sets the widget margins to these insets, rounded to whole px.
    pub fn apply(&self, widget: &impl IsA<gtk::Widget>) {
        let w = widget.as_ref();
        w.set_margin_start(self.start.round() as i32);
        w.set_margin_end(self.end.round() as i32);
        w.set_margin_top(self.top.round() as i32);
        w.set_margin_bottom(self.bottom.round() as i32);
    }
}

impl Add for Insets {
    type Output = Insets;

    fn add(self, rhs: Insets) -> Insets {
        Insets {
            start: self.start + rhs.start,
            end: self.end + rhs.end,
            top: self.top + rhs.top,
            bottom: self.bottom + rhs.bottom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a() -> Insets {
        Insets { start: 1.0, end: 2.0, top: 3.0, bottom: 4.0 }
    }

    fn b() -> Insets {
        Insets { start: 10.0, end: 20.0, top: 30.0, bottom: 40.0 }
    }

    fn c() -> Insets {
        Insets { start: 100.0, end: 200.0, top: 300.0, bottom: 400.0 }
    }

    #[test]
    fn add_sums_each_edge() {
        assert_eq!(a() + b(), Insets { start: 11.0, end: 22.0, top: 33.0, bottom: 44.0 });
    }

    #[test]
    fn add_is_associative() {
        assert_eq!((a() + b()) + c(), a() + (b() + c()));
    }

    #[test]
    fn zero_is_the_identity_on_both_sides() {
        assert_eq!(a() + Insets::ZERO, a());
        assert_eq!(Insets::ZERO + a(), a());
        assert_eq!(Insets::default(), Insets::ZERO);
    }

    #[test]
    fn all_puts_the_same_px_on_every_edge() {
        assert_eq!(Insets::all(24.0), Insets { start: 24.0, end: 24.0, top: 24.0, bottom: 24.0 });
    }
}
