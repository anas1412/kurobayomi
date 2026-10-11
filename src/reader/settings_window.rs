//! The reader settings sheet: the six kept reader settings in two tabs. "Reading mode" holds the reading mode chips of
//! this manga and the paged layout (scale type, zoom start position, crop borders, split wide pages); "General" holds the
//! background colour. Each control writes its own key straight to `ReaderPrefs`: `Pref::set` stores the value in the
//! preference store's memory at once, and the store's flusher saves it later, so the GTK thread never waits on a file.
//!
//! The settings diet cut every other reader setting (colour filters, E-Ink flash, tap zones, volume keys, long tap,
//! rotate-to-fit, the webtoon copies and the restore switch).

use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::pref::Pref;
use kuro_prefs::reader::{ReaderPrefs, ReadingMode};
use kuro_prefs::values::PrefType;

use super::strings;
use crate::settings_ui::reader::page::{SCALE_TYPES, ZOOM_STARTS};

/// The background colour chips, in the order of the Background color row.
pub const THEME_CHIPS: [(&str, i32); 4] = [
    (strings::BLACK_BACKGROUND, 1),
    (strings::GRAY_BACKGROUND, 2),
    (strings::WHITE_BACKGROUND, 0),
    (strings::AUTOMATIC_BACKGROUND, 3),
];

/// The reading mode choices of this manga, as the sheet reports them.
pub struct MangaChoices {
    pub on_mode: Rc<dyn Fn(ReadingMode)>,
}

fn save<T: PrefType>(pref: &Pref<T>, value: T) {
    if let Err(e) = pref.set(value) {
        tracing::warn!(key = pref.key(), error = %e, "a reader setting could not be saved");
    }
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("heading");
    label.set_xalign(0.0);
    label.set_margin_top(12);
    label
}

fn check(label: &str, pref: &Pref<bool>) -> gtk::CheckButton {
    let button = gtk::CheckButton::with_label(label);
    button.set_active(pref.get());
    let pref = pref.clone();
    button.connect_toggled(move |b| save(&pref, b.is_active()));
    button
}

/// A row of chips. Only one is active. `on_pick` gets the chosen value.
fn chips(options: &[(&str, i32)], current: i32, on_pick: impl Fn(i32) + 'static) -> gtk::FlowBox {
    let row = gtk::FlowBox::new();
    row.set_selection_mode(gtk::SelectionMode::None);
    row.set_max_children_per_line(6);
    let on_pick = Rc::new(on_pick);
    let mut first: Option<gtk::ToggleButton> = None;
    for (label, value) in options {
        let button = gtk::ToggleButton::with_label(label);
        match &first {
            Some(group) => button.set_group(Some(group)),
            None => first = Some(button.clone()),
        }
        button.set_active(*value == current);
        let (pick, v) = (Rc::clone(&on_pick), *value);
        button.connect_toggled(move |b| {
            if b.is_active() {
                pick(v);
            }
        });
        row.append(&button);
    }
    row
}

/// A chip group for a stored code: the table gives `(code, label)`, the chips take `(label, code)`.
fn code_chips(table: &[(i32, &str)], pref: &Pref<i32>) -> gtk::FlowBox {
    let options: Vec<(&str, i32)> = table.iter().map(|(code, label)| (*label, *code)).collect();
    let pref = pref.clone();
    chips(&options, pref.get(), move |v| save(&pref, v))
}

fn mode_name(mode: ReadingMode) -> &'static str {
    super::mode_sheet::mode_label(mode)
}

/// The reading-mode tab (T-043a-046). The mode chips apply to this manga immediately; the paged rows follow them.
pub fn reading_tab(prefs: &ReaderPrefs, mode: ReadingMode, choices: MangaChoices) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.append(&heading(strings::PREF_CATEGORY_FOR_THIS_SERIES));
    let mode_options: Vec<(&str, i32)> = ReadingMode::ALL.iter().map(|m| (mode_name(*m), m.flag_value())).collect();
    let on_mode = choices.on_mode.clone();
    root.append(&chips(&mode_options, mode.flag_value(), move |v| on_mode(ReadingMode::from_preference(v))));

    root.append(&heading(strings::PREF_CATEGORY_READING_MODE));
    root.append(&gtk::Label::new(Some("Scale type")));
    root.append(&code_chips(&SCALE_TYPES, &prefs.image_scale_type));
    root.append(&gtk::Label::new(Some("Zoom start position")));
    root.append(&code_chips(&ZOOM_STARTS, &prefs.zoom_start));
    root.append(&check("Crop borders", &prefs.crop_borders));
    root.append(&check("Split wide pages", &prefs.dual_page_split_paged));
    root
}

/// The General tab: the background colour.
pub fn general_tab(prefs: &ReaderPrefs) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.append(&heading(strings::PREF_CATEGORY_GENERAL));
    root.append(&gtk::Label::new(Some(strings::PREF_READER_THEME)));
    let theme = prefs.reader_theme.clone();
    root.append(&chips(&THEME_CHIPS, prefs.reader_theme.get(), move |v| save(&theme, v)));
    root
}

/// The two tabs of the sheet.
pub fn notebook(prefs: &ReaderPrefs, mode: ReadingMode, choices: MangaChoices) -> gtk::Notebook {
    let notebook = gtk::Notebook::new();
    let reading = gtk::ScrolledWindow::new();
    reading.set_child(Some(&reading_tab(prefs, mode, choices)));
    let general = gtk::ScrolledWindow::new();
    general.set_child(Some(&general_tab(prefs)));
    notebook.append_page(&reading, Some(&gtk::Label::new(Some(strings::PREF_CATEGORY_READING_MODE))));
    notebook.append_page(&general, Some(&gtk::Label::new(Some(strings::PREF_CATEGORY_GENERAL))));
    notebook
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_chips_offer_the_four_stored_codes() {
        let codes: Vec<i32> = THEME_CHIPS.iter().map(|(_, code)| *code).collect();
        assert_eq!(codes, [1, 2, 0, 3]);
    }

    #[test]
    fn the_sheet_builds_two_tabs_over_the_kept_keys() {
        crate::testing::with_display(|| {
            let prefs = ReaderPrefs::new(kuro_prefs::store::PrefStore::in_memory());
            let choices = MangaChoices { on_mode: Rc::new(|_| {}) };
            let book = notebook(&prefs, ReadingMode::LeftToRight, choices);
            assert_eq!(book.n_pages(), 2);
        });
    }
}
