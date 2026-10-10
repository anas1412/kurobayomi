//! A toast that stays for a given time at the bottom of one screen (T-42a-016, T-42a-024). The app's own toast layer
//! has fixed lengths, and the History rules name 4 seconds, so the screens keep this one in an overlay. Up to three
//! more texts wait behind the one on screen, and a fourth is dropped (U§1.7).
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;

/// How many texts may wait behind the one on screen.
pub const QUEUE_LIMIT: usize = 3;

/// The texts waiting for the one on screen, each with the time it stays. Pure.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ToastQueue {
    waiting: VecDeque<(String, u64)>,
}

impl ToastQueue {
    /// Queues a text. Returns `false` and drops it when [`QUEUE_LIMIT`] texts already wait.
    pub fn push(&mut self, text: &str, ms: u64) -> bool {
        if self.waiting.len() >= QUEUE_LIMIT {
            return false;
        }
        self.waiting.push_back((text.to_owned(), ms));
        true
    }

    /// The next text to show, oldest first.
    pub fn pop(&mut self) -> Option<(String, u64)> {
        self.waiting.pop_front()
    }

    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }
}

pub struct ScreenToast {
    label: gtk::Label,
    /// Counts shown toasts; a timer only acts on the toast it was started for.
    generation: Rc<Cell<u64>>,
    waiting: Rc<RefCell<ToastQueue>>,
}

impl Default for ScreenToast {
    fn default() -> Self {
        Self::new()
    }
}

impl ScreenToast {
    pub fn new() -> Self {
        let label = gtk::Label::builder()
            .wrap(true)
            .max_width_chars(60)
            .css_classes(["osd"])
            .halign(gtk::Align::Center)
            .valign(gtk::Align::End)
            .margin_bottom(24)
            .visible(false)
            .can_target(false)
            .focusable(false)
            .build();
        Self { label, generation: Rc::new(Cell::new(0)), waiting: Rc::default() }
    }

    /// The label to add as an overlay of the screen.
    pub fn widget(&self) -> &gtk::Label {
        &self.label
    }

    /// Shows `text` for `ms` milliseconds, or queues it behind the toast on screen (dropped when the queue is full).
    pub fn show(&self, text: &str, ms: u64) {
        if self.is_showing() {
            self.waiting.borrow_mut().push(text, ms);
            return;
        }
        display(self.label.clone(), Rc::clone(&self.generation), Rc::clone(&self.waiting), text, ms);
    }

    pub fn is_showing(&self) -> bool {
        self.label.is_visible()
    }
}

/// Puts `text` on screen for `ms`, then shows the next waiting text, or hides the label when none waits.
fn display(label: gtk::Label, current: Rc<Cell<u64>>, waiting: Rc<RefCell<ToastQueue>>, text: &str, ms: u64) {
    let generation = current.get() + 1;
    current.set(generation);
    label.set_label(text);
    label.set_visible(true);
    glib::timeout_add_local_once(Duration::from_millis(ms), move || {
        if current.get() != generation {
            return;
        }
        let next = waiting.borrow_mut().pop();
        match next {
            Some((text, ms)) => display(label, current, waiting, &text, ms),
            None => label.set_visible(false),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_keeps_three_waiting_texts_and_drops_the_fourth() {
        let mut queue = ToastQueue::default();
        assert!(queue.push("one", 4_000));
        assert!(queue.push("two", 4_000));
        assert!(queue.push("three", 4_000));
        assert!(!queue.push("four", 4_000), "a fourth waiting text is dropped");
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn queue_pops_oldest_first_and_keeps_the_time() {
        let mut queue = ToastQueue::default();
        queue.push("Updating library", 4_000);
        queue.push("An update is already running", 4_000);
        assert_eq!(queue.pop(), Some(("Updating library".to_owned(), 4_000)));
        assert_eq!(queue.pop(), Some(("An update is already running".to_owned(), 4_000)));
        assert!(queue.is_empty());
        assert_eq!(queue.pop(), None);
    }
}
