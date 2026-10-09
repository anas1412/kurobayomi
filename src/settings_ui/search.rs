//! The contract every settings screen of this module meets (task T-47a-006).
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::Preference;
use crate::pref_model::page::Built;

pub trait SearchableSettings {
    /// The screen title (English text of its title key).
    fn title(&self) -> &'static str;

    /// The preference tree. Evaluated on every open and every search, so rules see the current values.
    fn preferences(&self, prefs: &Prefs) -> Vec<Preference>;

    /// An optional app bar action as `(label, icon)`. Only the tracking and data screens have one.
    fn app_bar_action(&self) -> Option<(&'static str, &'static str)> {
        None
    }
}

/// Renders a screen's tree through the shared page renderer (needs the GTK main thread).
pub fn render(screen: &dyn SearchableSettings, prefs: &Prefs) -> Built {
    crate::pref_model::page::body(&screen.preferences(prefs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_ui::reader::page::ReaderScreen;

    #[test]
    fn a_screen_exposes_a_title_and_a_tree_without_an_action_by_default() {
        let screen = ReaderScreen;
        assert_eq!(screen.title(), "Reader");
        assert!(screen.app_bar_action().is_none());
        assert!(!screen.preferences(&Prefs::in_memory()).is_empty());
    }
}
