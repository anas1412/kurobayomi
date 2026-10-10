//! Advanced page rows of the player (task T-48a-035): the user-files switch, the mpv.conf and input.conf editors and a
//! folder action. Each editor stores its text in the preference only; the player writes the files at each start
//! (task T-48a-036), so no mirror file is written here.
use std::rc::Rc;

use gtk::gio::prelude::FileExt;
use kuro_prefs::player_advanced::AdvancedPlayerPrefs;
use kuro_prefs::prefs::Prefs;
use kuro_prefs::store::PrefStore;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::player::conf_edit::conf_row;
use crate::settings_ui::rows::{switch_sub, text};
use crate::settings_ui::storage;

pub const USER_FILES_SUBTITLE: &str = "Use files in mpv-config directory";
pub const CONF_TITLE: &str = "Edit MPV configuration file for further player settings";
pub const INPUT_TITLE: &str = "Edit MPV input file for keyboard mapping configuration";
pub const OPEN_FOLDER_TITLE: &str = "Open mpv-config folder";
pub const OPEN_FAILED: &str = "Could not open link";

/// The rows of the Advanced page, loose as the spec page has them, then the folder action (a desktop addition).
pub fn page_rows(prefs: &Prefs) -> Vec<PreferenceItem> {
    let advanced = AdvancedPlayerPrefs::new(prefs.store().clone());
    let mut folder = text(OPEN_FOLDER_TITLE, None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut folder {
        let store = prefs.store().clone();
        *on_click = Some(Rc::new(move || open_mpv_folder(&store)));
    }
    vec![
        switch_sub("Use user MPV files", USER_FILES_SUBTITLE, &advanced.mpv_user_files, true),
        conf_row(CONF_TITLE, &advanced.mpv_conf, true),
        conf_row(INPUT_TITLE, &advanced.mpv_input, true),
        folder,
    ]
}

/// Opens the mpv-config folder of the stored base folder with the default handler.
fn open_mpv_folder(store: &PrefStore) {
    let Some(dir) = storage::manager(store).and_then(|manager| manager.mpv_config_dir().ok()) else {
        crate::toast::notify(OPEN_FAILED);
        return;
    };
    let uri = gtk::gio::File::for_path(&dir).uri();
    if let Err(err) = gtk::gio::AppInfo::launch_default_for_uri(&uri, None::<&gtk::gio::AppLaunchContext>) {
        tracing::warn!(%err, "the mpv-config folder could not be opened");
        crate::toast::notify(OPEN_FAILED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_come_in_spec_order_and_the_folder_action_is_last() {
        let rows = page_rows(&Prefs::in_memory());
        let titles: Vec<&str> = rows.iter().map(PreferenceItem::title).collect();
        assert_eq!(titles, ["Use user MPV files", CONF_TITLE, INPUT_TITLE, OPEN_FOLDER_TITLE]);
    }

    #[test]
    fn the_user_files_switch_is_off_and_both_editors_start_empty() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let PreferenceItem::Switch { pref, .. } = &rows[0] else { panic!("a switch") };
        assert!(!pref.get());
        assert!(matches!(&rows[1], PreferenceItem::Custom { .. }), "a multi-line editor row");
        assert!(matches!(&rows[2], PreferenceItem::Custom { .. }), "a multi-line editor row");
        let stored = AdvancedPlayerPrefs::new(prefs.store().clone());
        assert_eq!((stored.mpv_conf.get(), stored.mpv_input.get()), (String::new(), String::new()));
    }

    #[test]
    fn the_folder_row_has_a_tap_action_and_no_stored_value() {
        let rows = page_rows(&Prefs::in_memory());
        let PreferenceItem::Text { on_click, .. } = &rows[3] else { panic!("an action row") };
        assert!(on_click.is_some());
    }

    #[test]
    fn a_stored_mpv_conf_is_the_editor_value() {
        let prefs = Prefs::in_memory();
        AdvancedPlayerPrefs::new(prefs.store().clone()).mpv_conf.set("hwdec=auto".to_owned()).unwrap();
        let rows = page_rows(&prefs);
        assert_eq!(rows[1].title(), CONF_TITLE);
        assert_eq!(AdvancedPlayerPrefs::new(prefs.store().clone()).mpv_conf.get(), "hwdec=auto");
    }
}
