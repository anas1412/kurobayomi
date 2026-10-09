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

pub fn value_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
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
