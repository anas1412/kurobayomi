//! The placeholder shown when a cover fails to load (T-49ba-016).

use gtk::prelude::*;

use super::icons::Icon;
use super::style::class;

/// The placeholder fill alpha, as written in the stylesheet: `rgba(136, 136, 136, 0.1216)`.
pub const PLACEHOLDER_ALPHA: f32 = 0.1216;

/// The 8-bit alpha that `PLACEHOLDER_ALPHA` rounds to (31).
pub fn placeholder_alpha_byte() -> u8 {
    (PLACEHOLDER_ALPHA * 255.0).round() as u8
}

/// The icon size inside a placeholder of `width` pixels: a third of the width.
pub fn icon_size(width: i32) -> i32 {
    width / 3
}

/// The placeholder for a cover that failed to load: the cover icon on the placeholder fill.
pub fn placeholder(width: i32) -> gtk::Box {
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    frame.add_css_class(class::COVER_PLACEHOLDER);
    frame.set_size_request(width, width);
    frame.set_halign(gtk::Align::Fill);
    frame.set_valign(gtk::Align::Fill);
    let image = gtk::Image::from_icon_name(Icon::Cover.name());
    image.set_pixel_size(icon_size(width));
    image.set_hexpand(true);
    image.set_vexpand(true);
    frame.append(&image);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_alpha_rounds_to_31() {
        assert_eq!(placeholder_alpha_byte(), 31);
    }

    #[test]
    fn icon_is_a_third_of_the_width() {
        assert_eq!(icon_size(300), 100);
        assert_eq!(icon_size(100), 33);
    }
}
