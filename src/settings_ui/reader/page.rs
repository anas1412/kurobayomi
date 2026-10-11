//! Reader settings rows: the six kept reader settings (settings diet, 2026-10-11). The top row is the default reading
//! mode; the Display group holds the background colour; the Paged group holds the scale type, the zoom start position,
//! crop borders and split wide pages. Every other reader setting is cut.
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::Preference;
use crate::settings_ui::rows::{group, int_choice, item, switch};

/// `ReadingMode` codes (`pref_default_reading_mode_key`); 0 (Default) is not offered.
pub const READING_MODES: [(i32, &str); 5] = [
    (1, "Paged (left to right)"),
    (2, "Paged (right to left)"),
    (3, "Paged (vertical)"),
    (4, "Long strip"),
    (5, "Long strip with gaps"),
];
/// Background codes (`pref_reader_theme_key`).
pub const BACKGROUNDS: [(i32, &str); 4] = [(1, "Black"), (2, "Gray"), (0, "White"), (3, "Auto")];
/// Scale type codes (`pref_image_scale_type_key`).
pub const SCALE_TYPES: [(i32, &str); 6] = [
    (1, "Fit screen"),
    (2, "Stretch"),
    (3, "Fit width"),
    (4, "Fit height"),
    (5, "Original size"),
    (6, "Smart fit"),
];
/// Zoom start codes (`pref_zoom_start_key`).
pub const ZOOM_STARTS: [(i32, &str); 4] = [(1, "Automatic"), (2, "Left"), (3, "Right"), (4, "Center")];

/// The reader settings rows, every one of them kept. The page is built from these rows alone; the spec only gives the title and toolbar.
pub fn rows_all(prefs: &Prefs) -> Vec<Preference> {
    let r = &prefs.reader;
    vec![
        item(int_choice("Default reading mode", &r.default_reading_mode, &READING_MODES, true)),
        group("Display", vec![int_choice("Background color", &r.reader_theme, &BACKGROUNDS, true)]),
        group(
            "Paged",
            vec![
                int_choice("Scale type", &r.image_scale_type, &SCALE_TYPES, true),
                int_choice("Zoom start position", &r.zoom_start, &ZOOM_STARTS, true),
                switch("Crop borders", &r.crop_borders, true),
                switch("Split wide pages", &r.dual_page_split_paged, true),
            ],
        ),
    ]
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

    #[test]
    fn the_page_has_the_top_row_then_display_and_paged_groups() {
        let prefs = Prefs::in_memory();
        let tree = rows_all(&prefs);
        assert_eq!(tree.len(), 3);
        assert!(matches!(tree[0], Preference::Item(_)));
        assert_eq!(group_titles(&tree), ["Display", "Paged"]);
    }

    #[test]
    fn the_paged_group_holds_the_four_kept_layout_rows() {
        let prefs = Prefs::in_memory();
        let tree = rows_all(&prefs);
        let Preference::Group(paged) = &tree[2] else { panic!("the Paged group") };
        let titles: Vec<&str> = paged.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles, ["Scale type", "Zoom start position", "Crop borders", "Split wide pages"]);
    }

    #[test]
    fn the_reading_mode_offers_the_five_modes_and_the_background_the_four_colours() {
        assert_eq!(READING_MODES.len(), 5);
        assert_eq!(BACKGROUNDS.len(), 4);
        assert_eq!(SCALE_TYPES.len(), 6);
        assert_eq!(ZOOM_STARTS.len(), 4);
    }
}
