//! The Data Saver group of Advanced settings (tasks T-47a-086 and T-47a-087): the image proxy, its server address, the
//! image quality, the downloader and image-kind flags, the Jpeg switch and the black-and-white switch. The readers and
//! the downloader read these preferences already (`reader::loader::saver`, `kuro_downloads`); this module binds them.
//!
//! Rules: a row the mode disables is dropped by the merge (see `merge.rs`), so a Data Saver row that is not in use is
//! hidden rather than greyed. The mode row redraws the page through `changed` once the mode is stored, so the rows
//! appear or disappear at once.
use std::rc::Rc;

use kuro_prefs::prefs::Prefs;
use kuro_prefs::source::DataSaver;

use crate::pref_model::bind;
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::rows::{edit_text, group, int_choice, switch, switch_with};

pub const GROUP_TITLE: &str = "Data Saver";
pub const MODE_TITLE: &str = "Data Saver";
pub const MODE_SUBTITLE: &str = "Compress images before downloading or loading in reader";
pub const SERVER_TITLE: &str = "Bandwidth Hero Proxy Server";
pub const DOWNLOADER_TITLE: &str = "Use data saver in the downloader";
pub const IGNORE_JPEG_TITLE: &str = "Ignore Jpeg Images";
pub const IGNORE_GIF_TITLE: &str = "Ignore Gif Animations";
pub const QUALITY_TITLE: &str = "Image Quality";
pub const QUALITY_SUBTITLE: &str = "Higher values mean that a higher percentage of the image quality is saved, but it also means the file size is larger, 80 percent is a good median between file size and image quality";
pub const JPEG_TITLE: &str = "Compress to Jpeg";
pub const JPEG_SUBTITLE: &str = "The Jpeg file size is considerably smaller then Webp is(meaning more data is saved), but it makes the images lose more quality as well.";
pub const BW_TITLE: &str = "Convert to Black And White";

/// The modes in list order, with the labels of the Android list.
pub const MODES: [(DataSaver, &str); 4] = [
    (DataSaver::NONE, "Disabled"),
    (DataSaver::BANDWIDTH_HERO, "Bandwidth Hero (requires a Bandwidth Hero Proxy server)"),
    (DataSaver::WSRV_NL, "wsrv.nl"),
    (DataSaver::RESMUSH_IT, "resmush.it"),
];

/// The image quality choices, labelled as percentages.
pub const QUALITY_OPTIONS: [(i32, &str); 8] =
    [(10, "10%"), (20, "20%"), (40, "40%"), (50, "50%"), (70, "70%"), (80, "80%"), (90, "90%"), (95, "95%")];

/// The server address row is enabled for Bandwidth Hero only.
pub fn server_enabled(mode: DataSaver) -> bool {
    mode == DataSaver::BANDWIDTH_HERO
}

/// The downloader, Ignore Jpeg, Ignore Gif, quality rows are enabled whenever a mode is chosen.
pub fn flags_enabled(mode: DataSaver) -> bool {
    mode != DataSaver::NONE
}

/// Compress to Jpeg is not offered for reSmush.it, which returns the format it has.
pub fn jpeg_enabled(mode: DataSaver) -> bool {
    flags_enabled(mode) && mode != DataSaver::RESMUSH_IT
}

/// Black and white is a Bandwidth Hero option.
pub fn bw_enabled(mode: DataSaver) -> bool {
    mode == DataSaver::BANDWIDTH_HERO
}

/// The Compress to Jpeg subtitle: the explanation, then the format in use.
pub fn jpeg_subtitle(on: bool) -> String {
    let format = if on { "Jpeg" } else { "Webp" };
    format!("{JPEG_SUBTITLE}\nCurrently compresses to {format}")
}

/// The stored mode for a list key (the enum's variant name), or `None` for an unknown key.
fn mode_of_key(key: &str) -> Option<DataSaver> {
    MODES.iter().map(|(mode, _)| *mode).find(|mode| format!("{mode:?}") == key)
}

/// The mode row: a list whose stored choice redraws the page on the next idle, so the rows the mode enables appear.
fn mode_row(prefs: &Prefs, changed: Rc<dyn Fn()>) -> PreferenceItem {
    let pref = prefs.sources.data_saver.clone();
    let stored = pref.get();
    let entries = MODES.iter().map(|(mode, label)| (format!("{mode:?}"), (*label).to_owned())).collect();
    let on_change = bind::veto(move |key: String| {
        let result = mode_of_key(&key).ok_or_else(|| format!("unknown data saver mode {key:?}")).and_then(|mode| pref.set(mode).map_err(|e| e.to_string()));
        if result.is_ok() {
            let changed = Rc::clone(&changed);
            gtk::glib::idle_add_local_once(move || changed());
        }
        async move { result.map(|()| true) }
    });
    PreferenceItem::BasicList {
        common: crate::pref_model::model::ItemCommon { subtitle: Some(MODE_SUBTITLE.to_owned()), ..crate::pref_model::model::ItemCommon::bound(MODE_TITLE) },
        value: format!("{stored:?}"),
        entries,
        on_change,
    }
}

/// The Data Saver group: the mode first, then the rows that mode enables.
pub fn page_rows(prefs: &Prefs, changed: Rc<dyn Fn()>) -> Preference {
    let mode = prefs.sources.data_saver.get();
    let mut items = vec![mode_row(prefs, changed)];
    items.push(edit_text(SERVER_TITLE, &prefs.sources.data_saver_server, true, accept_server, server_enabled(mode)));
    items.push(switch(DOWNLOADER_TITLE, &prefs.sources.data_saver_downloader, flags_enabled(mode)));
    items.push(switch(IGNORE_JPEG_TITLE, &prefs.sources.data_saver_ignore_jpeg, flags_enabled(mode)));
    items.push(switch(IGNORE_GIF_TITLE, &prefs.sources.data_saver_ignore_gif, flags_enabled(mode)));
    let mut quality = int_choice(QUALITY_TITLE, &prefs.sources.data_saver_image_quality, &QUALITY_OPTIONS, flags_enabled(mode));
    if let PreferenceItem::BasicList { common, .. } = &mut quality {
        common.subtitle = Some(QUALITY_SUBTITLE.to_owned());
    }
    items.push(quality);
    let jpeg_on = prefs.sources.data_saver_image_format_jpeg.get();
    items.push(switch_with(JPEG_TITLE, Some(&jpeg_subtitle(jpeg_on)), &prefs.sources.data_saver_image_format_jpeg, jpeg_enabled(mode), bind::accept()));
    items.push(switch(BW_TITLE, &prefs.sources.data_saver_color_bw, bw_enabled(mode)));
    group(GROUP_TITLE, items)
}

/// The server address is free text; blank is allowed (the proxy is then not used).
fn accept_server(_: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modes_are_the_four_in_list_order_with_the_android_labels() {
        let labels: Vec<&str> = MODES.iter().map(|(_, l)| *l).collect();
        assert_eq!(labels, ["Disabled", "Bandwidth Hero (requires a Bandwidth Hero Proxy server)", "wsrv.nl", "resmush.it"]);
    }

    #[test]
    fn the_quality_choices_are_the_eight_percentages() {
        let values: Vec<i32> = QUALITY_OPTIONS.iter().map(|(v, _)| *v).collect();
        assert_eq!(values, [10, 20, 40, 50, 70, 80, 90, 95]);
        assert_eq!(QUALITY_OPTIONS[5].1, "80%");
    }

    #[test]
    fn the_server_row_is_for_bandwidth_hero_only() {
        assert!(!server_enabled(DataSaver::NONE));
        assert!(server_enabled(DataSaver::BANDWIDTH_HERO));
        assert!(!server_enabled(DataSaver::WSRV_NL));
        assert!(!server_enabled(DataSaver::RESMUSH_IT));
    }

    #[test]
    fn the_flags_follow_the_mode_not_none() {
        assert!(!flags_enabled(DataSaver::NONE));
        assert!(flags_enabled(DataSaver::WSRV_NL));
        assert!(flags_enabled(DataSaver::RESMUSH_IT));
    }

    #[test]
    fn jpeg_is_not_offered_for_resmush_it_or_when_disabled() {
        assert!(!jpeg_enabled(DataSaver::NONE));
        assert!(!jpeg_enabled(DataSaver::RESMUSH_IT));
        assert!(jpeg_enabled(DataSaver::WSRV_NL));
        assert!(jpeg_enabled(DataSaver::BANDWIDTH_HERO));
    }

    #[test]
    fn black_and_white_is_for_bandwidth_hero_only() {
        assert!(bw_enabled(DataSaver::BANDWIDTH_HERO));
        assert!(!bw_enabled(DataSaver::WSRV_NL));
        assert!(!bw_enabled(DataSaver::NONE));
    }

    #[test]
    fn the_jpeg_subtitle_names_the_format_in_use() {
        assert!(jpeg_subtitle(true).ends_with("\nCurrently compresses to Jpeg"));
        assert!(jpeg_subtitle(false).ends_with("\nCurrently compresses to Webp"));
        assert!(jpeg_subtitle(false).starts_with(JPEG_SUBTITLE));
    }

    #[test]
    fn a_mode_key_maps_back_to_its_variant_and_an_unknown_key_maps_to_none() {
        assert_eq!(mode_of_key("WSRV_NL"), Some(DataSaver::WSRV_NL));
        assert_eq!(mode_of_key("NONE"), Some(DataSaver::NONE));
        assert_eq!(mode_of_key("NOT_A_MODE"), None);
    }

    /// The titles of the group's rows that are enabled, in order (a disabled row is dropped by the merge).
    fn enabled_titles(group: &Preference) -> Vec<String> {
        match group {
            Preference::Group(g) => g.items.iter().filter(|i| i.enabled()).map(|i| i.title().to_owned()).collect(),
            Preference::Item(_) => Vec::new(),
        }
    }

    #[test]
    fn with_the_mode_disabled_only_the_mode_row_is_enabled() {
        let prefs = Prefs::in_memory();
        assert_eq!(enabled_titles(&page_rows(&prefs, Rc::new(|| {}))), [MODE_TITLE]);
    }

    #[test]
    fn with_wsrv_nl_the_server_and_black_and_white_rows_stay_hidden() {
        let prefs = Prefs::in_memory();
        prefs.sources.data_saver.set(DataSaver::WSRV_NL).unwrap();
        let shown = enabled_titles(&page_rows(&prefs, Rc::new(|| {})));
        assert_eq!(shown, [MODE_TITLE, DOWNLOADER_TITLE, IGNORE_JPEG_TITLE, IGNORE_GIF_TITLE, QUALITY_TITLE, JPEG_TITLE]);
    }

    #[test]
    fn with_bandwidth_hero_every_row_is_enabled() {
        let prefs = Prefs::in_memory();
        prefs.sources.data_saver.set(DataSaver::BANDWIDTH_HERO).unwrap();
        assert_eq!(enabled_titles(&page_rows(&prefs, Rc::new(|| {}))).len(), 8);
    }
}
