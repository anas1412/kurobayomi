//! The settings tree: pages are data, and each item variant is rendered by one builder.
//! Mirrors Aniyomi `Preference.kt` (13 item variants, groups, defaults).

use std::collections::BTreeSet;
use std::ops::RangeInclusive;
use std::rc::Rc;

use kuro_prefs::pref::Pref;

use super::bind::Veto;

/// Top-level entry of a settings page: a standalone item or a titled group.
pub enum Preference {
    Item(PreferenceItem),
    Group(PreferenceGroup),
}

pub struct PreferenceGroup {
    pub title: String,
    /// Disabled groups are not built at all (Kotlin `PreferenceScreen`).
    pub enabled: bool,
    pub items: Vec<PreferenceItem>,
}

impl PreferenceGroup {
    pub fn new(title: impl Into<String>, items: Vec<PreferenceItem>) -> Self {
        Self { title: title.into(), enabled: true, items }
    }
}

/// Fields every item carries (Kotlin `Preference.kt` common parameters).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemCommon {
    pub title: String,
    pub enabled: bool,
    /// Template with one `%s`, or `None` for static rows.
    pub subtitle: Option<String>,
    /// GTK symbolic icon name, `None` for no icon.
    pub icon: Option<&'static str>,
    /// A live flag that overrides `enabled` after the build; flipping it slides the row in or makes it insensitive.
    pub enabled_flag: Option<super::enabled::EnabledFlag>,
    /// The row opens another page: it ends in the dim arrow (`ui::chevron`), like every link row in the app.
    pub link: bool,
}

/// Default subtitle template for bound lists and edit fields.
pub const BOUND_SUBTITLE: &str = "%s";

impl ItemCommon {
    /// Static row: no subtitle, no icon, enabled.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), enabled: true, subtitle: None, icon: None, enabled_flag: None, link: false }
    }

    /// Bound row: subtitle template `%s` (list, multi-select and edit fields).
    pub fn bound(title: impl Into<String>) -> Self {
        Self { subtitle: Some(BOUND_SUBTITLE.to_owned()), ..Self::new(title) }
    }

    pub fn with_subtitle(mut self, template: &str) -> Self {
        self.subtitle = Some(template.to_owned());
        self
    }

    pub fn with_icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Marks the row as one that opens another page.
    pub fn with_link(mut self) -> Self {
        self.link = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    #[cfg(test)]
    /// Binds the row's enabled state to `flag` (its current value is the initial state).
    pub fn with_enabled_flag(mut self, flag: super::enabled::EnabledFlag) -> Self {
        self.enabled = flag.get();
        self.enabled_flag = Some(flag);
        self
    }

    /// The subtitle text to show for `value_label`, or `None` when blank (see [`format_subtitle`]).
    pub fn subtitle_for(&self, value_label: &str) -> Option<String> {
        self.subtitle.as_deref().and_then(|t| format_subtitle(t, value_label))
    }
}

/// An item key and its visible label.
pub type Entry = (String, String);

/// Veto-gated value. A `None` on the `Veto` side means the caller owns the value.
pub enum PreferenceItem {
    Text { common: ItemCommon, on_click: Option<Rc<dyn Fn()>> },
    Switch { common: ItemCommon, pref: Pref<bool>, veto: Veto<bool> },
    Slider { common: ItemCommon, value: i32, range: RangeInclusive<i32>, steps: u32, on_change: Veto<i32> },
    List { common: ItemCommon, pref: Pref<String>, entries: Vec<Entry>, veto: Veto<String> },
    BasicList { common: ItemCommon, value: String, entries: Vec<Entry>, on_change: Veto<String> },
    MultiSelect { common: ItemCommon, pref: Pref<BTreeSet<String>>, entries: Vec<Entry>, veto: Veto<BTreeSet<String>> },
    EditText { common: ItemCommon, pref: Pref<String>, dialog_subtitle: Option<String>, can_be_blank: bool,
               validate: Rc<dyn Fn(&str) -> bool>, error_message: Option<Rc<dyn Fn(&str) -> String>>, veto: Veto<String> },
    MultiLineEdit { common: ItemCommon, pref: Pref<String>, can_be_blank: bool, veto: Veto<String> },
    MpvConf { common: ItemCommon, pref: Pref<String>, file_name: Option<String>, can_be_blank: bool, veto: Veto<String> },
    EditTextInfo { common: ItemCommon, pref: Pref<String>, dialog_subtitle: Option<String>, can_be_blank: bool,
                   validate: Rc<dyn Fn(&str) -> bool>, error_message: Option<Rc<dyn Fn(&str) -> String>>, veto: Veto<String> },
    Tracker { name: String, logged_in: bool, on_login: Rc<dyn Fn()>, on_logout: Rc<dyn Fn()> },
    Info { text: String, enabled: bool },
    Custom { title: String, build: Rc<dyn Fn() -> gtk::Widget> },
}

/// The 13 variants, without payloads. Used by tests and by the exhaustive render match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Switch,
    Slider,
    List,
    BasicList,
    MultiSelect,
    EditText,
    MultiLineEdit,
    MpvConf,
    EditTextInfo,
    Tracker,
    Info,
    Custom,
}

impl Kind {
    pub const ALL: [Kind; 13] = [
        Kind::Text,
        Kind::Switch,
        Kind::Slider,
        Kind::List,
        Kind::BasicList,
        Kind::MultiSelect,
        Kind::EditText,
        Kind::MultiLineEdit,
        Kind::MpvConf,
        Kind::EditTextInfo,
        Kind::Tracker,
        Kind::Info,
        Kind::Custom,
    ];
}

impl PreferenceItem {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Text { .. } => Kind::Text,
            Self::Switch { .. } => Kind::Switch,
            Self::Slider { .. } => Kind::Slider,
            Self::List { .. } => Kind::List,
            Self::BasicList { .. } => Kind::BasicList,
            Self::MultiSelect { .. } => Kind::MultiSelect,
            Self::EditText { .. } => Kind::EditText,
            Self::MultiLineEdit { .. } => Kind::MultiLineEdit,
            Self::MpvConf { .. } => Kind::MpvConf,
            Self::EditTextInfo { .. } => Kind::EditTextInfo,
            Self::Tracker { .. } => Kind::Tracker,
            Self::Info { .. } => Kind::Info,
            Self::Custom { .. } => Kind::Custom,
        }
    }

    /// The title used for highlight lookup. Trackers use their name, Info rows use their text.
    pub fn title(&self) -> &str {
        match self {
            Self::Text { common, .. }
            | Self::Switch { common, .. }
            | Self::Slider { common, .. }
            | Self::List { common, .. }
            | Self::BasicList { common, .. }
            | Self::MultiSelect { common, .. }
            | Self::EditText { common, .. }
            | Self::MultiLineEdit { common, .. }
            | Self::MpvConf { common, .. }
            | Self::EditTextInfo { common, .. } => &common.title,
            Self::Tracker { name, .. } => name,
            Self::Info { text, .. } => text,
            Self::Custom { title, .. } => title,
        }
    }

    /// The live enabled flag, when the item has one.
    pub fn enabled_flag(&self) -> Option<&super::enabled::EnabledFlag> {
        match self {
            Self::Text { common, .. }
            | Self::Switch { common, .. }
            | Self::Slider { common, .. }
            | Self::List { common, .. }
            | Self::BasicList { common, .. }
            | Self::MultiSelect { common, .. }
            | Self::EditText { common, .. }
            | Self::MultiLineEdit { common, .. }
            | Self::MpvConf { common, .. }
            | Self::EditTextInfo { common, .. } => common.enabled_flag.as_ref(),
            Self::Tracker { .. } | Self::Info { .. } | Self::Custom { .. } => None,
        }
    }

    /// Disabled items stay in the page, insensitive. Info and custom rows are always on.
    pub fn enabled(&self) -> bool {
        match self {
            Self::Text { common, .. }
            | Self::Switch { common, .. }
            | Self::Slider { common, .. }
            | Self::List { common, .. }
            | Self::BasicList { common, .. }
            | Self::MultiSelect { common, .. }
            | Self::EditText { common, .. }
            | Self::MultiLineEdit { common, .. }
            | Self::MpvConf { common, .. }
            | Self::EditTextInfo { common, .. } => common.enabled,
            Self::Info { enabled, .. } => *enabled,
            Self::Tracker { .. } | Self::Custom { .. } => true,
        }
    }
}

/// Number of steps for a slider over `first..=last`: `(last - first) - 1`, never negative.
/// Kotlin's `IntRange(from = 0)` would be -1 for a single-value range, so it is clamped.
pub fn slider_steps(first: i32, last: i32) -> u32 {
    (i64::from(last) - i64::from(first) - 1).max(0) as u32
}

/// Fills the `%s` of `template` with `value_label`. Blank results are hidden (`None`).
pub fn format_subtitle(template: &str, value_label: &str) -> Option<String> {
    let text = template.replacen("%s", value_label, 1);
    (!text.trim().is_empty()).then_some(text)
}

/// Subtitle of a multi-select: labels of the selected keys in entries order, or `None` when empty.
/// Keys that are not in `entries` are ignored.
pub fn multi_select_labels(entries: &[Entry], selected: &BTreeSet<String>) -> String {
    let labels: Vec<&str> = entries
        .iter()
        .filter(|(key, _)| selected.contains(key))
        .map(|(_, label)| label.as_str())
        .collect();
    if labels.is_empty() { super::strings::none() } else { labels.join(", ") }
}

/// First two lines of an MPV config, plus `\n...` when there are more than two lines.
pub fn mpv_subtitle(value: &str) -> String {
    let lines: Vec<&str> = value.split('\n').collect();
    if lines.len() > 2 {
        format!("{}\n{}\n...", lines[0], lines[1])
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_steps_match_the_kotlin_default_and_never_go_negative() {
        assert_eq!(slider_steps(5, 5), 0);
        assert_eq!(slider_steps(0, 10), 9);
        assert_eq!(slider_steps(0, 1), 0);
    }

    #[test]
    fn blank_subtitle_is_hidden_and_missing_label_is_shown_raw() {
        assert_eq!(format_subtitle("%s", ""), None);
        assert_eq!(format_subtitle("%s", "   "), None);
        assert_eq!(format_subtitle("x", "x"), Some("x".to_owned()));
        assert_eq!(format_subtitle("Mode: %s", "Dark").as_deref(), Some("Mode: Dark"));
    }

    #[test]
    fn multi_select_subtitle_uses_entries_order_and_none_when_empty() {
        let entries = vec![("a".to_owned(), "A".to_owned()), ("b".to_owned(), "B".to_owned())];
        let both: BTreeSet<String> = ["b", "a"].iter().map(|s| s.to_string()).collect();
        assert_eq!(multi_select_labels(&entries, &both), "A, B");
        assert_eq!(multi_select_labels(&entries, &BTreeSet::new()), "None");
        let unknown: BTreeSet<String> = ["zz"].iter().map(|s| s.to_string()).collect();
        assert_eq!(multi_select_labels(&entries, &unknown), "None");
    }

    #[test]
    fn mpv_subtitle_shows_two_lines_and_an_ellipsis_past_two() {
        assert_eq!(mpv_subtitle("a\nb"), "a\nb");
        assert_eq!(mpv_subtitle("a\nb\nc"), "a\nb\n...");
        assert_eq!(mpv_subtitle("a\nb\n"), "a\nb\n...");
        assert_eq!(mpv_subtitle(""), "");
    }

    #[test]
    fn bound_rows_default_to_the_percent_s_template_and_static_rows_to_none() {
        assert_eq!(ItemCommon::bound("Theme").subtitle.as_deref(), Some("%s"));
        assert_eq!(ItemCommon::new("Help").subtitle, None);
        assert!(ItemCommon::new("Help").enabled);
    }
}
