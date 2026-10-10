//! Root navigator (Kotlin `Navigator`, Voyager). One stack per window; `Route::Home` is always the root.
//!
//! `Navigator` is pure. `NavHost` puts the stack on a `gtk::Stack` and runs the slide transitions.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

/// Duration of a forward or back slide, in milliseconds (T-36a-002).
pub const TRANSITION_MS: u32 = 250;

/// Kind of catalogue an entry or search belongs to (Kotlin `DeepLinkScreenType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Anime,
    Manga,
}

/// One page of the stack. Only `Home` is ever the root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Home,
    Entry { kind: Kind, id: i64, from_source: bool },
    GlobalSearch { kind: Kind, query: String },
    Settings { dest: Option<String> },
    Downloads,
    Onboarding,
    RestoreBackup { uri: String },
}

/// Direction of the last stack change. A pop slides back; everything else slides forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Back,
}

/// The stack of routes. The invariant `stack[0] == Route::Home` holds for its whole life.
#[derive(Debug)]
pub struct Navigator {
    stack: Vec<Route>,
    last: Direction,
}

impl Default for Navigator {
    fn default() -> Self {
        Self::new()
    }
}

impl Navigator {
    pub fn new() -> Self {
        Self { stack: vec![Route::Home], last: Direction::Forward }
    }

    pub fn stack(&self) -> &[Route] {
        &self.stack
    }

    /// The top route (Voyager `lastItem`).
    pub fn last(&self) -> &Route {
        self.stack.last().unwrap_or(&Route::Home)
    }

    pub fn direction(&self) -> Direction {
        self.last
    }

    pub fn push(&mut self, route: Route) {
        self.stack.push(route);
        self.last = Direction::Forward;
    }

    /// Removes the top route. Returns `None` on the root, which is never removed.
    pub fn pop(&mut self) -> Option<Route> {
        if self.stack.len() <= 1 {
            return None;
        }
        self.last = Direction::Back;
        self.stack.pop()
    }

    /// Replaces the top route and returns the old one. A replace on the root is refused and returns `None`.
    pub fn replace(&mut self, route: Route) -> Option<Route> {
        if self.stack.len() <= 1 {
            return None;
        }
        self.last = Direction::Forward;
        self.stack.last_mut().map(|top| std::mem::replace(top, route))
    }

    /// Pops everything above `Home` and returns the popped routes, top first.
    pub fn pop_until_root(&mut self) -> Vec<Route> {
        let mut popped = Vec::new();
        while let Some(route) = self.pop() {
            popped.push(route);
        }
        popped
    }
}

/// A `gtk::Stack` showing the navigator's top page. Popped pages are removed once the back slide has finished.
pub struct NavHost {
    stack: gtk::Stack,
    nav: RefCell<Navigator>,
    /// Child names, parallel to the navigator stack (`names[0]` is the home page).
    names: RefCell<Vec<String>>,
    serial: RefCell<u64>,
}

impl NavHost {
    pub fn new(home: &impl IsA<gtk::Widget>) -> Rc<Self> {
        let stack = gtk::Stack::new();
        stack.set_transition_duration(TRANSITION_MS);
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.add_named(home, Some("route-0"));
        stack.set_visible_child_name("route-0");
        Rc::new(Self { stack, nav: RefCell::new(Navigator::new()), names: RefCell::new(vec!["route-0".into()]), serial: RefCell::new(0) })
    }

    pub fn widget(&self) -> &gtk::Stack {
        &self.stack
    }

    /// Pushes a page under a fresh child name, so two pushes of the same route are two pages.
    pub fn push(&self, route: Route, page: &impl IsA<gtk::Widget>) {
        let name = {
            let mut serial = self.serial.borrow_mut();
            *serial += 1;
            format!("route-{}", *serial)
        };
        self.stack.add_named(page, Some(&name));
        self.nav.borrow_mut().push(route);
        self.names.borrow_mut().push(name.clone());
        self.stack.set_transition_type(gtk::StackTransitionType::SlideLeft);
        self.stack.set_visible_child_name(&name);
    }

    /// Pops the top page. Returns false on the root.
    pub fn pop(&self) -> bool {
        if self.nav.borrow_mut().pop().is_none() {
            return false;
        }
        let Some(gone) = self.names.borrow_mut().pop() else { return false };
        let previous = self.names.borrow().last().cloned().unwrap_or_else(|| "route-0".into());
        self.stack.set_transition_type(gtk::StackTransitionType::SlideRight);
        self.stack.set_visible_child_name(&previous);
        // the page leaves the stack after the slide, so the transition is not cut short
        let stack = self.stack.clone();
        gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(u64::from(TRANSITION_MS) + 10), move || {
            if let Some(page) = stack.child_by_name(&gone) {
                stack.remove(&page);
            }
        });
        true
    }

    /// Pops every page above the root.
    pub fn pop_until_root(&self) {
        while self.pop() {}
    }

    /// The top route.
    pub fn last(&self) -> Route {
        self.nav.borrow().last().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64) -> Route {
        Route::Entry { kind: Kind::Anime, id, from_source: false }
    }

    #[test]
    fn root_is_home_and_never_popped() {
        let mut nav = Navigator::new();
        assert_eq!(nav.stack(), &[Route::Home]);
        assert_eq!(nav.pop(), None);
        assert_eq!(nav.stack(), &[Route::Home]);
    }

    #[test]
    fn push_then_pop_restores_the_previous_top() {
        let mut nav = Navigator::new();
        nav.push(entry(1));
        nav.push(Route::Downloads);
        assert_eq!(nav.last(), &Route::Downloads);
        assert_eq!(nav.pop(), Some(Route::Downloads));
        assert_eq!(nav.last(), &entry(1));
    }

    #[test]
    fn replace_swaps_the_top_and_refuses_the_root() {
        let mut nav = Navigator::new();
        assert_eq!(nav.replace(Route::Downloads), None);
        assert_eq!(nav.stack(), &[Route::Home]);
        nav.push(entry(1));
        assert_eq!(nav.replace(entry(2)), Some(entry(1)));
        assert_eq!(nav.stack(), &[Route::Home, entry(2)]);
    }

    #[test]
    fn pop_until_root_returns_the_popped_pages_top_first() {
        let mut nav = Navigator::new();
        nav.push(entry(1));
        nav.push(entry(2));
        nav.push(Route::Settings { dest: None });
        let popped = nav.pop_until_root();
        assert_eq!(popped, vec![Route::Settings { dest: None }, entry(2), entry(1)]);
        assert_eq!(nav.stack(), &[Route::Home]);
    }

    #[test]
    fn direction_follows_the_last_change() {
        let mut nav = Navigator::new();
        nav.push(entry(1));
        assert_eq!(nav.direction(), Direction::Forward);
        nav.pop();
        assert_eq!(nav.direction(), Direction::Back);
        nav.pop();
        assert_eq!(nav.direction(), Direction::Back, "a refused pop keeps the last direction");
    }
}
