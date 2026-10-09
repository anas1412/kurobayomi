//! The visible-on-screen check and the landscape zoom decision (T-34a-063, `ViewExtensions.isVisibleOnScreen`,
//! `ReaderPageImageView.landscapeZoom`). The page image calls [`landscape_zoom_on_load`] once, when its image
//! finishes loading.
#![allow(dead_code)] // called by the page image widget as page decoding lands; the rules are tested here

use gtk::prelude::*;

/// A rectangle in the root window's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Bounds {
    /// True when the two rectangles overlap by a non-empty area: touching edges do not count.
    pub fn intersects(self, other: Bounds) -> bool {
        self.width > 0.0
            && self.height > 0.0
            && other.width > 0.0
            && other.height > 0.0
            && self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// A widget is visible when it is mapped and its bounds intersect the root window's bounds.
pub fn is_visible_on_screen(widget: &impl IsA<gtk::Widget>) -> bool {
    if !widget.is_mapped() {
        return false;
    }
    let Some(root) = widget.root() else { return false };
    let root_widget: &gtk::Widget = root.upcast_ref();
    let Some(bounds) = widget.compute_bounds(root_widget) else { return false };
    let own = Bounds { x: bounds.x(), y: bounds.y(), width: bounds.width(), height: bounds.height() };
    own.intersects(Bounds { x: 0.0, y: 0.0, width: root_widget.width() as f32, height: root_widget.height() as f32 })
}

/// Whether the landscape zoom applies to a page image that just finished loading: the setting is on, the picture is
/// wider than it is tall, and the widget is on screen. A page loaded off screen is not zoomed.
pub fn landscape_zoom_applies(enabled: bool, image_size: (i32, i32), visible: bool) -> bool {
    enabled && image_size.0 > image_size.1 && visible
}

/// The one call made when a page image finishes loading.
pub fn landscape_zoom_on_load(widget: &impl IsA<gtk::Widget>, enabled: bool, image_size: (i32, i32)) -> bool {
    enabled && landscape_zoom_applies(enabled, image_size, is_visible_on_screen(widget))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: f32, y: f32, width: f32, height: f32) -> Bounds {
        Bounds { x, y, width, height }
    }

    #[test]
    fn overlapping_bounds_intersect_and_disjoint_or_touching_ones_do_not() {
        let root = b(0.0, 0.0, 800.0, 600.0);
        assert!(b(100.0, 100.0, 50.0, 50.0).intersects(root));
        assert!(b(-30.0, 10.0, 50.0, 50.0).intersects(root), "partly off the left edge");
        assert!(b(790.0, 590.0, 50.0, 50.0).intersects(root), "partly off the corner");
        assert!(!b(800.0, 0.0, 50.0, 50.0).intersects(root), "touching the right edge");
        assert!(!b(-60.0, 0.0, 50.0, 50.0).intersects(root), "fully off the left edge");
        assert!(!b(0.0, 700.0, 50.0, 50.0).intersects(root), "below the window");
        assert!(!b(10.0, 10.0, 0.0, 0.0).intersects(root), "an empty rectangle");
    }

    #[test]
    fn landscape_zoom_needs_the_setting_a_wide_picture_and_a_visible_widget() {
        assert!(landscape_zoom_applies(true, (1600, 900), true));
        assert!(!landscape_zoom_applies(false, (1600, 900), true));
        assert!(!landscape_zoom_applies(true, (900, 1600), true));
        assert!(!landscape_zoom_applies(true, (900, 900), true), "a square picture is not landscape");
        assert!(!landscape_zoom_applies(true, (1600, 900), false), "loaded off screen");
    }
}
