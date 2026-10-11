//! Refresh (T-49b-019): the header refresh button and the `win.refresh` action, on F5 and Ctrl+R.
//!
//! The button is insensitive and shows a spinner while a refresh runs. The caller's refresh
//! receives a `done` callback and must call it when the refresh ends, on success and on error.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;

/// The action name (`win.refresh`) and its accelerators.
pub const ACTION: &str = "refresh";
pub const ACCELS: [&str; 2] = ["F5", "<Control>r"];
/// The icon of the idle button.
pub const ICON: &str = "view-refresh-symbolic";

/// Runs at most one refresh at a time. Starting while one runs does nothing.
#[derive(Debug, Default)]
pub struct Gate {
    busy: Cell<bool>,
}

impl Gate {
    /// Takes the gate. False when a refresh is already running.
    pub fn begin(&self) -> bool {
        !self.busy.replace(true)
    }

    /// Releases the gate. Called when a refresh ends, on success and on error.
    pub fn end(&self) {
        self.busy.set(false);
    }

    pub fn busy(&self) -> bool {
        self.busy.get()
    }
}

/// Called by the refresh when it ends.
pub type Done = Rc<dyn Fn()>;

/// The refresh button and the action that starts a refresh.
pub struct Refresh {
    button: gtk::Button,
    start: Rc<dyn Fn()>,
}

impl Refresh {
    /// `on_refresh` runs one refresh and must call the `done` it receives when the refresh ends.
    pub fn new(on_refresh: impl Fn(Done) + 'static) -> Self {
        let gate = Rc::new(Gate::default());
        let icon = gtk::Image::from_icon_name(ICON);
        let spinner = gtk::Spinner::new();
        spinner.set_visible(false);
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        content.append(&icon);
        content.append(&spinner);
        let button = gtk::Button::new();
        button.set_child(Some(&content));

        let on_refresh = Rc::new(on_refresh);
        let start: Rc<dyn Fn()> = {
            let (gate, button, icon, spinner) = (gate.clone(), button.clone(), icon.clone(), spinner.clone());
            Rc::new(move || {
                if !gate.begin() {
                    return;
                }
                button.set_sensitive(false);
                icon.set_visible(false);
                spinner.set_visible(true);
                spinner.start();
                let done: Done = {
                    let (gate, button, icon, spinner) = (gate.clone(), button.clone(), icon.clone(), spinner.clone());
                    Rc::new(move || {
                        gate.end();
                        spinner.stop();
                        spinner.set_visible(false);
                        icon.set_visible(true);
                        button.set_sensitive(true);
                    })
                };
                on_refresh(done);
            })
        };
        {
            let start = start.clone();
            button.connect_clicked(move |_| start());
        }
        Refresh { button, start }
    }

    /// The header button.
    pub fn button(&self) -> &gtk::Button {
        &self.button
    }

    /// Starts a refresh, as the button and the action do.
    pub fn start(&self) {
        (self.start)();
    }

    /// Adds `win.refresh` to the window and binds F5 and Ctrl+R to it. The accelerators are set
    /// only when the window already belongs to the application.
    pub fn install_action(&self, window: &gtk::ApplicationWindow) {
        let action = gtk::gio::SimpleAction::new(ACTION, None);
        let start = self.start.clone();
        action.connect_activate(move |_, _| start());
        window.add_action(&action);
        if let Some(app) = window.application() {
            app.set_accels_for_action(&format!("win.{ACTION}"), &ACCELS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_start_is_ignored_while_one_runs() {
        let gate = Gate::default();
        assert!(gate.begin());
        assert!(!gate.begin());
        assert!(gate.busy());
    }

    #[test]
    fn ending_the_refresh_reopens_the_gate_on_success_and_on_error() {
        let gate = Gate::default();
        assert!(gate.begin());
        gate.end();
        assert!(!gate.busy());
        assert!(gate.begin(), "a refresh that ended with an error also reopens the gate");
    }

    #[test]
    fn accelerators_are_f5_and_ctrl_r() {
        assert_eq!(ACCELS, ["F5", "<Control>r"]);
    }
}
