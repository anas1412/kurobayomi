//! `[base]` preferences (Aniyomi `BasePreferences`), plus the two release-check keys Linux adds.
//!
//! `downloaded_only`, `incognito_mode` and `shown_onboarding_flow` are app state and live in
//! `state.toml`. The Android `extension_installer` choice is dropped on Linux (T-19a-037): it has no field, and a
//! backed-up value is skipped on restore. `pref_hardware_bitmap_threshold` is dropped too (T-13a-030): nothing reads
//! it, GTK manages textures, and a backed-up value is skipped on restore.

use crate::registry::define_prefs;

define_prefs! {
    /// `[base]` section: the keys of Aniyomi `BasePreferences` that Linux keeps, then the two release-check keys.
    pub struct BasePrefs in "base" {
        downloaded_only: bool = "__APP_STATE_pref_downloaded_only" => false;
        incognito_mode: bool = "__APP_STATE_incognito_mode" => false;
        shown_onboarding_flow: bool = "__APP_STATE_onboarding_complete" => false;
        display_profile: String = "pref_display_profile_key" => String::new();
        always_decode_long_strip_with_ssiv: bool = "pref_always_decode_long_strip_with_ssiv" => false;
        // Linux additions (T-07a-036): the startup release check is opt-in, and its last successful lookup is app state
        check_app_updates: bool = "check_app_updates" => false;
        last_app_check: i64 = "__APP_STATE_last_app_check" => 0;
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::paths::Paths;

    use super::*;
    use crate::toml_backend::{config_file, state_file};
    use crate::network::NetworkPrefs;
    use crate::security::SecurityPrefs;
    use crate::settings_load::open;
    use crate::store::PrefStore;
    use crate::testutil::{TestDir, TestPaths, assert_section_registered, read_toml};
    use crate::values::PrefValue;
    use std::fs;

    #[test]
    fn keys_are_the_aniyomi_keys_in_their_base_fields() {
        assert_eq!(
            BasePrefs::FIELDS,
            &[
                ("downloaded_only", "__APP_STATE_pref_downloaded_only"),
                ("incognito_mode", "__APP_STATE_incognito_mode"),
                ("shown_onboarding_flow", "__APP_STATE_onboarding_complete"),
                ("display_profile", "pref_display_profile_key"),
                (
                    "always_decode_long_strip_with_ssiv",
                    "pref_always_decode_long_strip_with_ssiv",
                ),
                ("check_app_updates", "check_app_updates"),
                ("last_app_check", "__APP_STATE_last_app_check"),
            ]
        );
        assert_eq!(BasePrefs::TABLE, "base");
    }

    #[test]
    fn every_field_is_registered_under_the_base_table() {
        let base = BasePrefs::new(PrefStore::in_memory());
        assert_section_registered(BasePrefs::TABLE, BasePrefs::FIELDS, &base.defaults());
    }

    #[test]
    fn defaults_are_the_plan_defaults() {
        let base = BasePrefs::new(PrefStore::in_memory());
        assert!(!base.downloaded_only.get());
        assert!(!base.incognito_mode.get());
        assert!(!base.shown_onboarding_flow.get());
        assert_eq!(base.display_profile.get(), "");
        assert!(!base.always_decode_long_strip_with_ssiv.get());
        assert!(!base.check_app_updates.get(), "the release check is opt-in");
        assert_eq!(base.last_app_check.get(), 0);
    }

    #[test]
    fn base_values_round_trip_and_app_state_goes_to_state_toml() {
        let paths = TestPaths(TestDir::new("base-round-trip"));
        {
            let store = open(&paths).unwrap();
            let base = BasePrefs::new(store.clone());
            base.display_profile.set("/icc/paper.icc".into()).unwrap();
            base.downloaded_only.set(true).unwrap();
            base.incognito_mode.set(true).unwrap();
            base.shown_onboarding_flow.set(true).unwrap();
            store.flush().unwrap();
        }
        let store = open(&paths).unwrap();
        let base = BasePrefs::new(store.clone());
        assert_eq!(base.display_profile.get(), "/icc/paper.icc");
        assert!(base.downloaded_only.get());
        assert!(base.incognito_mode.get());
        assert!(base.shown_onboarding_flow.get());
        assert_eq!(
            store.get_raw("__APP_STATE_pref_downloaded_only"),
            Some(PrefValue::Bool(true))
        );

        let config = read_toml(&config_file(&paths.config_dir()));
        assert_eq!(
            config["base"]["display_profile"].as_str(),
            Some("/icc/paper.icc")
        );
        assert!(config["base"].get("downloaded_only").is_none());

        let state = read_toml(&state_file(&paths.state_dir()));
        assert_eq!(state["base"]["downloaded_only"].as_bool(), Some(true));
        assert_eq!(state["base"]["incognito_mode"].as_bool(), Some(true));
        assert_eq!(state["base"]["shown_onboarding_flow"].as_bool(), Some(true));
        assert!(state["base"].get("display_profile").is_none());
    }

    #[test]
    fn a_file_with_only_unknown_keys_gives_every_default_and_keeps_its_keys() {
        let paths = TestPaths(TestDir::new("bootstrap-unknown"));
        fs::create_dir_all(paths.config_dir()).unwrap();
        fs::create_dir_all(paths.state_dir()).unwrap();
        fs::write(
            config_file(&paths.config_dir()),
            "# from a newer build\nfuture_key = 1\n\n[future]\nx = \"y\"\n",
        )
        .unwrap();
        fs::write(state_file(&paths.state_dir()), "[future]\nz = true\n").unwrap();

        let store = open(&paths).unwrap();
        let base = BasePrefs::new(store.clone());
        let security = SecurityPrefs::new(store.clone());
        let network = NetworkPrefs::new(store.clone());
        assert!(!base.downloaded_only.get());
        assert!(!security.use_authenticator.get());
        assert_eq!(security.lock_app_after.get(), 0);
        assert_eq!(network.doh_provider.get(), -1);
        assert_eq!(network.verbose_logging.get(), cfg!(debug_assertions));

        network.doh_provider.set(2).unwrap();
        store.flush().unwrap();
        let config = fs::read_to_string(config_file(&paths.config_dir())).unwrap();
        assert!(config.contains("# from a newer build"));
        assert!(config.contains("future_key = 1"));
        assert!(config.contains("[future]") && config.contains("x = \"y\""));
        let state = fs::read_to_string(state_file(&paths.state_dir())).unwrap();
        assert!(state.contains("[future]") && state.contains("z = true"));
    }
}
