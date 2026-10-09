//! The expandable description: three lines with a 24 px scrim at the bottom while collapsed. The caret button and the
//! text both toggle it, and the state lasts for the page session.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Orientation as O};

use crate::entry_core::text::{COLLAPSED_LINES, Description, Expansion, SCRIM_PX};

/// The scrim: the window background fading in over the last collapsed line.
fn scrim(host: &gtk::Widget) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_height(SCRIM_PX);
    area.set_valign(Align::End);
    area.set_can_target(false);
    let host = host.clone();
    area.set_draw_func(move |_, cr, w, h| {
        #[allow(deprecated)]
        let bg = host.style_context().lookup_color("theme_bg_color").unwrap_or_else(|| gtk::gdk::RGBA::new(0.0, 0.0, 0.0, 1.0));
        let (r, g, b) = (f64::from(bg.red()), f64::from(bg.green()), f64::from(bg.blue()));
        let fade = gtk::cairo::LinearGradient::new(0.0, 0.0, 0.0, f64::from(h));
        fade.add_color_stop_rgba(0.0, r, g, b, 0.0);
        fade.add_color_stop_rgba(1.0, r, g, b, 1.0);
        let _ = cr.set_source(&fade);
        cr.rectangle(0.0, 0.0, f64::from(w), f64::from(h));
        let _ = cr.fill();
    });
    area
}

/// Builds the description. `expansion` is shared with the page so the state outlives a rebuild of the widget.
pub fn build(description: &Description, expansion: Rc<Cell<Expansion>>) -> GBox {
    let label = gtk::Label::builder().xalign(0.0).wrap(true).build();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&label));
    let fade = scrim(label.upcast_ref());
    overlay.add_overlay(&fade);
    let caret = gtk::Button::builder().css_classes(["flat"]).halign(Align::Center).build();

    let render = {
        let (label, fade, caret, description) = (label.clone(), fade.clone(), caret.clone(), description.clone());
        move |state: Expansion| {
            if state.is_expanded() {
                label.set_label(&description.expanded);
                label.set_lines(-1);
                label.set_ellipsize(gtk::pango::EllipsizeMode::None);
                caret.set_icon_name("go-up-symbolic");
            } else {
                label.set_label(&description.shrunk);
                label.set_lines(COLLAPSED_LINES);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                caret.set_icon_name("go-down-symbolic");
            }
            fade.set_visible(state.scrim());
        }
    };
    render(expansion.get());
    let toggle = {
        let expansion = Rc::clone(&expansion);
        move || {
            let mut state = expansion.get();
            state.toggle();
            expansion.set(state);
            render(state);
        }
    };
    let on_text = toggle.clone();
    let tap = gtk::GestureClick::new();
    tap.connect_released(move |_, _, _, _| on_text());
    overlay.add_controller(tap);
    caret.connect_clicked(move |_| toggle());

    let root = GBox::new(O::Vertical, 0);
    root.append(&overlay);
    root.append(&caret);
    root
}
