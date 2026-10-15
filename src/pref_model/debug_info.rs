//! The Debug info page (T-47b-040, T-47b-041): the worker and schema links, the app's version and build time, and the
//! Linux device rows. Every row copies its value when pressed. The Android rows of the spec (profile compilation, the
//! WebView, the Android version) are replaced or dropped.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_core::app_info::APP_INFO;
use kuro_i18n::{MsgKey, tr};

use super::model::{ItemCommon, Preference, PreferenceGroup, PreferenceItem};
use crate::screens::Built;

/// Titles of the rows the spec has no Linux counterpart for.
const OS: &str = "OS";
const KERNEL: &str = "Kernel";
const DESKTOP: &str = "Desktop";
const SESSION: &str = "Session";
const GTK: &str = "GTK";

/// "Nightly <commit>" on a nightly build, else "Stable <version>", or "Debug <sha>" in a debug build.
pub fn version_text(build_type: &str, version: &str, sha: &str, nightly_commit: Option<&str>) -> String {
    match (nightly_commit, build_type) {
        (Some(commit), _) => format!("Nightly {commit}"),
        (None, "debug") => format!("Debug {sha}"),
        (None, _) => format!("Stable {version}"),
    }
}

/// The version line of this build, for About's Version row and the debug information.
pub fn current_version_text() -> String {
    let info = &*APP_INFO;
    let nightly = Some(crate::channel::short_commit()).filter(|commit| crate::channel::NIGHTLY && !commit.is_empty());
    version_text(info.build_type, &info.version_name, info.git_sha, nightly)
}

/// "<vendor> <product>" from the two DMI files; one missing gives the other, both missing gives `None`.
pub fn model_text(vendor: Option<&str>, product: Option<&str>) -> Option<String> {
    let clean = |v: Option<&str>| v.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
    match (clean(vendor), clean(product)) {
        // many makers repeat their name at the start of the product name
        (Some(v), Some(p)) if p.to_lowercase().starts_with(&v.to_lowercase()) => Some(p),
        (Some(v), Some(p)) => Some(format!("{v} {p}")),
        (Some(one), None) | (None, Some(one)) => Some(one),
        (None, None) => None,
    }
}

fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// The rows of the App info group: title and value.
pub fn app_rows(version_title: &str, build_time_title: &str) -> Vec<(String, String)> {
    let info = &*APP_INFO;
    vec![
        (version_title.to_owned(), current_version_text()),
        (build_time_title.to_owned(), info.build_time.to_owned()),
    ]
}

/// The rows of the Device info group. `facts` is `kuro_platform::system::facts()`; a fact that reads `unknown` is left
/// out, and so is the model when the DMI files are absent.
pub fn device_rows(model_title: &str, model: Option<String>, facts: &[(&'static str, String)], gtk_version: String) -> Vec<(String, String)> {
    let fact = |key: &str| facts.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()).filter(|v| v != "unknown");
    let mut rows = Vec::new();
    if let Some(model) = model {
        rows.push((model_title.to_owned(), model));
    }
    rows.extend(fact("os").map(|v| (OS.to_owned(), v)));
    rows.extend(fact("kernel").map(|v| (KERNEL.to_owned(), v)));
    rows.extend(fact("desktop").map(|v| (DESKTOP.to_owned(), v)));
    rows.extend(fact("session").map(|v| (SESSION.to_owned(), v)));
    rows.push((GTK.to_owned(), gtk_version));
    rows
}

fn gtk_version() -> String {
    format!("{}.{}.{}", gtk::major_version(), gtk::minor_version(), gtk::micro_version())
}

fn copy_row(title: &str, value: &str) -> PreferenceItem {
    let text = value.to_owned();
    let on_click: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(display) = gtk::gdk::Display::default() {
            display.clipboard().set_text(&text);
        }
        crate::toast::notify(tr(MsgKey::copied_to_clipboard_plain));
    });
    PreferenceItem::Text { common: ItemCommon::new(title).with_subtitle(value), on_click: Some(on_click) }
}

/// The page tree, built from the spec screen for its titles and links.
pub fn tree() -> Vec<Preference> {
    let screen = crate::ui::screen("settings_debug_info");
    let title_of = |group: usize, item: usize| -> String {
        screen.groups.get(group).and_then(|g| g.items.get(item)).map(super::spec_convert::item_title).unwrap_or_default()
    };
    let mut tree = Vec::new();
    // group 0: the links to the worker and schema pages
    if let Some(links) = screen.groups.first() {
        let items: Vec<PreferenceItem> = links
            .items
            .iter()
            .filter_map(|item| match item {
                crate::spec::Item::Link { title, subtitle, target } => {
                    let target = target.clone();
                    let on_click: Rc<dyn Fn()> = Rc::new(move || crate::ui::open(&target));
                    let common = match subtitle {
                        Some(text) => ItemCommon::new(&crate::spec::shown(title)).with_subtitle(&crate::spec::shown(text)),
                        None => ItemCommon::new(&crate::spec::shown(title)),
                    };
                    Some(PreferenceItem::Text { common: common.with_link(), on_click: Some(on_click) })
                }
                _ => None,
            })
            .collect();
        tree.extend(items.into_iter().map(Preference::Item));
    }
    let group = |title: Option<&String>, rows: Vec<(String, String)>| {
        let items = rows.iter().map(|(t, v)| copy_row(t, v)).collect();
        Preference::Group(PreferenceGroup::new(title.map(|t| crate::spec::shown(t)).unwrap_or_default(), items))
    };
    let model = model_text(read_file("/sys/class/dmi/id/sys_vendor").as_deref(), read_file("/sys/class/dmi/id/product_name").as_deref());
    tree.push(group(screen.groups.get(1).and_then(|g| g.title.as_ref()), app_rows(&title_of(1, 0), &title_of(1, 1))));
    tree.push(group(
        screen.groups.get(2).and_then(|g| g.title.as_ref()),
        device_rows(&title_of(2, 0), model, &kuro_platform::system::facts(), gtk_version()),
    ));
    tree
}

/// The page.
pub fn page() -> Option<Built> {
    let body = super::page::body(&tree());
    Some(Built { content: body.widget, end: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_47b_040_the_version_row_names_the_build() {
        assert_eq!(version_text("release", "0.1.0", "abc1234", None), "Stable 0.1.0");
        assert_eq!(version_text("debug", "0.1.0-77", "abc1234", None), "Debug abc1234");
        assert_eq!(version_text("release", "0.1.0", "abc1234", Some("6f273298")), "Nightly 6f273298", "a nightly says so");
    }

    #[test]
    fn t_47b_040_the_app_group_has_the_spec_titles_and_two_rows() {
        let rows = app_rows("Version", "Build time");
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].0.as_str(), rows[1].0.as_str()), ("Version", "Build time"));
        assert!(!rows[0].1.is_empty());
    }

    #[test]
    fn t_47b_041_the_model_joins_vendor_and_product_and_an_absent_file_is_left_out() {
        assert_eq!(model_text(Some("HP\n"), Some(" Pavilion 15")).as_deref(), Some("HP Pavilion 15"));
        assert_eq!(model_text(None, Some("Pavilion")).as_deref(), Some("Pavilion"));
        assert_eq!(model_text(Some("HP"), Some("HP Pavilion Gaming")).as_deref(), Some("HP Pavilion Gaming"), "no doubled maker");
        assert_eq!(model_text(Some(""), None), None);
    }

    #[test]
    fn t_47b_041_unknown_facts_are_omitted_and_the_rest_keep_their_order() {
        let facts = vec![
            ("os", "CachyOS Linux".to_owned()),
            ("kernel", "Linux 7.2".to_owned()),
            ("desktop", "unknown".to_owned()),
            ("session", "wayland".to_owned()),
        ];
        let rows = device_rows("Model", Some("HP Pavilion".into()), &facts, "4.16.0".into());
        let titles: Vec<&str> = rows.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(titles, ["Model", "OS", "Kernel", "Session", "GTK"]);
        assert_eq!(device_rows("Model", None, &facts, "4.16.0".into())[0].0, "OS", "no DMI, no model row");
    }

    #[test]
    fn t_47b_058_the_spec_screen_has_the_groups_the_page_fills() {
        let screen = crate::ui::screen("settings_debug_info");
        assert!(screen.groups.len() >= 3, "links, app info and device info");
        assert!(screen.groups[0].items.iter().any(|i| matches!(i, crate::spec::Item::Link { target, .. } if target == "settings_worker_info")));
    }
}
