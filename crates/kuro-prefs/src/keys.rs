//! Key namespaces. The prefix decides the store: no prefix is a normal setting (`config.toml`),
//! `__APP_STATE_` is app state (`state.toml`), and `__PRIVATE_` is a credential (OS secret store).

pub const APP_STATE_PREFIX: &str = "__APP_STATE_";
pub const PRIVATE_PREFIX: &str = "__PRIVATE_";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Namespace {
    Normal,
    AppState,
    Private,
}

impl Namespace {
    pub fn of(key: &str) -> Self {
        if is_app_state(key) {
            Self::AppState
        } else if is_private(key) {
            Self::Private
        } else {
            Self::Normal
        }
    }
}

pub fn is_app_state(key: &str) -> bool {
    key.starts_with(APP_STATE_PREFIX)
}

pub fn is_private(key: &str) -> bool {
    key.starts_with(PRIVATE_PREFIX)
}

pub fn app_state_key(key: &str) -> String {
    format!("{APP_STATE_PREFIX}{key}")
}

pub fn private_key(key: &str) -> String {
    format!("{PRIVATE_PREFIX}{key}")
}

/// Prefix of a Kurobayomi-only setting inside a backup (T-02a-040). Aniyomi's restorer accepts any string key, so
/// the entry restores there as an ordinary value and changes nothing else.
pub const KURO_PREFIX: &str = "kuro_";

/// The backup key for a stored key. A key the Kotlin registry knows keeps its name. A Kurobayomi-only normal key
/// gets [`KURO_PREFIX`]. App state and private keys are handled by their own rules and are returned unchanged.
pub fn backup_key(key: &str) -> String {
    let normal = Namespace::of(key) == Namespace::Normal;
    if normal && crate::registry::lookup(key).is_none() {
        format!("{KURO_PREFIX}{key}")
    } else {
        key.to_owned()
    }
}

/// The stored key for a key read from a backup: a `kuro_` key gets its prefix removed, every other key is kept.
pub fn restored_key(key: &str) -> String {
    key.strip_prefix(KURO_PREFIX).map_or_else(|| key.to_owned(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_follows_the_prefix() {
        assert_eq!(
            Namespace::of("__APP_STATE_pref_downloaded_only"),
            Namespace::AppState
        );
        assert_eq!(Namespace::of("__PRIVATE_track_token_3"), Namespace::Private);
        assert_eq!(Namespace::of("display_mode"), Namespace::Normal);
    }

    #[test]
    fn prefix_helpers_round_trip() {
        assert_eq!(
            app_state_key("incognito_mode"),
            "__APP_STATE_incognito_mode"
        );
        assert_eq!(private_key("x"), "__PRIVATE_x");
        assert!(is_app_state(&app_state_key("a")) && !is_private(&app_state_key("a")));
        assert!(is_private(&private_key("a")) && !is_app_state(&private_key("a")));
    }

    #[test]
    fn kurobayomi_only_keys_round_trip_through_the_kuro_prefix() {
        assert_eq!(backup_key("kuro_only_here"), "kuro_kuro_only_here");
        assert_eq!(restored_key(&backup_key("kuro_only_here")), "kuro_only_here");
        assert_eq!(backup_key("reader_tapping_inverted"), "reader_tapping_inverted", "a registry key keeps its name");
        assert_eq!(restored_key("reader_tapping_inverted"), "reader_tapping_inverted");
        assert_eq!(backup_key("__APP_STATE_incognito_mode"), "__APP_STATE_incognito_mode");
    }
}
