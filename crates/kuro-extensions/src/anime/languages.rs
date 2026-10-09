//! The language list and first-run enablement (T-19a-028, `GetAnimeExtensionLanguages.kt`, behavior 5.6).

use std::collections::BTreeSet;

use kuro_prefs::PrefError;
use kuro_prefs::source::{SourcePrefs, default_enabled_languages};

use crate::anime::model::Available;

/// The distinct source languages of the listing, enabled ones first, `all` first inside each group. Zero-source
/// extensions add nothing. Inside a group the order is by code (the display-name collation of `LocaleHelper` needs the
/// locale tables of area 02).
pub fn language_list(available: &[Available], enabled: &BTreeSet<String>) -> Vec<String> {
    let mut languages: Vec<String> = available
        .iter()
        .flat_map(|listed| listed.sources.iter().map(|source| source.lang.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    languages.sort_by_key(|lang| (!enabled.contains(lang), lang != "all", lang.to_lowercase()));
    languages
}

/// The language part of `locale` (`zh_TW.UTF-8` gives `zh`), as `Locale.getDefault().language`. The `C` and `POSIX`
/// locales count as English (F6).
fn device_language(locale: &str) -> String {
    let language = locale.split(['.', '@', '_', '-']).next().unwrap_or_default().to_ascii_lowercase();
    if language == "c" || language == "posix" { "en".to_owned() } else { language }
}

/// First-run enablement (behavior 5.6). It does nothing when the enabled languages were saved before, or when the
/// listing is empty. Otherwise it saves the default set plus every listed source language that starts with the device
/// language, such as `zh-Hans` for `zh`. Saving the set is the "done" marker, so a second call changes nothing. Returns
/// whether it saved.
pub fn enable_first_run_languages(
    prefs: &SourcePrefs,
    available: &[Available],
    locale: &str,
) -> Result<bool, PrefError> {
    if prefs.enabled_languages.is_set() || available.is_empty() {
        return Ok(false);
    }
    let device = device_language(locale);
    let mut enabled = default_enabled_languages(locale);
    if !device.is_empty() {
        let extra = available
            .iter()
            .flat_map(|listed| listed.sources.iter())
            .map(|source| source.lang.as_str())
            .filter(|lang| *lang != device && lang.starts_with(device.as_str()));
        enabled.extend(extra.map(str::to_owned));
    }
    prefs.enabled_languages.set(enabled)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::model::{AnimeSourceRef, Kind, StoreRef};
    use kuro_prefs::store::PrefStore;

    fn listed(langs: &[&str]) -> Available {
        Available {
            name: "A".into(),
            raw_name: "A".into(),
            pkg: "p.a".into(),
            version_name: "1".into(),
            version_code: 1,
            abi: None,
            aniyomi_lib: None,
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources: langs
                .iter()
                .enumerate()
                .map(|(i, lang)| AnimeSourceRef {
                    id: i as i64,
                    name: "S".into(),
                    lang: (*lang).into(),
                    base_url: String::new(),
                })
                .collect(),
            kind: Kind::Wasm,
            apk_url: String::new(),
            icon_url: String::new(),
            kuroext: None,
            store: StoreRef { index_url: "s".into(), name: "s".into() },
        }
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    #[test]
    fn tst_19_071_zero_source_extensions_give_an_empty_list() {
        assert!(language_list(&[listed(&[])], &set(&["en"])).is_empty());
        assert!(language_list(&[], &set(&["en"])).is_empty());
    }

    #[test]
    fn the_list_is_distinct_enabled_first_and_all_first() {
        let list = language_list(&[listed(&["ja", "en", "all", "de", "en"])], &set(&["en", "ja"]));
        assert_eq!(list, ["en", "ja", "all", "de"]);
    }

    #[test]
    fn tst_19_072_first_run_enables_the_device_sub_languages() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        let available = [listed(&["zh-Hans", "zh-Hant", "en", "ja"])];
        assert!(enable_first_run_languages(&prefs, &available, "zh_CN.UTF-8").unwrap());
        assert_eq!(prefs.enabled_languages.get(), set(&["all", "en", "zh", "zh-Hans", "zh-Hant"]));
    }

    #[test]
    fn tst_19_073_the_second_run_changes_nothing() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        let available = [listed(&["zh-Hans", "zh-Hant", "en"])];
        enable_first_run_languages(&prefs, &available, "zh").unwrap();
        prefs.enabled_languages.set(set(&["en"])).unwrap();
        assert!(!enable_first_run_languages(&prefs, &available, "zh").unwrap());
        assert_eq!(prefs.enabled_languages.get(), set(&["en"]), "the user's later choice stays");
    }

    #[test]
    fn the_c_and_posix_locales_count_as_english_not_as_every_language_starting_with_c() {
        for locale in ["C", "POSIX", "C.UTF-8"] {
            let prefs = SourcePrefs::new(PrefStore::in_memory());
            let available = [listed(&["cs", "ca", "en", "en-GB", "ja"])];
            assert!(enable_first_run_languages(&prefs, &available, locale).unwrap());
            assert_eq!(prefs.enabled_languages.get(), set(&["all", "en", "en-GB"]), "{locale}");
        }
    }

    #[test]
    fn an_empty_listing_leaves_the_first_run_for_later() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        assert!(!enable_first_run_languages(&prefs, &[], "zh").unwrap());
        assert!(!prefs.enabled_languages.is_set());
        assert!(enable_first_run_languages(&prefs, &[listed(&["zh-Hans"])], "zh").unwrap());
    }
}
