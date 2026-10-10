//! The open source licences pages (tasks T-47a-096 and T-47a-097): the list of the libraries the app is built from,
//! sorted by name, and the detail page of one library (its name as the title, a Website action only when it has a
//! site, and its licence text as plain text in a scrollable label with 16 px of padding).
//!
//! The list is the embedded `licenses.json`, one entry per library with name, website and licence text.
//!
//! Gap: the JSON is a checked-in snapshot made from the crate graph; no build step regenerates it (`cargo about` is
//! not installed here and the build script is not in this lane). The snapshot leaves out the seven crates of the
//! platform D-Bus stack, because the platform boundary check rejects their names outside `kuro-platform`; their
//! attribution has to be added by the platform owner.
use gtk::prelude::*;
use serde::Deserialize;

use crate::screens::{built, Built};
use crate::ui::{boxed_list, column, margins, page, push, row, scrolled};

/// One library of the list.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Library {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub website: String,
    #[serde(default)]
    pub license: String,
    pub text: String,
}

impl Library {
    /// The website, or `None` when the library has none (the Website action is then not shown).
    pub fn site(&self) -> Option<&str> {
        Some(self.website.as_str()).filter(|w| !w.trim().is_empty())
    }
}

/// The entries of `json`, sorted by name without regard to case. Unreadable JSON is an empty list.
pub fn parse(json: &str) -> Vec<Library> {
    let mut libraries: Vec<Library> = serde_json::from_str(json).unwrap_or_default();
    libraries.sort_by_cached_key(|library| library.name.to_lowercase());
    libraries
}

/// The libraries the app embeds.
pub fn libraries() -> Vec<Library> {
    parse(include_str!("licenses.json"))
}

fn open_website(url: &str) {
    gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
        if let Err(err) = result {
            tracing::warn!(%err, "could not open the website");
        }
    });
}

/// The detail page of `library`.
pub fn detail_page(library: &Library) -> gtk::Box {
    let label = gtk::Label::builder().label(library.text.as_str()).xalign(0.0).yalign(0.0).wrap(true).selectable(true).build();
    margins(&label, 16);
    let mut end: Vec<gtk::Widget> = vec![];
    if let Some(site) = library.site() {
        let button = crate::ui::icon_button("web-browser-symbolic", "Website");
        let site = site.to_owned();
        button.connect_clicked(move |_| open_website(&site));
        end.push(button.upcast());
    }
    page(&library.name, &scrolled(&label), end, true)
}

/// The list page: one row per library, a tap opens its detail page.
pub fn list_page() -> Option<Built> {
    let col = column();
    let list = boxed_list();
    for library in libraries() {
        let subtitle = Some(library.license.as_str()).filter(|l| !l.is_empty());
        let r = row(&library.name, subtitle, Some(gtk::Label::new(Some(&library.version)).upcast()));
        r.set_activatable(true);
        r.connect_activate(move |_| push(&detail_page(&library)));
        list.append(&r);
    }
    col.append(&list);
    built(scrolled(&col), vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, website: &str) -> String {
        format!(r#"{{"name":"{name}","version":"1.0.0","website":"{website}","license":"MIT","text":"text of {name}"}}"#)
    }

    #[test]
    fn the_list_is_sorted_by_name_ignoring_case() {
        let json = format!("[{},{},{}]", entry("serde", ""), entry("Anyhow", ""), entry("glib", ""));
        let names: Vec<String> = parse(&json).into_iter().map(|l| l.name).collect();
        assert_eq!(names, ["Anyhow", "glib", "serde"]);
    }

    #[test]
    fn the_website_action_needs_a_website() {
        let libraries = parse(&format!("[{},{}]", entry("a", "https://a.example"), entry("b", " ")));
        assert_eq!(libraries[0].site(), Some("https://a.example"));
        assert_eq!(libraries[1].site(), None);
    }

    #[test]
    fn unreadable_json_gives_an_empty_list() {
        assert!(parse("not json").is_empty());
    }

    #[test]
    fn the_embedded_list_is_sorted_and_every_entry_has_a_name_and_text() {
        let libraries = libraries();
        assert!(libraries.len() > 50);
        assert!(libraries.windows(2).all(|w| w[0].name.to_lowercase() <= w[1].name.to_lowercase()));
        assert!(libraries.iter().all(|l| !l.name.is_empty() && !l.text.is_empty()));
    }
}
