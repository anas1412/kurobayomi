//! A page that is either pushed on the main window's navigation stack or shown in a window of its own, and moves
//! between the two without being rebuilt (docs/design/player-reader.md). The player and the reader use it.
//!
//! The content widget itself moves: it leaves the stack (or the window) and enters the other one, so GTK unrealizes
//! it and realizes it again in the new toplevel. [`Detachable::is_moving`] is true during that hop, so content with
//! per-window state (the player's GL render context) can tell a move from a close. Main thread only.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

/// The Detach / Attach button's icon and tooltip for each state.
pub fn button_face(detached: bool) -> (&'static str, &'static str) {
    if detached { ("view-restore-symbolic", "Attach to the main window") } else { ("window-new-symbolic", "Detach into its own window") }
}

pub struct Detachable {
    /// Weak, so closing the detached window (or popping the page) frees the content and everything it holds.
    content: glib::WeakRef<gtk::Widget>,
    window: RefCell<Option<gtk::Window>>,
    moving: Cell<bool>,
    button: gtk::Button,
    title: RefCell<String>,
    /// The content draws its own header bar, so a detached window hides the default title bar.
    own_header: bool,
    on_moved: RefCell<Option<Box<dyn Fn(bool)>>>,
    on_left: RefCell<Option<Box<dyn Fn()>>>,
    /// Signal handlers on the main window and the stack (see [`Self::watch_nav`]), disconnected when the host drops.
    watches: RefCell<Vec<(glib::WeakRef<glib::Object>, glib::SignalHandlerId)>>,
}

impl Detachable {
    /// A host for `content`, which is not placed anywhere yet: call [`Self::show`], or push it yourself (the host
    /// then counts it as embedded). The host holds `content` weakly, so the caller keeps it alive until it is placed.
    /// The button is for the content's header.
    pub fn new(content: &impl IsA<gtk::Widget>, title: &str, own_header: bool) -> Rc<Self> {
        let host = Rc::new(Self {
            content: content.upcast_ref::<gtk::Widget>().downgrade(),
            window: RefCell::new(None),
            moving: Cell::new(false),
            button: gtk::Button::new(),
            title: RefCell::new(title.to_owned()),
            own_header,
            on_moved: RefCell::new(None),
            on_left: RefCell::new(None),
            watches: RefCell::new(Vec::new()),
        });
        host.button.set_focus_on_click(false);
        host.sync_button();
        let weak = Rc::downgrade(&host);
        host.button.connect_clicked(move |_| {
            if let Some(host) = weak.upgrade() {
                host.toggle();
            }
        });
        host
    }

    pub fn button(&self) -> &gtk::Button {
        &self.button
    }

    pub fn is_detached(&self) -> bool {
        self.window.borrow().is_some()
    }

    /// True while the content is between toplevels: its unrealize is a move, not a close.
    pub fn is_moving(&self) -> bool {
        self.moving.get()
    }

    /// The detached window's title, now and for a later detach.
    pub fn set_title(&self, title: &str) {
        *self.title.borrow_mut() = title.to_owned();
        if let Some(window) = self.window.borrow().as_ref() {
            window.set_title(Some(title));
        }
    }

    /// Runs after every Detach or Attach the user asks for, with the new state.
    pub fn connect_moved(&self, f: impl Fn(bool) + 'static) {
        *self.on_moved.borrow_mut() = Some(Box::new(f));
    }

    /// Runs at once when the embedded page stops being the visible page of the navigation stack by any route other
    /// than Detach (Back, Escape, Alt+Left, the mouse back button), before the stack removes it.
    pub fn connect_left(&self, f: impl Fn() + 'static) {
        *self.on_left.borrow_mut() = Some(Box::new(f));
    }

    /// Places the content for the first time: in its own window, or on the navigation stack. With the main window
    /// closed or hidden the content opens in its own window, since nobody would see the stack.
    pub fn show(self: &Rc<Self>, detached: bool) {
        let Some(content) = self.content.upgrade() else { return };
        self.watch_nav();
        if detached || !main_window_shows() {
            self.open_window(&content);
        } else {
            crate::ui::push(&content);
        }
        self.sync_button();
    }

    /// The button's action: detach an embedded page, attach a detached one. A detached page stays detached once the
    /// main window is closed (destroyed) or hidden: there is no stack left to show it, and the button hides.
    pub fn toggle(self: &Rc<Self>) {
        let Some(content) = self.content.upgrade() else { return };
        self.watch_nav();
        if self.is_detached() && !main_window_shows() {
            self.sync_button();
            return;
        }
        self.moving.set(true);
        if let Some(window) = self.window.take() {
            window.set_child(None::<&gtk::Widget>);
            window.destroy();
            crate::ui::push(&content);
        } else {
            if let Some(stack) = content.parent().and_downcast::<gtk::Stack>() {
                // the page below shows at once and this one leaves now, not after the pop transition
                if stack.visible_child().as_ref() == Some(&content)
                    && let Some(below) = content.prev_sibling()
                {
                    crate::shell::transitions::apply(&stack, crate::shell::transitions::Direction::Pop);
                    stack.set_visible_child(&below);
                }
                stack.remove(&content);
            }
            self.open_window(&content);
        }
        self.moving.set(false);
        self.sync_button();
        if let Some(f) = self.on_moved.borrow().as_ref() {
            f(self.is_detached());
        }
    }

    /// Leaves the content: closes the detached window, or pops the page off the stack.
    pub fn close(&self) {
        let window = self.window.borrow().clone();
        match window {
            Some(window) => window.close(),
            None => crate::ui::pop(),
        }
    }

    fn open_window(self: &Rc<Self>, content: &gtk::Widget) {
        let window = gtk::Window::builder().title(self.title.borrow().as_str()).default_width(960).default_height(540).build();
        if self.own_header {
            // an empty title bar hides GTK's default one; the content's header bar moves and closes the window
            window.set_titlebar(Some(&gtk::Box::new(gtk::Orientation::Horizontal, 0)));
        }
        if let Some(app) = gtk::gio::Application::default().and_downcast::<gtk::Application>() {
            window.set_application(Some(&app));
        }
        window.set_child(Some(content));
        let weak = Rc::downgrade(self);
        // closed by the user or by `close` (Attach destroys the window without a close request). The host lets go of
        // it, so the window and the content in it are freed once GTK destroys it.
        window.connect_close_request(move |_| {
            if let Some(host) = weak.upgrade() {
                host.window.take();
            }
            glib::Propagation::Proceed
        });
        *self.window.borrow_mut() = Some(window.clone());
        window.present();
    }

    /// Once, on first placement: the button follows the main window (hidden or closed: no Attach; shown again:
    /// Attach is back), and `connect_left` hears an embedded page that stops being the visible one. The handlers
    /// hold the host weakly and go when it drops.
    fn watch_nav(self: &Rc<Self>) {
        if !self.watches.borrow().is_empty() {
            return;
        }
        let window = crate::ui::window();
        let stack = crate::ui::stack();
        let sync = |host: &Rc<Self>| {
            let weak = Rc::downgrade(host);
            move |_: &gtk::ApplicationWindow| {
                if let Some(host) = weak.upgrade() {
                    host.sync_button();
                }
            }
        };
        let mut watches = self.watches.borrow_mut();
        watches.push((window.upcast_ref::<glib::Object>().downgrade(), window.connect_map(sync(self))));
        watches.push((window.upcast_ref::<glib::Object>().downgrade(), window.connect_unmap(sync(self))));
        let weak = Rc::downgrade(self);
        let id = stack.connect_visible_child_notify(move |stack| {
            let Some(host) = weak.upgrade() else { return };
            let Some(content) = host.content.upgrade() else { return };
            let embedded = !host.is_moving() && !host.is_detached() && content.parent().as_ref() == Some(stack.upcast_ref());
            if embedded && stack.visible_child().as_ref() != Some(&content)
                && let Some(f) = host.on_left.borrow().as_ref()
            {
                f();
            }
        });
        watches.push((stack.upcast_ref::<glib::Object>().downgrade(), id));
    }

    fn sync_button(&self) {
        let (icon, tip) = button_face(self.is_detached());
        self.button.set_visible(!self.is_detached() || main_window_shows());
        self.button.set_icon_name(icon);
        self.button.set_tooltip_text(Some(tip));
        self.button.update_property(&[gtk::accessible::Property::Label(tip)]);
    }
}

impl Drop for Detachable {
    fn drop(&mut self) {
        for (object, id) in self.watches.take() {
            if let Some(object) = object.upgrade() {
                object.disconnect(id);
            }
        }
    }
}

/// The main window can take the page back: it is neither closed (a destroyed window is unrealized and hidden) nor
/// hidden.
fn main_window_shows() -> bool {
    let window = crate::ui::window();
    window.is_realized() && window.is_visible()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_says_detach_while_embedded_and_attach_while_detached() {
        assert_eq!(button_face(false).1, "Detach into its own window");
        assert_eq!(button_face(true).1, "Attach to the main window");
        assert_ne!(button_face(false).0, button_face(true).0);
    }

    /// The content moves from the stack to a window and back as the same widget; it is unrealized only while the
    /// host says it is moving, and the remembered state follows each move.
    #[test]
    fn a_page_detaches_into_a_window_and_attaches_back_as_the_same_widget() {
        crate::testing::with_display(|| {
            let (stack, window) = crate::testing::nav();
            let below = gtk::Label::new(Some("below"));
            stack.add_child(&below);
            let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let host = Detachable::new(&page, "Episode 1", true);
            page.append(host.button());
            let moved = Rc::new(RefCell::new(Vec::new()));
            {
                let moved = Rc::clone(&moved);
                host.connect_moved(move |detached| moved.borrow_mut().push(detached));
            }
            let unrealized_while_moving = Rc::new(RefCell::new(Vec::new()));
            {
                let (host, seen) = (Rc::downgrade(&host), Rc::clone(&unrealized_while_moving));
                page.connect_unrealize(move |_| seen.borrow_mut().push(host.upgrade().is_some_and(|h| h.is_moving())));
            }
            let was_visible = window.is_visible();
            window.present();
            host.show(false);
            assert_eq!(stack.visible_child().as_ref(), Some(page.upcast_ref::<gtk::Widget>()));
            assert_eq!(host.button().tooltip_text().as_deref(), Some("Detach into its own window"));

            host.button().emit_clicked();
            assert!(host.is_detached());
            let own_window = page.root().and_downcast::<gtk::Window>().expect("the page is in a window");
            assert_ne!(Some(own_window.upcast::<gtk::Widget>()), stack.root().map(|r| r.upcast()), "a new window");
            assert_eq!(stack.visible_child().as_ref(), Some(below.upcast_ref::<gtk::Widget>()), "the page below shows");
            assert_eq!(host.button().tooltip_text().as_deref(), Some("Attach to the main window"));

            host.button().emit_clicked();
            assert!(!host.is_detached());
            assert_eq!(page.parent().as_ref(), Some(stack.upcast_ref::<gtk::Widget>()), "back on the stack");
            assert_eq!(stack.visible_child().as_ref(), Some(page.upcast_ref::<gtk::Widget>()));
            assert_eq!(*moved.borrow(), vec![true, false]);
            assert!(unrealized_while_moving.borrow().iter().all(|moving| *moving), "every unrealize so far was a move");

            // closing the detached window is not a move
            host.toggle();
            let detached = page.root().and_downcast::<gtk::Window>().expect("detached again");
            detached.close();
            while glib::MainContext::default().iteration(false) {}
            assert!(!host.is_detached(), "the host forgets a window the user closed");
            assert_eq!(unrealized_while_moving.borrow().last(), Some(&false));

            // the main window is gone (closed or hidden): Attach keeps the page in its own window and the button hides
            let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let host = Detachable::new(&page, "Episode 2", true);
            host.show(true);
            assert!(host.is_detached(), "show(true) detaches");
            window.set_visible(false);
            assert!(!main_window_shows(), "the main window is hidden (the test's `window` is the nav window)");
            assert!(!host.button().is_visible(), "the Attach button hides with the main window, no click needed");
            window.set_visible(true);
            while glib::MainContext::default().iteration(false) {}
            assert!(host.button().is_visible(), "and is back once the main window shows again");
            window.set_visible(false);
            host.button().emit_clicked();
            assert!(host.is_detached(), "nothing to attach to");
            let own = page.root().and_downcast::<gtk::Window>().expect("still in its own window");
            assert_ne!(own.upcast_ref::<gtk::Widget>(), window.upcast_ref::<gtk::Widget>());
            assert!(page.parent().as_ref() != Some(stack.upcast_ref::<gtk::Widget>()), "not pushed on the hidden stack");
            assert!(!host.button().is_visible(), "the Attach button hides");
            own.close();
            while glib::MainContext::default().iteration(false) {}

            // placed while the main window is hidden: its own window, not a stack nobody sees
            let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let host = Detachable::new(&page, "Episode 3", true);
            host.show(false);
            assert!(host.is_detached(), "no visible main window: opens detached");
            assert!(page.parent().as_ref() != Some(stack.upcast_ref::<gtk::Widget>()), "not pushed on the hidden stack");
            page.root().and_downcast::<gtk::Window>().expect("in its own window").close();
            while glib::MainContext::default().iteration(false) {}
            window.set_visible(was_visible);
        });
    }
}
