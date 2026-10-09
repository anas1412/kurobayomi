//! Preference to widget bridge (`PreferenceMutableState.kt`, T-52a-001).
//!
//! `bind` seeds a widget from `Pref::get()` and then follows the preference. A widget never changes on a UI
//! write: the write goes through `write` (`Pref::set`) and the widget updates when the change comes back through
//! the watch, the same path another window's write takes.
#![allow(dead_code)] // wired by the settings and reader screens as they land

use std::time::Duration;

use gtk::glib::{self, prelude::*};
use kuro_prefs::pref::Pref;
use kuro_prefs::values::PrefType;

/// How often the main loop drains the watch. One frame at 60 Hz.
// ponytail: one polling task per binding, polling a std mpsc; swap for a single dispatcher per store if bindings reach the hundreds.
const POLL: Duration = Duration::from_millis(16);

/// Calls `apply(widget, value)` now with the stored value and again each time the preference changes. The binding
/// ends when `widget` is dropped: the task holds only a weak reference and, once it fails to upgrade it, ends and
/// drops the watch with it.
pub fn bind<W, T>(widget: &W, pref: &Pref<T>, apply: impl Fn(&W, T) + 'static)
where
    W: IsA<glib::Object>,
    T: PrefType,
{
    // subscribe before the seed read so a write in between is not lost
    let watch = pref.watch();
    apply(widget, pref.get());
    let weak = widget.downgrade();
    glib::spawn_future_local(async move {
        loop {
            glib::timeout_future(POLL).await;
            let Some(widget) = weak.upgrade() else {
                return;
            };
            if let Some(value) = watch.try_changed() {
                apply(&widget, value);
            }
        }
    });
}

/// A write from the UI. It does not touch any widget; bound widgets follow once the store echoes the change. A save
/// failure keeps the value in memory and is logged.
pub fn write<T: PrefType>(pref: &Pref<T>, value: T) {
    if let Err(e) = pref.set(value) {
        tracing::warn!(key = pref.key(), "preference is not on disk yet: {e}");
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::Instant;

    use kuro_prefs::store::PrefStore;

    use super::*;

    fn widget() -> glib::Object {
        glib::Object::new::<glib::Object>()
    }

    /// Runs the context until `done` holds or five seconds pass.
    fn pump(ctx: &glib::MainContext, done: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() && Instant::now() < deadline {
            ctx.iteration(false);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Runs the context for a fixed time, for asserting that nothing happens.
    fn settle(ctx: &glib::MainContext) {
        let end = Instant::now() + Duration::from_millis(120);
        while Instant::now() < end {
            ctx.iteration(false);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn on_context(test: impl FnOnce(&glib::MainContext)) {
        let ctx = glib::MainContext::new();
        ctx.with_thread_default(|| test(&ctx)).expect("the test thread owns the context");
    }

    fn cell_binding(pref: &Pref<i32>) -> (glib::Object, Rc<Cell<i32>>) {
        let w = widget();
        let shown = Rc::new(Cell::new(-1));
        let sink = Rc::clone(&shown);
        bind(&w, pref, move |_, v| sink.set(v));
        (w, shown)
    }

    #[test]
    fn get_seeds_the_widget_and_a_set_from_another_window_reaches_two_bound_widgets() {
        on_context(|ctx| {
            let store = PrefStore::in_memory();
            let pref = store.pref("n", 7i32);
            let (_a, shown_a) = cell_binding(&pref);
            let (_b, shown_b) = cell_binding(&pref);
            assert_eq!((shown_a.get(), shown_b.get()), (7, 7));
            // a second window holds its own handle on the same store
            let other_window = store.pref("n", 7i32);
            other_window.set(9).unwrap();
            pump(ctx, || shown_a.get() == 9 && shown_b.get() == 9);
            assert_eq!((shown_a.get(), shown_b.get()), (9, 9));
        });
    }

    #[test]
    fn a_ui_write_changes_the_widget_only_after_the_watch_echo() {
        on_context(|ctx| {
            let store = PrefStore::in_memory();
            let pref = store.pref("n", 1i32);
            let (_w, shown) = cell_binding(&pref);
            write(&pref, 5);
            assert_eq!(pref.get(), 5);
            assert_eq!(shown.get(), 1, "the widget has not been touched by the write itself");
            pump(ctx, || shown.get() == 5);
            assert_eq!(shown.get(), 5);
        });
    }

    #[test]
    fn dropping_the_widget_ends_the_subscription_and_later_changes_do_not_reach_it() {
        struct Dropped(Rc<Cell<bool>>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        on_context(|ctx| {
            let store = PrefStore::in_memory();
            let pref = store.pref("n", 1i32);
            let w = widget();
            let calls = Rc::new(Cell::new(0));
            let closed = Rc::new(Cell::new(false));
            let (counter, guard) = (Rc::clone(&calls), Dropped(Rc::clone(&closed)));
            bind(&w, &pref, move |_, _| {
                let _keep = &guard;
                counter.set(counter.get() + 1);
            });
            assert_eq!(calls.get(), 1);
            drop(w);
            pref.set(2).unwrap();
            pump(ctx, || closed.get());
            assert!(closed.get(), "the timer and its watch are gone");
            pref.set(3).unwrap();
            settle(ctx);
            assert_eq!(calls.get(), 1, "the destroyed widget was never applied again");
        });
    }

    #[test]
    fn setting_an_equal_value_does_not_emit() {
        on_context(|ctx| {
            let store = PrefStore::in_memory();
            let pref = store.pref("n", 1i32);
            let w = widget();
            let calls = Rc::new(Cell::new(0));
            let counter = Rc::clone(&calls);
            bind(&w, &pref, move |_, _| counter.set(counter.get() + 1));
            pref.set(4).unwrap();
            pump(ctx, || calls.get() == 2);
            pref.set(4).unwrap();
            write(&pref, 4);
            settle(ctx);
            assert_eq!(calls.get(), 2, "seed plus the one real change");
        });
    }
}
