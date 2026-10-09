//! Repeating icon button (T-49a-041): clicks on press, then repeats on a schedule that starts at
//! 750 ms and shrinks by a quarter each step down to 5 ms. Release, leaving the button, a cancelled
//! gesture and unrealize stop the repeat (E-49-23, E-49-24, R-49-06). The timer is cancelled, not
//! left running. Enter and Space (the button's `activate` signal) fire one step with no timer
//! (T-49a-041, T-49a-055); a pointer click never emits `activate`, so it does not double up.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib::{self, SourceId};
use gtk::prelude::*;

use crate::theme::tokens::{REPEAT_DECAY, REPEAT_MAX_MS, REPEAT_MIN_MS};

/// The delay after `current`: `max(5, current - current * 0.25)`, truncated (TEST-49-14).
pub fn next_delay(current: u64) -> u64 {
    let next = (current as f64 - current as f64 * REPEAT_DECAY) as u64;
    next.max(REPEAT_MIN_MS)
}

/// The first `count` gaps between clicks, starting at 750 ms.
pub fn delays(count: usize) -> Vec<u64> {
    let mut out = Vec::with_capacity(count);
    let mut delay = REPEAT_MAX_MS;
    for _ in 0..count {
        out.push(delay);
        delay = next_delay(delay);
    }
    out
}

/// How many clicks happen while the button is held until `release_at_ms`. The press click is at 0.
pub fn clicks_until(release_at_ms: u64) -> usize {
    let mut at = 0u64;
    let mut delay = REPEAT_MAX_MS;
    let mut clicks = 0;
    while at <= release_at_ms {
        clicks += 1;
        at += delay;
        delay = next_delay(delay);
    }
    clicks
}

/// The repeat state: the callback and the one pending timer.
struct Repeater {
    pending: RefCell<Option<SourceId>>,
    on_click: Box<dyn Fn()>,
}

impl Repeater {
    /// Keyboard activation: one step, no repeat.
    fn step(&self) {
        (self.on_click)();
    }

    /// Press: click now and arm the first repeat.
    fn press(self: &Rc<Self>) {
        self.stop();
        (self.on_click)();
        self.arm(REPEAT_MAX_MS);
    }

    fn arm(self: &Rc<Self>, delay: u64) {
        let me = self.clone();
        let id = glib::timeout_add_local_once(Duration::from_millis(delay), move || {
            me.pending.borrow_mut().take();
            (me.on_click)();
            me.arm(next_delay(delay));
        });
        *self.pending.borrow_mut() = Some(id);
    }

    /// Cancels the pending repeat, if any.
    fn stop(&self) {
        let pending = self.pending.borrow_mut().take();
        if let Some(id) = pending {
            id.remove();
        }
    }
}

/// A flat icon button that repeats `on_click` while held.
pub fn repeat_button(icon_name: &str, on_click: impl Fn() + 'static) -> gtk::Button {
    let button = gtk::Button::from_icon_name(icon_name);
    let rep = Rc::new(Repeater { pending: RefCell::new(None), on_click: Box::new(on_click) });

    let press = gtk::GestureClick::new();
    {
        let rep = rep.clone();
        press.connect_pressed(move |_, _, _, _| rep.press());
    }
    {
        let rep = rep.clone();
        press.connect_released(move |_, _, _, _| rep.stop());
    }
    {
        let rep = rep.clone();
        press.connect_cancel(move |_, _| rep.stop());
    }
    button.add_controller(press);

    let motion = gtk::EventControllerMotion::new();
    {
        let rep = rep.clone();
        motion.connect_leave(move |_| rep.stop());
    }
    button.add_controller(motion);

    {
        let rep = rep.clone();
        button.connect_activate(move |_| rep.step());
    }

    button.connect_unrealize(move |_| rep.stop());
    button
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-49-14
    #[test]
    fn schedule_matches_the_source_for_twenty_steps() {
        let expected = [
            750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5, 5,
        ];
        assert_eq!(delays(20), expected.to_vec());
    }

    // TEST-49-15: release at 1500 ms leaves clicks at 0, 750 and 1312.
    #[test]
    fn release_cancels_the_later_clicks() {
        assert_eq!(clicks_until(1500), 3);
        assert_eq!(clicks_until(0), 1);
        assert_eq!(clicks_until(750), 2);
    }

    fn counting_repeater() -> (Rc<Repeater>, Rc<std::cell::Cell<u32>>) {
        let count = Rc::new(std::cell::Cell::new(0));
        let seen = count.clone();
        let rep = Rc::new(Repeater {
            pending: RefCell::new(None),
            on_click: Box::new(move || seen.set(seen.get() + 1)),
        });
        (rep, count)
    }

    // T-49a-041: Enter or Space is one step and arms no timer.
    #[test]
    fn keyboard_activation_fires_once_and_arms_nothing() {
        let (rep, count) = counting_repeater();
        rep.step();
        assert_eq!(count.get(), 1);
        assert!(rep.pending.borrow().is_none());
    }

    // Stopping with no pending timer (a cancel after a release, a leave with no press) is harmless.
    #[test]
    fn stop_without_a_pending_timer_is_a_no_op() {
        let (rep, count) = counting_repeater();
        rep.stop();
        rep.stop();
        assert_eq!(count.get(), 0);
        assert!(rep.pending.borrow().is_none());
    }

    #[test]
    fn delay_never_goes_below_five() {
        assert_eq!(next_delay(5), 5);
        assert_eq!(next_delay(6), 5);
    }
}
