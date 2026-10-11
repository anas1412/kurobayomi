//! The player settings root as a widget (task T-48a-010): the settings split view (`pref_model::shell::split`) at
//! 720 px and up, a plain list page whose rows push the sub-screens below it. The rows and the breakpoint are in
//! `mod.rs`.
use std::rc::Rc;

use gtk::prelude::*;

use super::{layout_for, root_rows, Layout};
use crate::pref_model::page;
use crate::pref_model::shell::{self, NavEntry};
use crate::screens::{built, Built};
use crate::settings_ui::search;

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
            // the same split view as the settings shell: the sidebar row names the open page
            let entries = super::ROOT_ROWS
                .iter()
                .map(|row| NavEntry { id: row.target.to_owned(), title: row.title.to_owned(), subtitle: Some(row.subtitle.to_owned()), icon: row.icon.to_owned() })
                .collect();
            built(shell::split(entries, super::ROOT_ROWS[0].target), end)
        }
    }
}
