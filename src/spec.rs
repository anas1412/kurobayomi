// Data model for spec/*.json (see spec/SCHEMA.md), extracted from the Aniyomi source.
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Item {
    Switch { title: String, #[serde(default)] subtitle: Option<String>, #[serde(default)] default: Value },
    List { title: String, #[serde(default)] subtitle: Option<String>, #[serde(default)] options: Vec<String>, #[serde(default)] default: Value },
    Multi { title: String, #[serde(default)] subtitle: Option<String>, #[serde(default)] options: Vec<String>, #[serde(default)] default: Value },
    Slider { title: String, #[serde(default)] min: Option<f64>, #[serde(default)] max: Option<f64>, #[serde(default)] step: Option<f64>, #[serde(default)] default: Value, #[serde(default)] unit: Option<String> },
    Text { title: String, #[serde(default)] subtitle: Option<String>, #[serde(default)] default: Value },
    Tristate { title: String },
    Sort { title: String, #[serde(default)] options: Vec<String> },
    Chips { title: String, #[serde(default)] options: Vec<String>, #[serde(default)] default: Value },
    Link { title: String, #[serde(default)] subtitle: Option<String>, #[serde(default)] target: String },
    Action { title: String, #[serde(default)] subtitle: Option<String> },
    Info { title: String },
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Group {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub items: Vec<Item>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Tool {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub icon: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct SheetTab {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Sheet {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tabs: Vec<SheetTab>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Dialog {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub buttons: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Screen {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub toolbar: Vec<Tool>,
    #[serde(default)]
    pub menu: Vec<String>,
    #[serde(default)]
    pub tabs: Vec<String>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub sheets: Vec<Sheet>,
    #[serde(default)]
    pub dialogs: Vec<Dialog>,
    #[serde(default)]
    pub empty: Option<String>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct NavEntry {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub icon: String,
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    screens: Vec<Screen>,
    #[serde(default)]
    main: Vec<NavEntry>,
    #[serde(default)]
    nav: Vec<NavEntry>,
}

pub struct Spec {
    pub screens: HashMap<String, Screen>,
    pub settings_main: Vec<NavEntry>,
    pub nav: Vec<NavEntry>,
}

impl Spec {
    pub fn load() -> Spec {
        let files = [
            include_str!("../spec/settings.json"),
            include_str!("../spec/app.json"),
            include_str!("../spec/entries.json"),
            include_str!("../spec/reader_player.json"),
        ];
        let mut spec = Spec { screens: HashMap::new(), settings_main: vec![], nav: vec![] };
        for src in files {
            let f: File = serde_json::from_str(src).expect("bad spec json");
            spec.settings_main.extend(f.main);
            spec.nav.extend(f.nav);
            for s in f.screens {
                spec.screens.insert(s.id.clone(), s);
            }
        }
        spec
    }

    pub fn screen(&self, id: &str) -> Option<&Screen> {
        self.screens.get(id)
    }
}

/// The text of a spec string as it is drawn: the message that reads exactly this English, in the active language, else the
/// English itself. The spec data keeps its English as identity (a row's title names its stored preference, a label finds its
/// action, an option is the stored value), so only the drawing code goes through here, never the logic. Do not use it on
/// text a user or a source wrote.
pub fn shown(text: &str) -> String {
    kuro_i18n::translate_known(text).unwrap_or_else(|| text.to_owned())
}

/// [`shown`] over a list, for the labels of options and tabs.
pub fn shown_all(texts: &[String]) -> Vec<String> {
    texts.iter().map(|t| shown(t)).collect()
}

pub fn value_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    // The text the person sees from the spec, as the drawing code reads it, has a message in the string table: the sidebar,
    // the settings list, the screens, the toolbars and menus, and the dialog buttons. (The spec holds more text, most of it
    // sample data and notes of the mock-up, which is never drawn as it stands.)
    #[test]
    fn the_spec_text_that_is_drawn_has_messages() {
        // sample text replaced when the dialog is shown (an episode title, a count)
        const SAMPLES: [&str; 3] = ["Episode 1", "Update trackers to chapter 12?", "Update trackers to episode 12?"];
        let spec = super::Spec::load();
        let mut missing = std::collections::BTreeSet::new();
        let mut check = |text: &str| {
            if !text.is_empty() && !SAMPLES.contains(&text) && kuro_i18n::translate_known(text).is_none() {
                missing.insert(text.to_owned());
            }
        };
        for entry in spec.nav.iter().chain(&spec.settings_main) {
            check(&entry.title);
            entry.subtitle.iter().for_each(|s| check(s));
        }
        for screen in spec.screens.values() {
            check(&screen.title);
            screen.toolbar.iter().for_each(|tool| check(&tool.label));
            screen.menu.iter().for_each(|entry| check(entry));
            screen.sheets.iter().for_each(|sheet| check(&sheet.title));
            for dialog in &screen.dialogs {
                check(&dialog.title);
                dialog.buttons.iter().for_each(|button| check(button));
            }
        }
        assert!(missing.is_empty(), "spec text with no message in the string table (add one with the same English): {missing:?}");
    }

    // Every spec file parses, has no unknown item types, and every link target exists.
    #[test]
    fn spec_is_consistent() {
        let spec = super::Spec::load();
        let mut errors = vec![];
        let mut check = |owner: &str, groups: &[super::Group]| {
            for g in groups {
                for it in &g.items {
                    match it {
                        super::Item::Unknown => errors.push(format!("{owner}: unknown item type")),
                        super::Item::Link { target, title, .. } if !spec.screens.contains_key(target) => {
                            errors.push(format!("{owner}: link '{title}' -> missing screen '{target}'"))
                        }
                        _ => {}
                    }
                }
            }
        };
        for s in spec.screens.values() {
            check(&s.id, &s.groups);
            for sh in &s.sheets {
                for t in &sh.tabs {
                    check(&format!("{}/{}", s.id, sh.id), &t.groups);
                }
            }
            for d in &s.dialogs {
                check(&format!("{}/{}", s.id, d.id), &d.groups);
            }
        }
        for e in &spec.settings_main {
            if !spec.screens.contains_key(&e.id) {
                errors.push(format!("settings main -> missing screen '{}'", e.id));
            }
        }
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }
}
