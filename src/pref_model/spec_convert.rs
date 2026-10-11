//! Spec-to-model conversion (`spec/settings.json` items to `PreferenceItem` values) and the page registry
//! keyed by screen id. Unsupported kinds are skipped with a log line instead of panicking.
//! Spec items carry no preference key, so each bound item gets the key `"<page id>/<title>"` in the given store.

use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;

use super::bind::accept;
use super::model::{slider_steps, ItemCommon, Preference, PreferenceGroup, PreferenceItem};
use crate::spec::{Group, Item, Spec};

#[derive(Debug, PartialEq, Eq)]
pub enum Skip {
    /// The spec kind has no model kind yet (named in the message).
    Unsupported(&'static str),
}

/// The title a spec item shows.
pub fn item_title(item: &Item) -> String {
    match item {
        Item::Switch { title, .. }
        | Item::List { title, .. }
        | Item::Multi { title, .. }
        | Item::Slider { title, .. }
        | Item::Text { title, .. }
        | Item::Tristate { title }
        | Item::Sort { title, .. }
        | Item::Chips { title, .. }
        | Item::Link { title, .. }
        | Item::Action { title, .. }
        | Item::Info { title } => title.clone(),
        Item::Unknown => String::new(),
    }
}

fn common(title: &str, subtitle: &Option<String>) -> ItemCommon {
    let common = ItemCommon::new(title);
    match subtitle {
        Some(text) => common.with_subtitle(text),
        None => common,
    }
}

fn entries(options: &[String]) -> Vec<(String, String)> {
    options.iter().map(|o| (o.clone(), o.clone())).collect()
}

/// Builds the model item for one spec item. `navigate` receives a link's target screen id.
pub fn build_item(
    store: &PrefStore,
    page: &str,
    item: &Item,
    navigate: &Rc<dyn Fn(&str)>,
) -> Result<PreferenceItem, Skip> {
    let key = |title: &str| format!("{page}/{title}");
    Ok(match item {
        Item::Switch { title, subtitle, default } => PreferenceItem::Switch {
            common: common(title, subtitle),
            pref: Pref::new(store.clone(), key(title), default.as_bool().unwrap_or(false)),
            veto: accept(),
        },
        Item::List { title, subtitle, options, default } => {
            let initial = default.as_str().map(str::to_owned).or_else(|| options.first().cloned()).unwrap_or_default();
            PreferenceItem::List {
                common: common(title, subtitle),
                pref: Pref::new(store.clone(), key(title), initial),
                entries: entries(options),
                veto: accept(),
            }
        }
        Item::Multi { title, subtitle, options, default } => {
            let initial: BTreeSet<String> = default
                .as_array()
                .map(|values| values.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                .unwrap_or_default();
            PreferenceItem::MultiSelect {
                common: common(title, subtitle),
                pref: Pref::new(store.clone(), key(title), initial),
                entries: entries(options),
                veto: accept(),
            }
        }
        Item::Slider { title, min, max, default, .. } => {
            let first = min.unwrap_or(0.0).round() as i32;
            let last = (max.unwrap_or(100.0).round() as i32).max(first);
            let value = (default.as_f64().unwrap_or(f64::from(first)).round() as i32).clamp(first, last);
            PreferenceItem::Slider {
                common: ItemCommon::new(title),
                value,
                range: first..=last,
                steps: slider_steps(first, last),
                on_change: accept(),
            }
        }
        Item::Text { title, subtitle, default } => PreferenceItem::EditText {
            common: common(title, subtitle),
            pref: Pref::new(store.clone(), key(title), default.as_str().unwrap_or_default().to_owned()),
            dialog_subtitle: None,
            can_be_blank: true,
            validate: Rc::new(|_| true),
            error_message: None,
            veto: accept(),
        },
        Item::Link { title, subtitle, target } => {
            let (navigate, target) = (navigate.clone(), target.clone());
            let on_click: Rc<dyn Fn()> = Rc::new(move || navigate(&target));
            PreferenceItem::Text { common: common(title, subtitle).with_link(), on_click: Some(on_click) }
        }
        Item::Action { title, subtitle } => {
            // an action whose title names a dialog, sheet or screen of this page opens it, as on the generic page
            let (page, title) = (page.to_owned(), title.clone());
            let on_click: Rc<dyn Fn()> = Rc::new({
                let title = title.clone();
                move || {
                    crate::ui::resolve(&page, &title);
                }
            });
            PreferenceItem::Text { common: common(&title, subtitle), on_click: Some(on_click) }
        }
        Item::Info { title } => PreferenceItem::Info { text: title.clone(), enabled: true },
        Item::Tristate { .. } => return Err(Skip::Unsupported("tristate")),
        Item::Sort { .. } => return Err(Skip::Unsupported("sort")),
        Item::Chips { .. } => return Err(Skip::Unsupported("chips")),
        Item::Unknown => return Err(Skip::Unsupported("unknown")),
    })
}

/// Spec rows (page id, title) that have no Linux counterpart: display cutout, the legacy long-strip decoder, the
/// display profile and Aniyomi's privacy page. [`Registry::build`] leaves them out instead of showing a row that does
/// nothing.
pub const ANDROID_ONLY: [(&str, &str); 4] = [
    ("settings_reader", "Show content in cutout area"),
    ("settings_advanced", "Use legacy decoder for long strip reader"),
    ("settings_advanced", "Custom display profile"),
    // Aniyomi's privacy page is about the Android app; this app has no page of its own to link
    ("settings_about", "Privacy policy"),
];

/// Whether the spec row `title` of page `page` is one of [`ANDROID_ONLY`].
pub fn is_android_only(page: &str, title: &str) -> bool {
    ANDROID_ONLY.iter().any(|(p, t)| *p == page && *t == title)
}

/// Settings pages by screen id, each a list of spec groups. Pages without groups are not registered.
pub struct Registry {
    pages: HashMap<String, Vec<Group>>,
}

impl Registry {
    pub fn new(spec: &Spec) -> Self {
        let pages = spec
            .screens
            .values()
            .filter(|screen| !screen.groups.is_empty())
            .map(|screen| (screen.id.clone(), screen.groups.clone()))
            .collect();
        Self { pages }
    }

    #[cfg(test)]
    pub fn contains(&self, id: &str) -> bool {
        self.pages.contains_key(id)
    }

    /// The page tree for `id`, or `None` for an unregistered page. Unsupported items are logged and dropped;
    /// a group left empty by that is dropped too. An untitled spec group becomes loose items.
    pub fn build(&self, id: &str, store: &PrefStore, navigate: &Rc<dyn Fn(&str)>) -> Option<Vec<Preference>> {
        let groups = self.pages.get(id)?;
        let mut tree = Vec::new();
        for group in groups {
            let mut items = Vec::new();
            for item in group.items.iter().filter(|item| !is_android_only(id, &item_title(item))) {
                match build_item(store, id, item, navigate) {
                    Ok(built) => items.push(built),
                    Err(Skip::Unsupported(what)) => {
                        tracing::warn!(page = id, kind = what, "settings item skipped: unsupported kind");
                    }
                }
            }
            match (&group.title, items.is_empty()) {
                (_, true) => {}
                (Some(title), false) => tree.push(Preference::Group(PreferenceGroup::new(title.clone(), items))),
                (None, false) => tree.extend(items.into_iter().map(Preference::Item)),
            }
        }
        Some(tree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(json: &str) -> Item {
        serde_json::from_str(json).expect("valid spec item")
    }

    fn nav() -> Rc<dyn Fn(&str)> {
        Rc::new(|_| {})
    }

    #[test]
    fn spec_defaults_become_preference_defaults() {
        let store = PrefStore::in_memory();
        let switch = build_item(&store, "p", &item(r#"{"type":"switch","title":"S","default":true}"#), &nav());
        let Ok(PreferenceItem::Switch { pref, .. }) = switch else { panic!("not a switch") };
        assert!(pref.get());
        assert_eq!(pref.key(), "p/S");
        let list = build_item(&store, "p", &item(r#"{"type":"list","title":"L","options":["x","y"]}"#), &nav());
        let Ok(PreferenceItem::List { pref, .. }) = list else { panic!("not a list") };
        assert_eq!(pref.get(), "x");
        let slider = build_item(&store, "p", &item(r#"{"type":"slider","title":"Z","min":1,"max":5,"default":9}"#), &nav());
        let Ok(PreferenceItem::Slider { value, range, steps, .. }) = slider else { panic!("not a slider") };
        assert_eq!((value, range, steps), (5, 1..=5, 3));
    }

    #[test]
    fn a_link_calls_navigate_with_its_target() {
        let store = PrefStore::in_memory();
        let seen = Rc::new(std::cell::RefCell::new(String::new()));
        let sink = seen.clone();
        let navigate: Rc<dyn Fn(&str)> = Rc::new(move |t| *sink.borrow_mut() = t.to_owned());
        let built = build_item(&store, "p", &item(r#"{"type":"link","title":"L","target":"settings_x"}"#), &navigate);
        let Ok(PreferenceItem::Text { on_click: Some(click), .. }) = built else { panic!("not a link") };
        click();
        assert_eq!(&*seen.borrow(), "settings_x");
    }

    #[test]
    fn unsupported_items_are_an_error_not_a_panic() {
        let store = PrefStore::in_memory();
        for json in [r#"{"type":"chips","title":"A"}"#, r#"{"type":"tristate","title":"A"}"#, r#"{"type":"sort","title":"A"}"#] {
            assert!(matches!(build_item(&store, "p", &item(json), &nav()), Err(Skip::Unsupported(_))), "{json}");
        }
    }

    #[test]
    fn the_android_only_rows_exist_in_the_spec_and_are_left_out_of_the_pages() {
        let spec = Spec::load();
        let registry = Registry::new(&spec);
        let store = PrefStore::in_memory();
        for (page, title) in ANDROID_ONLY {
            let in_spec = spec.screens[page].groups.iter().flat_map(|g| &g.items).any(|i| item_title(i) == title);
            assert!(in_spec, "{page}: {title} is a spec row, so this list is not stale");
            let tree = registry.build(page, &store, &nav()).expect("registered page");
            let shown = tree.iter().any(|entry| match entry {
                Preference::Item(i) => i.title() == title,
                Preference::Group(g) => g.items.iter().any(|i| i.title() == title),
            });
            assert!(!shown, "{page}: {title} is not shown");
        }
    }

    #[test]
    fn no_spec_title_has_a_stray_space_that_would_stop_a_typed_row_from_replacing_it() {
        let spec = Spec::load();
        for screen in spec.screens.values() {
            for item in screen.groups.iter().flat_map(|g| &g.items) {
                let title = item_title(item);
                assert_eq!(title, title.trim(), "{}: {title:?}", screen.id);
            }
        }
    }

}
