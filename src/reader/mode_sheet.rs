//! The reading-mode sheet (T-043a-043). A choice is local until Apply. Revert is offered only when the manga
//! has a mode of its own. The toast rule follows E-UI-4: Apply toasts the mode label only when the
//! `show_reading_mode` preference is off.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::reader::ReadingMode;

use super::bottom_bar::mode_icon;
use super::strings;

/// The five modes the sheet offers. Default is not a choice here: it is what Revert writes.
pub const SHEET_MODES: [ReadingMode; 5] = [
    ReadingMode::LeftToRight,
    ReadingMode::RightToLeft,
    ReadingMode::Vertical,
    ReadingMode::Webtoon,
    ReadingMode::ContinuousVertical,
];

/// The label of a mode, in the sheet and in the toast.
pub fn mode_label(mode: ReadingMode) -> &'static str {
    match mode {
        ReadingMode::Default => strings::LABEL_DEFAULT,
        ReadingMode::LeftToRight => strings::LEFT_TO_RIGHT_VIEWER,
        ReadingMode::RightToLeft => strings::RIGHT_TO_LEFT_VIEWER,
        ReadingMode::Vertical => strings::VERTICAL_VIEWER,
        ReadingMode::Webtoon => strings::WEBTOON_VIEWER,
        ReadingMode::ContinuousVertical => strings::VERTICAL_PLUS_VIEWER,
    }
}

/// The toast for an Apply: the mode label, only when the reading-mode toast preference is off.
pub fn apply_toast(show_reading_mode: bool, mode: ReadingMode) -> Option<&'static str> {
    if show_reading_mode { None } else { Some(mode_label(mode)) }
}

/// The selection, starting from the manga's saved mode. A Default manga starts with nothing selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModeChoice {
    saved: ReadingMode,
    selected: Option<ReadingMode>,
}

/// What the sheet's buttons do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Apply { mode: ReadingMode, toast: Option<&'static str> },
    Revert,
}

impl ModeChoice {
    pub fn new(saved: ReadingMode) -> Self {
        let selected = SHEET_MODES.contains(&saved).then_some(saved);
        Self { saved, selected }
    }

    pub fn selected(&self) -> Option<ReadingMode> {
        self.selected
    }

    /// Selecting only changes the choice; nothing is saved until Apply.
    pub fn select(&mut self, mode: ReadingMode) {
        if SHEET_MODES.contains(&mode) {
            self.selected = Some(mode);
        }
    }

    /// Revert is shown only when the manga's mode is not Default.
    pub fn can_revert(&self) -> bool {
        self.saved != ReadingMode::Default
    }

    /// Apply writes the selection. Nothing selected means nothing to apply.
    pub fn apply(&self, show_reading_mode: bool) -> Option<Outcome> {
        self.selected.map(|mode| Outcome::Apply { mode, toast: apply_toast(show_reading_mode, mode) })
    }

    /// Keyboard movement through the modes: the index moves and is clamped at both ends.
    pub fn move_by(&mut self, delta: i32) {
        let current = self.selected.and_then(|m| SHEET_MODES.iter().position(|x| *x == m)).map_or(0, |i| i as i32);
        let next = (current + delta).clamp(0, SHEET_MODES.len() as i32 - 1) as usize;
        self.selected = Some(SHEET_MODES[next]);
    }
}

/// The selection step of an arrow key: left and up go back, right and down go forward.
pub fn key_delta(key: gtk::gdk::Key) -> Option<i32> {
    use gtk::gdk::Key;
    match key {
        Key::Left | Key::Up => Some(-1),
        Key::Right | Key::Down => Some(1),
        _ => None,
    }
}

/// Whether the key is an Enter key.
pub fn is_enter(key: gtk::gdk::Key) -> bool {
    key == gtk::gdk::Key::Return || key == gtk::gdk::Key::KP_Enter
}

/// Builds the sheet content. `on_done` runs once with the outcome; the caller dismisses the sheet.
pub fn build(saved: ReadingMode, show_reading_mode: bool, on_done: Rc<dyn Fn(Outcome)>) -> gtk::Box {
    let choice = Rc::new(RefCell::new(ModeChoice::new(saved)));
    let root = gtk::Box::new(gtk::Orientation::Vertical, 12);
    root.set_widget_name("reading-mode-sheet");
    let title = gtk::Label::new(Some(strings::PREF_CATEGORY_READING_MODE));
    title.add_css_class("heading");
    title.set_xalign(0.0);
    root.append(&title);

    let grid = gtk::FlowBox::new();
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_homogeneous(true);
    grid.set_max_children_per_line(5);
    let mut group: Option<gtk::ToggleButton> = None;
    let mut toggles: Vec<gtk::ToggleButton> = Vec::new();
    for mode in SHEET_MODES {
        let item = gtk::ToggleButton::new();
        let cell = gtk::Box::new(gtk::Orientation::Vertical, 4);
        cell.append(&gtk::Image::from_icon_name(mode_icon(mode)));
        cell.append(&gtk::Label::new(Some(mode_label(mode))));
        item.set_child(Some(&cell));
        item.set_active(choice.borrow().selected() == Some(mode));
        match &group {
            Some(first) => item.set_group(Some(first)),
            None => group = Some(item.clone()),
        }
        let chosen = Rc::clone(&choice);
        item.connect_toggled(move |b| {
            if b.is_active() {
                chosen.borrow_mut().select(mode);
            }
        });
        grid.append(&item);
        // the toggle takes the focus, not its flow-box cell, so Space selects and the arrows are ours
        if let Some(cell) = item.parent() {
            cell.set_focusable(false);
        }
        toggles.push(item);
    }
    root.append(&grid);

    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.set_halign(gtk::Align::End);
    if choice.borrow().can_revert() {
        let revert = gtk::Button::with_label(strings::ACTION_REVERT_TO_DEFAULT);
        let done = Rc::clone(&on_done);
        revert.connect_clicked(move |_| done(Outcome::Revert));
        footer.append(&revert);
    }
    let apply = gtk::Button::with_label(strings::ACTION_APPLY);
    apply.add_css_class("suggested-action");
    {
        let chosen = Rc::clone(&choice);
        let done = Rc::clone(&on_done);
        apply.connect_clicked(move |_| {
            if let Some(outcome) = chosen.borrow().apply(show_reading_mode) {
                done(outcome);
            }
        });
    }
    footer.append(&apply);
    root.append(&footer);
    // Keys for the whole sheet, in the capture phase so the focused toggle does not take them first: the arrows move
    // the selection and the focus, Enter applies. Space is left to the focused toggle. A focused Revert or Apply
    // button keeps its own Enter.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let chosen = Rc::clone(&choice);
        let done = Rc::clone(&on_done);
        keys.connect_key_pressed(move |controller, key, _, _| {
            let focus = controller.widget().and_then(|w| w.root()).and_then(|r| r.focus());
            if let Some(delta) = key_delta(key) {
                chosen.borrow_mut().move_by(delta);
                let picked = chosen.borrow().selected().and_then(|m| SHEET_MODES.iter().position(|x| *x == m));
                if let Some(toggle) = picked.and_then(|i| toggles.get(i)) {
                    toggle.set_active(true);
                    toggle.grab_focus();
                }
                return gtk::glib::Propagation::Stop;
            }
            if !is_enter(key) || focus.is_some_and(|f| f.is::<gtk::Button>() && !f.is::<gtk::ToggleButton>()) {
                return gtk::glib::Propagation::Proceed;
            }
            let outcome = chosen.borrow().apply(show_reading_mode);
            if let Some(outcome) = outcome {
                done(outcome);
            }
            gtk::glib::Propagation::Stop
        });
    }
    root.add_controller(keys);
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_offers_five_modes_without_default() {
        assert_eq!(SHEET_MODES.len(), 5);
        assert!(!SHEET_MODES.contains(&ReadingMode::Default));
    }

    #[test]
    fn selection_is_local_until_apply() {
        let mut c = ModeChoice::new(ReadingMode::LeftToRight);
        c.select(ReadingMode::Webtoon);
        assert_eq!(c.selected(), Some(ReadingMode::Webtoon));
        assert_eq!(c.apply(true), Some(Outcome::Apply { mode: ReadingMode::Webtoon, toast: None }));
    }

    #[test]
    fn the_toast_shows_only_when_the_reading_mode_toast_is_off() {
        let mut c = ModeChoice::new(ReadingMode::Default);
        c.select(ReadingMode::Vertical);
        assert_eq!(c.apply(false), Some(Outcome::Apply { mode: ReadingMode::Vertical, toast: Some(VERTICAL_LABEL) }));
        assert_eq!(c.apply(true), Some(Outcome::Apply { mode: ReadingMode::Vertical, toast: None }));
    }

    const VERTICAL_LABEL: &str = strings::VERTICAL_VIEWER;

    #[test]
    fn revert_is_offered_only_for_a_manga_with_its_own_mode() {
        assert!(!ModeChoice::new(ReadingMode::Default).can_revert());
        assert!(ModeChoice::new(ReadingMode::RightToLeft).can_revert());
    }

    #[test]
    fn a_default_manga_starts_with_nothing_selected_and_apply_does_nothing() {
        let c = ModeChoice::new(ReadingMode::Default);
        assert_eq!(c.selected(), None);
        assert_eq!(c.apply(true), None);
    }

    #[test]
    fn the_arrow_keys_step_and_enter_is_recognised() {
        use gtk::gdk::Key;
        assert_eq!(key_delta(Key::Left), Some(-1));
        assert_eq!(key_delta(Key::Down), Some(1));
        assert_eq!(key_delta(Key::space), None);
        assert!(is_enter(Key::KP_Enter) && is_enter(Key::Return) && !is_enter(Key::space));
    }

    #[test]
    fn arrow_keys_move_and_clamp_at_both_ends() {
        let mut c = ModeChoice::new(ReadingMode::LeftToRight);
        c.move_by(-1);
        assert_eq!(c.selected(), Some(ReadingMode::LeftToRight));
        c.move_by(10);
        assert_eq!(c.selected(), Some(ReadingMode::ContinuousVertical));
    }
}
