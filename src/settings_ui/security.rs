//! The Security and privacy page rows this module binds (task T-47a-065): the Hide notification content switch. The
//! runner already shows the generic summary when it is on (`misc::runner`), so the row only writes the preference. The
//! lock rows and the secure screen list are left to the spec page.
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::Preference;
use crate::settings_ui::rows::{item, switch};

pub const HIDE_NOTIFICATION_TITLE: &str = "Hide notification content";

/// The typed rows of the Security and privacy page: the notification switch, default off, at the top level as the spec
/// lists it.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    vec![item(switch(HIDE_NOTIFICATION_TITLE, &prefs.security.hide_notification_content, true))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_model::model::PreferenceItem;

    #[test]
    fn the_switch_is_off_by_default_and_writes_the_security_key() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let Preference::Item(PreferenceItem::Switch { common, pref, .. }) = &rows[0] else { panic!("a switch") };
        assert_eq!(common.title, HIDE_NOTIFICATION_TITLE);
        assert!(!pref.get());
        assert_eq!(pref.key(), "hide_notification_content");
    }
}
