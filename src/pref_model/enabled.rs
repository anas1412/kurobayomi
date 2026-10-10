//! A live enabled flag for a settings item (T-47ba-004): flipping it makes the row sensitive or insensitive, and
//! turning it on slides the row in, without rebuilding the page.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;

#[derive(Clone)]
pub struct EnabledFlag(Rc<Inner>);

struct Inner {
    value: Cell<bool>,
    subscribers: RefCell<Vec<Rc<dyn Fn(bool)>>>,
}

impl EnabledFlag {
    pub fn new(value: bool) -> Self {
        Self(Rc::new(Inner { value: Cell::new(value), subscribers: RefCell::new(Vec::new()) }))
    }

    pub fn get(&self) -> bool {
        self.0.value.get()
    }

    /// Sets the flag and tells the subscribers, only when the value changed.
    pub fn set(&self, value: bool) {
        if self.0.value.replace(value) == value {
            return;
        }
        let subscribers: Vec<Rc<dyn Fn(bool)>> = self.0.subscribers.borrow().clone();
        for subscriber in subscribers {
            subscriber(value);
        }
    }

    pub fn subscribe(&self, subscriber: impl Fn(bool) + 'static) {
        self.0.subscribers.borrow_mut().push(Rc::new(subscriber));
    }
}

impl PartialEq for EnabledFlag {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for EnabledFlag {}

impl fmt::Debug for EnabledFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnabledFlag({})", self.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribers_hear_only_real_changes() {
        let flag = EnabledFlag::new(false);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        flag.subscribe(move |v| sink.borrow_mut().push(v));
        flag.set(false);
        flag.set(true);
        flag.set(true);
        flag.set(false);
        assert_eq!(*seen.borrow(), vec![true, false]);
        assert!(!flag.get());
    }

    #[test]
    fn clones_share_one_value() {
        let a = EnabledFlag::new(true);
        let b = a.clone();
        b.set(false);
        assert!(!a.get());
        assert_eq!(a, b);
        assert_ne!(a, EnabledFlag::new(false));
    }
}
