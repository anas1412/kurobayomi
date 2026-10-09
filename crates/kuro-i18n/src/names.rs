//! Native and English display names for the offered locales (plan 51-i18n T-51a-012).
//!
//! The table is committed data. Native names are written in each language; `zh-CN` and `zh-TW`
//! use the Simplified and Traditional names the Android picker shows. The table has the 67
//! offered tags (English plus 66 translations) and `ur`, which is not offered until it is
//! translated.
//!
//! ponytail: hand-written, not generated from ICU4X display names (no 2.x release of
//! icu_displaynames exists on crates.io yet). Regenerate if a name is wrong.

/// One row of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocaleName {
    pub tag: &'static str,
    pub native: &'static str,
    pub english: &'static str,
}

const fn row(tag: &'static str, native: &'static str, english: &'static str) -> LocaleName {
    LocaleName {
        tag,
        native,
        english,
    }
}

pub const NAMES: &[LocaleName] = &[
    row("en", "English", "English"),
    row("am", "አማርኛ", "Amharic"),
    row("ar", "العربية", "Arabic"),
    row("as", "অসমীয়া", "Assamese"),
    row("be", "Беларуская", "Belarusian"),
    row("bg", "Български", "Bulgarian"),
    row("bn", "বাংলা", "Bengali"),
    row("ca", "Català", "Catalan"),
    row("ceb", "Cebuano", "Cebuano"),
    row("cs", "Čeština", "Czech"),
    row("cv", "Чӑвашла", "Chuvash"),
    row("da", "Dansk", "Danish"),
    row("de", "Deutsch", "German"),
    row("el", "Ελληνικά", "Greek"),
    row("eo", "Esperanto", "Esperanto"),
    row("es", "Español", "Spanish"),
    row("eu", "Euskara", "Basque"),
    row("fa", "فارسی", "Persian"),
    row("fi", "Suomi", "Finnish"),
    row("fil", "Filipino", "Filipino"),
    row("fr", "Français", "French"),
    row("gl", "Galego", "Galician"),
    row("he", "עברית", "Hebrew"),
    row("hi", "हिन्दी", "Hindi"),
    row("hr", "Hrvatski", "Croatian"),
    row("hu", "Magyar", "Hungarian"),
    row("id", "Bahasa Indonesia", "Indonesian"),
    row("it", "Italiano", "Italian"),
    row("ja", "日本語", "Japanese"),
    row("jv", "Basa Jawa", "Javanese"),
    row("ka-GE", "ქართული", "Georgian"),
    row("kk", "Қазақша", "Kazakh"),
    row("km", "ភាសាខ្មែរ", "Khmer"),
    row("kn", "ಕನ್ನಡ", "Kannada"),
    row("ko", "한국어", "Korean"),
    row("lt", "Lietuvių", "Lithuanian"),
    row("lv", "Latviešu", "Latvian"),
    row("ml", "മലയാളം", "Malayalam"),
    row("mr", "मराठी", "Marathi"),
    row("ms", "Bahasa Melayu", "Malay"),
    row("my", "မြန်မာ", "Burmese"),
    row("nb-NO", "Norsk bokmål", "Norwegian Bokmål"),
    row("ne", "नेपाली", "Nepali"),
    row("nl", "Nederlands", "Dutch"),
    row("nn", "Norsk nynorsk", "Norwegian Nynorsk"),
    row("pl", "Polski", "Polish"),
    row("pt", "Português", "Portuguese"),
    row("pt-BR", "Português (Brasil)", "Portuguese (Brazil)"),
    row("ro", "Română", "Romanian"),
    row("ru", "Русский", "Russian"),
    row("sa", "संस्कृतम्", "Sanskrit"),
    row("sah", "Саха тыла", "Yakut"),
    row("sc", "Sardu", "Sardinian"),
    row("sdh", "کوردی خوارگ", "Southern Kurdish"),
    row("sk", "Slovenčina", "Slovak"),
    row("sq", "Shqip", "Albanian"),
    row("sr", "Српски", "Serbian"),
    row("sv", "Svenska", "Swedish"),
    row("ta", "தமிழ்", "Tamil"),
    row("te", "తెలుగు", "Telugu"),
    row("th", "ไทย", "Thai"),
    row("tr", "Türkçe", "Turkish"),
    row("uk", "Українська", "Ukrainian"),
    row("uz", "Oʻzbekcha", "Uzbek"),
    row("vi", "Tiếng Việt", "Vietnamese"),
    row("zh-CN", "简体中文", "Chinese (Simplified)"),
    row("zh-TW", "繁體中文", "Chinese (Traditional)"),
    row("ur", "اردو", "Urdu"),
];

/// The row for `tag`, if the table has one.
pub fn find(tag: &str) -> Option<&'static LocaleName> {
    NAMES.iter().find(|n| n.tag == tag)
}

/// The native name for `tag`, with its first character uppercased (the picker rule, topic
/// section 3). A tag with no row is returned unchanged.
pub fn native_name(tag: &str) -> String {
    match find(tag) {
        Some(name) => capitalise_first(name.native),
        None => tag.to_string(),
    }
}

/// The English name for `tag`, or the tag itself when the table has no row.
pub fn english_name(tag: &str) -> String {
    find(tag).map_or_else(|| tag.to_string(), |n| n.english.to_string())
}

/// Uppercases the first character, when the script has case. Other scripts are unchanged.
/// Georgian (U+10D0 to U+10FF) is left alone: its native names are written in lowercase
/// Mkhedruli, and Unicode 11 added Mtavruli capitals that Kurobayomi does not use (test D3).
pub fn capitalise_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if !('\u{10D0}'..='\u{10FF}').contains(&first) => {
            first.to_uppercase().chain(chars).collect()
        }
        Some(first) => std::iter::once(first).chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn table_has_the_offered_tags_and_ur() {
        // T-51a-012: en plus 66 translations, and ur.
        assert_eq!(NAMES.len(), 68);
        let tags: HashSet<&str> = NAMES.iter().map(|n| n.tag).collect();
        assert_eq!(tags.len(), NAMES.len(), "tags are unique");
        assert!(tags.contains("ur"));
    }

    #[test]
    fn native_names_are_unique() {
        // D7: two languages never share a native name.
        let natives: HashSet<&str> = NAMES.iter().map(|n| n.native).collect();
        assert_eq!(natives.len(), NAMES.len());
    }

    #[test]
    fn chinese_names_match_the_android_picker() {
        assert_eq!(native_name("zh-CN"), "简体中文");
        assert_eq!(native_name("zh-TW"), "繁體中文");
    }

    #[test]
    fn first_character_is_uppercased_where_the_script_has_case() {
        // D2: Türkçe keeps its T. D1 and D3: scripts without case are unchanged.
        assert_eq!(native_name("tr"), "Türkçe");
        assert_eq!(capitalise_first("ßtraße"), "SStraße");
        assert_eq!(native_name("ar"), "العربية");
        assert_eq!(native_name("ka-GE"), "ქართული");
        assert_eq!(capitalise_first("lietuvių"), "Lietuvių");
    }

    #[test]
    fn unknown_tag_is_returned_unchanged() {
        assert_eq!(native_name("xx"), "xx");
        assert_eq!(english_name("xx"), "xx");
        assert_eq!(english_name("de"), "German");
    }
}
