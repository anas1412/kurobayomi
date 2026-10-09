//! The banner and toast widget over `banner.rs`. One component shows both: a toast is a banner with no action. It
//! never takes keyboard focus, so a banner appearing does not move the caret. It sits over the page, so an action
//! of a banner is dropped once the page is gone (E-UI-1).

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, Revealer};

use crate::entry_core::banner::Banner;
use crate::entry_core::detail::StoreCmd;

pub struct BannerView {
    pub widget: Revealer,
    label: Label,
    action: Button,
}

fn unfocusable(w: &impl IsA<gtk::Widget>) {
    w.set_focusable(false);
    w.set_can_focus(false);
}

/// `on_action` runs when the action button is pressed, `on_close` when the close button is.
pub fn build(on_action: impl Fn() + 'static, on_close: impl Fn() + 'static) -> BannerView {
    let label = Label::builder().wrap(true).max_width_chars(60).xalign(0.0).hexpand(true).build();
    let action = Button::builder().css_classes(["flat"]).build();
    let close = Button::from_icon_name("window-close-symbolic");
    close.add_css_class("flat");
    let bar = GBox::new(O::Horizontal, 6);
    bar.add_css_class("osd");
    bar.set_margin_bottom(24);
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    bar.append(&label);
    bar.append(&action);
    bar.append(&close);
    let widget = Revealer::builder()
        .child(&bar)
        .halign(Align::Center)
        .valign(Align::End)
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .build();
    for w in [bar.upcast_ref::<gtk::Widget>(), label.upcast_ref(), action.upcast_ref(), close.upcast_ref(), widget.upcast_ref()] {
        unfocusable(w);
    }
    for b in [&action, &close] {
        b.set_focus_on_click(false);
    }
    action.connect_clicked(move |_| on_action());
    close.connect_clicked(move |_| on_close());
    BannerView { widget, label, action }
}

impl BannerView {
    /// Shows `banner`, or hides the view when there is none.
    pub fn show(&self, banner: Option<&Banner<StoreCmd>>) {
        match banner {
            Some(b) => {
                self.label.set_label(&b.text);
                match &b.action {
                    Some((text, _)) => {
                        self.action.set_label(text);
                        self.action.set_visible(true);
                    }
                    None => self.action.set_visible(false),
                }
                self.widget.set_reveal_child(true);
            }
            None => self.widget.set_reveal_child(false),
        }
    }
}
