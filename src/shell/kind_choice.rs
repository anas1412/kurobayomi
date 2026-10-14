//! The one Anime | Manga choice of the app. Every page with a kind switch (Library, Browse, Updates, History, Downloads,
//! Categories) follows it: a switch on one page moves them all, and the choice is kept across runs (`last_kind`).

use std::cell::{Cell, RefCell};

use gtk::glib;
use gtk::prelude::*;
use kuro_platform::kind::Kind;

thread_local! {
    /// Every following stack, with its page names for anime and for manga.
    static STACKS: RefCell<Vec<(glib::WeakRef<gtk::Stack>, [&'static str; 2])>> = const { RefCell::new(Vec::new()) };
    /// Set while the other stacks are being moved, so their own change is not taken as a new choice.
    static MOVING: Cell<bool> = const { Cell::new(false) };
}

/// The page name of `kind` among `names` (anime first).
fn name_of(kind: Kind, names: [&'static str; 2]) -> &'static str {
    if kind == Kind::Anime { names[0] } else { names[1] }
}

/// The kind a page name stands for.
fn kind_of(name: &str, names: [&'static str; 2]) -> Option<Kind> {
    [Kind::Anime, Kind::Manga].into_iter().find(|&kind| name_of(kind, names) == name)
}

/// The kept choice; anime before the app data is open.
pub fn current() -> Kind {
    if crate::data::is_installed() && crate::data::app().base.last_kind.get() == "manga" { Kind::Manga } else { Kind::Anime }
}

fn keep(kind: Kind) {
    if crate::data::is_installed() {
        let value = if kind == Kind::Manga { "manga" } else { "anime" };
        if let Err(error) = crate::data::app().base.last_kind.set(value.to_owned()) {
            tracing::warn!(%error, "the Anime | Manga choice could not be kept");
        }
    }
}

/// Makes `stack` (already holding both pages, named `names`, anime first) show the shared choice and follow it.
pub fn follow(stack: &gtk::Stack, names: [&'static str; 2]) {
    stack.set_visible_child_name(name_of(current(), names));
    STACKS.with(|stacks| stacks.borrow_mut().push((stack.downgrade(), names)));
    stack.connect_visible_child_name_notify(move |stack| {
        if MOVING.get() {
            return;
        }
        let Some(kind) = stack.visible_child_name().and_then(|name| kind_of(&name, names)) else { return };
        keep(kind);
        // a search belongs to the kind it was made in
        if let Some(page) = stack.parent() {
            crate::ui::close_searches(&page);
        }
        MOVING.set(true);
        STACKS.with(|stacks| {
            stacks.borrow_mut().retain(|(weak, _)| weak.upgrade().is_some());
            for (other, other_names) in stacks.borrow().iter() {
                if let Some(other) = other.upgrade().filter(|other| other != stack) {
                    other.set_visible_child_name(name_of(kind, *other_names));
                    if let Some(page) = other.parent() {
                        crate::ui::close_searches(&page);
                    }
                }
            }
        });
        MOVING.set(false);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(names: [&'static str; 2]) -> gtk::Stack {
        let stack = gtk::Stack::new();
        for name in names {
            stack.add_named(&gtk::Label::new(Some(name)), Some(name));
        }
        stack
    }

    #[test]
    fn a_switch_on_one_page_moves_every_page() {
        crate::testing::with_display(|| {
            let (library, browse) = (stack(["anime", "manga"]), stack(["0", "1"]));
            follow(&library, ["anime", "manga"]);
            follow(&browse, ["0", "1"]);
            library.set_visible_child_name("manga");
            assert_eq!(browse.visible_child_name().as_deref(), Some("1"), "browse follows the library");
            browse.set_visible_child_name("0");
            assert_eq!(library.visible_child_name().as_deref(), Some("anime"), "and the other way");
        });
    }

    #[test]
    fn switching_the_kind_closes_an_open_search() {
        crate::testing::with_display(|| {
            let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
            let bar = gtk::SearchBar::new();
            let kinds = stack(["anime", "manga"]);
            page.append(&bar);
            page.append(&kinds);
            follow(&kinds, ["anime", "manga"]);
            bar.set_search_mode(true);
            kinds.set_visible_child_name(if kinds.visible_child_name().as_deref() == Some("anime") { "manga" } else { "anime" });
            assert!(!bar.is_search_mode(), "the search closed with the switch");
        });
    }

    #[test]
    fn page_names_map_to_kinds_anime_first() {
        assert_eq!(kind_of("1", ["0", "1"]), Some(Kind::Manga));
        assert_eq!(kind_of("anime", ["anime", "manga"]), Some(Kind::Anime));
        assert_eq!(kind_of("spinner", ["anime", "manga"]), None);
    }
}
