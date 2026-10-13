//! Keyboard equivalents for the library refresh (T-26a-112) and the S Pen actions of the reader and player (T-35a-028).
//! The accelerators are data here so the keybindings list and the GTK shortcut controllers read one table. The
//! `attach_*` functions build the controllers; the screens that own the widgets call them with their own callbacks.
//!
//! Decision (T-35a-028): `p` is picture-in-picture in the player (`kuro-player::shortcuts`), so the player's "previous
//! episode" is Shift+P ([`PLAYER_PREVIOUS_ITEM`]) and `p` keeps its meaning there; the reader uses plain `p`. The
//! controllers run in the bubble phase, so a focused text entry takes its letters before any of these fire.


#[cfg(test)]
use std::rc::Rc;

#[cfg(test)]
use gtk::glib;
#[cfg(test)]
use gtk::prelude::*;

#[cfg(test)]
/// Refreshes the visible category (T-26a-112).
pub const REFRESH_CATEGORY: &str = "<Control>r";
#[cfg(test)]
/// Starts a global pass of the visible kind (T-26a-112).
pub const GLOBAL_PASS: &str = "<Control><Shift>u";
#[cfg(test)]
/// Next chapter or episode in the reader and the player (T-35a-028, the S Pen "next" action).
pub const NEXT_ITEM: &str = "n";
#[cfg(test)]
/// Previous chapter or episode in the reader and the player (T-35a-028, the S Pen "previous" action).
pub const PREVIOUS_ITEM: &str = "p";
#[cfg(test)]
/// Window back navigation (T-35a-028). Alt+Left and Backspace both go back in the reader.
pub const BACK_ALT_LEFT: &str = "<Alt>Left";
#[cfg(test)]
pub const BACK_BACKSPACE: &str = "BackSpace";
#[cfg(test)]
/// Previous episode in the player, where plain `p` is picture-in-picture (see the module decision).
pub const PLAYER_PREVIOUS_ITEM: &str = "<Shift>p";

#[cfg(test)]
/// One shortcut controller for `rows`, in the bubble phase and local to the widget it is added to.
fn controller(rows: Vec<(&'static str, Rc<dyn Fn()>)>) -> gtk::ShortcutController {
    let controller = gtk::ShortcutController::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
    controller.set_scope(gtk::ShortcutScope::Local);
    for (accelerator, run) in rows {
        let Some(trigger) = gtk::ShortcutTrigger::parse_string(accelerator) else { continue };
        let action = gtk::CallbackAction::new(move |_, _| {
            run();
            glib::Propagation::Stop
        });
        controller.add_shortcut(gtk::Shortcut::new(Some(trigger), Some(action)));
    }
    controller
}

#[cfg(test)]
/// Ctrl+R refreshes the visible category and Ctrl+Shift+U starts a global pass of the visible kind (T-26a-112). Attach
/// to the root of the library tab.
pub fn attach_library(widget: &impl IsA<gtk::Widget>, refresh: impl Fn() + 'static, global_pass: impl Fn() + 'static) {
    widget.add_controller(controller(vec![(REFRESH_CATEGORY, Rc::new(refresh)), (GLOBAL_PASS, Rc::new(global_pass))]));
}

#[cfg(test)]
/// N and P move to the next and previous chapter; Alt+Left and Backspace go back (T-35a-028). Page keys are the reader's
/// arrows and add nothing here.
pub fn attach_reader(widget: &impl IsA<gtk::Widget>, previous: impl Fn() + 'static, next: impl Fn() + 'static, back: impl Fn() + 'static) {
    let back: Rc<dyn Fn()> = Rc::new(back);
    widget.add_controller(controller(vec![
        (NEXT_ITEM, Rc::new(next)),
        (PREVIOUS_ITEM, Rc::new(previous)),
        (BACK_ALT_LEFT, back.clone()),
        (BACK_BACKSPACE, back),
    ]));
}

#[cfg(test)]
/// N and Shift+P move to the next and previous episode (T-35a-028).
pub fn attach_player(widget: &impl IsA<gtk::Widget>, previous: impl Fn() + 'static, next: impl Fn() + 'static) {
    widget.add_controller(controller(vec![(NEXT_ITEM, Rc::new(next)), (PLAYER_PREVIOUS_ITEM, Rc::new(previous))]));
}

#[cfg(test)]
/// The rows of the keybindings list: (accelerator, what it does). Page turns use the reader's arrow keys and add no
/// binding (T-35a-028).
pub const KEYBINDINGS: &[(&str, &str)] = &[
    (REFRESH_CATEGORY, "Refresh the visible library category"),
    (GLOBAL_PASS, "Update the whole library (visible kind)"),
    (NEXT_ITEM, "Next chapter or episode (reader and player)"),
    (PREVIOUS_ITEM, "Previous chapter or episode (reader and player)"),
    (BACK_ALT_LEFT, "Go back (reader)"),
    (BACK_BACKSPACE, "Go back (reader)"),
    (PLAYER_PREVIOUS_ITEM, "Previous episode (player; P is picture-in-picture there)"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_documented_accelerator_parses_as_a_gtk_trigger() {
        // T-26a-112, T-35a-028: each row is a shortcut GTK accepts.
        crate::testing::with_display(|| {
            for (accelerator, _) in KEYBINDINGS {
                assert!(gtk::ShortcutTrigger::parse_string(accelerator).is_some(), "{accelerator} must parse");
            }
        });
    }

    #[test]
    fn the_accelerators_do_not_repeat_within_the_list() {
        let mut seen: Vec<&str> = KEYBINDINGS.iter().map(|(accel, _)| *accel).collect();
        let total = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), total);
    }

    #[test]
    fn next_and_previous_are_n_and_p_and_ctrl_r_refreshes() {
        assert_eq!(NEXT_ITEM, "n");
        assert_eq!(PREVIOUS_ITEM, "p");
        assert_eq!(REFRESH_CATEGORY, "<Control>r");
        assert_eq!(GLOBAL_PASS, "<Control><Shift>u");
    }

    /// The shortcut controllers `widget` carries, as (trigger text, action).
    fn shortcuts_of(widget: &gtk::Box) -> Vec<(String, gtk::Shortcut)> {
        let model = widget.observe_controllers();
        let mut rows = Vec::new();
        for i in 0..model.n_items() {
            let Some(controller) = model.item(i).and_then(|c| c.downcast::<gtk::ShortcutController>().ok()) else { continue };
            for j in 0..gtk::prelude::ListModelExt::n_items(&controller) {
                let shortcut = gtk::prelude::ListModelExt::item(&controller, j).and_downcast::<gtk::Shortcut>().expect("a shortcut");
                rows.push((shortcut.trigger().expect("a trigger").to_str().to_string(), shortcut));
            }
        }
        rows
    }

    fn run(widget: &gtk::Box, trigger: &str) -> bool {
        let wanted = gtk::ShortcutTrigger::parse_string(trigger).expect("parses").to_str().to_string();
        let (_, shortcut) = shortcuts_of(widget).into_iter().find(|(text, _)| *text == wanted).expect("the trigger is attached");
        shortcut.action().expect("an action").activate(gtk::ShortcutActionFlags::empty(), widget, None)
    }

    #[test]
    fn the_library_tab_runs_refresh_on_ctrl_r_and_the_global_pass_on_ctrl_shift_u() {
        // T-26a-112
        crate::testing::with_display(|| {
            let tab = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let log = Rc::new(std::cell::RefCell::new(Vec::new()));
            let (a, b) = (log.clone(), log.clone());
            attach_library(&tab, move || a.borrow_mut().push("refresh"), move || b.borrow_mut().push("global"));
            assert!(run(&tab, GLOBAL_PASS));
            assert!(run(&tab, REFRESH_CATEGORY));
            assert_eq!(*log.borrow(), ["global", "refresh"]);
        });
    }

    #[test]
    fn the_reader_has_n_p_alt_left_and_backspace_and_the_player_has_n_and_shift_p() {
        // T-35a-028
        crate::testing::with_display(|| {
            let reader = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let log = Rc::new(std::cell::RefCell::new(Vec::new()));
            let (p, n, b) = (log.clone(), log.clone(), log.clone());
            attach_reader(&reader, move || p.borrow_mut().push("previous"), move || n.borrow_mut().push("next"), move || b.borrow_mut().push("back"));
            for key in [NEXT_ITEM, PREVIOUS_ITEM, BACK_ALT_LEFT, BACK_BACKSPACE] {
                assert!(run(&reader, key), "{key}");
            }
            assert_eq!(*log.borrow(), ["next", "previous", "back", "back"]);
            assert_eq!(shortcuts_of(&reader).len(), 4, "page keys add no binding");

            let player = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let log = Rc::new(std::cell::RefCell::new(Vec::new()));
            let (p, n) = (log.clone(), log.clone());
            attach_player(&player, move || p.borrow_mut().push("previous"), move || n.borrow_mut().push("next"));
            assert!(run(&player, PLAYER_PREVIOUS_ITEM) && run(&player, NEXT_ITEM));
            assert_eq!(*log.borrow(), ["previous", "next"]);
            assert!(shortcuts_of(&player).iter().all(|(text, _)| text != "p"), "plain p stays picture-in-picture");
        });
    }
}
