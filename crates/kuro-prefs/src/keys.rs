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
}
