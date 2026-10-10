//! App bar rules (Kotlin `AppBar`, T-36a-037 and T-36a-038): title in action mode, navigation icon, count pill and the
//! overflow button. The GTK header is built by `header_title` and `nav_button`.

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// Count text size in points.
pub const COUNT_PILL_PT: u32 = 14;

/// Pill background alpha over the foreground colour: 0.12 in dark mode, 0.08 in light mode.
pub fn pill_alpha(dark: bool) -> f32 {
    if dark { 0.12 } else { 0.08 }
}

/// The count pill shows only for a positive count.
pub fn show_count_pill(count: u32) -> bool {
    count > 0
}

/// The title text. In action mode the title is the selected count.
pub fn title_text(action_mode: bool, selected: u32, title: &str) -> String {
    if action_mode { selected.to_string() } else { title.to_string() }
}

/// The navigation slot: close (cancel) in action mode, up when a parent page exists, nothing otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavIcon {
    Close,
    Up,
    None,
}

pub fn nav_icon(action_mode: bool, has_parent: bool) -> NavIcon {
    if action_mode {
        NavIcon::Close
    } else if has_parent {
        NavIcon::Up
    } else {
        NavIcon::None
    }
}

/// The overflow button shows only when there is at least one overflow item.
pub fn overflow_visible(overflow_items: usize) -> bool {
    overflow_items > 0
}

/// The title and subtitle box: title one line with ellipsis, the count pill beside it when it applies, and the subtitle
/// one line with ellipsis and the full text as its tooltip (the marquee is dropped, behaviour 5.8).
pub fn header_title(title: &str, subtitle: Option<&str>, count: u32, dark: bool) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    let label = gtk::Label::builder().label(title).ellipsize(gtk::pango::EllipsizeMode::End).single_line_mode(true).max_width_chars(28).css_classes(["title"]).build();
    column.append(&label);
    if show_count_pill(count) {
        ensure_pill_css();
        let class = if dark { "kuro-count-pill-dark" } else { "kuro-count-pill-light" };
        let pill = gtk::Label::builder().label(count.to_string()).css_classes([class]).build();
        pill.set_attributes(Some(&size_attrs(COUNT_PILL_PT)));
        column.append(&pill);
    }
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vbox.append(&column);
    if let Some(sub) = subtitle {
        let sub_label = gtk::Label::builder().label(sub).tooltip_text(sub).ellipsize(gtk::pango::EllipsizeMode::End).single_line_mode(true).css_classes(["caption"]).xalign(0.0).build();
        vbox.append(&sub_label);
    }
    vbox
}

/// The navigation button for `icon`, with its tooltip and accessible name, or `None` for no button.
pub fn nav_button(icon: NavIcon) -> Option<gtk::Button> {
    let (icon_name, key) = match icon {
        NavIcon::Close => ("window-close-symbolic", MsgKey::action_cancel),
        NavIcon::Up => ("go-previous-symbolic", MsgKey::action_bar_up_description),
        NavIcon::None => return None,
    };
    let text = tr(key);
    Some(gtk::Button::builder().icon_name(icon_name).tooltip_text(text.as_str()).build())
}

/// An action button: tooltip is the title, sensitivity follows `enabled`.
pub fn action_button(title: &str, icon_name: &str, enabled: bool) -> gtk::Button {
    gtk::Button::builder().icon_name(icon_name).tooltip_text(title).sensitive(enabled).build()
}

/// The overflow button, or `None` when there is no overflow item. The caller fills its menu.
pub fn overflow_button(overflow_items: usize) -> Option<gtk::MenuButton> {
    if !overflow_visible(overflow_items) {
        return None;
    }
    let text = tr(MsgKey::action_menu_overflow_description);
    Some(gtk::MenuButton::builder().icon_name("view-more-symbolic").tooltip_text(text.as_str()).build())
}

/// What Escape does in the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeAction {
    /// Leave action mode and clear the selection.
    CancelActionMode,
    /// Not the bar's key: it goes on to the next handler (back, close search).
    Pass,
}

pub fn escape_action(action_mode: bool) -> EscapeAction {
    if action_mode { EscapeAction::CancelActionMode } else { EscapeAction::Pass }
}

/// Makes Escape on `widget` cancel action mode: `in_action_mode` says whether it is on, `cancel` leaves it.
pub fn cancel_on_escape(widget: &impl IsA<gtk::Widget>, in_action_mode: impl Fn() -> bool + 'static, cancel: impl Fn() + 'static) {
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape && escape_action(in_action_mode()) == EscapeAction::CancelActionMode {
            cancel();
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    widget.add_controller(keys);
}

thread_local! {
    static PILL_CSS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Installs the two pill rules once per display: the foreground colour at the light or the dark alpha.
fn ensure_pill_css() {
    if PILL_CSS.with(|done| done.replace(true)) {
        return;
    }
    let Some(display) = gtk::gdk::Display::default() else { return };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&format!(
        ".kuro-count-pill-light {{ background-color: alpha(@window_fg_color, {light}); border-radius: 999px; padding: 0 6px; }} \
         .kuro-count-pill-dark {{ background-color: alpha(@window_fg_color, {dark}); border-radius: 999px; padding: 0 6px; }}",
        light = pill_alpha(false),
        dark = pill_alpha(true),
    ));
    gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

fn size_attrs(points: u32) -> gtk::pango::AttrList {
    let list = gtk::pango::AttrList::new();
    list.insert(gtk::pango::AttrSize::new(points as i32 * gtk::pango::SCALE));
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pill_alpha_is_8_percent_light_and_12_percent_dark() {
        assert!((pill_alpha(false) - 0.08).abs() < f32::EPSILON);
        assert!((pill_alpha(true) - 0.12).abs() < f32::EPSILON);
    }

    #[test]
    fn count_pill_shows_only_above_zero() {
        assert!(!show_count_pill(0));
        assert!(show_count_pill(1));
    }

    #[test]
    fn action_mode_title_is_the_selected_count() {
        assert_eq!(title_text(true, 3, "Library"), "3");
        assert_eq!(title_text(false, 3, "Library"), "Library");
    }

    #[test]
    fn nav_icon_is_close_in_action_mode_and_up_only_with_a_parent() {
        assert_eq!(nav_icon(true, true), NavIcon::Close);
        assert_eq!(nav_icon(true, false), NavIcon::Close);
        assert_eq!(nav_icon(false, true), NavIcon::Up);
        assert_eq!(nav_icon(false, false), NavIcon::None);
    }

    #[test]
    fn escape_cancels_action_mode_and_otherwise_passes() {
        assert_eq!(escape_action(true), EscapeAction::CancelActionMode);
        assert_eq!(escape_action(false), EscapeAction::Pass);
    }

    #[test]
    fn overflow_button_needs_an_overflow_item() {
        assert!(!overflow_visible(0));
        assert!(overflow_visible(2));
    }
}
