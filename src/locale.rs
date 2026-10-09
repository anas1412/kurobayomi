//! Language display names, the language comparator and source group names (`LocaleHelper.kt`, T-52a-028).
//!
//! Names are each language written in itself, as `Locale.getDisplayName(locale)` gives them. They come from the
//! table below, not from a runtime ICU crate (plans C-11). A tag the table does not know shows as the tag with its
//! first letter in upper case, which is what `Locale` prints for an unknown code.
#![allow(dead_code)] // used by the source browser and the language settings as they land

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Mutex;

use kuro_i18n::{MsgKey, tr};

/// Group keys of the source list. `all` and `other` are plain language keys.
pub const PINNED_KEY: &str = "pinned";
pub const LAST_USED_KEY: &str = "last_used";

/// Self names by lower-case tag. A region or script tag is listed only where its name differs from the language's.
// ponytail: a hand-written subset of the languages Aniyomi sources use; the plans' xtask table replaces it when generated.
const SELF_NAMES: &[(&str, &str)] = &[
    ("af", "Afrikaans"),
    ("ar", "العربية"),
    ("az", "Azərbaycan"),
    ("be", "Беларуская"),
    ("bg", "Български"),
    ("bn", "বাংলা"),
    ("bs", "Bosanski"),
    ("ca", "Català"),
    ("cs", "Čeština"),
    ("cy", "Cymraeg"),
    ("da", "Dansk"),
    ("de", "Deutsch"),
    ("el", "Ελληνικά"),
    ("en", "English"),
    ("es", "Español"),
    ("es-419", "Español (Latinoamérica)"),
    ("et", "Eesti"),
    ("eu", "Euskara"),
    ("fa", "فارسی"),
    ("fi", "Suomi"),
    ("fil", "Filipino"),
    ("fr", "Français"),
    ("ga", "Gaeilge"),
    ("gl", "Galego"),
    ("he", "עברית"),
    ("hi", "हिन्दी"),
    ("hr", "Hrvatski"),
    ("hu", "Magyar"),
    ("hy", "Հայերեն"),
    ("id", "Indonesia"),
    ("is", "Íslenska"),
    ("it", "Italiano"),
    ("ja", "日本語"),
    ("ka", "ქართული"),
    ("kk", "Қазақ тілі"),
    ("km", "ខ្មែរ"),
    ("kn", "ಕನ್ನಡ"),
    ("ko", "한국어"),
    ("la", "Latina"),
    ("lo", "ລາວ"),
    ("lt", "Lietuvių"),
    ("lv", "Latviešu"),
    ("mk", "Македонски"),
    ("ml", "മലയാളം"),
    ("mn", "Монгол"),
    ("mr", "मराठी"),
    ("ms", "Melayu"),
    ("mt", "Malti"),
    ("my", "မြန်မာ"),
    ("nb", "Norsk bokmål"),
    ("ne", "नेपाली"),
    ("nl", "Nederlands"),
    ("no", "Norsk"),
    ("pl", "Polski"),
    ("pt", "Português"),
    ("pt-br", "Português (Brasil)"),
    ("pt-pt", "Português (Portugal)"),
    ("ro", "Română"),
    ("ru", "Русский"),
    ("si", "සිංහල"),
    ("sk", "Slovenčina"),
    ("sl", "Slovenščina"),
    ("sq", "Shqip"),
    ("sr", "Српски"),
    ("sv", "Svenska"),
    ("sw", "Kiswahili"),
    ("ta", "தமிழ்"),
    ("te", "తెలుగు"),
    ("th", "ไทย"),
    ("tr", "Türkçe"),
    ("uk", "Українська"),
    ("ur", "اردو"),
    ("uz", "Oʻzbek"),
    ("vi", "Tiếng Việt"),
    ("zh", "中文"),
    ("zh-hans", "中文 (简体)"),
    ("zh-hant", "中文 (繁體)"),
];

/// `zh-CN` is the Simplified script and `zh-TW` the Traditional one.
fn normalise(lang: &str) -> &str {
    match lang {
        "zh-CN" => "zh-Hans",
        "zh-TW" => "zh-Hant",
        other => other,
    }
}

/// The table row for a tag, trying the whole tag and then its language subtag.
fn self_name(tag: &str) -> Option<&'static str> {
    let lower = tag.replace('_', "-").to_ascii_lowercase();
    let find = |key: &str| SELF_NAMES.iter().find(|(k, _)| *k == key).map(|(_, name)| *name);
    find(&lower).or_else(|| lower.split('-').next().and_then(find))
}

/// `replaceFirstChar { it.uppercase() }`.
fn uppercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A `glib::language_names()` entry as a language tag: `de_DE.UTF-8@euro` becomes `de-DE`. The `C` and `POSIX`
/// locales and an empty entry mean no language was chosen, so they give `en`.
pub fn system_tag(raw: &str) -> String {
    let base = raw.split(['.', '@']).next().unwrap_or_default();
    if base.is_empty() || base == "C" || base == "POSIX" {
        "en".to_owned()
    } else {
        base.replace('_', "-")
    }
}

/// The user's first language, read once at startup.
pub fn system_language() -> String {
    gtk::glib::language_names().first().map_or_else(|| "en".to_owned(), |name| system_tag(name))
}

/// The names for one run of the program. Results are cached by tag, so a language change while running shows after a
/// restart.
pub struct LocaleNames {
    system: String,
    cache: Mutex<HashMap<String, String>>,
}

impl LocaleNames {
    /// `system` is the tag an empty language stands for (see [`system_language`]).
    pub fn new(system: String) -> Self {
        Self { system, cache: Mutex::new(HashMap::new()) }
    }

    /// `getLocalizedDisplayName`: the language in itself, first letter upper case. `None` gives an empty string and an
    /// empty tag means the system language.
    pub fn localized_display_name(&self, lang: Option<&str>) -> String {
        let Some(lang) = lang else { return String::new() };
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(hit) = cache.get(lang) {
            return hit.clone();
        }
        let tag = if lang.is_empty() { self.system.as_str() } else { normalise(lang) };
        let name = uppercase_first(self_name(tag).unwrap_or(tag));
        cache.insert(lang.to_owned(), name.clone());
        name
    }

    /// `getSourceDisplayName`: the group sentinels are words, anything else is a language name.
    pub fn source_display_name(&self, lang: Option<&str>) -> String {
        match lang {
            Some(LAST_USED_KEY) => tr(MsgKey::last_used_source),
            Some(PINNED_KEY) => tr(MsgKey::pinned_sources),
            Some("other") => tr(MsgKey::other_source),
            Some("all") => tr(MsgKey::multi_lang),
            other => self.localized_display_name(other),
        }
    }

    /// `LocaleHelper.comparator`: `all` first, then the display names in UTF-16 code unit order. Two `all` are equal
    /// (upstream answers -1, which is not a valid comparator).
    pub fn compare(&self, a: &str, b: &str) -> Ordering {
        match (a == "all", b == "all") {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => self
                .localized_display_name(Some(a))
                .encode_utf16()
                .cmp(self.localized_display_name(Some(b)).encode_utf16()),
        }
    }

    #[cfg(test)]
    fn cached(&self) -> usize {
        self.cache.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> LocaleNames {
        LocaleNames::new("de-DE".to_owned())
    }

    #[test]
    fn all_sorts_first_and_two_all_are_equal() {
        let n = names();
        assert_eq!(n.compare("all", "en"), Ordering::Less);
        assert_eq!(n.compare("en", "all"), Ordering::Greater);
        assert_eq!(n.compare("all", "all"), Ordering::Equal);
        let mut langs = vec!["fr", "ja", "all", "en", "de"];
        langs.sort_by(|a, b| n.compare(a, b));
        assert_eq!(langs, ["all", "de", "en", "fr", "ja"], "by display name: Deutsch, English, Français, 日本語");
    }

    #[test]
    fn chinese_region_tags_name_the_script() {
        let n = names();
        assert_eq!(n.localized_display_name(Some("zh-CN")), n.localized_display_name(Some("zh-Hans")));
        assert_eq!(n.localized_display_name(Some("zh-TW")), n.localized_display_name(Some("zh-Hant")));
        assert_ne!(n.localized_display_name(Some("zh-CN")), n.localized_display_name(Some("zh-TW")));
    }

    #[test]
    fn a_missing_language_is_empty_and_the_empty_tag_is_the_system_language() {
        let n = names();
        assert_eq!(n.localized_display_name(None), "");
        assert_eq!(n.source_display_name(None), "");
        assert_eq!(n.localized_display_name(Some("")), "Deutsch");
    }

    #[test]
    fn group_sentinels_render_as_words() {
        let n = names();
        assert_eq!(n.source_display_name(Some("last_used")), "Last used");
        assert_eq!(n.source_display_name(Some("pinned")), "Pinned");
        assert_eq!(n.source_display_name(Some("other")), "Other");
        assert_eq!(n.source_display_name(Some("all")), "Multi");
        assert_eq!(n.source_display_name(Some("ja")), "日本語");
    }

    #[test]
    fn names_are_cached_per_tag_and_do_not_follow_a_later_system_change() {
        let n = names();
        assert_eq!(n.cached(), 0);
        assert_eq!(n.localized_display_name(Some("en")), "English");
        assert_eq!(n.localized_display_name(Some("en")), "English");
        assert_eq!(n.cached(), 1);
        n.localized_display_name(Some("fr"));
        assert_eq!(n.cached(), 2);
        assert_eq!(n.localized_display_name(Some("")), "Deutsch");
        // a second instance with another system language is a restart
        assert_eq!(LocaleNames::new("fr".to_owned()).localized_display_name(Some("")), "Français");
    }

    #[test]
    fn regions_fall_back_to_the_language_and_unknown_tags_show_the_tag() {
        let n = names();
        assert_eq!(n.localized_display_name(Some("pt-BR")), "Português (Brasil)");
        assert_eq!(n.localized_display_name(Some("de-AT")), "Deutsch");
        assert_eq!(n.localized_display_name(Some("xx")), "Xx");
    }

    #[test]
    fn system_tags_lose_their_encoding_and_modifier() {
        assert_eq!(system_tag("de_DE.UTF-8"), "de-DE");
        assert_eq!(system_tag("sr_RS@latin"), "sr-RS");
        assert_eq!(system_tag("en_US.UTF-8@euro"), "en-US");
        assert_eq!(system_tag("C"), "en");
        assert_eq!(system_tag("POSIX"), "en");
        assert_eq!(system_tag(""), "en");
        assert_eq!(system_tag("fr"), "fr");
    }
}
