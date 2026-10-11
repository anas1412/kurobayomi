//! Browse rows this module binds to the typed preferences: the two "hide entries already in library" switches of the
//! Sources group (task T-47a-060), the extension repo rows with their counts (task T-47a-062) and the NSFW (18+) group
//! (task T-47a-061). Each switch is independent; the Browse lists and global search read them.
use std::rc::Rc;

use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::bind::{self, Veto};
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::auth;
use crate::settings_ui::rows::{group, info, switch, switch_with, text};

pub const NSFW_SUBTITLE: &str = "Requires app restart to take effect";
pub const NSFW_INFO: &str = "This does not prevent unofficial or potentially incorrectly flagged extensions from surfacing NSFW (18+) content within the app.";
pub const NSFW_GATE_TITLE: &str = "NSFW (18+) sources";
pub const NSFW_REFUSED: &str = "Authentication is required to change this setting";
pub const ANIME_REPOS_TITLE: &str = "Anime extension repos";
pub const MANGA_REPOS_TITLE: &str = "Manga extension repos";
pub const ANIME_REPOS_SCREEN: &str = "settings_anime_extension_stores";
pub const MANGA_REPOS_SCREEN: &str = "settings_manga_extension_repos";

/// The number of stores (anime) or repos (manga) the two repo rows show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RepoCounts {
    pub anime: i64,
    pub manga: i64,
}

/// `0 stores`, `1 store`, `N stores` (the `num_repos` plural).
pub fn store_count_label(count: i64) -> String {
    if count == 1 { "1 store".to_owned() } else { format!("{count} stores") }
}

/// The Sources group, the NSFW (18+) group. `counts` are the stores and repos the two repo rows show.
pub fn page_rows(prefs: &Prefs, counts: RepoCounts) -> Vec<Preference> {
    vec![
        group(
            "Sources",
            vec![
                switch("Hide anime entries already in library", &prefs.sources.hide_in_anime_library_items, true),
                switch("Hide manga entries already in library", &prefs.sources.hide_in_manga_library_items, true),
                repo_row(ANIME_REPOS_TITLE, counts.anime, ANIME_REPOS_SCREEN),
                repo_row(MANGA_REPOS_TITLE, counts.manga, MANGA_REPOS_SCREEN),
            ],
        ),
        group(
            "NSFW (18+) sources",
            vec![
                nsfw_switch("Show in sources and extensions lists", &prefs.sources.show_nsfw_source),
                info(NSFW_INFO),
            ],
        ),
    ]
}

/// A repo row: its subtitle is the count, a tap opens the repo screen.
fn repo_row(title: &str, count: i64, screen: &'static str) -> PreferenceItem {
    let mut row = text(title, Some(&store_count_label(count)), true);
    if let PreferenceItem::Text { common, on_click } = &mut row {
        *on_click = Some(Rc::new(move || crate::ui::open(screen)));
        common.link = true;
    }
    row
}

/// The NSFW switch: a change passes the unlock confirm (T-47a-068); a refused or cancelled unlock keeps the value.
fn nsfw_switch(title: &str, pref: &Pref<bool>) -> PreferenceItem {
    let veto: Veto<bool> = bind::veto(move |_on: bool| async move {
        let confirmed = auth::confirm(NSFW_GATE_TITLE, auth::CONFIRM_LOCK_CHANGE).await;
        if !confirmed {
            crate::toast::notify(NSFW_REFUSED);
        }
        Ok(confirmed)
    });
    switch_with(title, Some(NSFW_SUBTITLE), pref, true, veto)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_of<'a>(tree: &'a [Preference], title: &str) -> &'a crate::pref_model::model::PreferenceGroup {
        tree.iter()
            .find_map(|entry| match entry {
                Preference::Group(g) if g.title == title => Some(g),
                _ => None,
            })
            .expect("the group")
    }

    #[test]
    fn the_two_hide_switches_read_their_own_keys_and_default_to_off() {
        let prefs = Prefs::in_memory();
        let tree = page_rows(&prefs, RepoCounts::default());
        let sources = group_of(&tree, "Sources");
        let PreferenceItem::Switch { pref, .. } = &sources.items[0] else { panic!("a switch") };
        assert!(!pref.get());
        assert_eq!(sources.items[0].title(), "Hide anime entries already in library");
        assert_eq!(sources.items[1].title(), "Hide manga entries already in library");
    }

    #[test]
    fn the_repo_rows_show_the_plural_count_and_open_their_screen() {
        assert_eq!(store_count_label(0), "0 stores");
        assert_eq!(store_count_label(1), "1 store");
        assert_eq!(store_count_label(3), "3 stores");
        let tree = page_rows(&Prefs::in_memory(), RepoCounts { anime: 2, manga: 1 });
        let sources = group_of(&tree, "Sources");
        let PreferenceItem::Text { common, on_click } = &sources.items[2] else { panic!("an action row") };
        assert_eq!(common.title, ANIME_REPOS_TITLE);
        assert_eq!(common.subtitle.as_deref(), Some("2 stores"));
        assert!(on_click.is_some());
        let PreferenceItem::Text { common, .. } = &sources.items[3] else { panic!("an action row") };
        assert_eq!(common.subtitle.as_deref(), Some("1 store"));
    }

    #[test]
    fn the_nsfw_group_has_the_switch_with_its_subtitle_and_the_info_text() {
        let tree = page_rows(&Prefs::in_memory(), RepoCounts::default());
        let nsfw = group_of(&tree, "NSFW (18+) sources");
        let PreferenceItem::Switch { common, pref, .. } = &nsfw.items[0] else { panic!("a switch") };
        assert_eq!(common.title, "Show in sources and extensions lists");
        assert_eq!(common.subtitle.as_deref(), Some(NSFW_SUBTITLE));
        assert!(pref.get(), "shown by default");
        assert!(matches!(&nsfw.items[1], PreferenceItem::Info { text, .. } if text == NSFW_INFO));
    }

    #[test]
    fn a_nsfw_change_passes_the_unlock_confirm_and_needs_no_dialog_while_no_passphrase_is_set() {
        let prefs = Prefs::in_memory();
        let tree = page_rows(&prefs, RepoCounts::default());
        let nsfw = group_of(&tree, "NSFW (18+) sources");
        let PreferenceItem::Switch { veto, .. } = &nsfw.items[0] else { panic!("a switch") };
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut future = std::pin::pin!(veto(false));
        assert!(matches!(future.as_mut().poll(&mut context), std::task::Poll::Ready(Ok(true))));
    }
}
