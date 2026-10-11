//! Library rows this module owns: the default category lists (task T-47a-020) and the chapter and episode behavior
//! groups (task T-47a-028).
use std::rc::Rc;

use kuro_domain::kind::EntryKind;
use kuro_prefs::prefs::Prefs;
use kuro_prefs::pref::Pref;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::category::Category;
use crate::settings_ui::rows::{group, int_choice, multi_choice, switch, text};

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

/// The page of the Categories screen a kind opens on (`screens::kind_switch` names its pages `anime` and `manga`).
pub fn categories_tab(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Anime => "anime",
        EntryKind::Manga => "manga",
    }
}

/// `Edit anime categories`-style row: the subtitle is the count of user categories, a tap runs `open`.
pub fn edit_categories_row(title: &str, user_categories: usize, open: Rc<dyn Fn()>) -> PreferenceItem {
    let mut row = text(title, Some(&category_count(user_categories)), true);
    if let PreferenceItem::Text { common, on_click } = &mut row {
        *on_click = Some(open);
        common.link = true;
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

/// The chapter mark-duplicate options: marked as read after reading a chapter, or after fetching a new one.
pub const DUPLICATE_READ_CHAPTER: [(&str, &str); 2] = [("existing", "After reading a chapter"), ("new", "After fetching new chapter")];
/// The episode mark-duplicate options: marked as seen after watching an episode, or after fetching a new one.
pub const DUPLICATE_SEEN_EPISODE: [(&str, &str); 2] =
    [("existing_episode", "After watching an episode"), ("new_episode", "After fetching new episode")];

/// The chapter behavior group, titled as `spec/settings.json` titles it.
pub fn chapter_behavior_group(prefs: &Prefs) -> Preference {
    let l = &prefs.library;
    group(
        "Chapter behavior",
        vec![
            multi_choice(
                "Mark duplicate read chapter as read",
                &l.mark_duplicate_read_chapter_as_read,
                &DUPLICATE_READ_CHAPTER,
                true,
            ),
        ],
    )
}

/// The episode behavior group, titled as `spec/settings.json` titles it.
pub fn episode_behavior_group(prefs: &Prefs) -> Preference {
    let l = &prefs.library;
    group(
        "Episode behavior",
        vec![
            multi_choice(
                "Mark duplicate seen episode as seen",
                &l.mark_duplicate_seen_episode_as_seen,
                &DUPLICATE_SEEN_EPISODE,
                true,
            ),
        ],
    )
}

/// The chapter and episode behavior groups as top-level entries of the library page.
pub fn behavior_rows(prefs: &Prefs) -> Vec<Preference> {
    vec![chapter_behavior_group(prefs), episode_behavior_group(prefs)]
}

/// The rows of the library page this module builds: the Categories group (edit rows that open the Categories screen,
/// default category lists) and the behavior groups. `open` shows the Categories screen of a kind.
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
            switch("Hide hidden categories from categories and storage screen", &l.hide_hidden_categories_settings, true),
        ],
    )];
    tree.push(crate::settings_ui::library_update::global_update_group(prefs, anime, manga));
    tree.extend(behavior_rows(prefs));
    tree.push(crate::settings_ui::rows::item(crate::settings_ui::notifications::notify_row(&prefs.base)));
    tree
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
        let rows = behavior_rows(&prefs);
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
        assert_eq!(tree.len(), 5, "Categories, Global update, Chapter behavior, Episode behavior, the notification switch");
    }
}
