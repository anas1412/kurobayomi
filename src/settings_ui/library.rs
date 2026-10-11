//! Library rows this module owns: the default category lists (task T-47a-020) and the chapter and episode behavior
//! groups (task T-47a-028).
use std::rc::Rc;

use kuro_domain::category::service::{CategoryService, categorized_display_changed};
use kuro_domain::kind::EntryKind;
use kuro_prefs::prefs::Prefs;
use kuro_prefs::pref::Pref;

use crate::pref_model::bind;
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::category::Category;
use crate::settings_ui::rows::{group, int_choice, multi_choice, switch, switch_with, text};

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

/// The per-category sort switch. Turning it off first resets the flags of every manga and every anime category to the
/// global sort (Kotlin `ResetMangaCategoryFlags` and `ResetAnimeCategoryFlags`, the domain
/// `categorized_display_changed`); a failed reset refuses the change with its message. Turning it on only stores the
/// value.
///
/// Decision (T-37a-040, as in Kotlin): the per-category sorts are not kept while the switch is off, so turning it back
/// on starts every category from the global sort; nothing restores the earlier per-category choices.
pub fn categorized_row(prefs: &Prefs) -> PreferenceItem {
    categorized_row_over(prefs, crate::data::app, crate::data::App::categories)
}

/// [`categorized_row`] over the app data `app` hands out and the way `categories` picks a kind's service from it.
fn categorized_row_over<A: 'static>(
    prefs: &Prefs,
    app: impl Fn() -> Rc<A> + 'static,
    categories: fn(&A, EntryKind) -> &CategoryService,
) -> PreferenceItem {
    let veto = bind::veto(move |on: bool| {
        let app = app();
        async move {
            categorized_display_changed(categories(&app, EntryKind::Manga), categories(&app, EntryKind::Anime), on)
                .await
                .map(|()| true)
                .map_err(|err| err.to_string())
        }
    });
    switch_with("Per-category settings for sort", None, &prefs.library.categorized_display_settings, true, veto)
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
            categorized_row(prefs),
            switch("Hide hidden categories from categories and storage screen", &l.hide_hidden_categories_settings, true),
        ],
    )];
    tree.push(crate::settings_ui::library_update::global_update_group(prefs, anime, manga));
    tree.push(season_group(prefs));
    tree.extend(behavior_rows(prefs));
    tree
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
        assert_eq!(tree.len(), 5, "Categories, Global update, Season, Chapter behavior, Episode behavior");
    }

    /// Two real category services over fresh databases, standing in for the app data.
    struct Fake {
        manga_db: Arc<kuro_db::handle::Db>,
        manga: CategoryService,
        anime: CategoryService,
        library: Arc<kuro_prefs::library::LibraryPrefs>,
        _dir: tempfile::TempDir,
    }

    impl Fake {
        fn new() -> Rc<Self> {
            use kuro_db::handle::Db;
            let dir = tempfile::tempdir().unwrap();
            let (mp, ap) = (dir.path().join("manga.db"), dir.path().join("anime.db"));
            let manga_db = Arc::new(Db::new(&mp, kuro_db::schema_manga::open(&mp).unwrap()).unwrap());
            let anime_db = Arc::new(Db::new(&ap, kuro_db::schema_anime::open(&ap).unwrap()).unwrap());
            let library = Arc::new(kuro_prefs::library::LibraryPrefs::new(kuro_prefs::store::PrefStore::in_memory()));
            Rc::new(Self {
                manga: CategoryService::new(Arc::clone(&manga_db), EntryKind::Manga, Arc::clone(&library)),
                anime: CategoryService::new(anime_db, EntryKind::Anime, Arc::clone(&library)),
                manga_db,
                library,
                _dir: dir,
            })
        }

        fn categories(&self, kind: EntryKind) -> &CategoryService {
            match kind {
                EntryKind::Manga => &self.manga,
                EntryKind::Anime => &self.anime,
            }
        }
    }

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
    }

    fn flags(svc: &CategoryService) -> Vec<i64> {
        svc.repo().all().unwrap().iter().map(|c| c.flags).collect()
    }

    // T-08a-012, T-37a-040: the row's own veto, run with false, resets the flags of both kinds to the global sort.
    #[test]
    fn turning_the_categorized_switch_off_resets_both_kinds_to_the_global_sort() {
        use kuro_domain::library::sort::{Direction, LibrarySort, SortType};
        use kuro_prefs::library::SortSetting;
        let fake = Fake::new();
        run(async {
            fake.manga.create("M").await.unwrap();
            fake.anime.create("A").await.unwrap();
        });
        fake.library.manga_sorting_mode.set(SortSetting("DATE_ADDED,DESCENDING".into()));
        fake.library.anime_sorting_mode.set(SortSetting("LAST_SEEN,ASCENDING".into()));
        let manga_sort = LibrarySort { ty: SortType::DateAdded, dir: Direction::Descending }.flag();
        let anime_sort = LibrarySort { ty: SortType::LastActivity, dir: Direction::Ascending }.flag();
        assert_ne!(flags(&fake.manga), vec![manga_sort; 2], "the categories start on another sort");

        let prefs = Prefs::in_memory();
        let shared = Rc::clone(&fake);
        let PreferenceItem::Switch { veto, .. } = categorized_row_over(&prefs, move || Rc::clone(&shared), Fake::categories) else {
            panic!("switch")
        };
        assert_eq!(run(veto(false)), Ok(true));
        assert_eq!(flags(&fake.manga), vec![manga_sort; 2], "system category and M");
        assert_eq!(flags(&fake.anime), vec![anime_sort; 2], "system category and A");
    }

    #[test]
    fn turning_the_categorized_switch_on_resets_nothing() {
        let fake = Fake::new();
        run(async { fake.manga.create("M").await.unwrap() });
        let before = flags(&fake.manga);
        let prefs = Prefs::in_memory();
        let shared = Rc::clone(&fake);
        let PreferenceItem::Switch { veto, .. } = categorized_row_over(&prefs, move || Rc::clone(&shared), Fake::categories) else {
            panic!("switch")
        };
        assert_eq!(run(veto(true)), Ok(true));
        assert_eq!(flags(&fake.manga), before);
    }

    // A failed reset refuses the change: the veto reports the error, `bind::write` then leaves the pref alone.
    #[test]
    fn a_failed_reset_refuses_the_change_and_keeps_the_pref() {
        let fake = Fake::new();
        run(async { fake.manga.create("M").await.unwrap() });
        fake.manga_db.write(|tx| {
            tx.execute_batch("CREATE TRIGGER refuse BEFORE UPDATE ON categories BEGIN SELECT RAISE(ABORT, 'refused'); END;")?;
            Ok(())
        })
        .unwrap();
        let prefs = Prefs::in_memory();
        prefs.library.categorized_display_settings.set(true).unwrap();
        let shared = Rc::clone(&fake);
        let PreferenceItem::Switch { veto, pref, .. } = categorized_row_over(&prefs, move || Rc::clone(&shared), Fake::categories) else {
            panic!("switch")
        };
        let result = run(veto(false));
        let message = result.clone().expect_err("the reset failed, so the change is refused");
        assert!(message.contains("refused"), "{message}");
        assert_eq!(bind::decide(result, true), Err(message));
        assert!(pref.get(), "the pref is only written after an accepted veto");
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
