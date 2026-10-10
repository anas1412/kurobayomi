//! Library rows this module owns: the default category lists (task T-47a-020) and the chapter and episode swipe
//! actions (task T-47a-028).
use std::rc::Rc;

use kuro_domain::kind::EntryKind;
use kuro_domain::library::swipe::{chapter_swipe_label, episode_swipe_label};
use kuro_prefs::library::{ChapterSwipeAction, EpisodeSwipeAction};
use kuro_prefs::prefs::Prefs;
use kuro_prefs::pref::Pref;

use crate::pref_model::bind;
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::category::Category;
use crate::settings_ui::rows::{enum_choice, group, int_choice, item, multi_choice, switch, switch_with, text};

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

/// The per-category sort switch. Turning it off first resets every manga category's flags to the global sort (Kotlin
/// `ResetMangaCategoryFlags`, the domain `reset_category_flags`); a failed reset refuses the change with its message.
/// Turning it on only stores the value.
pub fn categorized_row(prefs: &Prefs) -> PreferenceItem {
    let veto = bind::veto(|on: bool| {
        let app = crate::data::app();
        async move {
            if on {
                return Ok(true);
            }
            app.categories(EntryKind::Manga).reset_category_flags().await.map(|()| true).map_err(|err| err.to_string())
        }
    });
    switch_with("Per-category settings for sort", None, &prefs.library.categorized_display_settings, true, veto)
}

/// The page of the Categories screen a kind opens on (`kind_tabs` names its pages `anime` and `manga`).
pub fn categories_tab(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Anime => "anime",
        EntryKind::Manga => "manga",
    }
}

/// `Edit anime categories`-style row: the subtitle is the count of user categories, a tap runs `open`.
pub fn edit_categories_row(title: &str, user_categories: usize, open: Rc<dyn Fn()>) -> PreferenceItem {
    let mut row = text(title, Some(&category_count(user_categories)), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(open);
    }
    row
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

/// The chapter mark-duplicate options: marked as read after reading a chapter, or after fetching a new one.
pub const DUPLICATE_READ_CHAPTER: [(&str, &str); 2] = [("existing", "After reading a chapter"), ("new", "After fetching new chapter")];
/// The episode mark-duplicate options: marked as seen after watching an episode, or after fetching a new one.
pub const DUPLICATE_SEEN_EPISODE: [(&str, &str); 2] =
    [("existing_episode", "After watching an episode"), ("new_episode", "After fetching new episode")];

/// The chapter swipe rows, titled as `spec/settings.json` titles them.
pub fn chapter_swipe_group(prefs: &Prefs) -> Preference {
    let l = &prefs.library;
    group(
        "Chapter behavior",
        vec![
            enum_choice("Chapter on swipe to left", &l.swipe_left_chapter, &chapter_swipe_options(), true),
            enum_choice("Chapter on swipe to right", &l.swipe_right_chapter, &chapter_swipe_options(), true),
            multi_choice(
                "Mark duplicate read chapter as read",
                &l.mark_duplicate_read_chapter_as_read,
                &DUPLICATE_READ_CHAPTER,
                true,
            ),
        ],
    )
}

/// The episode swipe rows, titled as `spec/settings.json` titles them.
pub fn episode_swipe_group(prefs: &Prefs) -> Preference {
    let l = &prefs.library;
    group(
        "Episode behavior",
        vec![
            enum_choice("Episode on swipe to left", &l.swipe_left_episode, &episode_swipe_options(), true),
            enum_choice("Episode on swipe to right", &l.swipe_right_episode, &episode_swipe_options(), true),
            multi_choice(
                "Mark duplicate seen episode as seen",
                &l.mark_duplicate_seen_episode_as_seen,
                &DUPLICATE_SEEN_EPISODE,
                true,
            ),
        ],
    )
}

/// The Season group: whether refresh and library update also refresh the seasons with episodes (task T-47a-027).
pub fn season_group(prefs: &Prefs) -> Preference {
    group(
        "Season",
        vec![
            switch("Update seasons with episodes on refresh", &prefs.library.update_season_on_refresh, true),
            switch("Update seasons with episodes on library update", &prefs.library.update_season_on_library_update, true),
        ],
    )
}

/// The swipe groups as top-level entries of the library page.
pub fn swipe_rows(prefs: &Prefs) -> Vec<Preference> {
    vec![chapter_swipe_group(prefs), episode_swipe_group(prefs)]
}

/// The rows of the library page this module builds: the Categories group (edit rows that open the Categories screen,
/// default category lists) and the swipe groups. `open` shows the Categories screen of a kind.
pub fn page_rows(prefs: &Prefs, anime: &[Category], manga: &[Category], open: Rc<dyn Fn(EntryKind)>) -> Vec<Preference> {
    let edit = |title: &str, kind: EntryKind, count: usize| {
        let open = Rc::clone(&open);
        edit_categories_row(title, count, Rc::new(move || open(kind)))
    };
    let l = &prefs.library;
    let mut tree = vec![group(
        "Categories",
        vec![
            edit("Edit anime categories", EntryKind::Anime, anime.len()),
            default_category_row("Default anime category", &l.default_anime_category, anime),
            edit("Edit manga categories", EntryKind::Manga, manga.len()),
            default_category_row("Default manga category", &l.default_manga_category, manga),
            categorized_row(prefs),
            switch("Hide hidden categories from categories and storage screen", &l.hide_hidden_categories_settings, true),
        ],
    )];
    tree.push(season_group(prefs));
    tree.extend(swipe_rows(prefs));
    tree
}

/// Default anime category row, from the anime category list.
#[allow(dead_code)] // `page_rows` builds both default rows itself
pub fn default_anime_category(prefs: &Prefs, categories: &[Category]) -> Preference {
    item(default_category_row("Default anime category", &prefs.library.default_anime_category, categories))
}

/// Default manga category row, from the manga category list.
#[allow(dead_code)] // `page_rows` builds both default rows itself
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

    /// Runs a veto future that resolves at once (the writes here never wait).
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("the veto is not immediately ready"),
        }
    }

    #[test]
    fn the_mark_duplicate_rows_store_their_selection_under_the_two_keys() {
        let prefs = Prefs::in_memory();
        let rows = swipe_rows(&prefs);
        let mut found = 0;
        for entry in &rows {
            let Preference::Group(g) = entry else { panic!("group") };
            for item in &g.items {
                let PreferenceItem::MultiSelect { entries, veto, pref, .. } = item else { continue };
                found += 1;
                assert_eq!(entries.len(), 2);
                let picked: std::collections::BTreeSet<String> = ["new".to_owned()].into();
                let picked_episode: std::collections::BTreeSet<String> = ["new_episode".to_owned()].into();
                let value = if pref.key().contains("chapter") { picked } else { picked_episode };
                assert_eq!(block_on(veto(value.clone())), Ok(true));
                assert_eq!(pref.get(), value);
            }
        }
        assert_eq!(found, 2);
        assert!(prefs.library.mark_duplicate_read_chapter_as_read.get().contains("new"));
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
    fn edit_categories_row_subtitle_is_the_count_and_a_tap_opens_the_screen() {
        let opened = Rc::new(std::cell::Cell::new(false));
        let seen = Rc::clone(&opened);
        let PreferenceItem::Text { common, on_click, .. } =
            edit_categories_row("Edit manga categories", 1, Rc::new(move || seen.set(true)))
        else {
            panic!("text row")
        };
        assert_eq!(common.subtitle.as_deref(), Some("1 category"));
        (on_click.expect("tapping the row does something"))();
        assert!(opened.get());
    }

    #[test]
    fn the_page_rows_open_the_tab_of_their_kind() {
        let prefs = Prefs::in_memory();
        let opened = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = Rc::clone(&opened);
        let tree = page_rows(&prefs, &categories(), &[], Rc::new(move |kind| sink.borrow_mut().push(categories_tab(kind))));
        let Preference::Group(g) = &tree[0] else { panic!("categories group") };
        assert_eq!(g.title, "Categories");
        let titles: Vec<&str> = g.items.iter().map(|i| i.title()).collect();
        assert_eq!(
            titles,
            [
                "Edit anime categories",
                "Default anime category",
                "Edit manga categories",
                "Default manga category",
                "Per-category settings for sort",
                "Hide hidden categories from categories and storage screen",
            ]
        );
        for row in [&g.items[2], &g.items[0]] {
            let PreferenceItem::Text { on_click, .. } = row else { panic!("text row") };
            (on_click.as_ref().expect("a tap handler"))();
        }
        assert_eq!(*opened.borrow(), ["manga", "anime"]);
        let PreferenceItem::Text { common, .. } = &g.items[0] else { panic!("text row") };
        assert_eq!(common.subtitle.as_deref(), Some("2 categories"));
        assert_eq!(tree.len(), 4, "Categories, Season, Chapter behavior, Episode behavior");
    }

    #[test]
    fn the_categorized_switch_is_the_per_category_sort_setting_and_stores_its_value() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::Switch { common, pref, .. } = categorized_row(&prefs) else { panic!("switch") };
        assert_eq!(common.title, "Per-category settings for sort");
        assert!(!pref.get());
        assert_eq!(pref.key(), prefs.library.categorized_display_settings.key());
    }

    #[test]
    fn the_season_group_holds_the_two_update_switches_under_the_season_heading() {
        let prefs = Prefs::in_memory();
        let Preference::Group(g) = season_group(&prefs) else { panic!("group") };
        assert_eq!(g.title, "Season");
        let titles: Vec<&str> = g.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles, ["Update seasons with episodes on refresh", "Update seasons with episodes on library update"]);
        let PreferenceItem::Switch { pref, .. } = &g.items[1] else { panic!("switch") };
        assert_eq!(pref.key(), prefs.library.update_season_on_library_update.key());
    }
}
