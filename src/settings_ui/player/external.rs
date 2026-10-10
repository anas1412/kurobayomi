//! The External player group of the internal player page (task T-48a-018). The list holds "None" (the system chooser)
//! and the apps that handle `video/*`. A stored app that is not installed stays selected and reads "(unavailable)"
//! (edge E-PS-11), so the choice is not lost when the app is removed.
use gtk::gio::prelude::AppInfoExt;
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::bind;
use crate::pref_model::model::{ItemCommon, Preference, PreferenceItem};
use crate::settings_ui::rows::{group, switch};

pub const NONE_LABEL: &str = "None";
pub const UNAVAILABLE_LABEL: &str = "(unavailable)";
pub const MIME_TYPE: &str = "video/*";

/// The list entries and the stored value. `installed` holds `(id, name)` of each `video/*` handler. An empty stored id
/// is "None"; a stored id that is not installed is added as an unavailable entry so it stays the value.
pub fn player_choices(installed: &[(String, String)], stored: &str) -> (Vec<(String, String)>, String) {
    let mut entries = vec![(String::new(), NONE_LABEL.to_owned())];
    entries.extend(installed.iter().cloned());
    if !stored.is_empty() && !installed.iter().any(|(id, _)| id == stored) {
        entries.push((stored.to_owned(), UNAVAILABLE_LABEL.to_owned()));
    }
    (entries, stored.to_owned())
}

/// The apps that open `video/*`, as `(id, name)`, sorted by name. Read from the desktop's app database.
pub fn installed_video_players() -> Vec<(String, String)> {
    let mut apps: Vec<(String, String)> = gtk::gio::AppInfo::all_for_type(MIME_TYPE)
        .into_iter()
        .filter_map(|app| Some((app.id()?.to_string(), app.display_name().to_string())))
        .collect();
    apps.sort_by_key(|(_, name)| name.to_lowercase());
    apps
}

/// The player list: writes the chosen id (empty for None) to the preference.
fn player_list(pref: &Pref<String>, entries: Vec<(String, String)>, value: String) -> PreferenceItem {
    let writer = pref.clone();
    let on_change = bind::veto(move |id: String| {
        let result = writer.set(id).map(|()| true).map_err(|err| err.to_string());
        async move { result }
    });
    PreferenceItem::BasicList { common: ItemCommon::bound("External player preference"), value, entries, on_change }
}

/// The External player group: the always-external switch and the player list.
pub fn group_row(prefs: &Prefs) -> Preference {
    group_from(prefs, &installed_video_players())
}

fn group_from(prefs: &Prefs, installed: &[(String, String)]) -> Preference {
    let p = &prefs.player;
    let (entries, value) = player_choices(installed, &p.external_player_preference.get());
    group(
        "External player",
        vec![
            switch("Always use external player", &p.always_use_external_player, true),
            player_list(&p.external_player_preference, entries, value),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apps() -> Vec<(String, String)> {
        vec![("mpv.desktop".to_owned(), "mpv".to_owned()), ("vlc.desktop".to_owned(), "VLC".to_owned())]
    }

    #[test]
    fn an_empty_choice_is_none_first_then_the_installed_players() {
        let (entries, value) = player_choices(&apps(), "");
        assert_eq!(value, "");
        assert_eq!(entries[0], (String::new(), "None".to_owned()));
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[2].1, "VLC");
    }

    #[test]
    fn a_stored_player_that_is_installed_is_the_value_with_no_extra_entry() {
        let (entries, value) = player_choices(&apps(), "vlc.desktop");
        assert_eq!(value, "vlc.desktop");
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn a_stored_player_that_is_gone_stays_selected_and_reads_unavailable() {
        let (entries, value) = player_choices(&apps(), "removed.desktop");
        assert_eq!(value, "removed.desktop");
        let last = entries.last().expect("an entry");
        assert_eq!(last, &("removed.desktop".to_owned(), UNAVAILABLE_LABEL.to_owned()));
    }

    #[test]
    fn the_group_has_the_switch_and_the_list_and_writes_the_chosen_id() {
        let prefs = Prefs::in_memory();
        let Preference::Group(g) = group_from(&prefs, &apps()) else { panic!("a group") };
        assert_eq!(g.title, "External player");
        assert_eq!(g.items.len(), 2);
        assert_eq!(g.items[0].title(), "Always use external player");
        let PreferenceItem::BasicList { on_change, .. } = &g.items[1] else { panic!("a list") };
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut future = std::pin::pin!(on_change("mpv.desktop".to_owned()));
        assert!(matches!(future.as_mut().poll(&mut context), std::task::Poll::Ready(Ok(true))));
        assert_eq!(prefs.player.external_player_preference.get(), "mpv.desktop");
    }
}
