//! Headless integration tests for the reader chrome (T-43a-085): the navigator with its RTL mirror, the bottom bar with its
//! crop toggle, the top bar with the bookmark label, the reading-mode sheet, and the settings split binding (E-UI-3). They
//! build real widgets under [`crate::testing::with_display`], find the buttons by walking the widget tree, and skip when
//! there is no display. CI runs them under a virtual display (`xvfb-run`) so they do not skip there.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::reader::{ReaderPrefs, ReadingMode};
use kuro_prefs::store::PrefStore;

use super::bottom_bar::{BottomBar, bar_buttons};
use super::chrome::{TopBar, TopBarActions, build_top_bar};
use super::mode_sheet::{self, Outcome, SHEET_MODES};
use super::navigator::{Navigator, NavigatorState};
use super::settings_window::{self, MangaChoices};
use super::strings;

/// Every button under `root`, in tree order.
fn buttons_under(root: &gtk::Widget) -> Vec<gtk::Button> {
    let mut found = Vec::new();
    collect(root, &mut found);
    found
}

fn collect(widget: &gtk::Widget, found: &mut Vec<gtk::Button>) {
    if let Some(button) = widget.downcast_ref::<gtk::Button>() {
        found.push(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        collect(&next, found);
        child = next.next_sibling();
    }
}

/// The index of the button whose tooltip is `text`, in tree order.
fn position_of(buttons: &[gtk::Button], text: &str) -> Option<usize> {
    buttons.iter().position(|b| b.tooltip_text().as_deref() == Some(text))
}

fn navigator_state(rtl: bool, previous_enabled: bool, next_enabled: bool) -> NavigatorState {
    NavigatorState { total: 10, current_page: 3, previous_enabled, next_enabled, rtl, loading: false }
}

#[test]
fn the_navigator_puts_next_on_the_left_in_rtl_and_disables_the_missing_chapter() {
    crate::testing::with_display(|| {
        let navigator = Navigator::new();
        navigator.update(&navigator_state(true, true, false), (800, 600));
        let buttons = buttons_under(navigator.root.upcast_ref());
        let next = position_of(&buttons, strings::ACTION_NEXT_CHAPTER).expect("a next button");
        let previous = position_of(&buttons, strings::ACTION_PREVIOUS_CHAPTER).expect("a previous button");
        assert!(next < previous, "in RTL the next chapter button is on the left");
        assert!(!buttons[next].is_sensitive(), "no next chapter, so the button is disabled");
        assert!(buttons[previous].is_sensitive());
    });
}

#[test]
fn the_navigator_keeps_previous_on_the_left_in_ltr() {
    crate::testing::with_display(|| {
        let navigator = Navigator::new();
        navigator.update(&navigator_state(false, true, true), (800, 600));
        let buttons = buttons_under(navigator.root.upcast_ref());
        let next = position_of(&buttons, strings::ACTION_NEXT_CHAPTER).expect("a next button");
        let previous = position_of(&buttons, strings::ACTION_PREVIOUS_CHAPTER).expect("a previous button");
        assert!(previous < next);
        assert!(buttons[next].is_sensitive() && buttons[previous].is_sensitive());
    });
}

#[test]
fn the_bottom_bar_shows_the_crop_button_toggled_while_crop_is_on() {
    crate::testing::with_display(|| {
        let bar = BottomBar::new();
        bar.update(ReadingMode::LeftToRight, true);
        let buttons = buttons_under(bar.root.upcast_ref());
        let crop = bar_buttons(ReadingMode::LeftToRight, true)[1].description;
        let index = position_of(&buttons, crop).expect("the crop button");
        assert!(buttons[index].has_css_class("toggled"));

        bar.update(ReadingMode::LeftToRight, false);
        assert!(!buttons[index].has_css_class("toggled"), "the same button follows the new state");
    });
}

#[test]
fn the_top_bar_labels_the_bookmark_by_its_state_and_has_a_named_back_button() {
    crate::testing::with_display(|| {
        let bar = TopBar { manga_title: "Manga".into(), chapter_name: "Chapter 1".into(), bookmarked: true, is_http_source: false };
        let actions = TopBarActions {
            on_back: Box::new(|| {}),
            on_open_manga: Box::new(|| {}),
            on_bookmark: Box::new(|| {}),
            on_overflow: Box::new(|_| {}),
        };
        let revealer = build_top_bar(&bar, actions);
        let buttons = buttons_under(revealer.upcast_ref());
        assert!(position_of(&buttons, strings::ACTION_REMOVE_BOOKMARK).is_some(), "a bookmarked chapter offers to unbookmark");
        assert!(position_of(&buttons, "Back").is_some(), "the back button is named");
    });
}

/// Every widget of type `T` under `root`, in tree order.
fn all_of<T: IsA<gtk::Widget>>(root: &gtk::Widget) -> Vec<T> {
    let mut found = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(widget) = stack.pop() {
        if let Some(it) = widget.downcast_ref::<T>() {
            found.push(it.clone());
        }
        let mut children = Vec::new();
        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            children.push(next);
        }
        stack.extend(children.into_iter().rev());
    }
    found
}

fn button_labelled(root: &gtk::Widget, label: &str) -> Option<gtk::Button> {
    all_of::<gtk::Button>(root).into_iter().find(|b| b.label().as_deref() == Some(label))
}

/// A reading-mode sheet whose outcomes are collected.
fn sheet(saved: ReadingMode, show_reading_mode: bool) -> (gtk::Box, Rc<RefCell<Vec<Outcome>>>) {
    let outcomes: Rc<RefCell<Vec<Outcome>>> = Rc::default();
    let sink = Rc::clone(&outcomes);
    (mode_sheet::build(saved, show_reading_mode, Rc::new(move |outcome| sink.borrow_mut().push(outcome))), outcomes)
}

#[test]
fn the_reading_mode_sheet_selects_one_mode_and_apply_reports_it_with_the_toast_rule() {
    crate::testing::with_display(|| {
        let (sheet, outcomes) = sheet(ReadingMode::LeftToRight, false);
        let toggles = all_of::<gtk::ToggleButton>(sheet.upcast_ref());
        assert_eq!(toggles.len(), SHEET_MODES.len(), "five modes, Default is not one");
        assert!(toggles[0].is_active(), "the saved mode starts selected");
        let webtoon = SHEET_MODES.iter().position(|m| *m == ReadingMode::Webtoon).expect("webtoon is offered");
        toggles[webtoon].set_active(true);
        assert!(!toggles[0].is_active(), "one mode at a time");
        assert!(outcomes.borrow().is_empty(), "a choice is local until Apply");
        button_labelled(sheet.upcast_ref(), strings::ACTION_APPLY).expect("an Apply button").emit_clicked();
        assert_eq!(
            *outcomes.borrow(),
            [Outcome::Apply { mode: ReadingMode::Webtoon, toast: Some(strings::WEBTOON_VIEWER) }],
            "the mode label is toasted only while the reading-mode toast is off"
        );

        let (quiet, outcomes) = self::sheet(ReadingMode::RightToLeft, true);
        button_labelled(quiet.upcast_ref(), strings::ACTION_APPLY).expect("an Apply button").emit_clicked();
        assert_eq!(*outcomes.borrow(), [Outcome::Apply { mode: ReadingMode::RightToLeft, toast: None }]);
    });
}

#[test]
fn the_reading_mode_sheet_offers_revert_only_for_a_manga_with_its_own_mode() {
    crate::testing::with_display(|| {
        let (own, outcomes) = sheet(ReadingMode::Vertical, true);
        button_labelled(own.upcast_ref(), strings::ACTION_REVERT_TO_DEFAULT).expect("Revert for a saved mode").emit_clicked();
        assert_eq!(*outcomes.borrow(), [Outcome::Revert]);

        let (default, outcomes) = sheet(ReadingMode::Default, true);
        assert!(button_labelled(default.upcast_ref(), strings::ACTION_REVERT_TO_DEFAULT).is_none(), "nothing to revert");
        assert!(all_of::<gtk::ToggleButton>(default.upcast_ref()).iter().all(|t| !t.is_active()), "nothing is selected");
        button_labelled(default.upcast_ref(), strings::ACTION_APPLY).expect("an Apply button").emit_clicked();
        assert!(outcomes.borrow().is_empty(), "Apply with nothing selected does nothing");
    });
}

fn settings_tab(prefs: &ReaderPrefs) -> gtk::Box {
    let choices = MangaChoices { on_mode: Rc::new(|_| {}) };
    settings_window::reading_tab(prefs, ReadingMode::LeftToRight, choices)
}

/// The nth check button with `label` in the tab: 0 is the pager block, 1 the webtoon block.
fn check_labelled(tab: &gtk::Box, label: &str, nth: usize) -> gtk::CheckButton {
    all_of::<gtk::CheckButton>(tab.upcast_ref()).into_iter().filter(|c| c.label().as_deref() == Some(label)).nth(nth).expect("the check")
}

#[test]
fn the_split_check_writes_the_split_key_and_the_invert_check_the_invert_key_for_each_family() {
    crate::testing::with_display(|| {
        let prefs = ReaderPrefs::new(PrefStore::in_memory());
        let tab = settings_tab(&prefs);
        let split = |nth| check_labelled(&tab, strings::PREF_DUAL_PAGE_SPLIT, nth);
        let invert = |nth| check_labelled(&tab, strings::PREF_DUAL_PAGE_INVERT, nth);

        split(0).set_active(true);
        assert!(prefs.dual_page_split_paged.get(), "the paged split check writes pref_dual_page_split");
        assert!(!prefs.dual_page_invert_paged.get(), "and not the invert key (E-UI-3)");
        invert(0).set_active(true);
        assert!(prefs.dual_page_invert_paged.get(), "the paged invert check writes pref_dual_page_invert");
        assert!(prefs.dual_page_split_paged.get(), "and leaves the split key alone");
        assert!(!prefs.dual_page_split_webtoon.get() && !prefs.dual_page_invert_webtoon.get(), "the webtoon keys are untouched");

        split(1).set_active(true);
        assert!(prefs.dual_page_split_webtoon.get() && !prefs.dual_page_invert_webtoon.get());
        invert(1).set_active(true);
        assert!(prefs.dual_page_invert_webtoon.get());
        invert(0).set_active(false);
        assert!(!prefs.dual_page_invert_paged.get() && prefs.dual_page_invert_webtoon.get(), "each control writes only its own key");
    });
}

#[test]
fn the_split_and_rotate_checks_exclude_each_other() {
    crate::testing::with_display(|| {
        let prefs = ReaderPrefs::new(PrefStore::in_memory());
        let tab = settings_tab(&prefs);
        check_labelled(&tab, strings::PREF_DUAL_PAGE_SPLIT, 0).set_active(true);
        check_labelled(&tab, strings::PREF_PAGE_ROTATE, 0).set_active(true);
        assert!(prefs.dual_page_rotate_to_fit.get() && !prefs.dual_page_split_paged.get(), "rotating turns the split off");
        check_labelled(&tab, strings::PREF_DUAL_PAGE_SPLIT, 0).set_active(true);
        assert!(prefs.dual_page_split_paged.get() && !prefs.dual_page_rotate_to_fit.get(), "and splitting turns the rotate off");
    });
}
