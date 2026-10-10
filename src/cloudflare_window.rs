//! The visible Cloudflare check window (T-12a-032): 960 x 720, titled `cloudflare_check_title`, with the hint
//! `cloudflare_check_hint` above the page area and a Cancel button. Escape, Cancel and the window's close button all
//! cancel the solve (`Dismissal`); the window closes by itself when the check clears. The rules (size, deadline, what a
//! dismissal means) are in `kuro_platform::webview::visible`. The page area, [`CheckWindow::content`], is where the
//! WebKitGTK view goes when the WebView host (T-12a-017) lands; WebKitGTK is not available to this build, so no host
//! shows this window yet.
use std::cell::Cell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use kuro_i18n::{tr, MsgKey};
use kuro_platform::webview::visible::{Dismissal, WINDOW_HEIGHT, WINDOW_WIDTH};

/// The window of one visible solve.
pub struct CheckWindow {
    window: gtk::Window,
    content: gtk::Box,
}

/// Whether a key press cancels the check.
pub fn is_cancel_key(key: gtk::gdk::Key) -> bool {
    key == gtk::gdk::Key::Escape
}

impl CheckWindow {
    /// Shows the window over `parent`. `on_dismiss` runs once, with the way the person ended it.
    pub fn present(parent: Option<&gtk::Window>, on_dismiss: impl Fn(Dismissal) + 'static) -> Self {
        let window = gtk::Window::builder()
            .title(tr(MsgKey::cloudflare_check_title))
            .default_width(WINDOW_WIDTH)
            .default_height(WINDOW_HEIGHT)
            .build();
        window.set_transient_for(parent);

        let hint = gtk::Label::builder().label(tr(MsgKey::cloudflare_check_hint)).wrap(true).xalign(0.0).build();
        let content = gtk::Box::builder().orientation(gtk::Orientation::Vertical).vexpand(true).hexpand(true).build();
        let cancel = gtk::Button::with_label(&tr(MsgKey::action_cancel));
        cancel.set_halign(gtk::Align::End);
        let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
        for (margin, child) in [(12, hint.upcast_ref::<gtk::Widget>()), (0, content.upcast_ref()), (12, cancel.upcast_ref())] {
            child.set_margin_start(12);
            child.set_margin_end(12);
            child.set_margin_top(margin);
            child.set_margin_bottom(margin);
            column.append(child);
        }
        window.set_child(Some(&column));

        let done = Rc::new(Cell::new(false));
        let on_dismiss: Rc<dyn Fn(Dismissal)> = Rc::new(on_dismiss);
        let dismiss = {
            let (window, done) = (window.clone(), Rc::clone(&done));
            move |how: Dismissal| {
                if !done.replace(true) {
                    on_dismiss(how);
                    window.destroy();
                }
            }
        };
        let dismiss = Rc::new(dismiss);
        cancel.connect_clicked({
            let dismiss = Rc::clone(&dismiss);
            move |_| dismiss(Dismissal::CancelButton)
        });
        window.connect_close_request({
            let dismiss = Rc::clone(&dismiss);
            move |_| {
                dismiss(Dismissal::CloseButton);
                glib::Propagation::Proceed
            }
        });
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let dismiss = Rc::clone(&dismiss);
            move |_, key, _, _| {
                if is_cancel_key(key) {
                    dismiss(Dismissal::Escape);
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            }
        });
        window.add_controller(keys);
        window.present();
        Self { window, content }
    }

    /// Where the page of the check is shown.
    pub fn content(&self) -> &gtk::Box {
        &self.content
    }

    /// Closes the window after the check cleared. The dismissal callback does not run.
    pub fn close(&self) {
        self.window.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::with_display;

    #[test]
    fn the_window_has_its_title_size_hint_and_page_area() {
        with_display(|| {
            let shown = CheckWindow::present(None, |_| {});
            assert_eq!(shown.window.title().as_deref(), Some("Cloudflare check"));
            assert_eq!(shown.window.default_size(), (960, 720));
            assert!(shown.content.vexpands());
            let mut labels = Vec::new();
            let mut stack = vec![shown.window.child().unwrap()];
            while let Some(widget) = stack.pop() {
                if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                    labels.push(label.text().to_string());
                }
                if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                    labels.push(button.label().unwrap_or_default().to_string());
                }
                let mut child = widget.first_child();
                while let Some(c) = child {
                    child = c.next_sibling();
                    stack.push(c);
                }
            }
            assert!(labels.contains(&"Complete the check in this window; it closes by itself when it succeeds.".to_owned()), "{labels:?}");
            assert!(labels.contains(&"Cancel".to_owned()), "{labels:?}");
            shown.close();
        });
    }

    #[test]
    fn closing_after_the_check_cleared_is_not_a_dismissal() {
        with_display(|| {
            let seen = Rc::new(Cell::new(0));
            let counter = Rc::clone(&seen);
            let shown = CheckWindow::present(None, move |_| counter.set(counter.get() + 1));
            shown.close();
            assert_eq!(seen.get(), 0);
        });
    }

    #[test]
    fn only_escape_cancels_by_key() {
        assert!(is_cancel_key(gtk::gdk::Key::Escape));
        assert!(!is_cancel_key(gtk::gdk::Key::Return) && !is_cancel_key(gtk::gdk::Key::q));
    }
}
