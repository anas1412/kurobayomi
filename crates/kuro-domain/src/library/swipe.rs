//! Swipe actions and duplicate-marking choices (T-08a-029). The enums and their stored names are in `kuro-prefs`; this
//! file adds the settings labels and the swipe threshold. Desktop shows the same actions as row context-menu items.

pub use kuro_prefs::library::{
    ChapterSwipeAction, EpisodeSwipeAction, MARK_DUPLICATE_CHAPTER_READ_EXISTING, MARK_DUPLICATE_CHAPTER_READ_NEW,
    MARK_DUPLICATE_EPISODE_SEEN_EXISTING, MARK_DUPLICATE_EPISODE_SEEN_NEW,
};

/// Horizontal drag distance, in pixels, that starts a swipe action on a touch screen.
pub const SWIPE_THRESHOLD_PX: f64 = 56.0;

/// Settings label of a chapter swipe action.
pub fn chapter_swipe_label(action: ChapterSwipeAction) -> &'static str {
    match action {
        ChapterSwipeAction::ToggleRead => "Mark as read",
        ChapterSwipeAction::ToggleBookmark => "Bookmark chapter",
        ChapterSwipeAction::Download => "Download",
        ChapterSwipeAction::Disabled => "Disabled",
    }
}

/// Settings label of an episode swipe action.
pub fn episode_swipe_label(action: EpisodeSwipeAction) -> &'static str {
    match action {
        EpisodeSwipeAction::ToggleSeen => "Mark as watched",
        EpisodeSwipeAction::ToggleBookmark => "Bookmark episode",
        EpisodeSwipeAction::ToggleFillermark => "Fillermark episode",
        EpisodeSwipeAction::Download => "Download",
        EpisodeSwipeAction::Disabled => "Disabled",
    }
}

/// The four duplicate-marking choices, as stored value and settings label. Chapters and episodes each have a
/// `existing` and a `new` choice.
pub const DUPLICATE_CHOICES: [(&str, &str); 4] = [
    (MARK_DUPLICATE_CHAPTER_READ_EXISTING, "After reading a chapter"),
    (MARK_DUPLICATE_CHAPTER_READ_NEW, "After fetching new chapter"),
    (MARK_DUPLICATE_EPISODE_SEEN_EXISTING, "After watching an episode"),
    (MARK_DUPLICATE_EPISODE_SEEN_NEW, "After fetching new episode"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_labels_match_the_settings_list() {
        let labels: Vec<&str> = [
            ChapterSwipeAction::Disabled,
            ChapterSwipeAction::ToggleBookmark,
            ChapterSwipeAction::ToggleRead,
            ChapterSwipeAction::Download,
        ]
        .map(chapter_swipe_label)
        .to_vec();
        assert_eq!(labels, ["Disabled", "Bookmark chapter", "Mark as read", "Download"]);
    }

    #[test]
    fn episode_labels_match_the_settings_list() {
        let labels: Vec<&str> = [
            EpisodeSwipeAction::Disabled,
            EpisodeSwipeAction::ToggleBookmark,
            EpisodeSwipeAction::ToggleFillermark,
            EpisodeSwipeAction::ToggleSeen,
            EpisodeSwipeAction::Download,
        ]
        .map(episode_swipe_label)
        .to_vec();
        assert_eq!(labels, ["Disabled", "Bookmark episode", "Fillermark episode", "Mark as watched", "Download"]);
    }

    #[test]
    fn duplicate_choices_keep_their_stored_values_and_labels() {
        assert_eq!(
            DUPLICATE_CHOICES.map(|(value, _)| value),
            ["existing", "new", "existing_episode", "new_episode"]
        );
        assert_eq!(DUPLICATE_CHOICES[3].1, "After fetching new episode");
    }

    #[test]
    fn the_swipe_threshold_is_56_pixels() {
        assert_eq!(SWIPE_THRESHOLD_PX, 56.0);
    }

    #[test]
    fn the_stored_names_are_the_constant_names() {
        assert_eq!(ChapterSwipeAction::ToggleRead.name(), "ToggleRead");
        assert_eq!(EpisodeSwipeAction::from_name("ToggleFillermark"), Some(EpisodeSwipeAction::ToggleFillermark));
    }
}
