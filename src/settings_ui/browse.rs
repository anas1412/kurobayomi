//! Browse rows this module binds to the typed preferences: the Source languages filter and the extension repo rows
//! with their counts (task T-47a-062) of the Sources group, and the NSFW (18+) group (task T-47a-061). The two "hide
//! entries already in library" switches are cut (settings diet).
use std::rc::Rc;

use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::bind::{self, Veto};
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::auth;
use crate::settings_ui::rows::{group, info, multi_choice, switch_with, text};

pub const SOURCE_LANGUAGES_TITLE: &str = "Source languages";
pub const NSFW_SUBTITLE: &str = "Requires app restart to take effect";
pub const NSFW_INFO: &str = "This does not prevent unofficial or potentially incorrectly flagged extensions from surfacing NSFW (18+) content within the app.";
pub const NSFW_GATE_TITLE: &str = "NSFW (18+) sources";
pub const NSFW_REFUSED: &str = "Authentication is required to change this setting";
pub const REPOS_TITLE: &str = "Repositories";
pub const REPOS_SCREEN: &str = "settings_extension_repos";

/// The number of repository links the one repo row shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RepoCounts {
    pub repos: i64,
}

/// `0 repos`, `1 repo`, `N repos` (the `num_repos` plural).
pub fn repo_count_label(count: i64) -> String {
    if count == 1 { "1 repo".to_owned() } else { format!("{count} repos") }
}

/// The Source languages filter: the languages whose sources show. It offers `all` (Multi) and every language the app
/// has a name for, and any other stored tag so that a stored choice is never lost; a tick is written at once.
pub fn source_languages_row(prefs: &Prefs) -> PreferenceItem {
    let pref = &prefs.sources.enabled_languages;
    let mut keys: Vec<String> = std::iter::once("all".to_owned()).chain(kuro_i18n::names::NAMES.iter().map(|n| n.tag.to_owned())).collect();
    for stored in pref.get() {
        if !keys.contains(&stored) {
            keys.push(stored);
        }
    }
    kuro_i18n::source_names::sort_keys(&mut keys);
    let labels: Vec<String> = keys.iter().map(|key| kuro_i18n::source_names::display_name(key)).collect();
    let options: Vec<(&str, &str)> = keys.iter().map(String::as_str).zip(labels.iter().map(String::as_str)).collect();
    multi_choice(SOURCE_LANGUAGES_TITLE, pref, &options, true)
}

/// The Sources group, the NSFW (18+) group. `counts` is the number of repository links the repo row shows.
pub fn page_rows(prefs: &Prefs, counts: RepoCounts) -> Vec<Preference> {
    vec![
        group(
            "Sources",
            vec![
                source_languages_row(prefs),
                repo_row(REPOS_TITLE, counts.repos, REPOS_SCREEN),
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
    let mut row = text(title, Some(&repo_count_label(count)), true);
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
    fn the_sources_group_starts_with_the_language_filter_and_has_no_hide_switch() {
        let prefs = Prefs::in_memory();
        prefs.sources.enabled_languages.set(["en".to_owned(), "xx-odd".to_owned()].into()).unwrap();
        let tree = page_rows(&prefs, RepoCounts::default());
        let sources = group_of(&tree, "Sources");
        let titles: Vec<&str> = sources.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles, [SOURCE_LANGUAGES_TITLE, REPOS_TITLE]);
        let PreferenceItem::MultiSelect { pref, entries, .. } = &sources.items[0] else { panic!("a multi-select") };
        assert_eq!(pref.key(), prefs.sources.enabled_languages.key());
        assert!(entries.iter().any(|(key, _)| key == "all") && entries.iter().any(|(key, _)| key == "en"));
        assert!(entries.iter().any(|(key, _)| key == "xx-odd"), "a stored tag the table does not know is kept");
    }

    #[test]
    fn the_repo_rows_show_the_plural_count_and_open_their_screen() {
        assert_eq!(repo_count_label(0), "0 repos");
        assert_eq!(repo_count_label(1), "1 repo");
        assert_eq!(repo_count_label(3), "3 repos");
        let tree = page_rows(&Prefs::in_memory(), RepoCounts { repos: 3 });
        let sources = group_of(&tree, "Sources");
        let PreferenceItem::Text { common, on_click } = &sources.items[1] else { panic!("an action row") };
        assert_eq!(common.title, REPOS_TITLE);
        assert_eq!(common.subtitle.as_deref(), Some("3 repos"));
        assert!(on_click.is_some());
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
