//! App and source preference creators with the key rules (T-21a-020, T-21a-021, T-21a-044).

use crate::schema::{BackupPreference, BackupSourcePreferences, PrefValue};

const APP_STATE_PREFIX: &str = "__APP_STATE_";
const PRIVATE_PREFIX: &str = "__PRIVATE_";

/// Keys with `__APP_STATE_` are never written; `__PRIVATE_` only when sensitive settings are included (E-43 keeps
/// every value variant, since `PrefValue` is closed).
fn keep(key: &str, include_private: bool) -> bool {
    if key.starts_with(APP_STATE_PREFIX) {
        return false;
    }
    include_private || !key.starts_with(PRIVATE_PREFIX)
}

fn to_backup(all: &[(String, PrefValue)], include_private: bool) -> Vec<BackupPreference> {
    all.iter()
        .filter(|(k, _)| keep(k, include_private))
        .map(|(k, v)| BackupPreference { key: k.clone(), value: v.clone() })
        .collect()
}

/// App preferences (B-08h). A Kurobayomi-only setting gets the `kuro_` key prefix (T-02a-040), so Aniyomi's restorer
/// reads it as an ordinary extra value. Source preferences keep their keys.
pub fn app(all: &[(String, PrefValue)], include_private: bool) -> Vec<BackupPreference> {
    let mut prefs = to_backup(all, include_private);
    for pref in &mut prefs {
        pref.key = kuro_prefs::keys::backup_key(&pref.key);
    }
    prefs
}

/// One `BackupSourcePreferences` per configurable source with at least one kept preference (B-08h).
pub fn sources(configurable: &[(String, Vec<(String, PrefValue)>)], include_private: bool) -> Vec<BackupSourcePreferences> {
    configurable
        .iter()
        .filter_map(|(source_key, prefs)| {
            let prefs = to_backup(prefs, include_private);
            (!prefs.is_empty()).then(|| BackupSourcePreferences { source_key: source_key.clone(), prefs })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<(String, PrefValue)> {
        vec![
            ("pref_theme_mode_key".into(), PrefValue::String("dark".into())),
            ("__APP_STATE_last_auto_backup_timestamp".into(), PrefValue::Long(5)),
            ("__PRIVATE_track_token_2".into(), PrefValue::String("secret".into())),
            ("kuro_only_setting".into(), PrefValue::Int(3)),
        ]
    }

    #[test]
    fn app_prefs_exclude_app_state_and_private() {
        let off: Vec<String> = app(&sample(), false).into_iter().map(|p| p.key).collect();
        assert_eq!(off, vec!["pref_theme_mode_key", "kuro_kuro_only_setting"]);
        let on: Vec<String> = app(&sample(), true).into_iter().map(|p| p.key).collect();
        assert_eq!(on, vec!["pref_theme_mode_key", "__PRIVATE_track_token_2", "kuro_kuro_only_setting"]);
    }

    /// T-18a-043 and T-19a-065: the trust set is app state, so no backup carries it, whatever the private flag says.
    #[test]
    fn trust_entries_are_app_state_and_never_written() {
        let trust = vec![("__APP_STATE_trusted_extensions".to_string(), PrefValue::StringSet(["eu.a:1:ab".to_string()].into()))];
        assert!(app(&trust, true).is_empty());
    }

    #[test]
    fn value_types_preserved() {
        let all = vec![
            ("i".into(), PrefValue::Int(1)),
            ("l".into(), PrefValue::Long(2)),
            ("f".into(), PrefValue::Float(1.5)),
            ("s".into(), PrefValue::String("x".into())),
            ("b".into(), PrefValue::Boolean(true)),
            ("set".into(), PrefValue::StringSet(["a".to_string()].into_iter().collect())),
        ];
        let got = app(&all, false);
        assert_eq!(got.len(), 6);
        assert_eq!(got[0].value, PrefValue::Int(1));
        assert_eq!(got[4].value, PrefValue::Boolean(true));
        assert!(matches!(got[5].value, PrefValue::StringSet(_)));
    }

    /// T-02a-040: a Kurobayomi-only key leaves with the prefix and comes back without it; Aniyomi keys and source
    /// keys are untouched.
    #[test]
    fn kurobayomi_only_keys_round_trip_through_the_prefix() {
        let all = vec![
            ("pref_theme_mode_key".to_string(), PrefValue::String("SYSTEM".into())),
            ("texture_probe_note".to_string(), PrefValue::Int(7)),
        ];
        let written = app(&all, false);
        assert_eq!(written[0].key, "pref_theme_mode_key");
        assert_eq!(written[1].key, "kuro_texture_probe_note");
        let back: Vec<String> = written.iter().map(|p| kuro_prefs::keys::restored_key(&p.key)).collect();
        assert_eq!(back, vec!["pref_theme_mode_key", "texture_probe_note"]);
        let source = vec![("source_1".to_string(), vec![("texture_probe_note".to_string(), PrefValue::Int(7))])];
        assert_eq!(sources(&source, false)[0].prefs[0].key, "texture_probe_note");
    }

    #[test]
    fn source_prefs_only_non_empty() {
        let configurable = vec![
            ("source_1".to_string(), vec![("a".to_string(), PrefValue::Boolean(true))]),
            ("source_2".to_string(), vec![]),
            ("source_3".to_string(), vec![("__APP_STATE_x".to_string(), PrefValue::Int(1))]),
        ];
        let got = sources(&configurable, false);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].source_key, "source_1");
    }

    #[test]
    fn private_source_keys_follow_the_flag() {
        let configurable = vec![("source_1".to_string(), vec![("__PRIVATE_pw".to_string(), PrefValue::String("p".into()))])];
        assert!(sources(&configurable, false).is_empty());
        assert_eq!(sources(&configurable, true)[0].prefs.len(), 1);
    }
}
