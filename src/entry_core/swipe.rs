//! Chapter row swipe actions (T-08a-029). On a touch screen a horizontal drag of at least `SWIPE_THRESHOLD_PX` runs the
//! action stored for that side; on the desktop the same actions are items of the row's context menu. The preferences
//! and the labels are `kuro-prefs` and `kuro_domain::library::swipe`; this file picks the action and the label.

use kuro_domain::library::swipe::{SWIPE_THRESHOLD_PX, chapter_swipe_label};
use kuro_prefs::library::{ChapterSwipeAction, LibraryPrefs};

/// The side a row is swiped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// The side of a finished drag of `dx` and `dy` pixels, or `None` when it is shorter than the threshold or more
/// vertical than horizontal (a scroll).
pub fn side_of_drag(dx: f64, dy: f64) -> Option<Side> {
    (dx.abs() >= SWIPE_THRESHOLD_PX && dx.abs() > dy.abs()).then_some(if dx < 0.0 { Side::Left } else { Side::Right })
}

/// The stored action of a side. The key names are inverted in Aniyomi, `kuro-prefs` documents which is which.
pub fn action_for(prefs: &LibraryPrefs, side: Side) -> ChapterSwipeAction {
    match side {
        Side::Left => prefs.swipe_left_chapter.get(),
        Side::Right => prefs.swipe_right_chapter.get(),
    }
}

/// Whether the manga page can run `action`. Download has no download call on this page yet, and Disabled is nothing.
pub fn runs_here(action: ChapterSwipeAction) -> bool {
    matches!(action, ChapterSwipeAction::ToggleRead | ChapterSwipeAction::ToggleBookmark)
}

/// The context menu: the action of each side that runs here, once each, with its settings label.
pub fn menu(prefs: &LibraryPrefs) -> Vec<(ChapterSwipeAction, &'static str)> {
    let mut items: Vec<ChapterSwipeAction> = Vec::new();
    for side in [Side::Left, Side::Right] {
        let action = action_for(prefs, side);
        if runs_here(action) && !items.contains(&action) {
            items.push(action);
        }
    }
    items.into_iter().map(|a| (a, chapter_swipe_label(a))).collect()
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;

    #[test]
    fn a_drag_swipes_from_56_pixels_when_it_is_mostly_horizontal() {
        assert_eq!(side_of_drag(-56.0, 0.0), Some(Side::Left));
        assert_eq!(side_of_drag(56.0, 10.0), Some(Side::Right));
        assert_eq!(side_of_drag(55.9, 0.0), None);
        assert_eq!(side_of_drag(60.0, 80.0), None, "a vertical drag is a scroll");
    }

    #[test]
    fn each_side_reads_its_own_preference() {
        let prefs = LibraryPrefs::new(PrefStore::in_memory());
        assert_eq!(action_for(&prefs, Side::Left), ChapterSwipeAction::ToggleRead);
        assert_eq!(action_for(&prefs, Side::Right), ChapterSwipeAction::ToggleBookmark);
    }

    #[test]
    fn the_menu_lists_the_runnable_actions_once_with_the_domain_labels() {
        let prefs = LibraryPrefs::new(PrefStore::in_memory());
        let labels: Vec<_> = menu(&prefs).into_iter().map(|(_, l)| l).collect();
        assert_eq!(labels, ["Mark as read", "Bookmark chapter"]);
        prefs.swipe_right_chapter.set(ChapterSwipeAction::ToggleRead).unwrap();
        assert_eq!(menu(&prefs).len(), 1, "the same action on both sides is one item");
        prefs.swipe_left_chapter.set(ChapterSwipeAction::Download).unwrap();
        prefs.swipe_right_chapter.set(ChapterSwipeAction::Disabled).unwrap();
        assert!(menu(&prefs).is_empty());
    }
}
