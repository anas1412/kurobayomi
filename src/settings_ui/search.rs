//! The contract every settings screen of this module meets (task T-47a-006), and the settings search (tasks T-47a-004
//! and T-47a-008): the index of every row of the main and player screens, the matcher, and the results page.
#![allow(dead_code)]
use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::highlight;
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::pref_model::page::Built as PageBuilt;
use crate::screens::{built, Built};
use crate::settings_ui::route;

pub trait SearchableSettings {
    /// The screen title (English text of its title key).
    fn title(&self) -> &'static str;

    /// The preference tree. Evaluated on every open and every search, so rules see the current values.
    fn preferences(&self, prefs: &Prefs) -> Vec<Preference>;

    /// An optional app bar action as `(label, icon)`. Only the tracking and data screens have one.
    fn app_bar_action(&self) -> Option<(&'static str, &'static str)> {
        None
    }
}

/// Renders a screen's tree through the shared page renderer (needs the GTK main thread).
pub fn render(screen: &dyn SearchableSettings, prefs: &Prefs) -> PageBuilt {
    crate::pref_model::page::body(&screen.preferences(prefs))
}

/// The main index, in index order (Android `settingScreens`). About is not indexed.
pub const MAIN_SCREENS: [&str; 9] = [
    "settings_appearance",
    "settings_library",
    "settings_reader",
    "settings_downloads",
    "settings_tracking",
    "settings_browse",
    "settings_data_storage",
    "settings_security",
    "settings_advanced",
];

/// The player index (Android `playerSettingScreens`), appended after the main index.
pub const PLAYER_SCREENS: [&str; 7] = [
    "settings_player",
    "settings_player_gestures",
    "settings_player_decoder",
    "settings_player_subtitle",
    "settings_player_audio",
    "settings_player_torrents",
    "settings_player_advanced",
];

/// The first breadcrumb node of a hit from the player index during a non-player search.
pub const PLAYER_PREFIX: &str = "Player settings";

/// At most this many results are shown, in index order.
pub const MAX_RESULTS: usize = 10;

/// One searchable row: the screen it lives on and the text it is matched against.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub screen: String,
    pub screen_title: String,
    pub group: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub player: bool,
}

/// A result: where to go, the row title to highlight there and the `Category > Group` breadcrumb.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub screen: String,
    pub key: String,
    pub crumb: String,
    pub title: String,
}

fn subtitle_of(item: &PreferenceItem) -> Option<String> {
    match item {
        PreferenceItem::Text { common, .. }
        | PreferenceItem::Switch { common, .. }
        | PreferenceItem::Slider { common, .. }
        | PreferenceItem::List { common, .. }
        | PreferenceItem::BasicList { common, .. }
        | PreferenceItem::MultiSelect { common, .. }
        | PreferenceItem::EditText { common, .. }
        | PreferenceItem::MultiLineEdit { common, .. }
        | PreferenceItem::MpvConf { common, .. }
        | PreferenceItem::EditTextInfo { common, .. } => common.subtitle.clone(),
        _ => None,
    }
}

/// Info rows, tracker rows and rows with no title are not searchable; a disabled row is not either.
fn searchable(item: &PreferenceItem) -> bool {
    !matches!(item, PreferenceItem::Info { .. } | PreferenceItem::Tracker { .. })
        && item.enabled()
        && !item.title().trim().is_empty()
}

fn row(screen: &str, screen_title: &str, group: Option<&str>, item: &PreferenceItem, player: bool) -> Option<Row> {
    searchable(item).then(|| Row {
        screen: screen.to_owned(),
        screen_title: screen_title.to_owned(),
        group: group.map(str::to_owned),
        title: item.title().to_owned(),
        subtitle: subtitle_of(item),
        player,
    })
}

/// The searchable rows of one screen's tree, in tree order. A disabled group contributes nothing.
pub fn rows_of(screen: &str, screen_title: &str, tree: &[Preference], player: bool) -> Vec<Row> {
    let mut rows = Vec::new();
    for entry in tree {
        match entry {
            Preference::Group(group) if group.enabled => rows.extend(
                group.items.iter().filter_map(|item| row(screen, screen_title, Some(&group.title), item, player)),
            ),
            Preference::Group(_) => {}
            Preference::Item(item) => rows.extend(row(screen, screen_title, None, item, player)),
        }
    }
    rows
}

/// The index of every screen in [`MAIN_SCREENS`] then [`PLAYER_SCREENS`], with each screen's tree from `build`.
pub fn index_from(build: impl Fn(&str) -> Vec<Preference>) -> Vec<Row> {
    let mut rows = Vec::new();
    for (ids, player) in [(&MAIN_SCREENS[..], false), (&PLAYER_SCREENS[..], true)] {
        for id in ids {
            let title = crate::ui::with_spec(|spec| spec.screen(id).map(|sc| sc.title.clone())).unwrap_or_default();
            rows.extend(rows_of(id, &title, &build(id), player));
        }
    }
    rows
}

/// The index of the running app. Built again for every search, so the rules see the current values.
pub fn index() -> Vec<Row> {
    index_from(route::tree)
}

/// `Category > Group` (or `Category` for a top-level row); the player prefix comes first for a player row.
pub fn crumb(row: &Row) -> String {
    let mut nodes: Vec<&str> = Vec::new();
    if row.player {
        nodes.push(PLAYER_PREFIX);
    }
    nodes.push(&row.screen_title);
    if let Some(group) = &row.group {
        nodes.push(group);
    }
    nodes.join(" > ")
}

/// The results for `query`, the raw field text. An empty query has none; a title or subtitle matches when it contains
/// the query ignoring case, in lower case or upper case (so the Turkish dotted i meets too). Results keep index order.
pub fn hits(rows: &[Row], query: &str) -> Vec<Hit> {
    if query.is_empty() {
        return Vec::new();
    }
    let lower = query.to_lowercase();
    let upper = query.to_uppercase();
    let found = |text: &str| text.to_lowercase().contains(&lower) || text.to_uppercase().contains(&upper);
    rows.iter()
        .filter(|row| found(&row.title) || row.subtitle.as_deref().is_some_and(&found))
        .take(MAX_RESULTS)
        .map(|row| Hit { screen: row.screen.clone(), key: row.title.clone(), crumb: crumb(row), title: row.title.clone() })
        .collect()
}

fn clear(list: &gtk::ListBox) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
}

fn result_row(hit: &Hit) -> gtk::ListBoxRow {
    let title = gtk::Label::builder().label(&hit.title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    let crumb = gtk::Label::builder().label(&hit.crumb).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
    crumb.add_css_class("dim-label");
    let body = gtk::Box::new(gtk::Orientation::Vertical, 2);
    body.set_margin_top(10);
    body.set_margin_bottom(10);
    body.set_margin_start(12);
    body.set_margin_end(12);
    body.append(&title);
    body.append(&crumb);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&body));
    row
}

/// The settings search screen. Each keystroke re-runs the search. Opening it focuses the field; Enter gives the focus
/// back; a result opens its screen and asks that screen to highlight the row.
pub fn page() -> Option<Built> {
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    col.set_margin_start(16);
    col.set_margin_end(16);
    col.set_margin_top(8);
    let entry = gtk::SearchEntry::builder().placeholder_text("Search settings").hexpand(true).build();
    entry.update_property(&[gtk::accessible::Property::Label("Search settings")]);
    col.append(&entry);
    let results = gtk::ListBox::new();
    results.add_css_class("boxed-list");
    results.set_selection_mode(gtk::SelectionMode::None);
    col.append(&results);
    let empty = gtk::Label::new(Some("No results found"));
    empty.set_visible(false);
    col.append(&empty);

    let shown: Rc<RefCell<Vec<Hit>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let (results, empty, shown) = (results.clone(), empty.clone(), Rc::clone(&shown));
        entry.connect_search_changed(move |entry| {
            clear(&results);
            let query = entry.text();
            let found = hits(&index(), query.as_str());
            empty.set_visible(!query.is_empty() && found.is_empty());
            for hit in &found {
                results.append(&result_row(hit));
            }
            *shown.borrow_mut() = found;
        });
    }
    {
        let shown = Rc::clone(&shown);
        results.connect_row_activated(move |_, row| {
            let hit = usize::try_from(row.index()).ok().and_then(|at| shown.borrow().get(at).cloned());
            let Some(hit) = hit else { return };
            highlight::request(hit.key.clone());
            crate::ui::open(&hit.screen);
        });
    }
    entry.connect_activate(|entry| {
        if let Some(root) = entry.root() {
            root.set_focus(None::<&gtk::Widget>);
        }
    });
    entry.connect_map(|entry| {
        entry.grab_focus();
    });
    let scroll = gtk::ScrolledWindow::builder().hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).child(&col).build();
    built(scroll, vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_ui::rows::{group, item, switch, text};
    use crate::pref_model::model::PreferenceItem as Item;
    use crate::settings_ui::reader::page::ReaderScreen;

    #[test]
    fn a_screen_exposes_a_title_and_a_tree_without_an_action_by_default() {
        let screen = ReaderScreen;
        assert_eq!(screen.title(), "Reader");
        assert!(screen.app_bar_action().is_none());
        assert!(!screen.preferences(&Prefs::in_memory()).is_empty());
    }

    fn row_of(title: &str, subtitle: Option<&str>, screen: &str, group: Option<&str>, player: bool) -> Row {
        Row {
            screen: screen.into(),
            screen_title: "Library".into(),
            group: group.map(str::to_owned),
            title: title.into(),
            subtitle: subtitle.map(str::to_owned),
            player,
        }
    }

    #[test]
    fn the_index_order_is_the_main_screens_then_the_player_screens_and_about_is_absent() {
        let rows = index_from(|id| vec![item(text(&format!("{id} row"), None, true))]);
        let screens: Vec<&str> = rows.iter().map(|r| r.screen.as_str()).collect();
        let expected: Vec<&str> = MAIN_SCREENS.iter().chain(PLAYER_SCREENS.iter()).copied().collect();
        assert_eq!(screens, expected);
        assert!(!screens.contains(&"settings_about"));
        assert_eq!(rows.iter().filter(|r| r.player).count(), 7);
        assert_eq!(rows[0].screen_title, "Appearance", "titles come from the spec");
    }

    #[test]
    fn disabled_groups_and_rows_and_info_and_blank_titles_are_not_searchable() {
        let prefs = Prefs::in_memory();
        let tree = vec![
            group("Shown", vec![switch("Crop borders", &prefs.reader.crop_borders, true), switch("Off row", &prefs.reader.crop_borders, false)]),
            Preference::Group(crate::pref_model::model::PreferenceGroup {
                title: "Hidden".into(),
                enabled: false,
                items: vec![switch("Inside a hidden group", &prefs.reader.crop_borders, true)],
            }),
            item(Item::Info { text: "a note".into(), enabled: true }),
            item(text("", None, true)),
        ];
        let titles: Vec<String> = rows_of("settings_reader", "Reader", &tree, false).into_iter().map(|r| r.title).collect();
        assert_eq!(titles, ["Crop borders"]);
    }

    #[test]
    fn a_row_keeps_its_group_for_the_breadcrumb_and_a_top_level_row_has_none() {
        let prefs = Prefs::in_memory();
        let tree = vec![item(switch("Top", &prefs.reader.crop_borders, true)), group("Paged", vec![switch("Pan", &prefs.reader.crop_borders, true)])];
        let rows = rows_of("settings_reader", "Reader", &tree, false);
        assert_eq!(rows[0].group, None);
        assert_eq!(rows[1].group.as_deref(), Some("Paged"));
        assert_eq!(crumb(&rows[0]), "Reader");
        assert_eq!(crumb(&rows[1]), "Reader > Paged");
    }

    #[test]
    fn a_player_hit_gets_the_player_prefix_first() {
        let mut row = row_of("Seeking", None, "settings_player_gestures", Some("Seeking"), true);
        row.screen_title = "Gestures".into();
        assert_eq!(crumb(&row), "Player settings > Gestures > Seeking");
        row.player = false;
        assert_eq!(crumb(&row), "Gestures > Seeking", "the prefix is only for the player index");
    }

    #[test]
    fn matching_ignores_case_and_looks_at_the_subtitle_too() {
        let rows = vec![
            row_of("Crop Borders", None, "settings_reader", None, false),
            row_of("Pan", Some("Move the page with a finger"), "settings_reader", None, false),
        ];
        let found: Vec<String> = hits(&rows, "CROP").into_iter().map(|h| h.title).collect();
        assert_eq!(found, ["Crop Borders"]);
        let by_subtitle: Vec<String> = hits(&rows, "finger").into_iter().map(|h| h.title).collect();
        assert_eq!(by_subtitle, ["Pan"]);
    }

    #[test]
    fn an_empty_query_has_no_results_and_a_spaces_only_query_is_searched() {
        let rows = vec![row_of("Crop borders", None, "settings_reader", None, false)];
        assert!(hits(&rows, "").is_empty());
        assert_eq!(hits(&rows, " ").len(), 1, "a spaces-only query is searched, as the raw text");
        assert_eq!(hits(&rows, "rop b").len(), 1);
    }

    #[test]
    fn results_keep_index_order_and_stop_at_ten() {
        let rows: Vec<Row> = (0..15).map(|n| row_of(&format!("Item {n}"), None, "settings_reader", None, false)).collect();
        let found: Vec<String> = hits(&rows, "item").into_iter().map(|h| h.title).collect();
        assert_eq!(found.len(), MAX_RESULTS);
        assert_eq!(found[0], "Item 0");
        assert_eq!(found[9], "Item 9");
    }

    #[test]
    fn a_hit_carries_its_screen_and_the_title_to_highlight() {
        let rows = vec![row_of("Crop borders", None, "settings_reader", Some("Paged"), false)];
        let hit = &hits(&rows, "crop")[0];
        assert_eq!(hit.screen, "settings_reader");
        assert_eq!(hit.key, "Crop borders");
        assert_eq!(hit.crumb, "Library > Paged");
    }
}
