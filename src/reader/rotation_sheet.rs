//! The rotation sheet (T-43a-044, `OrientationSelectDialog.kt`). Six choices, the selection is local until Apply,
//! and Apply always toasts the chosen label. The sheet says that rotation has no effect on this platform (E-ORI-1):
//! the value is stored and round-trips through backup, but no window rotation follows.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::reader::ReaderOrientation;

use super::strings;

/// The choices the sheet offers, in order. Default is not one of them: it is what a manga without its own value has.
pub const CHOICES: [ReaderOrientation; 6] = [
    ReaderOrientation::Free,
    ReaderOrientation::Portrait,
    ReaderOrientation::Landscape,
    ReaderOrientation::LockedPortrait,
    ReaderOrientation::LockedLandscape,
    ReaderOrientation::ReversePortrait,
];

/// The label of a choice in the sheet and in its toast.
pub fn label(orientation: ReaderOrientation) -> &'static str {
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

/// The selection the sheet opens with: the manga's own value when it is one of the choices, else nothing.
pub fn initial_selection(saved: ReaderOrientation) -> Option<ReaderOrientation> {
    CHOICES.contains(&saved).then_some(saved)
}

/// What Apply does with a selection: nothing without a choice, else the value and its toast label.
pub fn apply_outcome(selected: Option<ReaderOrientation>) -> Option<(ReaderOrientation, &'static str)> {
    selected.map(|orientation| (orientation, label(orientation)))
}

/// The sheet. `on_apply` gets the chosen value and the toast text; the caller stores the value and dismisses.
pub fn build(saved: ReaderOrientation, on_apply: Rc<dyn Fn(ReaderOrientation, &'static str)>) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_start(16);
    column.set_margin_end(16);
    column.set_margin_top(12);
    column.set_margin_bottom(12);
    let title = gtk::Label::new(Some(strings::ROTATION_TYPE));
    title.add_css_class("title-4");
    column.append(&title);

    let selected: Rc<Cell<Option<ReaderOrientation>>> = Rc::new(Cell::new(initial_selection(saved)));
    let mut group: Option<gtk::CheckButton> = None;
    for orientation in CHOICES {
        let button = gtk::CheckButton::with_label(label(orientation));
        match &group {
            Some(first) => button.set_group(Some(first)),
            None => group = Some(button.clone()),
        }
        button.set_active(selected.get() == Some(orientation));
        let selected = Rc::clone(&selected);
        button.connect_toggled(move |b| {
            if b.is_active() {
                selected.set(Some(orientation));
            }
        });
        column.append(&button);
    }

    let note = gtk::Label::new(Some(strings::ROTATION_NO_EFFECT));
    note.set_wrap(true);
    note.add_css_class("dim-label");
    column.append(&note);

    let apply = gtk::Button::with_label(strings::ACTION_APPLY);
    apply.add_css_class("suggested-action");
    apply.set_halign(gtk::Align::End);
    apply.connect_clicked(move |_| {
        if let Some((orientation, toast)) = apply_outcome(selected.get()) {
            on_apply(orientation, toast);
        }
    });
    column.append(&apply);
    column
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_offers_six_choices_in_order_and_no_default() {
        assert_eq!(CHOICES.len(), 6);
        assert_eq!(label(CHOICES[0]), "Free");
        assert_eq!(label(CHOICES[5]), "Reverse portrait");
        assert!(!CHOICES.contains(&ReaderOrientation::Default));
    }

    #[test]
    fn a_saved_choice_is_selected_and_the_default_starts_with_nothing() {
        assert_eq!(initial_selection(ReaderOrientation::LockedLandscape), Some(ReaderOrientation::LockedLandscape));
        assert_eq!(initial_selection(ReaderOrientation::Default), None);
    }

    #[test]
    fn apply_toasts_the_label_of_the_chosen_value_and_does_nothing_without_one() {
        assert_eq!(apply_outcome(Some(ReaderOrientation::Portrait)), Some((ReaderOrientation::Portrait, "Portrait")));
        assert_eq!(apply_outcome(None), None);
    }
}
