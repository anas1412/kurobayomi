//! The `EmptyScreen` widget (T-49b-025): a kaomoji face, a message, and action buttons. The face is
//! chosen once per widget and kept (EM4).

use gtk::pango::prelude::FontExt;
use gtk::prelude::*;

use super::icons::Icon;
use super::style;

/// The 12 faces of `EmptyScreen.kt`, verbatim. The 6th has no closing bracket and the 11th starts
/// with a space.
pub const ERROR_FACES: [&str; 12] = [
    "(･o･;)",
    "Σ(ಠ_ಠ)",
    "ಥ_ಥ",
    "(˘･_･˘)",
    "(；￣Д￣)",
    "(･Д･。",
    "(╬ಠ益ಠ)",
    "(╥﹏╥)",
    "(⋟﹏⋞)",
    "Ò︵Ó",
    " ˙ᯅ˙)",
    "(¬_¬)",
];

/// Shown when the face glyphs are missing from the font.
pub const FALLBACK_FACE: &str = ":(";

/// Actions per row. More than this wrap to a second row (EM2).
pub const ACTIONS_PER_ROW: usize = 3;

/// Messages wrap at this many characters (EM3).
pub const MESSAGE_WIDTH_CHARS: i32 = 60;

/// Gap between action buttons, in logical pixels.
pub const ACTION_GAP: i32 = 8;

/// The face for an index, taken modulo the list length.
pub fn face_at(index: usize) -> &'static str {
    ERROR_FACES[index % ERROR_FACES.len()]
}

/// A uniform index into `ERROR_FACES`, from the standard library's randomly keyed hasher.
pub fn random_face_index() -> usize {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64),
    );
    (hasher.finish() % ERROR_FACES.len() as u64) as usize
}

/// The face to show: the chosen glyphs when the font has them, otherwise the ASCII fallback.
pub fn face_text(face: &'static str, glyphs_present: bool) -> &'static str {
    if glyphs_present { face } else { FALLBACK_FACE }
}

/// The action rows: `count` actions in consecutive ranges of at most `ACTIONS_PER_ROW`. No actions
/// means no rows at all (EM1).
pub fn action_rows(count: usize) -> Vec<std::ops::Range<usize>> {
    (0..count)
        .step_by(ACTIONS_PER_ROW)
        .map(|start| start..(start + ACTIONS_PER_ROW).min(count))
        .collect()
}

/// One action button: its label, its icon, and what it does.
pub struct Action {
    pub label: String,
    pub icon: Icon,
    pub on_click: Box<dyn Fn()>,
}

/// True when every non-ASCII character of `face` has a glyph in the widget's font.
pub fn glyphs_present(widget: &impl IsA<gtk::Widget>, face: &str) -> bool {
    let context = widget.pango_context();
    let desc = context.font_description().unwrap_or_default();
    let Some(font) = context.load_font(&desc) else {
        return false;
    };
    face.chars().filter(|c| !c.is_ascii()).all(|c| font.has_char(c))
}

/// Builds the empty-state widget. The whole content scrolls when it is taller than its parent (EM7).
pub fn widget(message: &str, actions: Vec<Action>) -> gtk::ScrolledWindow {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.set_halign(gtk::Align::Center);
    column.set_valign(gtk::Align::Center);
    column.set_margin_start(24);
    column.set_margin_end(24);

    let face_label = gtk::Label::new(None);
    let face = face_at(random_face_index());
    face_label.set_text(face_text(face, glyphs_present(&face_label, face)));
    face_label.add_css_class("title-1");
    face_label.add_css_class(style::class::SECONDARY_ITEM);
    face_label.set_direction(gtk::TextDirection::Ltr);
    column.append(&face_label);

    let text = gtk::Label::new(Some(message));
    text.set_wrap(true);
    text.set_max_width_chars(MESSAGE_WIDTH_CHARS);
    text.set_justify(gtk::Justification::Center);
    text.set_margin_top(24);
    text.add_css_class(style::class::SECONDARY_ITEM);
    column.append(&text);

    for range in action_rows(actions.len()) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, ACTION_GAP);
        row.set_homogeneous(true);
        row.set_margin_top(24);
        for action in &actions[range] {
            row.append(&action_button(action));
        }
        column.append(&row);
    }

    let scroller = gtk::ScrolledWindow::new();
    scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroller.set_child(Some(&column));
    scroller
}

fn action_button(action: &Action) -> gtk::Button {
    let content = gtk::Box::new(gtk::Orientation::Horizontal, ACTION_GAP);
    content.set_halign(gtk::Align::Center);
    content.append(&gtk::Image::from_icon_name(action.icon.name()));
    content.append(&gtk::Label::new(Some(&action.label)));
    let button = gtk::Button::new();
    button.set_child(Some(&content));
    button.set_hexpand(true);
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_is_taken_modulo_the_list() {
        assert_eq!(face_at(0), ERROR_FACES[0]);
        assert_eq!(face_at(12), ERROR_FACES[0]);
        assert_eq!(face_at(13), ERROR_FACES[1]);
    }

    #[test]
    fn random_index_stays_in_range() {
        for _ in 0..200 {
            assert!(random_face_index() < ERROR_FACES.len());
        }
    }

    #[test]
    fn missing_glyphs_use_the_ascii_fallback() {
        assert_eq!(face_text(ERROR_FACES[2], true), ERROR_FACES[2]);
        assert_eq!(face_text(ERROR_FACES[2], false), ":(");
    }

    #[test]
    fn action_rows_wrap_after_three() {
        assert!(action_rows(0).is_empty());
        assert_eq!(action_rows(2), vec![0..2]);
        assert_eq!(action_rows(3), vec![0..3]);
        assert_eq!(action_rows(5), vec![0..3, 3..5]);
        assert_eq!(action_rows(7), vec![0..3, 3..6, 6..7]);
    }
}
