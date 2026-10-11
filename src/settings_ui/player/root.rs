//! The player settings root page (task T-48a-010): the kept settings as rows, then the links to the pages that stay.
//! The rows and the link targets are in `mod.rs`.
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use super::{controls, external, language, link_rows};
use crate::pref_model::bind;
use crate::pref_model::model::{ItemCommon, Preference, PreferenceItem};
use crate::pref_model::page;
use crate::screens::{built, Built};
use crate::settings_ui::rows::{group, switch};
use crate::settings_ui::search;

pub const SUBTITLE_LANGUAGE_INFO: &str = "Subtitle language(s) to be selected by default on a video with multiple subtitles, Two- or three-letter languages codes work. Multiple values can be delimited by a comma.";
pub const AUDIO_LANGUAGE_INFO: &str = "Audio language(s) to be selected by default on a video with multiple audio streams, Two- or three-letter languages codes work. Multiple values can be delimited by a comma.";
pub const VOLUME_TITLE: &str = "Default volume";
/// The stored value that means "the volume the player was last left at" (`player_volume_value`, not stored yet).
pub const LAST_USED: f32 = -1.0;

/// The volume levels offered by the Default volume row, 0 to 100 in steps of 10.
pub fn volume_levels() -> Vec<f32> {
    (0..=10).map(|step| (step * 10) as f32).collect()
}

fn label(level: f32) -> String {
    if level < 0.0 { "Last used".to_owned() } else { format!("{level:.0}%") }
}

/// The Default volume row. A stored level outside the list is kept, shown as `Custom (N%)` and listed first.
pub fn volume_row(pref: &Pref<f32>) -> PreferenceItem {
    let stored = pref.get();
    let in_list = stored == LAST_USED || volume_levels().iter().any(|level| (level - stored).abs() < 1e-6);
    let mut entries: Vec<(String, String)> = Vec::new();
    if !in_list {
        entries.push((format!("{stored}"), format!("Custom ({stored:.0}%)")));
    }
    entries.push((format!("{LAST_USED}"), label(LAST_USED)));
    entries.extend(volume_levels().into_iter().map(|level| (format!("{level}"), label(level))));
    let value = format!("{stored}");
    let pref = pref.clone();
    let on_change = bind::veto(move |key: String| {
        let result = key
            .parse::<f32>()
            .map_err(|err| err.to_string())
            .and_then(|value| pref.set(value).map_err(|err| err.to_string()));
        async move { result.map(|()| true) }
    });
    PreferenceItem::BasicList {
        common: ItemCommon::bound(VOLUME_TITLE),
        value,
        entries,
        on_change,
    }
}

/// The root page rows: the kept settings, the external player, the mini player when the display allows it, and the links.
pub fn page_rows(prefs: &Prefs, open: Rc<dyn Fn(&'static str)>) -> Vec<Preference> {
    let p = &prefs.player;
    let store = prefs.store();
    let subtitles = Pref::new(store.clone(), "pref_subtitle_lang", String::new());
    let audio = Pref::new(store.clone(), "pref_audio_lang", String::new());
    let mut tree = vec![group(
        "",
        vec![
            language::language_row("Preferred subtitle languages", &subtitles, SUBTITLE_LANGUAGE_INFO),
            language::language_row("Preferred audio languages", &audio, AUDIO_LANGUAGE_INFO),
            volume_row(&p.player_volume_value),
            switch("Autoplay next episode", &p.autoplay_enabled, true),
        ],
    )];
    tree.push(external::group_row(prefs));
    if controls::probe_mini_player().is_some() {
        tree.push(controls::mini_player_group(prefs));
    }
    tree.extend(link_rows(open));
    tree
}

fn search_button() -> gtk::Widget {
    let button = crate::ui::icon_button("system-search-symbolic", "Search");
    button.update_property(&[gtk::accessible::Property::Label("Search")]);
    button.connect_clicked(|_| {
        if let Some(page) = search::player_page() {
            crate::ui::push(&crate::ui::page("Search settings", &page.content, page.end, true));
        }
    });
    button.upcast()
}

/// The root page. The rows are read once when the page opens.
pub fn body(prefs: &Prefs) -> Option<Built> {
    let end = vec![search_button()];
    built(page::body(&page_rows(prefs, Rc::new(crate::ui::open))).widget, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    #[test]
    fn the_root_holds_the_four_kept_settings_first() {
        let prefs = Prefs::in_memory();
        let tree = page_rows(&prefs, Rc::new(|_| {}));
        let Preference::Group(first) = &tree[0] else { panic!("group") };
        let titles: Vec<&str> = first.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles, ["Preferred subtitle languages", "Preferred audio languages", VOLUME_TITLE, "Autoplay next episode"]);
    }

    #[test]
    fn the_default_volume_starts_at_last_used_and_offers_eleven_levels() {
        let store = PrefStore::in_memory();
        let pref = Pref::new(store, "player_volume_value", LAST_USED);
        let PreferenceItem::BasicList { value, entries, .. } = volume_row(&pref) else { panic!("a list") };
        assert_eq!(value, "-1");
        assert_eq!(entries.len(), 12);
        assert_eq!(entries[0].1, "Last used");
        assert_eq!(entries[11].1, "100%");
    }

    #[test]
    fn a_stored_level_outside_the_list_is_listed_first_as_custom() {
        let store = PrefStore::in_memory();
        let pref = Pref::new(store, "player_volume_value", 37.0f32);
        let PreferenceItem::BasicList { entries, .. } = volume_row(&pref) else { panic!("a list") };
        assert_eq!(entries[0].1, "Custom (37%)");
    }
}
