//! The player settings root as a widget (task T-48a-010): a sidebar beside the open page at 720 px and up, a plain
//! list page whose rows push the sub-screens below it. The rows and the breakpoint are in `mod.rs`.
use std::rc::Rc;

use gtk::prelude::*;

use super::{layout_for, root_rows, Layout};
use crate::pref_model::page;
use crate::screens::{built, Built};
use crate::settings_ui::search;

/// The sub-screen `id` for the right-hand pane: its title and header actions on a bar, then its content.
fn pane(id: &str) -> gtk::Widget {
    let screen = crate::ui::screen(id);
    let (content, end): (gtk::Widget, Vec<gtk::Widget>) = match crate::screens::body(id) {
        Some(b) => (b.content, b.end),
        None => {
            let column = crate::ui::column();
            crate::ui::groups_into(&column, &screen.groups);
            (crate::ui::scrolled(&column).upcast(), crate::ui::toolbar(&screen))
        }
    };
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    crate::ui::margins(&bar, 6);
    bar.append(&gtk::Label::builder().label(&screen.title).xalign(0.0).hexpand(true).css_classes(["title-4"]).build());
    for widget in end {
        bar.append(&widget);
    }
    let pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
    pane.append(&bar);
    content.set_vexpand(true);
    pane.append(&content);
    pane.upcast()
}

/// The header search button of the player settings: the settings search over the player screens (T-48a-011).
fn search_button() -> gtk::Widget {
    let button = crate::ui::icon_button("system-search-symbolic", "Search");
    button.update_property(&[gtk::accessible::Property::Label("Search")]);
    button.connect_clicked(|_| {
        if let Some(page) = search::player_page() {
            crate::ui::push(&crate::ui::page("Search settings", &page.content, page.end, true));
        }
    });
    button.upcast()
}

/// The root for a window `width` pixels wide.
pub fn body(width: i32) -> Option<Built> {
    let end = vec![search_button()];
    match layout_for(width) {
        Layout::Narrow => built(page::body(&root_rows(Rc::new(crate::ui::open))).widget, end),
        Layout::Sidebar => {
            let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
            right.set_hexpand(true);
            let show = {
                let right = right.clone();
                move |id: &'static str| {
                    while let Some(child) = right.first_child() {
                        right.remove(&child);
                    }
                    right.append(&pane(id));
                }
            };
            show(super::ROOT_ROWS[0].target);
            let left = page::body(&root_rows(Rc::new(show))).widget;
            left.set_size_request(300, -1);
            let both = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            both.append(&left);
            both.append(&gtk::Separator::new(gtk::Orientation::Vertical));
            both.append(&right);
            built(both, end)
        }
    }
}
