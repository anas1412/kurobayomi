//! The Downloads page rows this module binds (task T-47a-044): the "Only on Wi-Fi" switch, default on. The rest of the
//! page stays as the spec builds it. The network check (NetworkManager or the metered flag) is read by the downloader.
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::rows::{item, switch};

pub const WIFI_TITLE: &str = "Only on Wi-Fi";
pub const WIFI_KEY: &str = "pref_download_only_over_wifi_key";

/// The one row this module binds, loose as the spec page has it.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    let only_over_wifi = Pref::new(prefs.store().clone(), WIFI_KEY, true);
    vec![item(switch(WIFI_TITLE, &only_over_wifi, true))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_switch_is_on_by_default_and_writes_the_downloader_key() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let Preference::Item(PreferenceItem::Switch { common, pref, .. }) = &rows[0] else {
            panic!("a switch")
        };
        assert!(pref.get());
        assert_eq!(pref.key(), WIFI_KEY);
        assert_eq!(common.title, WIFI_TITLE);
    }
}
