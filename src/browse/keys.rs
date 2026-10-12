//! The keyboard map of the browse pages (T-40a-035, U§9). Pure: the views apply the answer. Ctrl+F focuses the search,
//! Escape closes the search and then leaves the page, and Ctrl+1 to Ctrl+6 pick a Browse sub-tab. In the migrate dialog Ctrl+Enter migrates and Ctrl+Shift+Enter copies.

use gtk::gdk::{Key, ModifierType};
use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

/// The shortcuts of the browse pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowseShortcut {
    FocusSearch,
    /// Ctrl+1 is 1, and so on to Ctrl+6. The number is the position of the sub-tab, counted from one.
    SubTab(u8),
}

/// The shortcuts of the browse pages, when no text entry is taking the key.
pub fn browse_shortcut(key: Key, modifiers: ModifierType) -> Option<BrowseShortcut> {
    let ctrl = modifiers.contains(ModifierType::CONTROL_MASK);
    let shift = modifiers.contains(ModifierType::SHIFT_MASK);
    if !ctrl || shift {
        return None;
    }
    match key.to_unicode()? {
        'f' | 'F' => Some(BrowseShortcut::FocusSearch),
        digit @ '1'..='6' => Some(BrowseShortcut::SubTab(digit as u8 - b'0')),
        _ => None,
    }
}

/// The migrate dialog's shortcuts (U§9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrateShortcut {
    Migrate,
    Copy,
}

pub fn migrate_shortcut(key: Key, modifiers: ModifierType) -> Option<MigrateShortcut> {
    let ctrl = modifiers.contains(ModifierType::CONTROL_MASK);
    let shift = modifiers.contains(ModifierType::SHIFT_MASK);
    let enter = matches!(key, Key::Return | Key::KP_Enter | Key::ISO_Enter);
    match (enter, ctrl, shift) {
        (true, true, false) => Some(MigrateShortcut::Migrate),
        (true, true, true) => Some(MigrateShortcut::Copy),
        _ => None,
    }
}

/// What Escape does on a browse page: close the open search first, then leave the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeStep {
    CloseSearch,
    Leave,
}

pub fn escape_step(search_open: bool) -> EscapeStep {
    if search_open { EscapeStep::CloseSearch } else { EscapeStep::Leave }
}

/// The lines of the help overlay for the browse pages: the keys, and what they do (U§9). Every shortcut above is
/// here, so the list cannot fall behind the table.
pub fn help_entries() -> Vec<(String, String)> {
    let line = |keys: &str, what: MsgKey| (keys.to_owned(), tr(what));
    vec![
        line("Ctrl+F", MsgKey::action_search),
        line("Escape", MsgKey::action_cancel),
        line("Ctrl+1 \u{2026} Ctrl+6", MsgKey::label_sources),
        line("Ctrl+Enter", MsgKey::migrate),
        line("Ctrl+Shift+Enter", MsgKey::copy),
    ]
}

/// Puts the migrate dialog's shortcuts on `dialog`: Ctrl+Enter migrates, Ctrl+Shift+Enter copies.
pub fn attach_migrate_keys(dialog: &impl IsA<gtk::Widget>, on_migrate: impl Fn() + 'static, on_copy: impl Fn() + 'static) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, modifiers| match migrate_shortcut(key, modifiers) {
        Some(MigrateShortcut::Migrate) => {
            on_migrate();
            gtk::glib::Propagation::Stop
        }
        Some(MigrateShortcut::Copy) => {
            on_copy();
            gtk::glib::Propagation::Stop
        }
        None => gtk::glib::Propagation::Proceed,
    });
    dialog.add_controller(keys);
}

/// Escape on a page with a search box: while `search_open` says the search is open, Escape runs `close_search` and the
/// key stops there; otherwise it goes on, and the window's own handler leaves the page (`escape_step`).
pub fn attach_escape_step(page: &impl IsA<gtk::Widget>, search_open: impl Fn() -> bool + 'static, close_search: impl Fn() + 'static) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, _| match (key == Key::Escape).then(|| escape_step(search_open())) {
        Some(EscapeStep::CloseSearch) => {
            close_search();
            gtk::glib::Propagation::Stop
        }
        _ => gtk::glib::Propagation::Proceed,
    });
    page.add_controller(keys);
}

/// Escape closes `window` (the filter sheet). Nothing is reverted: the edits already live in the caller's state.
pub fn close_on_escape(window: &gtk::Window) {
    let keys = gtk::EventControllerKey::new();
    let closing = window.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == Key::Escape {
            closing.close();
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    window.add_controller(keys);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl() -> ModifierType {
        ModifierType::CONTROL_MASK
    }

    #[test]
    fn ctrl_f_focuses_the_search() {
        assert_eq!(browse_shortcut(Key::f, ctrl()), Some(BrowseShortcut::FocusSearch));
        assert_eq!(browse_shortcut(Key::F, ctrl()), Some(BrowseShortcut::FocusSearch));
        assert_eq!(browse_shortcut(Key::F, ModifierType::empty()), None, "a plain F types into a field");
    }

    #[test]
    fn ctrl_digits_one_to_six_select_the_browse_sub_tabs() {
        assert_eq!(browse_shortcut(Key::_1, ctrl()), Some(BrowseShortcut::SubTab(1)));
        assert_eq!(browse_shortcut(Key::_6, ctrl()), Some(BrowseShortcut::SubTab(6)));
        assert_eq!(browse_shortcut(Key::_7, ctrl()), None, "there is no seventh sub-tab");
        assert_eq!(browse_shortcut(Key::_1, ModifierType::empty()), None);
    }

    #[test]
    fn the_migrate_dialog_migrates_on_ctrl_enter_and_copies_on_ctrl_shift_enter() {
        let shift = ModifierType::SHIFT_MASK | ModifierType::CONTROL_MASK;
        assert_eq!(migrate_shortcut(Key::Return, ctrl()), Some(MigrateShortcut::Migrate));
        assert_eq!(migrate_shortcut(Key::KP_Enter, ctrl()), Some(MigrateShortcut::Migrate));
        assert_eq!(migrate_shortcut(Key::Return, shift), Some(MigrateShortcut::Copy));
        assert_eq!(migrate_shortcut(Key::Return, ModifierType::empty()), None, "a plain Enter is the dialog's default");
    }

    #[test]
    fn escape_closes_the_search_first_and_otherwise_leaves() {
        assert_eq!(escape_step(true), EscapeStep::CloseSearch);
        assert_eq!(escape_step(false), EscapeStep::Leave);
    }

    #[test]
    fn the_help_lists_every_shortcut_with_a_text() {
        let help = help_entries();
        for keys in ["Ctrl+F", "Escape", "Ctrl+Enter", "Ctrl+Shift+Enter"] {
            assert!(help.iter().any(|(k, what)| k == keys && !what.is_empty()), "{keys}");
        }
        assert!(help.iter().any(|(k, _)| k.starts_with("Ctrl+1")), "the sub-tab keys");
    }
}
