//! Reader colours (T-043a-052) and the reader stylesheet. The page background follows the reader theme preference.
//! Chrome colours come from the active GTK theme, not from these values (ui-parity section 8).

use gtk::prelude::*;

/// The reader background choices, by `pref_reader_theme_key`: 0 white, 1 black (default), 2 gray, 3 automatic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    White,
    Black,
    Gray,
}

impl Background {
    /// The CSS class that paints the background.
    pub fn class(self) -> &'static str {
        match self {
            Self::White => "reader-bg-white",
            Self::Black => "reader-bg-black",
            Self::Gray => "reader-bg-gray",
        }
    }
}

/// The background for a stored theme value. Automatic is gray when the system prefers dark, else white.
/// Unknown values fall back to black, the default.
pub fn background_for(theme: i32, system_dark: bool) -> Background {
    match theme {
        0 => Background::White,
        2 => Background::Gray,
        3 if system_dark => Background::Gray,
        3 => Background::White,
        _ => Background::Black,
    }
}

/// Whether the desktop asks for a dark theme. Read through GTK settings, since libadwaita is not used.
pub fn system_prefers_dark() -> bool {
    gtk::Settings::default().is_some_and(|s| s.is_gtk_application_prefer_dark_theme())
}

/// Sets the background class of a reader widget. Earlier background classes are removed first.
pub fn apply_background(widget: &impl IsA<gtk::Widget>, theme: i32) {
    let background = background_for(theme, system_prefers_dark());
    for class in [Background::White, Background::Black, Background::Gray] {
        widget.remove_css_class(class.class());
    }
    widget.add_css_class(background.class());
}

/// The reader stylesheet: the background colours and the page indicator.
pub fn css() -> &'static str {
    ".reader-bg-white { background-color: #ffffff; }
.reader-bg-black { background-color: #000000; }
.reader-bg-gray { background-color: #202125; }
.reader-scrim { background-color: rgba(0, 0, 0, 0.5); }
.reader-gray { filter: grayscale(1); }
.reader-invert { filter: invert(1); }
.reader-gray-invert { filter: grayscale(1) invert(1); }
.reader-page-indicator { font-weight: bold; color: #ebebeb; text-shadow: 2px 2px 2px #2d2d2d; }"
}

/// Adds the reader stylesheet to the default display once. A missing display (headless tests) does nothing.
pub fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else { return };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(css());
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_values_map_to_the_four_choices() {
        assert_eq!(background_for(0, false), Background::White);
        assert_eq!(background_for(1, false), Background::Black);
        assert_eq!(background_for(2, true), Background::Gray);
        assert_eq!(background_for(3, true), Background::Gray);
        assert_eq!(background_for(3, false), Background::White);
    }

    #[test]
    fn an_unknown_theme_value_is_black() {
        assert_eq!(background_for(9, false), Background::Black);
    }

    #[test]
    fn the_gray_is_the_spec_colour() {
        assert!(css().contains("#202125"));
    }
}
