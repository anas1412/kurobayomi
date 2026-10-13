//! App bar rules (Kotlin `AppBar`, T-36a-037 and T-36a-038): title in action mode, navigation icon, count pill and the
//! overflow button. The GTK header is built by `header_title` and `nav_button`.


#[cfg(test)]
/// The count pill shows only for a positive count.
pub fn show_count_pill(count: u32) -> bool {
    count > 0
}

#[cfg(test)]
/// The title text. In action mode the title is the selected count.
pub fn title_text(action_mode: bool, selected: u32, title: &str) -> String {
    if action_mode { selected.to_string() } else { title.to_string() }
}

#[cfg(test)]
/// The navigation slot: close (cancel) in action mode, up when a parent page exists, nothing otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavIcon {
    Close,
    Up,
    None,
}

#[cfg(test)]
pub fn nav_icon(action_mode: bool, has_parent: bool) -> NavIcon {
    if action_mode {
        NavIcon::Close
    } else if has_parent {
        NavIcon::Up
    } else {
        NavIcon::None
    }
}

#[cfg(test)]
/// The overflow button shows only when there is at least one overflow item.
pub fn overflow_visible(overflow_items: usize) -> bool {
    overflow_items > 0
}

#[cfg(test)]
/// What Escape does in the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeAction {
    /// Leave action mode and clear the selection.
    CancelActionMode,
    /// Not the bar's key: it goes on to the next handler (back, close search).
    Pass,
}

#[cfg(test)]
pub fn escape_action(action_mode: bool) -> EscapeAction {
    if action_mode { EscapeAction::CancelActionMode } else { EscapeAction::Pass }
}

#[cfg(test)]
mod tests {
    use super::*;

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
