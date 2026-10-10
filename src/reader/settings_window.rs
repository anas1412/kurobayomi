//! The reader settings window (T-043a-046 reading-mode tab, T-043a-047 general tab). The tab bodies bind each
//! control straight to its `ReaderPrefs` key; the pure helpers hold the key choices, ranges and stored values so
//! they can be tested without a display. The custom filter tab is T-043a-048.
//!
//! Preference writes (T-043a-081): a control sets its `Pref` directly. `Pref::set` stores the value in the
//! preference store's memory at once, so the next read, and the reader's change feed, see it, and the store's own
//! flusher thread saves it to disk after a short quiet period, so the GTK thread never waits on a file. The
//! database writer actor (`writer`) carries the reader's database writes only. This is the recorded decision for
//! the criterion that routes preference writes through the writer actor.
//!
//! Dependent rows follow the rules of `settings_ui::reader::rules` (one `row_enabled` for the settings screen and this
//! sheet): they stay in place and go insensitive while their parent setting does not allow them, and they are re-checked
//! after every write. Turning "Split wide pages" on turns "Rotate wide pages to fit" off, and the other way round.
//!
//! Decision on the spec's `pref` key (T-43b-031): this sheet is built straight over `ReaderPrefs` in this file, as the
//! sheet of the reader has always been, so `src/spec.rs` (`Item`) and `src/ui.rs` (`item_row`) carry no `pref` key for it;
//! the acceptance bullet is amended to "every control binds its `ReaderPrefs` key in `settings_window`".
//!
//! Every control writes only its own key. The "Split wide pages" control writes `pref_dual_page_split` and its
//! invert control writes `pref_dual_page_invert` (E-UI-3: the mockup had them swapped).

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::pref::Pref;
use kuro_prefs::reader::{FlashColor, ReaderHideThreshold, ReaderOrientation, ReaderPrefs, ReadingMode, TappingInvertMode};
use kuro_prefs::values::PrefType;

use super::overlay::{self, BRIGHTNESS_RANGE, Channel};
use super::restore;
use super::strings;
use crate::settings_ui::reader::rules::{ReaderState, Row, row_enabled};

/// The tap zone chip index that disables taps, which hides the inversion row.
pub const TAP_ZONE_DISABLED: i32 = 5;
/// Flash duration as the slider sees it: units of 100 ms, `1..=15`.
pub const FLASH_SLIDER_RANGE: (i32, i32) = (1, 15);
pub const FLASH_INTERVAL_RANGE: (i32, i32) = (1, 10);
pub const SIDE_PADDING_RANGE: (i32, i32) = kuro_prefs::ranges::WEBTOON_SIDE_PADDING.bounds().expect("a range");
/// One channel of the colour filter value.
const CHANNEL_RANGE: (i32, i32) = kuro_prefs::ranges::COLOR_FILTER_CHANNEL.bounds().expect("a range");

/// Background chips in display order, with the stored `pref_reader_theme_key` value of each.
pub const THEME_CHIPS: [(&str, i32); 4] = [
    (strings::BLACK_BACKGROUND, 1),
    (strings::GRAY_BACKGROUND, 2),
    (strings::WHITE_BACKGROUND, 0),
    (strings::AUTOMATIC_BACKGROUND, 3),
];

/// Tap zone chip labels, index 0..5 (stored as the index).
pub const TAP_ZONE_CHIPS: [&str; 6] = [
    strings::LABEL_DEFAULT,
    strings::L_NAV,
    strings::KINDLISH_NAV,
    strings::EDGE_NAV,
    strings::RIGHT_AND_LEFT_NAV,
    strings::DISABLED_NAV,
];

/// Tapping inversion chips, in `TappingInvertMode` order.
pub const INVERT_CHIPS: [(&str, TappingInvertMode); 4] = [
    (strings::TAPPING_INVERTED_NONE, TappingInvertMode::NONE),
    (strings::TAPPING_INVERTED_HORIZONTAL, TappingInvertMode::HORIZONTAL),
    (strings::TAPPING_INVERTED_VERTICAL, TappingInvertMode::VERTICAL),
    (strings::TAPPING_INVERTED_BOTH, TappingInvertMode::BOTH),
];

/// Scale type chips: the stored value is the index plus one.
pub const SCALE_TYPE_CHIPS: [&str; 6] = [
    strings::SCALE_TYPE_FIT_SCREEN,
    strings::SCALE_TYPE_STRETCH,
    strings::SCALE_TYPE_FIT_WIDTH,
    strings::SCALE_TYPE_FIT_HEIGHT,
    strings::SCALE_TYPE_ORIGINAL_SIZE,
    strings::SCALE_TYPE_SMART_FIT,
];

/// Zoom start chips: the stored value is the index plus one.
pub const ZOOM_START_CHIPS: [&str; 4] = [
    strings::ZOOM_START_AUTOMATIC,
    strings::ZOOM_START_LEFT,
    strings::ZOOM_START_RIGHT,
    strings::ZOOM_START_CENTER,
];

/// Flash colour chips in `FlashColor` order.
pub const FLASH_CHIPS: [(&str, FlashColor); 3] = [
    (strings::PREF_FLASH_STYLE_BLACK, FlashColor::BLACK),
    (strings::PREF_FLASH_STYLE_WHITE, FlashColor::WHITE),
    (strings::PREF_FLASH_STYLE_WHITE_BLACK, FlashColor::WHITE_BLACK),
];

/// Double tap animation chips: the stored duration in ms and the label. 1 ms turns the animation off.
pub const DOUBLE_TAP_SPEED_CHIPS: [(&str, i32); 3] = [
    (strings::DOUBLE_TAP_ANIM_SPEED_0, 1),
    (strings::DOUBLE_TAP_ANIM_SPEED_NORMAL, 500),
    (strings::DOUBLE_TAP_ANIM_SPEED_FAST, 250),
];

/// Hide-menu threshold chips of the webtoon strip, in `ReaderHideThreshold` order (5, 13, 31 and 47 px).
pub const HIDE_THRESHOLD_CHIPS: [(&str, ReaderHideThreshold); 4] = [
    (strings::PREF_HIGHEST, ReaderHideThreshold::HIGHEST),
    (strings::PREF_HIGH, ReaderHideThreshold::HIGH),
    (strings::PREF_LOW, ReaderHideThreshold::LOW),
    (strings::PREF_LOWEST, ReaderHideThreshold::LOWEST),
];

/// The four dual-page keys of one viewer family: paged or webtoon.
pub struct DualPagePrefs<'a> {
    pub split: &'a Pref<bool>,
    pub invert: &'a Pref<bool>,
    pub rotate: &'a Pref<bool>,
    pub rotate_invert: &'a Pref<bool>,
}

/// The dual-page preferences for a viewer family. Paged uses the paged keys, webtoon the webtoon keys.
pub fn dual_page_prefs(prefs: &ReaderPrefs, webtoon: bool) -> DualPagePrefs<'_> {
    if webtoon {
        DualPagePrefs {
            split: &prefs.dual_page_split_webtoon,
            invert: &prefs.dual_page_invert_webtoon,
            rotate: &prefs.dual_page_rotate_to_fit_webtoon,
            rotate_invert: &prefs.dual_page_rotate_to_fit_invert_webtoon,
        }
    } else {
        DualPagePrefs {
            split: &prefs.dual_page_split_paged,
            invert: &prefs.dual_page_invert_paged,
            rotate: &prefs.dual_page_rotate_to_fit,
            rotate_invert: &prefs.dual_page_rotate_to_fit_invert,
        }
    }
}

/// The stored flash duration (ms) as the slider value (units of 100 ms).
pub fn flash_slider_from_millis(millis: i32) -> i32 {
    millis / 100
}

/// The slider value back to the stored duration in milliseconds.
pub fn flash_millis_from_slider(value: i32) -> i32 {
    value * 100
}

/// The side padding text: a percent of the value (`NumberFormat.getPercentInstance`).
pub fn side_padding_text(value: i32) -> String {
    format!("{value}%")
}

/// Whether the viewer family of a reading mode is webtoon: the long strip and continuous vertical modes.
/// A Default manga resolves through the default reading mode preference first.
pub fn is_webtoon_family(mode: ReadingMode, default_pref: i32) -> bool {
    let resolved = if mode == ReadingMode::Default { ReadingMode::from_preference(default_pref) } else { mode };
    matches!(resolved, ReadingMode::Webtoon | ReadingMode::ContinuousVertical)
}

/// Callbacks for the chips that apply to this manga right away.
#[derive(Clone)]
pub struct MangaChoices {
    pub on_mode: Rc<dyn Fn(ReadingMode)>,
    pub on_orientation: Rc<dyn Fn(ReaderOrientation)>,
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

/// The rows of one sheet that depend on another setting. Each is enabled by `row_enabled` and checked again by
/// [`Dependents::refresh`], which every controlling write calls.
#[derive(Clone)]
struct Dependents {
    prefs: ReaderPrefs,
    rows: Rc<RefCell<Vec<(Row, gtk::Widget)>>>,
}

impl Dependents {
    fn new(prefs: &ReaderPrefs) -> Self {
        Self { prefs: prefs.clone(), rows: Rc::default() }
    }

    /// Registers a row under its rule and applies the rule now.
    fn add(&self, row: Row, widget: &impl IsA<gtk::Widget>) {
        widget.set_sensitive(row_enabled(row, &ReaderState::from_prefs(&self.prefs)));
        self.rows.borrow_mut().push((row, widget.clone().upcast()));
    }

    fn refresh(&self) {
        let state = ReaderState::from_prefs(&self.prefs);
        for (row, widget) in self.rows.borrow().iter() {
            widget.set_sensitive(row_enabled(*row, &state));
        }
    }
}

fn check(label: &str, pref: &Pref<bool>) -> gtk::CheckButton {
    check_then(label, pref, || {})
}

/// A check that writes its key and then calls `after`.
fn check_then(label: &str, pref: &Pref<bool>, after: impl Fn() + 'static) -> gtk::CheckButton {
    let button = gtk::CheckButton::with_label(label);
    button.set_active(pref.get());
    let pref = pref.clone();
    button.connect_toggled(move |b| {
        save(&pref, b.is_active());
        after();
    });
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

/// A labelled slider. `text` shows the value; `on_change` gets the new value.
fn slider(
    label: &str,
    range: (i32, i32),
    value: i32,
    text: fn(i32) -> String,
    on_change: impl Fn(i32) + 'static,
) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::new(Some(label));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    let shown = gtk::Label::new(Some(&text(value)));
    head.append(&name);
    head.append(&shown);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, f64::from(range.0), f64::from(range.1), 1.0);
    scale.set_draw_value(false);
    scale.set_digits(0);
    scale.set_value(f64::from(value));
    scale.connect_value_changed(move |s| {
        let v = s.value().round() as i32;
        shown.set_label(&text(v));
        on_change(v);
    });
    root.append(&head);
    root.append(&scale);
    root
}

/// The reading-mode tab (T-043a-046). The mode and rotation chips apply to this manga immediately.
pub fn reading_tab(prefs: &ReaderPrefs, mode: ReadingMode, orientation: ReaderOrientation, choices: MangaChoices) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.append(&heading(strings::PREF_CATEGORY_FOR_THIS_SERIES));

    // both viewer blocks are built; the one that fits the chosen mode is shown, and a chip swaps them at once
    let deps = Dependents::new(prefs);
    let pager = pager_block(prefs, &deps);
    let webtoon = webtoon_block(prefs, &deps);
    let default_mode = prefs.default_reading_mode.clone();
    let show_block = {
        let (pager, webtoon) = (pager.clone(), webtoon.clone());
        move |mode: ReadingMode| {
            let (show_pager, show_webtoon) = block_visibility(mode, default_mode.get());
            pager.set_visible(show_pager);
            webtoon.set_visible(show_webtoon);
        }
    };
    show_block(mode);

    let mode_options: Vec<(&str, i32)> = ReadingMode::ALL
        .iter()
        .map(|m| (mode_name(*m), m.flag_value()))
        .collect();
    let on_mode = choices.on_mode.clone();
    root.append(&chips(&mode_options, mode.flag_value(), move |v| {
        let chosen = ReadingMode::from_preference(v);
        show_block(chosen);
        on_mode(chosen);
    }));

    let orientation_options: Vec<(&str, i32)> = ReaderOrientation::ALL
        .iter()
        .map(|o| (orientation_name(*o), o.flag_value()))
        .collect();
    let on_orientation = choices.on_orientation.clone();
    root.append(&chips(&orientation_options, orientation.flag_value(), move |v| {
        on_orientation(ReaderOrientation::from_preference(v));
    }));

    root.append(&pager);
    root.append(&webtoon);
    root
}

/// Which viewer block shows for a reading mode: `(paged, webtoon)`. Exactly one of them.
pub fn block_visibility(mode: ReadingMode, default_pref: i32) -> (bool, bool) {
    let webtoon = is_webtoon_family(mode, default_pref);
    (!webtoon, webtoon)
}

fn mode_name(mode: ReadingMode) -> &'static str {
    super::mode_sheet::mode_label(mode)
}

fn orientation_name(orientation: ReaderOrientation) -> &'static str {
    match orientation {
        ReaderOrientation::Default => strings::LABEL_DEFAULT,
        ReaderOrientation::Free => strings::ROTATION_FREE,
        ReaderOrientation::Portrait => strings::ROTATION_PORTRAIT,
        ReaderOrientation::Landscape => strings::ROTATION_LANDSCAPE,
        ReaderOrientation::LockedPortrait => strings::ROTATION_FORCE_PORTRAIT,
        ReaderOrientation::LockedLandscape => strings::ROTATION_FORCE_LANDSCAPE,
        ReaderOrientation::ReversePortrait => strings::ROTATION_REVERSE_PORTRAIT,
    }
}

/// Which dependent rows a viewer family's controls feed.
struct FamilyRows {
    invert_split: Row,
    flip_rotated: Row,
    invert_tap_zones: Row,
}

const PAGER_ROWS: FamilyRows = FamilyRows { invert_split: Row::PagerInvertSplit, flip_rotated: Row::PagerFlipRotated, invert_tap_zones: Row::PagerInvertTapZones };
const WEBTOON_ROWS: FamilyRows =
    FamilyRows { invert_split: Row::WebtoonInvertSplit, flip_rotated: Row::WebtoonFlipRotated, invert_tap_zones: Row::WebtoonInvertTapZones };

/// The dual-page checks of one family. The invert and flip rows are enabled only while their parent is on, and split and
/// rotate are exclusive: turning one on turns the other off (`SettingsReaderScreen`'s `onValueChanged`).
fn dual_page_rows(root: &gtk::Box, prefs: &ReaderPrefs, webtoon: bool, deps: &Dependents) {
    let keys = dual_page_prefs(prefs, webtoon);
    let rows = if webtoon { WEBTOON_ROWS } else { PAGER_ROWS };
    let (split, rotate) = (keys.split.clone(), keys.rotate.clone());

    let refresh = deps.clone();
    let split_check = check_then(strings::PREF_DUAL_PAGE_SPLIT, keys.split, move || refresh.refresh());
    let refresh = deps.clone();
    let rotate_check = check_then(strings::PREF_PAGE_ROTATE, keys.rotate, move || refresh.refresh());
    {
        let (rotate_check, rotate) = (rotate_check.clone(), rotate.clone());
        split_check.connect_toggled(move |b| {
            if b.is_active() && rotate.get() {
                rotate_check.set_active(false);
            }
        });
    }
    {
        let (split_check, split) = (split_check.clone(), split.clone());
        rotate_check.connect_toggled(move |b| {
            if b.is_active() && split.get() {
                split_check.set_active(false);
            }
        });
    }

    let invert = check(strings::PREF_DUAL_PAGE_INVERT, keys.invert);
    invert.set_margin_start(24);
    deps.add(rows.invert_split, &invert);
    root.append(&split_check);
    root.append(&invert);

    let rotate_invert = check(strings::PREF_PAGE_ROTATE_INVERT, keys.rotate_invert);
    rotate_invert.set_margin_start(24);
    deps.add(rows.flip_rotated, &rotate_invert);
    root.append(&rotate_check);
    root.append(&rotate_invert);
}

/// The tap zone chips and the inversion chips, which are insensitive while taps are disabled.
fn tap_zone_rows(root: &gtk::Box, zone: &Pref<i32>, invert: &Pref<TappingInvertMode>, invert_row: Row, deps: &Dependents) {
    let inversion = gtk::Box::new(gtk::Orientation::Vertical, 4);
    inversion.append(&gtk::Label::new(Some(strings::PREF_READ_WITH_TAPPING_INVERTED)));
    let invert_options: Vec<(&str, i32)> = INVERT_CHIPS.iter().enumerate().map(|(i, (l, _))| (*l, i as i32)).collect();
    let current = INVERT_CHIPS.iter().position(|(_, m)| *m == invert.get()).unwrap_or(0) as i32;
    let invert_pref = invert.clone();
    inversion.append(&chips(&invert_options, current, move |v| save(&invert_pref, INVERT_CHIPS[v as usize].1)));
    deps.add(invert_row, &inversion);
    let zone_options: Vec<(&str, i32)> = TAP_ZONE_CHIPS.iter().enumerate().map(|(i, l)| (*l, i as i32)).collect();
    let (zone_pref, refresh) = (zone.clone(), deps.clone());
    root.append(&gtk::Label::new(Some(strings::PREF_VIEWER_NAV)));
    root.append(&chips(&zone_options, zone.get(), move |v| {
        save(&zone_pref, v);
        refresh.refresh();
    }));
    root.append(&inversion);
}

fn pager_block(prefs: &ReaderPrefs, deps: &Dependents) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.append(&heading(strings::PAGER_VIEWER));
    tap_zone_rows(&root, &prefs.navigation_mode_pager, &prefs.pager_nav_inverted, PAGER_ROWS.invert_tap_zones, deps);

    let scale: Vec<(&str, i32)> = SCALE_TYPE_CHIPS.iter().enumerate().map(|(i, l)| (*l, i as i32 + 1)).collect();
    root.append(&gtk::Label::new(Some(strings::PREF_IMAGE_SCALE_TYPE)));
    let (scale_pref, refresh) = (prefs.image_scale_type.clone(), deps.clone());
    root.append(&chips(&scale, prefs.image_scale_type.get(), move |v| {
        save(&scale_pref, v);
        refresh.refresh();
    }));

    let zoom: Vec<(&str, i32)> = ZOOM_START_CHIPS.iter().enumerate().map(|(i, l)| (*l, i as i32 + 1)).collect();
    root.append(&gtk::Label::new(Some(strings::PREF_ZOOM_START)));
    let zoom_pref = prefs.zoom_start.clone();
    root.append(&chips(&zoom, prefs.zoom_start.get(), move |v| save(&zoom_pref, v)));

    root.append(&check(strings::PREF_CROP_BORDERS, &prefs.crop_borders));
    let landscape = check(strings::PREF_LANDSCAPE_ZOOM, &prefs.landscape_zoom);
    deps.add(Row::PagerLandscapeZoom, &landscape);
    root.append(&landscape);
    let pan = check(strings::PREF_NAVIGATE_PAN, &prefs.navigate_to_pan);
    deps.add(Row::PagerPanWide, &pan);
    root.append(&pan);
    dual_page_rows(&root, prefs, false, deps);
    root
}

fn webtoon_block(prefs: &ReaderPrefs, deps: &Dependents) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.append(&heading(strings::WEBTOON_VIEWER));
    tap_zone_rows(&root, &prefs.navigation_mode_webtoon, &prefs.webtoon_nav_inverted, WEBTOON_ROWS.invert_tap_zones, deps);
    let padding = prefs.webtoon_side_padding.clone();
    root.append(&slider(
        strings::PREF_WEBTOON_SIDE_PADDING,
        SIDE_PADDING_RANGE,
        padding.get(),
        side_padding_text,
        move |v| save(&padding, v),
    ));
    root.append(&check(strings::PREF_CROP_BORDERS, &prefs.crop_borders_webtoon));
    dual_page_rows(&root, prefs, true, deps);
    root.append(&check(strings::PREF_DOUBLE_TAP_ZOOM, &prefs.webtoon_double_tap_zoom_enabled));
    root.append(&check(strings::PREF_WEBTOON_DISABLE_ZOOM_OUT, &prefs.webtoon_disable_zoom_out));
    root.append(&gtk::Label::new(Some(strings::PREF_HIDE_THRESHOLD)));
    let options: Vec<(&str, i32)> = HIDE_THRESHOLD_CHIPS.iter().enumerate().map(|(i, (l, _))| (*l, i as i32)).collect();
    let current = HIDE_THRESHOLD_CHIPS.iter().position(|(_, t)| *t == prefs.reader_hide_threshold.get()).unwrap_or(0) as i32;
    let threshold = prefs.reader_hide_threshold.clone();
    root.append(&chips(&options, current, move |v| save(&threshold, HIDE_THRESHOLD_CHIPS[v as usize].1)));
    root
}

/// The general tab (T-043a-047). The cutout check is hidden on Linux (T-043a-069).
pub fn general_tab(prefs: &ReaderPrefs) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.append(&heading(strings::PREF_CATEGORY_GENERAL));
    let theme = prefs.reader_theme.clone();
    root.append(&gtk::Label::new(Some(strings::PREF_READER_THEME)));
    root.append(&chips(&THEME_CHIPS, prefs.reader_theme.get(), move |v| save(&theme, v)));
    root.append(&check(strings::PREF_SHOW_PAGE_NUMBER, &prefs.show_page_number));
    root.append(&check(strings::PREF_FULLSCREEN, &prefs.fullscreen));
    root.append(&check(strings::PREF_KEEP_SCREEN_ON, &prefs.keep_screen_on));
    root.append(&check(strings::PREF_READ_WITH_LONG_TAP, &prefs.read_with_long_tap));
    root.append(&check(strings::PREF_ALWAYS_SHOW_CHAPTER_TRANSITION, &prefs.always_show_chapter_transition));
    root.append(&check(strings::PREF_PAGE_TRANSITIONS, &prefs.page_transitions));
    root.append(&gtk::Label::new(Some(strings::PREF_DOUBLE_TAP_ANIM_SPEED)));
    let speed = prefs.double_tap_anim_speed.clone();
    root.append(&chips(&DOUBLE_TAP_SPEED_CHIPS, prefs.double_tap_anim_speed.get(), move |v| save(&speed, v)));
    root.append(&check(strings::PREF_SKIP_READ_CHAPTERS, &prefs.skip_read));
    root.append(&check(strings::PREF_SKIP_FILTERED_CHAPTERS, &prefs.skip_filtered));
    root.append(&check(strings::PREF_SKIP_DUPE_CHAPTERS, &prefs.skip_dupe));
    root.append(&check(strings::PREF_PRESERVE_READING_POSITION, &prefs.preserve_reading_position));
    let deps = Dependents::new(prefs);
    let refresh = deps.clone();
    root.append(&check_then(strings::PREF_READ_WITH_VOLUME_KEYS, &prefs.read_with_volume_keys, move || refresh.refresh()));
    let inverted = check(strings::PREF_READ_WITH_VOLUME_KEYS_INVERTED, &prefs.read_with_volume_keys_inverted);
    inverted.set_margin_start(24);
    deps.add(Row::InvertVolumeKeys, &inverted);
    root.append(&inverted);
    let caveat = gtk::Label::new(Some(strings::VOLUME_KEYS_MEDIA_CAVEAT));
    caveat.set_wrap(true);
    caveat.set_xalign(0.0);
    caveat.set_margin_start(24);
    caveat.add_css_class("dim-label");
    root.append(&caveat);
    root.append(&restore_row(prefs));
    root.append(&flash_rows(prefs));
    root
}

/// The switch of the crash restore (T-43a-039). It is on by default and lives under its own key in the store.
fn restore_row(prefs: &ReaderPrefs) -> gtk::CheckButton {
    check(strings::PREF_RESTORE_AFTER_CRASH, &prefs.store().pref(restore::PREF_KEY, true))
}

fn flash_rows(prefs: &ReaderPrefs) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let deps = Dependents::new(prefs);
    let refresh = deps.clone();
    let flash = check_then(strings::PREF_FLASH_PAGE, &prefs.flash_on_page_change, move || refresh.refresh());
    let details = gtk::Box::new(gtk::Orientation::Vertical, 4);
    details.set_margin_start(24);
    // every control of the group is disabled, not hidden, while the switch is off
    deps.add(Row::FlashDuration, &details);

    let duration = prefs.flash_duration_millis.clone();
    details.append(&slider(
        strings::PREF_FLASH_DURATION,
        FLASH_SLIDER_RANGE,
        flash_slider_from_millis(prefs.flash_duration_millis.get()),
        |v| strings::flash_duration_summary(flash_millis_from_slider(v)),
        move |v| save(&duration, flash_millis_from_slider(v)),
    ));
    let interval = prefs.flash_page_interval.clone();
    details.append(&slider(
        strings::PREF_FLASH_PAGE_INTERVAL,
        FLASH_INTERVAL_RANGE,
        prefs.flash_page_interval.get(),
        strings::pages,
        move |v| save(&interval, v),
    ));
    details.append(&gtk::Label::new(Some(strings::PREF_FLASH_WITH)));
    let colour_options: Vec<(&str, i32)> = FLASH_CHIPS.iter().enumerate().map(|(i, (l, _))| (*l, i as i32)).collect();
    let current = FLASH_CHIPS.iter().position(|(_, c)| *c == prefs.flash_color.get()).unwrap_or(0) as i32;
    let colour = prefs.flash_color.clone();
    details.append(&chips(&colour_options, current, move |v| save(&colour, FLASH_CHIPS[v as usize].1)));

    root.append(&flash);
    root.append(&details);
    root
}

/// The blend mode chips: the stored index and the label, in the order of `COLOR_FILTER_MODE`.
pub const BLEND_CHIPS: [(&str, i32); 6] = [
    (strings::LABEL_DEFAULT, 0),
    (strings::FILTER_MODE_MULTIPLY, 1),
    (strings::FILTER_MODE_SCREEN, 2),
    (strings::FILTER_MODE_OVERLAY, 3),
    (strings::FILTER_MODE_LIGHTEN, 4),
    (strings::FILTER_MODE_DARKEN, 5),
];

const CHANNELS: [(&str, Channel); 4] = [
    (strings::COLOR_FILTER_R_VALUE, Channel::Red),
    (strings::COLOR_FILTER_G_VALUE, Channel::Green),
    (strings::COLOR_FILTER_B_VALUE, Channel::Blue),
    (strings::COLOR_FILTER_A_VALUE, Channel::Alpha),
];

fn number_text(value: i32) -> String {
    value.to_string()
}

/// The custom filter tab (T-043a-048). The brightness slider shows only while custom brightness is on; the four
/// colour sliders and the blend chips only while the colour filter is on. Grayscale and invert are always shown.
pub fn filter_tab(prefs: &ReaderPrefs) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 4);
    root.set_margin_start(16);
    root.set_margin_end(16);

    let brightness_check = check(strings::PREF_CUSTOM_BRIGHTNESS, &prefs.custom_brightness);
    // a positive value would need the display backlight, which the reader cannot reach (E-BRIGHT-2)
    let unavailable = gtk::Label::new(Some(strings::CUSTOM_BRIGHTNESS_UNAVAILABLE));
    unavailable.set_wrap(true);
    unavailable.set_xalign(0.0);
    unavailable.add_css_class("dim-label");
    unavailable.set_visible(prefs.custom_brightness.get() && prefs.custom_brightness_value.get() > 0);
    let brightness_value = prefs.custom_brightness_value.clone();
    let (note_on_slide, check_on_slide) = (unavailable.clone(), brightness_check.clone());
    let brightness = slider(
        strings::PREF_CUSTOM_BRIGHTNESS,
        BRIGHTNESS_RANGE,
        prefs.custom_brightness_value.get(),
        number_text,
        move |v| {
            save(&brightness_value, v);
            note_on_slide.set_visible(check_on_slide.is_active() && v > 0);
        },
    );
    brightness.set_visible(prefs.custom_brightness.get());
    let shown = brightness.clone();
    let (note_on_toggle, value_on_toggle) = (unavailable.clone(), prefs.custom_brightness_value.clone());
    brightness_check.connect_toggled(move |b| {
        shown.set_visible(b.is_active());
        note_on_toggle.set_visible(b.is_active() && value_on_toggle.get() > 0);
    });
    root.append(&brightness_check);
    root.append(&brightness);
    root.append(&unavailable);

    let filter_check = check(strings::PREF_CUSTOM_COLOR_FILTER, &prefs.color_filter);
    let colours = gtk::Box::new(gtk::Orientation::Vertical, 4);
    for (label, channel) in CHANNELS {
        let value = prefs.color_filter_value.clone();
        colours.append(&slider(
            label,
            CHANNEL_RANGE,
            overlay::channel_get(prefs.color_filter_value.get(), channel),
            number_text,
            // each write replaces one byte and keeps the other three of the stored value
            move |v| save(&value, overlay::channel_set(value.get(), channel, v)),
        ));
    }
    colours.append(&gtk::Label::new(Some(strings::PREF_COLOR_FILTER_MODE)));
    let mode = prefs.color_filter_mode.clone();
    colours.append(&chips(&BLEND_CHIPS, prefs.color_filter_mode.get(), move |v| save(&mode, v)));
    colours.set_visible(prefs.color_filter.get());
    let shown = colours.clone();
    filter_check.connect_toggled(move |b| shown.set_visible(b.is_active()));
    root.append(&filter_check);
    root.append(&colours);

    root.append(&check(strings::PREF_GRAYSCALE, &prefs.grayscale));
    root.append(&check(strings::PREF_INVERTED_COLORS, &prefs.inverted_colors));
    root
}

/// The whole window body: a notebook with the reading mode, general and custom filter tabs, in that order.
pub fn notebook(prefs: &ReaderPrefs, mode: ReadingMode, orientation: ReaderOrientation, choices: MangaChoices) -> gtk::Notebook {
    let notebook = gtk::Notebook::new();
    let reading = gtk::ScrolledWindow::new();
    reading.set_child(Some(&reading_tab(prefs, mode, orientation, choices)));
    let general = gtk::ScrolledWindow::new();
    general.set_child(Some(&general_tab(prefs)));
    notebook.append_page(&reading, Some(&gtk::Label::new(Some(strings::PREF_CATEGORY_READING_MODE))));
    notebook.append_page(&general, Some(&gtk::Label::new(Some(strings::PREF_CATEGORY_GENERAL))));
    let filter = gtk::ScrolledWindow::new();
    filter.set_child(Some(&filter_tab(prefs)));
    notebook.append_page(&filter, Some(&gtk::Label::new(Some(strings::CUSTOM_FILTER))));
    notebook
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    fn prefs() -> ReaderPrefs {
        ReaderPrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn split_writes_the_split_key_and_its_invert_writes_the_invert_key() {
        let p = prefs();
        assert_eq!(dual_page_prefs(&p, false).split.key(), "pref_dual_page_split");
        assert_eq!(dual_page_prefs(&p, false).invert.key(), "pref_dual_page_invert");
        assert_eq!(dual_page_prefs(&p, true).split.key(), "pref_dual_page_split_webtoon");
        assert_eq!(dual_page_prefs(&p, true).invert.key(), "pref_dual_page_invert_webtoon");
        assert_ne!(dual_page_prefs(&p, false).split.key(), dual_page_prefs(&p, false).rotate_invert.key());
    }

    #[test]
    fn a_split_write_leaves_the_other_dual_page_keys_alone() {
        let p = prefs();
        dual_page_prefs(&p, false).split.set(true).unwrap();
        assert!(p.dual_page_split_paged.get());
        assert!(!p.dual_page_invert_paged.get());
        assert!(!p.dual_page_rotate_to_fit_invert.get());
    }

    #[test]
    fn flash_duration_is_stored_in_milliseconds_and_slides_in_hundreds() {
        assert_eq!(flash_slider_from_millis(100), 1);
        assert_eq!(flash_slider_from_millis(1500), 15);
        assert_eq!(flash_millis_from_slider(15), 1500);
        assert_eq!(strings::flash_duration_summary(flash_millis_from_slider(3)), "300 ms");
    }

    #[test]
    fn the_general_defaults_match_the_spec() {
        let p = prefs();
        assert_eq!(p.reader_theme.get(), 1, "black background");
        assert!(p.show_page_number.get());
        assert!(p.fullscreen.get());
        assert!(p.keep_screen_on.get());
        assert!(p.read_with_long_tap.get());
        assert!(p.always_show_chapter_transition.get());
        assert!(p.page_transitions.get());
        assert!(!p.flash_on_page_change.get());
        assert_eq!(p.flash_duration_millis.get(), 100);
        assert_eq!(p.flash_page_interval.get(), 1);
        assert_eq!(p.flash_color.get(), FlashColor::BLACK);
    }

    #[test]
    fn background_chips_follow_the_spec_order_and_values() {
        let values: Vec<i32> = THEME_CHIPS.iter().map(|(_, v)| *v).collect();
        assert_eq!(values, [1, 2, 0, 3]);
        assert_eq!(THEME_CHIPS[0].0, "Black");
    }

    #[test]
    fn the_pager_and_the_webtoon_family_follow_the_resolved_mode() {
        assert!(is_webtoon_family(ReadingMode::Webtoon, 2));
        assert!(is_webtoon_family(ReadingMode::ContinuousVertical, 2));
        assert!(!is_webtoon_family(ReadingMode::RightToLeft, 4));
        assert!(is_webtoon_family(ReadingMode::Default, 4), "a default manga follows the preference");
    }

    #[test]
    fn exactly_one_viewer_block_shows_for_each_mode() {
        assert_eq!(block_visibility(ReadingMode::RightToLeft, 2), (true, false));
        assert_eq!(block_visibility(ReadingMode::Webtoon, 2), (false, true));
        assert_eq!(block_visibility(ReadingMode::ContinuousVertical, 2), (false, true));
        // a chip back to Default follows the default reading mode preference
        assert_eq!(block_visibility(ReadingMode::Default, ReadingMode::Webtoon.flag_value()), (false, true));
        assert_eq!(block_visibility(ReadingMode::Default, ReadingMode::LeftToRight.flag_value()), (true, false));
    }

    #[test]
    fn the_tab_titles_and_blend_chips_follow_the_spec() {
        assert_eq!(strings::CUSTOM_FILTER, "Custom filter");
        let labels: Vec<&str> = BLEND_CHIPS.iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, ["Default", "Multiply", "Screen", "Overlay", "Dodge / Lighten", "Burn / Darken"]);
        let stored: Vec<i32> = BLEND_CHIPS.iter().map(|(_, v)| *v).collect();
        assert_eq!(stored, [0, 1, 2, 3, 4, 5]);
        assert_eq!(BRIGHTNESS_RANGE, (-75, 100));
    }

    #[test]
    fn a_colour_slider_write_keeps_the_other_three_bytes() {
        let p = prefs();
        p.color_filter_value.set(0x1122_3344_u32 as i32).unwrap();
        save(&p.color_filter_value, overlay::channel_set(p.color_filter_value.get(), Channel::Red, 0xAA));
        assert_eq!(p.color_filter_value.get() as u32, 0x11AA_3344);
    }

    #[test]
    fn a_saved_setting_reads_back_at_once_and_reaches_the_change_feed() {
        let store = PrefStore::in_memory();
        let changes = store.subscribe();
        let p = ReaderPrefs::new(store);
        save(&p.show_page_number, false);
        assert!(!p.show_page_number.get(), "the store's memory is updated before the call returns");
        assert_eq!(changes.try_recv().unwrap().key.as_deref(), Some("pref_show_page_number_key"));
    }

    #[test]
    fn disabled_taps_is_the_last_zone_and_the_side_padding_is_a_percent() {
        assert_eq!(TAP_ZONE_CHIPS[TAP_ZONE_DISABLED as usize], "Disabled");
        assert_eq!(side_padding_text(10), "10%");
        assert_eq!(SIDE_PADDING_RANGE, (0, 25));
    }
}
