//! Display names for the source-language filter (plan 51-i18n T-51a-034).

use std::cmp::Ordering;

use crate::names;
use crate::{MsgKey, tr};

/// The label for one filter key: the three special groups, `all` ("Multi"), or the native name
/// of a language tag.
pub fn display_name(key: &str) -> String {
    match key {
        "last_used" => tr(MsgKey::last_used_source),
        "pinned" => tr(MsgKey::pinned_sources),
        "other" => tr(MsgKey::other_source),
        "all" => tr(MsgKey::multi_lang),
        tag => names::native_name(tag),
    }
}

/// Sorts language keys with `all` first, then by display name.
pub fn sort_keys(keys: &mut [String]) {
    keys.sort_by(|a, b| match (a.as_str(), b.as_str()) {
        ("all", "all") => Ordering::Equal,
        ("all", _) => Ordering::Less,
        (_, "all") => Ordering::Greater,
        _ => display_name(a).cmp(&display_name(b)),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_keys_have_their_labels() {
        assert_eq!(display_name("last_used"), "Last used");
        assert_eq!(display_name("pinned"), "Pinned");
        assert_eq!(display_name("other"), "Other");
        assert_eq!(display_name("all"), "Multi");
    }

    #[test]
    fn language_tags_use_the_native_name() {
        assert_eq!(display_name("de"), "Deutsch");
        assert_eq!(display_name("zh-CN"), "简体中文");
    }

    #[test]
    fn all_sorts_first_then_by_display_name() {
        let mut keys: Vec<String> = ["tr", "all", "de", "ar"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        sort_keys(&mut keys);
        assert_eq!(keys[0], "all");
        let rest: Vec<&str> = keys[1..].iter().map(String::as_str).collect();
        // Latin native names sort before Arabic in code-point order: Deutsch, Türkçe, العربية.
        assert_eq!(rest, ["de", "tr", "ar"]);
    }
}
