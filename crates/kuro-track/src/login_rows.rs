//! Settings > Tracking login rows (behaviour 12.2 and 12.3, T-27a-044). The order and the action of each row. The GUI
//! draws the check mark from the stored login state, and runs the action.

use crate::manager::TrackerManager;
use crate::model::TrackerId;
use crate::oauth::PendingLogins;

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

/// A login row with its check mark: shown when the tracker is logged in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginRowView {
    pub row: LoginRow,
    pub logged_in: bool,
}

/// The rows in order, each with the login state read from `trackers`. A tracker missing from the registry shows no
/// check mark.
pub fn login_row_views(trackers: &TrackerManager) -> Vec<LoginRowView> {
    LOGIN_ROWS
        .iter()
        .map(|&row| LoginRowView {
            row,
            logged_in: trackers.get(row.tracker).is_some_and(|t| t.is_logged_in()),
        })
        .collect()
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
}

/// The effect of tapping `view`. A browser row begins an attempt in `pending`. `None` when the tracker has no
/// authorise URL, so nothing opens.
pub fn tap_row(view: &LoginRowView, pending: &mut PendingLogins, now_ms: i64) -> Option<RowTap> {
    if view.logged_in {
        return Some(RowTap::ConfirmLogout);
    }
    match view.row.action {
        LoginAction::Credentials => Some(RowTap::OpenCredentials),
        LoginAction::Browser => pending
            .begin(view.row.tracker, now_ms)
            .map(RowTap::OpenBrowser),
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
            tap_row(&views[2], &mut pending, 0),
            Some(RowTap::ConfirmLogout)
        );
        assert_eq!(
            tap_row(&views[3], &mut pending, 0),
            Some(RowTap::OpenCredentials)
        );
        let Some(RowTap::OpenBrowser(url)) = tap_row(&views[1], &mut pending, 0) else {
            panic!("AniList opens the browser");
        };
        assert!(url.starts_with("https://"), "{url}");
        assert_eq!(INFO_TEXT_ID, "tracking_info");
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
}
