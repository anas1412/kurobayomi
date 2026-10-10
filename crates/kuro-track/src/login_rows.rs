//! Settings > Tracking login rows (behaviour 12.2 and 12.3, T-27a-044). The order and the action of each row. The GUI
//! draws the check mark from the stored login state, and runs the action.

use crate::manager::TrackerManager;
use crate::messages::MsgKey;
use crate::model::TrackerId;
use crate::oauth::{ClientSecrets, PendingLogins};
use crate::tracker::Tracker;

/// What a row does when tapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginAction {
    /// Opens the authorise URL in the default browser (`PendingLogins::begin`).
    Browser,
    /// Opens the username and password dialog.
    Credentials,
}

/// One row of the tracking settings page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginRow {
    pub tracker: TrackerId,
    pub name: &'static str,
    pub action: LoginAction,
}

/// The seven login rows in display order: MyAnimeList, AniList, Kitsu, MangaUpdates, Shikimori, Simkl, Bangumi.
/// Enhanced trackers have no login row.
pub const LOGIN_ROWS: [LoginRow; 7] = [
    LoginRow {
        tracker: TrackerId::MYANIMELIST,
        name: "MyAnimeList",
        action: LoginAction::Browser,
    },
    LoginRow {
        tracker: TrackerId::ANILIST,
        name: "AniList",
        action: LoginAction::Browser,
    },
    LoginRow {
        tracker: TrackerId::KITSU,
        name: "Kitsu",
        action: LoginAction::Credentials,
    },
    LoginRow {
        tracker: TrackerId::MANGAUPDATES,
        name: "MangaUpdates",
        action: LoginAction::Credentials,
    },
    LoginRow {
        tracker: TrackerId::SHIKIMORI,
        name: "Shikimori",
        action: LoginAction::Browser,
    },
    LoginRow {
        tracker: TrackerId::SIMKL,
        name: "Simkl",
        action: LoginAction::Browser,
    },
    LoginRow {
        tracker: TrackerId::BANGUMI,
        name: "Bangumi",
        action: LoginAction::Browser,
    },
];

/// Fluent id of the info text under the rows ("tracking_info"). The GUI resolves it with the i18n crate.
pub const INFO_TEXT_ID: &str = "tracking_info";

/// A login row with its check mark: shown when the tracker is logged in. `expired` is the stored auth-expired flag;
/// `configured` is false while the tracker cannot start a login (MyAnimeList without a client id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginRowView {
    pub row: LoginRow,
    pub logged_in: bool,
    pub expired: bool,
    pub configured: bool,
}

/// The rows in order, each with the login state read from `trackers`. A tracker missing from the registry shows no
/// check mark.
pub fn login_row_views(trackers: &TrackerManager) -> Vec<LoginRowView> {
    LOGIN_ROWS
        .iter()
        .map(|&row| {
            let tracker = trackers.get(row.tracker);
            LoginRowView {
                row,
                logged_in: tracker.as_ref().is_some_and(|t| t.is_logged_in()),
                expired: tracker.as_ref().is_some_and(|t| t.auth_expired()),
                configured: tracker.as_ref().is_none_or(|t| t.setup_message().is_none()),
            }
        })
        .collect()
}

/// The subtitle of a login row (T-28a-048). "Logging in..." while `in_progress` (AniList, the Viewer query runs), and
/// "Session expired, log in again" when the stored login expired. MyAnimeList (T-29a-047) shows its setup hint first,
/// then "Waiting for MyAnimeList..." while the loopback login waits, then "Log in again" when the login expired, and
/// keeps its check mark in that case. `None` shows no subtitle.
pub fn row_subtitle(tracker: &dyn Tracker, in_progress: bool) -> Option<MsgKey> {
    if tracker.id() == TrackerId::MYANIMELIST {
        if let Some(hint) = tracker.setup_message() {
            return Some(hint);
        }
        if in_progress {
            return Some(MsgKey::MalWaitingForLogin);
        }
        if tracker.auth_expired() {
            return Some(MsgKey::MalLogInAgain);
        }
        return None;
    }
    if in_progress && tracker.id() == TrackerId::ANILIST {
        return Some(MsgKey::AniListLoggingIn);
    }
    if tracker.auth_expired() && !tracker.is_logged_in() {
        return Some(MsgKey::AniListSessionExpired);
    }
    None
}

/// What tapping a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowTap {
    /// Open this authorise URL in the default browser. The attempt is pending until the redirect arrives.
    OpenBrowser(String),
    /// Show the username and password dialog.
    OpenCredentials,
    /// Show the logout dialog. A logged-in row never starts a new login.
    ConfirmLogout,
    /// Start the MyAnimeList login with `mal::MalTracker::begin_login` (loopback listener plus paste field).
    StartMalLogin,
    /// Show this toast and open nothing: the tracker's client secret is not configured (T-27b-022).
    Toast(MsgKey),
}

/// The effect of tapping `view`. A browser row begins an attempt in `pending`. `None` when the tracker has no
/// authorise URL, so nothing opens. A browser row whose client secret is missing in `secrets` shows a toast instead.
pub fn tap_row(
    view: &LoginRowView,
    pending: &mut PendingLogins,
    secrets: &ClientSecrets,
    now_ms: i64,
) -> Option<RowTap> {
    // An expired MyAnimeList login is shown checked, and a tap starts a new login ("Log in again").
    if view.logged_in && !view.expired {
        return Some(RowTap::ConfirmLogout);
    }
    match view.row.action {
        LoginAction::Credentials => Some(RowTap::OpenCredentials),
        LoginAction::Browser if view.row.tracker == TrackerId::MYANIMELIST => {
            view.configured.then_some(RowTap::StartMalLogin)
        }
        LoginAction::Browser => Some(
            match pending.begin_configured(view.row.tracker, view.row.name, secrets, now_ms) {
                Ok(url) => RowTap::OpenBrowser(url),
                Err(toast) => RowTap::Toast(toast),
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::Harness;
    use crate::manager::REGISTRY_ORDER;

    #[tokio::test]
    async fn tr_27a_044_the_check_mark_follows_the_login_state_and_a_tap_follows_the_action() {
        let h = Harness::new();
        h.login(TrackerId::KITSU);
        let views = login_row_views(&h.ctx.trackers);
        let marks: Vec<_> = views.iter().map(|v| (v.row.name, v.logged_in)).collect();
        assert_eq!(marks[2], ("Kitsu", true));
        assert_eq!(marks.iter().filter(|m| m.1).count(), 1);

        let mut pending = PendingLogins::default();
        assert_eq!(
            tap_row(&views[2], &mut pending, &ClientSecrets::default(), 0),
            Some(RowTap::ConfirmLogout)
        );
        assert_eq!(
            tap_row(&views[3], &mut pending, &ClientSecrets::default(), 0),
            Some(RowTap::OpenCredentials)
        );
        let Some(RowTap::OpenBrowser(url)) = tap_row(&views[1], &mut pending, &ClientSecrets::default(), 0) else {
            panic!("AniList opens the browser");
        };
        assert!(url.starts_with("https://"), "{url}");
        assert_eq!(INFO_TEXT_ID, "tracking_info");
    }

    #[tokio::test]
    async fn t_27b_022_a_row_without_its_client_secret_shows_a_toast() {
        let h = Harness::new();
        let views = login_row_views(&h.ctx.trackers);
        let mut pending = PendingLogins::default();
        let shikimori = &views[4];
        assert_eq!(
            tap_row(shikimori, &mut pending, &ClientSecrets::default(), 0),
            Some(RowTap::Toast(MsgKey::TrackerNotConfigured("Shikimori".into())))
        );
        let secrets = ClientSecrets {
            shikimori: Some("s".into()),
            ..ClientSecrets::default()
        };
        assert!(matches!(
            tap_row(shikimori, &mut pending, &secrets, 0),
            Some(RowTap::OpenBrowser(_))
        ));
    }

    #[test]
    fn tr_27a_044_rows_keep_the_source_order_and_actions() {
        let names: Vec<_> = LOGIN_ROWS.iter().map(|r| r.name).collect();
        assert_eq!(
            names,
            [
                "MyAnimeList",
                "AniList",
                "Kitsu",
                "MangaUpdates",
                "Shikimori",
                "Simkl",
                "Bangumi"
            ]
        );
        let credentials: Vec<_> = LOGIN_ROWS
            .iter()
            .filter(|r| r.action == LoginAction::Credentials)
            .map(|r| r.name)
            .collect();
        assert_eq!(credentials, ["Kitsu", "MangaUpdates"]);
    }

    #[test]
    fn tr_27a_044_every_row_is_a_registered_tracker_and_appears_once() {
        for row in LOGIN_ROWS {
            assert!(REGISTRY_ORDER.contains(&row.tracker), "{}", row.name);
        }
        let mut ids: Vec<_> = LOGIN_ROWS.iter().map(|r| r.tracker).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), LOGIN_ROWS.len());
    }

    #[test]
    fn t_28a_048_row_subtitle_shows_the_login_progress_and_the_expiry()
    -> Result<(), Box<dyn std::error::Error>> {
        use crate::fakes::MemSecrets;
        use crate::tracker::anilist::AniList;
        use crate::tracker::anilist::auth::AuthState;
        use crate::tracker::anilist::testkit::FakeTransport;
        use kuro_prefs::store::PrefStore;

        let api = AniList::from_parts(
            FakeTransport::new(vec![]),
            MemSecrets::new(),
            &PrefStore::in_memory(),
            AuthState::LoggedOut,
        );
        assert_eq!(row_subtitle(&api, true), Some(MsgKey::AniListLoggingIn));
        assert_eq!(row_subtitle(&api, false), None);
        api.base().set_auth_expired(true)?;
        assert_eq!(
            row_subtitle(&api, false),
            Some(MsgKey::AniListSessionExpired)
        );
        Ok(())
    }

    fn mal_view(logged_in: bool, expired: bool, configured: bool) -> LoginRowView {
        LoginRowView {
            row: LOGIN_ROWS[0],
            logged_in,
            expired,
            configured,
        }
    }

    #[test]
    fn t_29a_047_the_mal_row_states_follow_the_login_and_expiry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let secrets = crate::fakes::MemSecrets::new();
        let tokens: std::sync::Arc<dyn crate::mal::store::TokenStore> =
            std::sync::Arc::new(crate::mal::store::SecretTokenStore::new(secrets));
        let mal = crate::mal::tracker::MalTracker::new(dir.path(), tokens, Some("cid".into()))
            .expect("tracker builds");
        assert_eq!(
            row_subtitle(&mal, false),
            None,
            "logged out, ready: no subtitle"
        );
        assert_eq!(
            row_subtitle(&mal, true),
            Some(MsgKey::MalWaitingForLogin),
            "the loopback login waits"
        );

        let mut pending = PendingLogins::default();
        let logged_out = mal_view(false, false, true);
        assert_eq!(
            tap_row(&logged_out, &mut pending, &ClientSecrets::default(), 0),
            Some(RowTap::StartMalLogin),
            "a logged-out MAL row starts the loopback login"
        );
        let expired = mal_view(true, true, true);
        assert_eq!(
            tap_row(&expired, &mut pending, &ClientSecrets::default(), 0),
            Some(RowTap::StartMalLogin),
            "an expired login is started again"
        );
        let logged_in = mal_view(true, false, true);
        assert_eq!(
            tap_row(&logged_in, &mut pending, &ClientSecrets::default(), 0),
            Some(RowTap::ConfirmLogout)
        );
        assert_eq!(
            tap_row(&mal_view(false, false, false), &mut pending, &ClientSecrets::default(), 0),
            None,
            "no client id: the flow does not start"
        );
    }

    #[test]
    fn t_29a_047_an_expired_mal_login_shows_log_in_again_and_stays_checked() {
        let dir = tempfile::tempdir().expect("tempdir");
        let secrets = crate::fakes::MemSecrets::new();
        let tokens: std::sync::Arc<dyn crate::mal::store::TokenStore> =
            std::sync::Arc::new(crate::mal::store::SecretTokenStore::new(secrets));
        let mal = crate::mal::tracker::MalTracker::new(dir.path(), tokens, Some("cid".into()))
            .expect("tracker builds");
        mal.settings_for_test().set_username("ann").unwrap();
        mal.auth()
            .set_token(crate::mal::testkit::fresh_token("a"))
            .unwrap();
        mal.auth().set_expired(true).unwrap();
        assert!(
            mal.is_logged_in(),
            "the row reads username and token, so it stays checked"
        );
        assert_eq!(row_subtitle(&mal, false), Some(MsgKey::MalLogInAgain));
    }

    #[test]
    fn t_29a_029_a_mal_without_client_id_shows_the_setup_hint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let secrets = crate::fakes::MemSecrets::new();
        let tokens: std::sync::Arc<dyn crate::mal::store::TokenStore> =
            std::sync::Arc::new(crate::mal::store::SecretTokenStore::new(secrets));
        let mal =
            crate::mal::tracker::MalTracker::new(dir.path(), tokens, None).expect("tracker builds");
        assert_eq!(
            row_subtitle(&mal, true),
            Some(MsgKey::MalNoClientId),
            "the hint wins over the waiting text"
        );
        assert_eq!(
            crate::messages::english(&MsgKey::MalNoClientId),
            "Set a MyAnimeList client id to log in"
        );
    }
}
