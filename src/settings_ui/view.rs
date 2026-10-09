//! A settings page that rebuilds itself when the inputs of its enable rules change, so a row a switch turns on or off
//! appears or disappears at once instead of at the next open (task T-47a-002).
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use kuro_prefs::store::PrefStore;

use crate::pref_model::model::Preference;
use crate::screens::Built;

/// How often the store's change queue is looked at.
const POLL: Duration = Duration::from_millis(150);

/// The page for `tree`. `state` reads what the rules depend on; when a store change leaves it different the page is
/// rebuilt with the scroll position kept. A change that leaves it equal (a slider drag) does not touch the widgets.
pub fn live_page<S: PartialEq + 'static>(
    store: &PrefStore,
    state: impl Fn() -> S + 'static,
    tree: impl Fn() -> Vec<Preference> + 'static,
    end: Vec<gtk::Widget>,
) -> Built {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_vexpand(true);
    holder.append(&crate::pref_model::page::body(&tree()).widget);
    let last = Rc::new(RefCell::new(state()));
    let events = store.subscribe();
    let weak = holder.downgrade();
    glib::timeout_add_local(POLL, move || {
        let Some(holder) = weak.upgrade() else { return glib::ControlFlow::Break };
        let mut any = false;
        while events.try_recv().is_ok() {
            any = true;
        }
        if any {
            let now = state();
            if now != *last.borrow() {
                *last.borrow_mut() = now;
                rebuild(&holder, &tree());
            }
        }
        glib::ControlFlow::Continue
    });
    Built { content: holder.upcast(), end }
}

fn rebuild(holder: &gtk::Box, tree: &[Preference]) {
    let old = holder.first_child().and_downcast::<gtk::ScrolledWindow>();
    let at = old.as_ref().map(|s| s.vadjustment().value()).unwrap_or(0.0);
    if let Some(old) = old {
        holder.remove(&old);
    }
    let page = crate::pref_model::page::body(tree);
    holder.append(&page.widget);
    let adjustment = page.scrolled.vadjustment();
    glib::idle_add_local_once(move || adjustment.set_value(at));
}

/// Every `gtk::Stack` at or below `root`.
fn stacks(root: &gtk::Widget, found: &mut Vec<gtk::Stack>) {
    if let Some(stack) = root.downcast_ref::<gtk::Stack>() {
        found.push(stack.clone());
    }
    let mut child = root.first_child();
    while let Some(widget) = child {
        stacks(&widget, found);
        child = widget.next_sibling();
    }
}

/// Shows the page `name` in every stack below `root` that has one (the Anime | Manga switcher of a library or
/// Categories page, and the header stack that follows it).
pub fn show_kind(root: &impl IsA<gtk::Widget>, name: &str) {
    let mut found = Vec::new();
    stacks(root.as_ref(), &mut found);
    for stack in found.iter().filter(|s| s.child_by_name(name).is_some()) {
        stack.set_visible_child_name(name);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::settings_ui::rows::{item, switch};
    use kuro_prefs::prefs::Prefs;

    fn spin(ms: u64) {
        let until = std::time::Instant::now() + Duration::from_millis(ms);
        while std::time::Instant::now() < until {
            glib::MainContext::default().iteration(false);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Needs a display; run alone: `cargo test -p kurobayomi settings_ui::view -- --ignored`.
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi settings_ui::view -- --ignored"]
    fn a_changed_rule_input_rebuilds_the_page_and_an_unchanged_one_does_not() {
        if gtk::init().is_err() {
            eprintln!("skipped: no display for the live page check");
            return;
        }
        let prefs = Rc::new(Prefs::in_memory());
        let builds = Rc::new(Cell::new(0));
        let (for_state, for_tree, counted) = (Rc::clone(&prefs), Rc::clone(&prefs), Rc::clone(&builds));
        let built = live_page(
            prefs.store(),
            move || for_state.reader.dual_page_split_paged.get(),
            move || {
                counted.set(counted.get() + 1);
                vec![item(switch("Split", &for_tree.reader.dual_page_split_paged, true))]
            },
            vec![],
        );
        assert_eq!(builds.get(), 1, "built once on open");
        prefs.reader.crop_borders.set(true).unwrap();
        spin(400);
        assert_eq!(builds.get(), 1, "a change that leaves the rule input alone does not rebuild");
        prefs.reader.dual_page_split_paged.set(true).unwrap();
        spin(400);
        assert_eq!(builds.get(), 2, "a changed rule input rebuilds once");
        drop(built);
    }
}
