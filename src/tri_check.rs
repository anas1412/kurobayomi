//! Tri-state check widget (T-01a-036). Off for None, a tick for Include, a dash for Exclude. A click, Space or
//! Enter cycles None, Include, Exclude, None; `set_state` changes the look without calling `on_change`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::checkbox::TriState;

type State = TriState<()>;

/// The cycle and the "programmatic changes stay silent" rule, without any widget.
struct Model {
    state: RefCell<State>,
    /// True while this code is changing the button, because GTK emits `toggled` for those changes too.
    applying: Cell<bool>,
    on_change: Box<dyn Fn(State)>,
}

impl Model {
    fn clicked(&self, show: &dyn Fn(&State)) {
        if self.applying.replace(true) {
            return;
        }
        let next = self.state.borrow().clone().next();
        *self.state.borrow_mut() = next.clone();
        show(&next);
        self.applying.set(false);
        (self.on_change)(next);
    }

    #[cfg(test)]
    fn set_state(&self, state: State, show: &dyn Fn(&State)) {
        self.applying.set(true);
        *self.state.borrow_mut() = state.clone();
        show(&state);
        self.applying.set(false);
    }
}

fn show(button: &gtk::CheckButton, state: &State) {
    let (active, inconsistent) = state.gtk_flags();
    button.set_inconsistent(inconsistent);
    button.set_active(active);
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct TriCheck {
    button: gtk::CheckButton,
    model: Rc<Model>,
}

/// A tri-state check showing `initial`. `on_change` receives the new state after each user cycle.
pub fn tri_check(initial: State, on_change: impl Fn(State) + 'static) -> TriCheck {
    let button = gtk::CheckButton::new();
    let model = Rc::new(Model { state: RefCell::new(initial.clone()), applying: Cell::new(false), on_change: Box::new(on_change) });
    show(&button, &initial);
    button.connect_toggled({
        let model = model.clone();
        move |b| model.clicked(&|s| show(b, s))
    });
    // Space is the check button's own binding; Enter activates it too
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed({
        let b = button.clone();
        move |_, key, _, _| {
            if matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::ISO_Enter) {
                b.activate();
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        }
    });
    button.add_controller(keys);
    TriCheck { button, model }
}

#[allow(dead_code)] // `state` and `set_state` are the widget's API; the filter sheets call them as they land
impl TriCheck {
    pub fn widget(&self) -> &gtk::CheckButton {
        &self.button
    }

    #[cfg(test)]
    pub fn state(&self) -> State {
        self.model.state.borrow().clone()
    }

    #[cfg(test)]
    /// Changes the state and the look without calling `on_change`.
    pub fn set_state(&self, state: State) {
        self.model.set_state(state, &|s| show(&self.button, s));
    }

    /// Cycles as a click does (used by the list row that holds the check).
    pub fn activate(&self) {
        self.button.activate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(log: &Rc<RefCell<Vec<State>>>) -> Model {
        let log = log.clone();
        Model { state: RefCell::new(TriState::None(())), applying: Cell::new(false), on_change: Box::new(move |s| log.borrow_mut().push(s)) }
    }

    #[test]
    fn clicks_cycle_none_include_exclude_none() {
        let log = Rc::new(RefCell::new(vec![]));
        let m = model(&log);
        for _ in 0..4 {
            m.clicked(&|_| {});
        }
        let want = [TriState::Include(()), TriState::Exclude(()), TriState::None(()), TriState::Include(())];
        assert_eq!(*log.borrow(), want);
    }

    #[test]
    fn set_state_is_silent_even_when_the_widget_echoes_a_toggle() {
        let log = Rc::new(RefCell::new(vec![]));
        let m = Rc::new(model(&log));
        // GTK emits `toggled` while the button is changed; the echo must not cycle or notify
        let echo = m.clone();
        m.set_state(TriState::Exclude(()), &|_| echo.clicked(&|_| {}));
        assert!(log.borrow().is_empty());
        assert_eq!(*m.state.borrow(), TriState::Exclude(()));
        m.clicked(&|_| {});
        assert_eq!(*log.borrow(), [TriState::None(())]);
    }

    #[test]
    fn the_user_cycle_ignores_the_echo_of_its_own_update() {
        let log = Rc::new(RefCell::new(vec![]));
        let m = Rc::new(model(&log));
        let echo = m.clone();
        m.clicked(&|_| echo.clicked(&|_| {}));
        assert_eq!(*log.borrow(), [TriState::Include(())]);
    }
}
