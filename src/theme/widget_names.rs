//! Widget names that other code looks up (T-49ba-036).

use gtk::prelude::*;

/// The name of the reader pager. The window treats a visible pager as the immersive reader, which
/// hides the sidebar and the tab bar.
pub const READER_PAGER: &str = "reader_pager";

/// Names a widget as the reader pager.
pub fn name_reader_pager(widget: &impl IsA<gtk::Widget>) {
    widget.set_widget_name(READER_PAGER);
}

/// True when the widget name is the reader pager's.
pub fn is_reader_pager(name: &str) -> bool {
    name == READER_PAGER
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_pager_has_its_fixed_name() {
        assert_eq!(READER_PAGER, "reader_pager");
        assert!(is_reader_pager("reader_pager"));
        assert!(!is_reader_pager("sidebar"));
    }
}
