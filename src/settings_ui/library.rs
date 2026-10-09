//! Library rows this module owns: the default category lists (task T-47a-020) and the chapter and episode swipe
//! actions (task T-47a-028).
use kuro_domain::library::swipe::{chapter_swipe_label, episode_swipe_label};
use kuro_prefs::library::{ChapterSwipeAction, EpisodeSwipeAction};
use kuro_prefs::prefs::Prefs;
use kuro_prefs::pref::Pref;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::category::Category;
use crate::settings_ui::rows::{enum_choice, group, int_choice, item, text};

/// Stored value meaning "ask every time" for the default category.
pub const ALWAYS_ASK: i32 = -1;

/// "0 categories", "1 category", "N categories".
pub fn category_count(count: usize) -> String {
    match count {
        1 => "1 category".to_owned(),
        n => format!("{n} categories"),
    }
}

/// The stored default category, or [`ALWAYS_ASK`] when it no longer names a category.
pub fn normalize_default(value: i32, ids: &[i32]) -> i32 {
    if value == ALWAYS_ASK || ids.contains(&value) { value } else { ALWAYS_ASK }
}

/// `Edit anime categories`-style row: the subtitle is the count of user categories.
pub fn edit_categories_row(title: &str, user_categories: usize) -> PreferenceItem {
    text(title, Some(&category_count(user_categories)), true)
}

/// A "Default ... category" list: "Always ask" first, then each category in list order. A stored id that was
/// deleted is written back as [`ALWAYS_ASK`] on this read.
pub fn default_category_row(title: &str, pref: &Pref<i32>, categories: &[Category]) -> PreferenceItem {
    let mut ordered: Vec<&Category> = categories.iter().collect();
    ordered.sort_by_key(|c| c.order);
    let ids: Vec<i32> = ordered.iter().map(|c| c.id).collect();
    let stored = pref.get();
    let normalized = normalize_default(stored, &ids);
    if normalized != stored {
        let _ = pref.set(normalized);
    }
    let mut options: Vec<(i32, &str)> = vec![(ALWAYS_ASK, "Always ask")];
    options.extend(ordered.iter().map(|c| (c.id, c.name.as_str())));
    int_choice(title, pref, &options, true)
}

/// Chapter swipe actions in menu order, labelled by `kuro_domain::library::swipe`. The row titled "Swipe to left" reads
/// `pref_chapter_swipe_end_action` (see `kuro-prefs`).
pub fn chapter_swipe_options() -> [(ChapterSwipeAction, &'static str); 4] {
    use ChapterSwipeAction::*;
    [Disabled, ToggleBookmark, ToggleRead, Download].map(|action| (action, chapter_swipe_label(action)))
}

/// Episode swipe actions in menu order, labelled by `kuro_domain::library::swipe`.
pub fn episode_swipe_options() -> [(EpisodeSwipeAction, &'static str); 5] {
    use EpisodeSwipeAction::*;
    [Disabled, ToggleBookmark, ToggleFillermark, ToggleSeen, Download].map(|action| (action, episode_swipe_label(action)))
}

/// The chapter swipe group (left and right).
pub fn chapter_swipe_group(prefs: &Prefs) -> Preference {
    let l = &prefs.library;
    group(
        "Chapter",
        vec![
            enum_choice("Swipe to left", &l.swipe_left_chapter, &chapter_swipe_options(), true),
            enum_choice("Swipe to right", &l.swipe_right_chapter, &chapter_swipe_options(), true),
        ],
    )
}

/// The episode swipe group (left and right).
pub fn episode_swipe_group(prefs: &Prefs) -> Preference {
    let l = &prefs.library;
    group(
        "Episode",
        vec![
            enum_choice("Swipe to left", &l.swipe_left_episode, &episode_swipe_options(), true),
            enum_choice("Swipe to right", &l.swipe_right_episode, &episode_swipe_options(), true),
        ],
    )
}

/// The swipe groups as top-level entries of the library page.
pub fn swipe_rows(prefs: &Prefs) -> Vec<Preference> {
    vec![chapter_swipe_group(prefs), episode_swipe_group(prefs)]
}

/// Default anime category row, from the anime category list.
pub fn default_anime_category(prefs: &Prefs, categories: &[Category]) -> Preference {
    item(default_category_row("Default anime category", &prefs.library.default_anime_category, categories))
}

/// Default manga category row, from the manga category list.
pub fn default_manga_category(prefs: &Prefs, categories: &[Category]) -> Preference {
    item(default_category_row("Default manga category", &prefs.library.default_manga_category, categories))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn categories() -> Vec<Category> {
        vec![
            Category { id: 7, name: "Later".into(), order: 2 },
            Category { id: 3, name: "Reading".into(), order: 1 },
        ]
    }

    #[test]
    fn category_count_uses_singular_for_one() {
        assert_eq!(category_count(0), "0 categories");
        assert_eq!(category_count(1), "1 category");
        assert_eq!(category_count(4), "4 categories");
    }

    #[test]
    fn a_deleted_default_resets_to_always_ask() {
        assert_eq!(normalize_default(ALWAYS_ASK, &[3]), ALWAYS_ASK);
        assert_eq!(normalize_default(3, &[3, 7]), 3);
        assert_eq!(normalize_default(9, &[3, 7]), ALWAYS_ASK);
    }

    #[test]
    fn default_list_offers_always_ask_then_categories_in_order() {
        let prefs = Prefs::in_memory();
        let Preference::Item(PreferenceItem::BasicList { entries, value, .. }) =
            default_anime_category(&prefs, &categories())
        else {
            panic!("list")
        };
        let labels: Vec<&str> = entries.iter().map(|(_, l)| l.as_str()).collect();
        assert_eq!(labels, ["Always ask", "Reading", "Later"]);
        assert_eq!(value, "-1");
    }

    #[test]
    fn deleted_default_is_written_back_on_read() {
        let prefs = Prefs::in_memory();
        prefs.library.default_manga_category.set(42).unwrap();
        let _ = default_manga_category(&prefs, &categories());
        assert_eq!(prefs.library.default_manga_category.get(), ALWAYS_ASK);
    }

    #[test]
    fn chapter_swipe_rows_read_the_inverted_keys() {
        let prefs = Prefs::in_memory();
        assert_eq!(prefs.library.swipe_left_chapter.get(), ChapterSwipeAction::ToggleRead);
        assert_eq!(prefs.library.swipe_right_chapter.get(), ChapterSwipeAction::ToggleBookmark);
        let rows = swipe_rows(&prefs);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn episode_defaults_are_seen_left_and_bookmark_right() {
        let prefs = Prefs::in_memory();
        assert_eq!(prefs.library.swipe_left_episode.get(), EpisodeSwipeAction::ToggleSeen);
        assert_eq!(prefs.library.swipe_right_episode.get(), EpisodeSwipeAction::ToggleBookmark);
        let labels: Vec<&str> = episode_swipe_options().iter().map(|(_, l)| *l).collect();
        assert_eq!(labels, ["Disabled", "Bookmark episode", "Fillermark episode", "Mark as watched", "Download"]);
    }

    #[test]
    fn edit_categories_row_subtitle_is_the_count() {
        let PreferenceItem::Text { common, .. } = edit_categories_row("Edit manga categories", 1) else {
            panic!("text row")
        };
        assert_eq!(common.subtitle.as_deref(), Some("1 category"));
    }
}
