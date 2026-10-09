//! Reader settings rows: the top-level rows and the Display, Reading, Paged and Long strip groups (T-47a-031..038).
//! Rows that the rule table disables are left out of the tree (hidden, as on Android).
use kuro_prefs::prefs::Prefs;
use kuro_prefs::reader::{ReaderHideThreshold, TappingInvertMode};

use super::rules::{row_enabled, ReaderState, Row};
use crate::settings_ui::search::SearchableSettings;
use crate::settings_ui::rows::{enum_choice, exclusive_switch, group, int_choice, item, slider, switch, switch_sub, visible};
use crate::pref_model::model::{Preference, PreferenceItem};

/// `ReadingMode` codes (`pref_default_reading_mode_key`); 0 (Default) is not offered.
pub const READING_MODES: [(i32, &str); 5] = [
    (1, "Paged (left to right)"),
    (2, "Paged (right to left)"),
    (3, "Paged (vertical)"),
    (4, "Long strip"),
    (5, "Long strip with gaps"),
];
/// `ReaderOrientation` codes (`pref_default_orientation_type_key`).
pub const ROTATIONS: [(i32, &str); 6] = [
    (8, "Free"),
    (16, "Portrait"),
    (24, "Landscape"),
    (32, "Locked portrait"),
    (40, "Locked landscape"),
    (48, "Reverse portrait"),
];
/// Background codes (`pref_reader_theme_key`).
pub const BACKGROUNDS: [(i32, &str); 4] = [(1, "Black"), (2, "Gray"), (0, "White"), (3, "Auto")];
/// Side padding range in percent (`webtoon_side_padding`).
pub const PADDING_RANGE: std::ops::RangeInclusive<i32> = 0..=25;
/// Tap-zone codes shared by the paged and long strip viewers.
pub const TAP_ZONES: [(i32, &str); 6] = [
    (0, "Default"),
    (1, "L shaped"),
    (2, "Kindle-ish"),
    (3, "Edge"),
    (4, "Right and Left"),
    (5, "Disabled"),
];
pub const SCALE_TYPES: [(i32, &str); 6] = [
    (1, "Fit screen"),
    (2, "Stretch"),
    (3, "Fit width"),
    (4, "Fit height"),
    (5, "Original size"),
    (6, "Smart fit"),
];
pub const ZOOM_STARTS: [(i32, &str); 4] = [(1, "Automatic"), (2, "Left"), (3, "Right"), (4, "Center")];
pub const TAPPING_INVERT: [(TappingInvertMode, &str); 4] = [
    (TappingInvertMode::NONE, "None"),
    (TappingInvertMode::HORIZONTAL, "Horizontal"),
    (TappingInvertMode::VERTICAL, "Vertical"),
    (TappingInvertMode::BOTH, "Both"),
];
pub const HIDE_THRESHOLDS: [(ReaderHideThreshold, &str); 4] = [
    (ReaderHideThreshold::HIGHEST, "Highest"),
    (ReaderHideThreshold::HIGH, "High"),
    (ReaderHideThreshold::LOW, "Low"),
    (ReaderHideThreshold::LOWEST, "Lowest"),
];

/// The reader screen, for the settings search contract.
#[allow(dead_code)] // the settings search is not routed yet
pub struct ReaderScreen;

impl SearchableSettings for ReaderScreen {
    fn title(&self) -> &'static str {
        "Reader"
    }

    fn preferences(&self, prefs: &Prefs) -> Vec<Preference> {
        rows(prefs)
    }
}

/// The whole reader page as a preference tree, with the rows the rule table disables left out.
#[allow(dead_code)] // the settings search is not routed yet
pub fn rows(prefs: &Prefs) -> Vec<Preference> {
    tree(prefs, true)
}

/// The same tree with the disabled rows kept (flagged), so a page built from the spec can tell a hidden row from one
/// this module does not build.
pub fn rows_all(prefs: &Prefs) -> Vec<Preference> {
    tree(prefs, false)
}

fn tree(prefs: &Prefs, hide: bool) -> Vec<Preference> {
    let vis = |items: Vec<PreferenceItem>| if hide { visible(items) } else { items };
    let r = &prefs.reader;
    let s = ReaderState::from_prefs(r);
    let mut tree: Vec<Preference> = vec![
        item(int_choice("Default reading mode", &r.default_reading_mode, &READING_MODES, true)),
        item(switch_sub("Show reading mode", "Briefly show current mode when reader is opened", &r.show_reading_mode, true)),
        item(switch_sub(
            "Show tap zones overlay",
            "Briefly show when reader is opened",
            &r.show_navigation_overlay_on_start,
            true,
        )),
        item(switch("Animate page transitions", &r.page_transitions, true)),
    ];
    tree.push(group(
        "Display",
        vis(vec![
            int_choice("Default rotation", &r.default_orientation_type, &ROTATIONS, true),
            int_choice("Background color", &r.reader_theme, &BACKGROUNDS, true),
            switch("Fullscreen", &r.fullscreen, true),
            switch("Keep screen on", &r.keep_screen_on, true),
            switch("Show page number", &r.show_page_number, true),
        ]),
    ));
    tree.push(group(
        "Reading",
        vis(vec![
            switch("Skip chapters marked read", &r.skip_read, true),
            switch("Skip filtered chapters", &r.skip_filtered, true),
            switch("Skip duplicate chapters", &r.skip_dupe, true),
            switch("Always show chapter transition", &r.always_show_chapter_transition, true),
            switch("Preserve reading position on read chapters", &r.preserve_reading_position, true),
        ]),
    ));
    tree.push(group(
        "Paged",
        vis(vec![
            int_choice("Tap zones", &r.navigation_mode_pager, &TAP_ZONES, true),
            enum_choice(
                "Invert tap zones",
                &r.pager_nav_inverted,
                &TAPPING_INVERT,
                row_enabled(Row::PagerInvertTapZones, &s),
            ),
            int_choice("Scale type", &r.image_scale_type, &SCALE_TYPES, true),
            int_choice("Zoom start position", &r.zoom_start, &ZOOM_STARTS, true),
            switch("Crop borders", &r.crop_borders, true),
            switch("Automatically zoom into wide images", &r.landscape_zoom, row_enabled(Row::PagerLandscapeZoom, &s)),
            switch("Pan wide images", &r.navigate_to_pan, row_enabled(Row::PagerPanWide, &s)),
            exclusive_switch("Split wide pages", &r.dual_page_split_paged, &r.dual_page_rotate_to_fit, true),
            switch(
                "Invert split page placement",
                &r.dual_page_invert_paged,
                row_enabled(Row::PagerInvertSplit, &s),
            ),
            exclusive_switch("Rotate wide pages to fit", &r.dual_page_rotate_to_fit, &r.dual_page_split_paged, true),
            switch(
                "Flip orientation of rotated wide pages",
                &r.dual_page_rotate_to_fit_invert,
                row_enabled(Row::PagerFlipRotated, &s),
            ),
        ]),
    ));
    let padding = format!("{}%", r.webtoon_side_padding.get());
    tree.push(group(
        "Long strip",
        vis(vec![
            int_choice("Tap zones", &r.navigation_mode_webtoon, &TAP_ZONES, true),
            enum_choice(
                "Invert tap zones",
                &r.webtoon_nav_inverted,
                &TAPPING_INVERT,
                row_enabled(Row::WebtoonInvertTapZones, &s),
            ),
            slider(
                "Side padding",
                &r.webtoon_side_padding,
                PADDING_RANGE,
                Some(padding.as_str()),
                true,
            ),
            enum_choice("Sensitivity for hiding menu on scroll", &r.reader_hide_threshold, &HIDE_THRESHOLDS, true),
            switch("Crop borders", &r.crop_borders_webtoon, true),
            exclusive_switch("Split wide pages", &r.dual_page_split_webtoon, &r.dual_page_rotate_to_fit_webtoon, true),
            switch(
                "Invert split page placement",
                &r.dual_page_invert_webtoon,
                row_enabled(Row::WebtoonInvertSplit, &s),
            ),
            exclusive_switch("Rotate wide pages to fit", &r.dual_page_rotate_to_fit_webtoon, &r.dual_page_split_webtoon, true),
            switch(
                "Flip orientation of rotated wide pages",
                &r.dual_page_rotate_to_fit_invert_webtoon,
                row_enabled(Row::WebtoonFlipRotated, &s),
            ),
            switch("Double tap to zoom", &r.webtoon_double_tap_zoom_enabled, true),
            switch("Disable zoom out", &r.webtoon_disable_zoom_out, true),
        ]),
    ));
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_titles(tree: &[Preference]) -> Vec<&str> {
        tree.iter()
            .filter_map(|entry| match entry {
                Preference::Group(g) => Some(g.title.as_str()),
                Preference::Item(_) => None,
            })
            .collect()
    }

    fn titles_of(tree: &[Preference], group: &str) -> Vec<String> {
        tree.iter()
            .find_map(|entry| match entry {
                Preference::Group(g) if g.title == group => Some(g.items.iter().map(|i| i.title().to_owned()).collect()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn the_page_has_four_groups_in_order_after_the_top_level_rows() {
        let prefs = Prefs::in_memory();
        let tree = rows(&prefs);
        assert_eq!(group_titles(&tree), ["Display", "Reading", "Paged", "Long strip"]);
        let top: Vec<&str> = tree
            .iter()
            .filter_map(|e| match e {
                Preference::Item(i) => Some(i.title()),
                Preference::Group(_) => None,
            })
            .collect();
        assert_eq!(top, ["Default reading mode", "Show reading mode", "Show tap zones overlay", "Animate page transitions"]);
    }

    #[test]
    fn default_display_group_has_its_five_rows_in_android_order() {
        let tree = rows(&Prefs::in_memory());
        assert_eq!(
            titles_of(&tree, "Display"),
            ["Default rotation", "Background color", "Fullscreen", "Keep screen on", "Show page number"]
        );
    }

    #[test]
    fn paged_dependent_rows_are_hidden_until_their_switch_is_on() {
        let prefs = Prefs::in_memory();
        let before = titles_of(&rows(&prefs), "Paged");
        assert!(!before.iter().any(|t| t == "Invert split page placement"));
        assert!(!before.iter().any(|t| t == "Flip orientation of rotated wide pages"));
        prefs.reader.dual_page_split_paged.set(true).unwrap();
        let after = titles_of(&rows(&prefs), "Paged");
        assert!(after.iter().any(|t| t == "Invert split page placement"));
    }

    #[test]
    fn disabled_paged_tap_zones_hide_the_invert_and_pan_rows() {
        let prefs = Prefs::in_memory();
        prefs.reader.navigation_mode_pager.set(5).unwrap();
        let titles = titles_of(&rows(&prefs), "Paged");
        assert!(!titles.iter().any(|t| t == "Invert tap zones"));
        assert!(!titles.iter().any(|t| t == "Pan wide images"));
    }

    #[test]
    fn split_and_rotate_are_exclusive_through_their_vetoes() {
        let prefs = Prefs::in_memory();
        prefs.reader.dual_page_rotate_to_fit.set(true).unwrap();
        let tree = rows(&prefs);
        let PreferenceItem::Switch { veto, .. } = find_item(&tree, "Paged", "Split wide pages") else {
            panic!("split is a switch")
        };
        assert_eq!(block_on(veto(true)), Ok(true));
        assert!(!prefs.reader.dual_page_rotate_to_fit.get());
    }

    #[test]
    fn webtoon_padding_subtitle_is_the_percent_of_the_value() {
        let prefs = Prefs::in_memory();
        prefs.reader.webtoon_side_padding.set(12).unwrap();
        let tree = rows(&prefs);
        let PreferenceItem::Slider { common, range, .. } = find_item(&tree, "Long strip", "Side padding") else {
            panic!("slider")
        };
        assert_eq!(common.subtitle.as_deref(), Some("12%"));
        assert_eq!(*range.end(), 25);
    }

    fn find_item<'a>(tree: &'a [Preference], group: &str, title: &str) -> &'a PreferenceItem {
        tree.iter()
            .find_map(|entry| match entry {
                Preference::Group(g) if g.title == group => g.items.iter().find(|i| i.title() == title),
                _ => None,
            })
            .expect("row exists")
    }

    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("veto is not immediately ready"),
        }
    }
}
