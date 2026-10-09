//! `[security]` preferences (Aniyomi `SecurityPreferences`): the app lock and the secure screen mode.
//!
//! `last_app_closed` is app state and lives in `state.toml`. The secure screen mode is stored but
//! has no effect on Linux.

use crate::pref_enum;
use crate::registry::define_prefs;

pref_enum! {
    /// `secure_screen_v2`, stored as the Kotlin constant name.
    pub enum SecureScreenMode { ALWAYS, INCOGNITO, NEVER }
}

define_prefs! {
    /// `[security]` section: the five keys of Aniyomi `SecurityPreferences`.
    pub struct SecurityPrefs in "security" {
        use_authenticator: bool = "use_biometric_lock" => false;
        lock_app_after: i32 = "lock_app_after" => 0;
        secure_screen: SecureScreenMode = "secure_screen_v2" => SecureScreenMode::INCOGNITO;
        hide_notification_content: bool = "hide_notification_content" => false;
        last_app_closed: i64 = "__APP_STATE_last_app_closed" => 0;
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::paths::Paths;

    use super::*;
    use crate::toml_backend::{config_file, state_file};
    use crate::settings_load::open;
    use crate::store::PrefStore;
    use crate::testutil::{TestDir, TestPaths, assert_section_registered, read_toml};
    use crate::values::PrefValue;

    #[test]
    fn keys_are_the_aniyomi_keys_in_their_security_fields() {
        assert_eq!(
            SecurityPrefs::FIELDS,
            &[
                ("use_authenticator", "use_biometric_lock"),
                ("lock_app_after", "lock_app_after"),
                ("secure_screen", "secure_screen_v2"),
                ("hide_notification_content", "hide_notification_content"),
                ("last_app_closed", "__APP_STATE_last_app_closed"),
            ]
        );
        assert_eq!(SecurityPrefs::TABLE, "security");
    }

    #[test]
    fn every_field_is_registered_under_the_security_table() {
        let sec = SecurityPrefs::new(PrefStore::in_memory());
        assert_section_registered(SecurityPrefs::TABLE, SecurityPrefs::FIELDS, &sec.defaults());
    }

    #[test]
    fn defaults_are_the_plan_defaults() {
        let sec = SecurityPrefs::new(PrefStore::in_memory());
        assert!(!sec.use_authenticator.get());
        assert_eq!(sec.lock_app_after.get(), 0);
        assert_eq!(sec.secure_screen.get(), SecureScreenMode::INCOGNITO);
        assert!(!sec.hide_notification_content.get());
        assert_eq!(sec.last_app_closed.get(), 0);
    }

    #[test]
    fn an_unknown_secure_screen_name_gives_the_default() {
        let store = PrefStore::in_memory();
        store.set_raw("secure_screen_v2", PrefValue::Str("SOMETIMES".into()));
        let sec = SecurityPrefs::new(store);
        assert_eq!(sec.secure_screen.get(), SecureScreenMode::INCOGNITO);
    }

    #[test]
    fn secure_screen_is_stored_by_its_constant_name() {
        let store = PrefStore::in_memory();
        SecurityPrefs::new(store.clone())
            .secure_screen
            .set(SecureScreenMode::NEVER).unwrap();
        assert_eq!(
            store.get_raw("secure_screen_v2"),
            Some(PrefValue::Str("NEVER".into()))
        );
    }

    #[test]
    fn security_values_round_trip_and_app_state_goes_to_state_toml() {
        let paths = TestPaths(TestDir::new("security-round-trip"));
        {
            let store = open(&paths).unwrap();
            let sec = SecurityPrefs::new(store.clone());
            sec.use_authenticator.set(true).unwrap();
            sec.lock_app_after.set(5).unwrap();
            sec.hide_notification_content.set(true).unwrap();
            sec.last_app_closed.set(1_700_000_000_000).unwrap();
            store.flush().unwrap();
        }
        let store = open(&paths).unwrap();
        let sec = SecurityPrefs::new(store);
        assert!(sec.use_authenticator.get());
        assert_eq!(sec.lock_app_after.get(), 5);
        assert!(sec.hide_notification_content.get());
        assert_eq!(sec.last_app_closed.get(), 1_700_000_000_000);
        let config = read_toml(&config_file(&paths.config_dir()));
        assert_eq!(
            config["security"]["use_authenticator"].as_bool(),
            Some(true)
        );
        assert_eq!(config["security"]["lock_app_after"].as_integer(), Some(5));
        let state = read_toml(&state_file(&paths.state_dir()));
        assert_eq!(
            state["security"]["last_app_closed"].as_integer(),
            Some(1_700_000_000_000)
        );
        assert!(config["security"].get("last_app_closed").is_none());
    }
}
