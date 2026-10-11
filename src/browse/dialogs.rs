//! Modal dialogs of the browse pages: the untrusted-extension dialog, the remove confirm and the duplicate dialog all
//! come through `DialogSlot`, which keeps at most one of them open (B-EC-27: a second request replaces the first). GTK
//! only; every decision that follows an answer is in the pure modules (`trust`, `favorite`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

/// How a dialog button looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonStyle {
    Plain,
    Suggested,
    Destructive,
}

/// What a dialog says and offers. Buttons are answered by index; closing the window without a press answers `None`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogSpec {
    pub title: String,
    pub body: Option<String>,
    pub buttons: Vec<(String, ButtonStyle)>,
}

/// The one dialog a page may have open.
#[derive(Clone, Default)]
pub struct DialogSlot {
    current: Rc<RefCell<Option<gtk::Window>>>,
}

impl DialogSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Shows `spec` over `anchor`'s window and replaces any dialog this slot still has open. `on_answer` runs once for
    /// the dialog: `Some(index)` for a button, `None` when it was closed without one (also when a newer dialog
    /// replaced it), and then the dialog is gone.
    pub fn show(&self, anchor: &impl IsA<gtk::Widget>, spec: DialogSpec, on_answer: impl Fn(Option<usize>) + 'static) -> gtk::Window {
        self.show_with(anchor, spec, None, on_answer)
    }

    /// As `show`, with `extra` between the body and the buttons (the category chooser's check buttons).
    pub fn show_with(
        &self,
        anchor: &impl IsA<gtk::Widget>,
        spec: DialogSpec,
        extra: Option<gtk::Widget>,
        on_answer: impl Fn(Option<usize>) + 'static,
    ) -> gtk::Window {
        // the borrow ends before `close`, which runs the old dialog's close handler
        let old = self.current.borrow_mut().take();
        if let Some(old) = old {
            old.close();
        }
        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        for set in [gtk::Box::set_margin_start, gtk::Box::set_margin_end, gtk::Box::set_margin_top, gtk::Box::set_margin_bottom] {
            set(&column, 18);
        }
        if let Some(body) = &spec.body {
            column.append(&gtk::Label::builder().label(body).wrap(true).xalign(0.0).max_width_chars(50).build());
        }
        if let Some(extra) = &extra {
            column.append(extra);
        }
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        buttons.set_halign(gtk::Align::End);
        column.append(&buttons);

        let parent = anchor.upcast_ref::<gtk::Widget>().root().and_downcast::<gtk::Window>();
        let window = gtk::Window::builder().title(&spec.title).modal(true).resizable(false).child(&column).build();
        window.set_transient_for(parent.as_ref());

        let answered = Rc::new(Cell::new(false));
        let on_answer: Rc<dyn Fn(Option<usize>)> = Rc::new(on_answer);
        for (index, (label, style)) in spec.buttons.iter().enumerate() {
            let button = gtk::Button::with_label(label);
            match style {
                ButtonStyle::Plain => {}
                ButtonStyle::Suggested => button.add_css_class("suggested-action"),
                ButtonStyle::Destructive => button.add_css_class("destructive-action"),
            }
            let (window, answered, on_answer) = (window.clone(), Rc::clone(&answered), Rc::clone(&on_answer));
            button.connect_clicked(move |_| {
                if !answered.replace(true) {
                    on_answer(Some(index));
                }
                window.close();
            });
            buttons.append(&button);
        }
        {
            let (answered, slot) = (Rc::clone(&answered), Rc::clone(&self.current));
            let this = window.clone();
            window.connect_close_request(move |_| {
                if !answered.replace(true) {
                    on_answer(None);
                }
                let mut current = slot.borrow_mut();
                if current.as_ref() == Some(&this) {
                    current.take();
                }
                gtk::glib::Propagation::Proceed
            });
        }
        // Escape cancels: it closes the dialog, which answers `None` like any dismissal
        let escape = gtk::EventControllerKey::new();
        let closing = window.clone();
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                closing.close();
                gtk::glib::Propagation::Stop
            } else {
                gtk::glib::Propagation::Proceed
            }
        });
        window.add_controller(escape);
        *self.current.borrow_mut() = Some(window.clone());
        window.present();
        window
    }

    /// Registers a window the caller built itself (a dialog that stays open while it works): any dialog this slot has
    /// open is closed first, and the slot forgets the window when it closes. A close handler the caller connected
    /// before this call can still refuse the close.
    pub fn adopt(&self, window: &gtk::Window) {
        let old = self.current.borrow_mut().take();
        if let Some(old) = old {
            old.close();
        }
        *self.current.borrow_mut() = Some(window.clone());
        let (slot, this) = (Rc::clone(&self.current), window.clone());
        window.connect_close_request(move |_| {
            let mut current = slot.borrow_mut();
            if current.as_ref() == Some(&this) {
                current.take();
            }
            gtk::glib::Propagation::Proceed
        });
    }

    #[cfg(test)]
    /// The dialog on screen, if any.
    pub fn current(&self) -> Option<gtk::Window> {
        self.current.borrow().clone()
    }
}

/// The buttons of a dialog window, in order. Used by the widget checks.
#[cfg(test)]
pub(crate) fn buttons_of(window: &gtk::Window) -> Vec<gtk::Button> {
    fn walk(widget: &gtk::Widget, out: &mut Vec<gtk::Button>) {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            out.push(button.clone());
        }
        let mut child = widget.first_child();
        while let Some(w) = child {
            walk(&w, out);
            child = w.next_sibling();
        }
    }
    let mut out = Vec::new();
    if let Some(child) = window.child() {
        walk(&child, &mut out);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn spec(title: &str) -> DialogSpec {
        DialogSpec {
            title: title.into(),
            body: Some("body".into()),
            buttons: vec![("One".into(), ButtonStyle::Suggested), ("Two".into(), ButtonStyle::Destructive)],
        }
    }

    fn a_button_answers_with_its_index_once_and_closing_without_one_answers_none() {
        let slot = DialogSlot::new();
        let anchor = gtk::Label::new(None);
        let answers: Rc<RefCell<Vec<Option<usize>>>> = Rc::default();
        let log = Rc::clone(&answers);
        let window = slot.show(&anchor, spec("Ask"), move |a| log.borrow_mut().push(a));
        let buttons = buttons_of(&window);
        assert_eq!(buttons.len(), 2);
        assert!(buttons[1].has_css_class("destructive-action"));
        buttons[1].emit_clicked();
        assert_eq!(*answers.borrow(), [Some(1)], "the window closing after a press does not answer again");
        assert!(slot.current().is_none());

        let log = Rc::clone(&answers);
        slot.show(&anchor, spec("Dismissed"), move |a| log.borrow_mut().push(a)).close();
        assert_eq!(*answers.borrow(), [Some(1), None], "dismissing is its own answer");
    }

    fn a_second_dialog_replaces_the_open_one() {
        let slot = DialogSlot::new();
        let anchor = gtk::Label::new(None);
        let answers: Rc<RefCell<Vec<(&'static str, Option<usize>)>>> = Rc::default();
        let log = Rc::clone(&answers);
        let first = slot.show(&anchor, spec("First"), move |a| log.borrow_mut().push(("first", a)));
        let log = Rc::clone(&answers);
        let second = slot.show(&anchor, spec("Second"), move |a| log.borrow_mut().push(("second", a)));
        assert_ne!(first, second);
        assert_eq!(slot.current().as_ref(), Some(&second), "only the newest dialog is open");
        assert_eq!(*answers.borrow(), [("first", None)], "the replaced dialog changed nothing");
        second.close();
    }

    fn escape_cancels_the_dialog_with_the_dismiss_answer() {
        let slot = DialogSlot::new();
        let anchor = gtk::Label::new(None);
        let answers: Rc<RefCell<Vec<Option<usize>>>> = Rc::default();
        let log = Rc::clone(&answers);
        let window = slot.show(&anchor, spec("Ask"), move |a| log.borrow_mut().push(a));
        let controllers = window.observe_controllers();
        let keys = (0..controllers.n_items())
            .filter_map(|i| controllers.item(i))
            .find_map(|c| c.downcast::<gtk::EventControllerKey>().ok())
            .expect("key controller");
        let none = gtk::gdk::ModifierType::empty();
        assert!(!keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::a, &0_u32, &none]), "other keys go on");
        assert!(answers.borrow().is_empty());
        assert!(keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::Escape, &0_u32, &none]));
        assert_eq!(*answers.borrow(), [None], "Escape is a dismissal");
        assert!(slot.current().is_none());
    }

    pub(crate) fn smoke() {
        escape_cancels_the_dialog_with_the_dismiss_answer();
        a_button_answers_with_its_index_once_and_closing_without_one_answers_none();
        a_second_dialog_replaces_the_open_one();
    }
}
