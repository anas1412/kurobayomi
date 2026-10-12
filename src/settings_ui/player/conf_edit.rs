//! The mpv.conf and input.conf rows (tasks T-48a-035 and T-48a-029): a row that shows the first two lines of the text
//! and opens a multi-line dialog. OK is on only when the text changed and, unless `can_be_blank`, is not blank; the
//! write goes through the row's veto like every edit row. The text is coloured with the conf rules.
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::pref::Pref;

use super::syntax;
use crate::pref_model::bind::{self, Outcome, WriteGate};
use crate::pref_model::model::{mpv_subtitle, PreferenceItem};
use crate::pref_model::widgets::dialog::{self, Dialog};
use crate::pref_model::widgets::edit_text::can_confirm;
use crate::pref_model::widgets::row::{self, set_subtitle, RowSpec};
use crate::settings_ui::rows::text_veto;

/// A multi-line text row bound to `pref`, coloured with the conf rules. A blank text is stored only when `can_be_blank`.
pub fn conf_row(title: &str, pref: &Pref<String>, can_be_blank: bool) -> PreferenceItem {
    editor_row(title, pref, can_be_blank, true)
}

/// A multi-line text row bound to `pref` with no syntax colouring (the generic multi-line field, T-47ba-044).
pub fn multi_line_row(title: &str, pref: &Pref<String>, can_be_blank: bool) -> PreferenceItem {
    editor_row(title, pref, can_be_blank, false)
}

fn editor_row(title: &str, pref: &Pref<String>, can_be_blank: bool, colour: bool) -> PreferenceItem {
    let (name, pref) = (title.to_owned(), pref.clone());
    PreferenceItem::Custom {
        title: title.to_owned(),
        build: Rc::new(move || build(&name, &pref, can_be_blank, colour).upcast()),
    }
}

fn build(title: &str, pref: &Pref<String>, can_be_blank: bool, colour: bool) -> gtk::Box {
    let built = row::build(RowSpec { title, subtitle: Some(&mpv_subtitle(&pref.get())), icon: None, trailing: None, on_click: None });
    let content = built.row.child().and_downcast::<gtk::Box>().expect("the row content");
    built.row.set_child(None::<&gtk::Widget>);
    content.set_focusable(true);
    content.set_cursor_from_name(Some("pointer"));
    let stored = Rc::new(RefCell::new(pref.get()));
    let veto = text_veto(pref, can_be_blank, |_| Ok(()));
    let gate = Rc::new(WriteGate::default());
    let open: Rc<dyn Fn()> = {
        let (content, subtitle, pref, stored) = (content.clone(), built.subtitle.clone(), pref.clone(), stored.clone());
        let title = title.to_owned();
        Rc::new(move || {
            let Some(parent) = content.root().and_downcast::<gtk::Window>() else { return };
            let snapshot = stored.borrow().clone();
            let (subtitle, pref, stored, veto, gate) = (subtitle.clone(), pref.clone(), stored.clone(), veto.clone(), gate.clone());
            show(&parent, &title, &snapshot, can_be_blank, colour, move |text, dialog| {
                let (subtitle, stored) = (subtitle.clone(), stored.clone());
                let written = text.clone();
                bind::write(gate.clone(), veto.clone(), pref.clone(), text, move |outcome| match outcome {
                    Outcome::Written => {
                        set_subtitle(&subtitle, Some(&mpv_subtitle(&written)));
                        *stored.borrow_mut() = written;
                        dialog.finish();
                    }
                    Outcome::Kept => {}
                    Outcome::Failed(message) => dialog.set_error(Some(&message)),
                });
            });
        })
    };
    let click = gtk::GestureClick::new();
    let on_click = Rc::clone(&open);
    click.connect_released(move |_, _, _, _| on_click());
    content.add_controller(click);
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, _| {
        if dialog::is_enter(key) || key == gtk::gdk::Key::space {
            open();
            gtk::glib::Propagation::Stop
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    content.add_controller(keys);
    let weak = content.downgrade();
    let subtitle = built.subtitle.clone();
    bind::follow(pref, move || weak.upgrade().is_some(), move |value: String| {
        set_subtitle(&subtitle, Some(&mpv_subtitle(&value)));
        *stored.borrow_mut() = value;
    });
    content
}

fn show(parent: &gtk::Window, title: &str, snapshot: &str, can_be_blank: bool, colour: bool, on_ok: impl Fn(String, Dialog) + 'static) {
    let buffer = gtk::TextBuffer::new(None);
    buffer.set_text(snapshot);
    if colour {
        syntax::attach(&buffer);
    }
    let view = gtk::TextView::builder().buffer(&buffer).monospace(true).left_margin(6).right_margin(6).top_margin(6).bottom_margin(6).build();
    let frame = gtk::ScrolledWindow::builder().child(&view).min_content_height(200).has_frame(true).build();
    let slot: Rc<RefCell<Option<Dialog>>> = Rc::new(RefCell::new(None));
    // a bare Enter adds a line; Ctrl+Enter submits (T-47ba-044). Capture phase so the text view does not take it first.
    let submit_keys = gtk::EventControllerKey::new();
    submit_keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let submit_slot = slot.clone();
    submit_keys.connect_key_pressed(move |_, key, _, state| {
        if !dialog::is_ctrl_enter(key, state) {
            return gtk::glib::Propagation::Proceed;
        }
        if let Some(dialog) = submit_slot.borrow().as_ref() {
            dialog.submit();
        }
        gtk::glib::Propagation::Stop
    });
    view.add_controller(submit_keys);
    let snapshot = snapshot.to_owned();
    let text_of = move |buffer: &gtk::TextBuffer| buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string();
    let b = buffer.clone();
    let confirm = move |dialog: Dialog| on_ok(text_of(&b), dialog);
    let dialog = dialog::show(parent, title, &frame, Some(&crate::pref_model::strings::ok()), confirm, || {});
    dialog.set_confirm_enabled(false);
    *slot.borrow_mut() = Some(dialog);
    buffer.connect_changed(move |buffer| {
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        if let Some(dialog) = slot.borrow().as_ref() {
            dialog.set_confirm_enabled(can_confirm(&text, &snapshot, can_be_blank, true));
        }
    });
}
