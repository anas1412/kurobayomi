//! Source-ready placeholder (T-01a-037). A view that needs sources shows a centred spinner until the
//! source gate opens, then crossfades to its content over 150 ms. A gate that is already open shows the
//! content at once, with no spinner.

use std::sync::OnceLock;

use gtk::prelude::*;
use gtk::{Stack, StackTransitionType, Widget, glib};
use kuro_core::sources::SourceGate;
use tokio::sync::watch;

/// Length of the crossfade from the spinner to the content.
// No call site until the source screens land (T-01a-037 wiring).
pub const CROSSFADE_MS: u32 = 150;

static LOADED: OnceLock<watch::Sender<bool>> = OnceLock::new();

fn loaded() -> &'static watch::Sender<bool> {
    LOADED.get_or_init(|| watch::channel(false).0)
}

/// The source-ready flag: `false` until the built-in and recipe sources have been read at start, then `true` for good.
/// Every view that needs sources waits on a receiver from here, so the spinner is real.
pub fn subscribe() -> watch::Receiver<bool> {
    loaded().subscribe()
}

/// Called once the sources are read at start (after the built-in and recipe sources have started).
pub fn mark_loaded() {
    loaded().send_replace(true);
}

/// What the placeholder shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Spinner,
    Content,
}

/// The state a placeholder starts in for a gate that is already open or still closed.
pub fn initial(gate_open: bool) -> Shown {
    if gate_open { Shown::Content } else { Shown::Spinner }
}

/// `content` once the source gate is open. Until then a centred spinner is shown, and the content
/// replaces it with a crossfade when the gate opens.
// No call site until the source screens land (T-01a-037 wiring).
pub fn when_sources_ready(gate: &SourceGate, content: &impl IsA<Widget>) -> Widget {
    let content: Widget = content.clone().upcast();
    let mut ready = gate.ready_watch();
    if initial(*ready.borrow_and_update()) == Shown::Content {
        return content;
    }
    let spinner = crate::ui::loading(crate::ui::LoadSize::Page);
    let stack = Stack::builder()
        .transition_type(StackTransitionType::Crossfade)
        .transition_duration(CROSSFADE_MS)
        .build();
    stack.add_named(&spinner, Some("spinner"));
    stack.add_named(&content, Some("content"));
    stack.set_visible_child_name("spinner");
    let page = stack.clone();
    glib::spawn_future_local(async move {
        while !*ready.borrow_and_update() {
            if ready.changed().await.is_err() {
                return;
            }
        }
        page.set_visible_child_name("content");
    });
    stack.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_gate_shows_the_spinner_first() {
        let (_registry, ready) = tokio::sync::watch::channel(false);
        let gate = SourceGate::new(ready);
        assert_eq!(initial(*gate.ready_watch().borrow()), Shown::Spinner);
    }

    #[test]
    fn the_source_flag_is_closed_until_the_sources_are_read() {
        // Nothing in the test binary calls mark_loaded, so the process-wide flag starts closed.
        assert!(!*subscribe().borrow());
        let gate = SourceGate::new(subscribe());
        assert_eq!(initial(*gate.ready_watch().borrow()), Shown::Spinner);
    }

    #[test]
    fn an_already_open_gate_skips_the_spinner() {
        let (registry, ready) = tokio::sync::watch::channel(false);
        registry.send_replace(true);
        let gate = SourceGate::new(ready);
        assert_eq!(initial(*gate.ready_watch().borrow()), Shown::Content);
    }
}
