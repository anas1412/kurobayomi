//! Tabbed page shell (T-42a-007, T-42a-021): a title, an optional search entry, a two-button sub-tab strip (Anime,
//! Manga) and a stack with a slide transition. Each sub-tab keeps its own search text; the entry shows the visible one.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GBox, Label, Orientation as O};

/// The sub-tab titles, in stack order: index 0 is Anime, index 1 is Manga (behaviour 0.2).
pub const SUB_TABS: [&str; 2] = ["Anime", "Manga"];

/// The search text of each sub-tab, kept apart.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubTabSearch {
    texts: [String; 2],
}

impl SubTabSearch {
    pub fn set(&mut self, sub_tab: usize, text: &str) {
        if let Some(slot) = self.texts.get_mut(sub_tab) {
            *slot = text.to_owned();
        }
    }

    pub fn get(&self, sub_tab: usize) -> &str {
        self.texts.get(sub_tab).map_or("", String::as_str)
    }
}

/// Builds the shell around two children (Anime, Manga). `searchable` adds the search entry.
pub fn tabbed(title: &str, searchable: bool, children: [gtk::Widget; 2]) -> GBox {
    let root = GBox::new(O::Vertical, 0);
    root.append(&Label::builder().label(title).css_classes(["title-2"]).xalign(0.0).margin_start(16).margin_top(12).build());

    let stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::SlideLeftRight).vexpand(true).build();
    for (i, child) in children.iter().enumerate() {
        stack.add_titled(child, Some(&i.to_string()), SUB_TABS[i]);
    }

    let search_state = Rc::new(RefCell::new(SubTabSearch::default()));
    let strip = GBox::new(O::Horizontal, 0);
    strip.add_css_class("linked");
    strip.set_margin_start(16);
    strip.set_margin_end(16);
    let mut first: Option<gtk::ToggleButton> = None;
    for (i, label) in SUB_TABS.iter().enumerate() {
        let button = gtk::ToggleButton::builder().label(*label).hexpand(true).build();
        if let Some(group) = &first {
            button.set_group(Some(group));
        } else {
            first = Some(button.clone());
        }
        let stack_for_click = stack.clone();
        button.connect_toggled(move |b| {
            if b.is_active() {
                stack_for_click.set_visible_child_name(&i.to_string());
            }
        });
        strip.append(&button);
    }
    if let Some(first) = &first {
        first.set_active(true);
    }

    if searchable {
        let entry = gtk::SearchEntry::builder().placeholder_text("Search").margin_start(16).margin_end(16).build();
        let state = search_state.clone();
        let stack_for_text = stack.clone();
        let entry_for_text = entry.clone();
        entry.connect_search_changed(move |e| {
            let index = visible_index(&stack_for_text);
            state.borrow_mut().set(index, &e.text());
            entry_for_text.set_text(&e.text());
        });
        let state = search_state.clone();
        let entry_for_switch = entry.clone();
        stack.connect_visible_child_name_notify(move |s| {
            let text = state.borrow().get(visible_index(s)).to_owned();
            entry_for_switch.set_text(&text);
        });
        root.append(&entry);
    }
    root.append(&strip);
    root.append(&stack);
    root
}

fn visible_index(stack: &gtk::Stack) -> usize {
    stack.visible_child_name().and_then(|name| name.parse().ok()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_search_text_is_kept_per_sub_tab() {
        let mut search = SubTabSearch::default();
        search.set(0, "naruto");
        search.set(1, "berserk");
        assert_eq!(search.get(0), "naruto");
        assert_eq!(search.get(1), "berserk");
        search.set(5, "ignored");
        assert_eq!(search.get(5), "");
    }

    #[test]
    fn tabs_labels_match_behaviour() {
        assert_eq!(SUB_TABS, ["Anime", "Manga"]);
    }
}
