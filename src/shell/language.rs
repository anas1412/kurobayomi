//! Language selection replaces per-app locales (Android `AppLocalesMetadataHolderService`, T-53a-031).
//!
//! The `app_language` preference is read once at startup (`data::open_blocking` calls `kuro_i18n::init`), so a change
//! applies after a restart. No AppCompat locale service and no per-app locale permission is ported. A locale that lacks a
//! string falls back through its chain to English, so every key resolves for any chosen locale (TC-53-23).

#[cfg(test)]
use kuro_i18n::LocaleInfo;

#[cfg(test)]
/// The catalogue chain for a saved choice and the system language list: the chosen locale first, English last.
pub fn chain(saved: &str, system: &[String]) -> Vec<&'static str> {
    kuro_i18n::locale::resolve(saved, system, &kuro_i18n::offered_locales())
}

#[cfg(test)]
/// Whether the saved choice differs from the language the running app was started with, so the restart note applies.
pub fn needs_restart(saved: &str, system: &[String], running: LocaleInfo) -> bool {
    chain(saved, system).first().copied() != Some(running.tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn every_offered_language_resolves_with_english_as_the_last_fallback() {
        // TC-53-23: a key missing in the chosen locale still renders, from English
        for tag in kuro_i18n::offered_locales() {
            let chain = chain(tag, &[]);
            assert_eq!(chain.first(), Some(&tag), "{tag}");
            assert_eq!(chain.last(), Some(&"en"), "{tag}");
        }
    }

    #[test]
    fn an_empty_choice_follows_the_system_list_and_an_unknown_one_falls_back_to_english() {
        assert_eq!(chain("", &system(&["de_DE.UTF-8", "en"])).first(), Some(&"de"));
        assert_eq!(chain("xx", &[]), ["en"]);
    }

    #[test]
    fn a_changed_choice_applies_after_a_restart() {
        let running = LocaleInfo::new("en");
        assert!(!needs_restart("", &system(&["en_US.UTF-8"]), running));
        assert!(!needs_restart("en", &[], running));
        assert!(needs_restart("de", &[], running));
    }
}
