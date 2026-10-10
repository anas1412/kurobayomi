//! Logic of the credential login dialog and the logout dialog (`TrackingLoginDialog`, `TrackingLogoutDialog` in
//! `SettingsTrackingScreen.kt`, behaviour 12.5 to 12.7). The widgets live in the GUI crate; this is what they
//! decide.

use crate::error::TrackError;
use crate::messages::MsgKey;
use crate::model::TrackerId;
use crate::tracker::Tracker;

/// Title of the login dialog.
pub fn login_title(tracker_name: &str) -> String {
    format!("Log in to {tracker_name}")
}

/// Label of the first field. Kitsu asks for an email address, the others for a username.
pub fn username_label(tracker: TrackerId) -> &'static str {
    if tracker == TrackerId::KITSU {
        "Email address"
    } else {
        "Username"
    }
}

/// Fields and state of the login dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
    /// A request is running.
    pub processing: bool,
    /// The last attempt failed. Both fields are marked, and the typed values are kept.
    pub input_error: bool,
}

impl LoginForm {
    /// The form starts with the tracker's stored values.
    pub fn for_tracker(tracker: &dyn Tracker) -> Self {
        Self {
            username: tracker.username(),
            password: tracker.password(),
            processing: false,
            input_error: false,
        }
    }

    /// OK is enabled only when both fields are non-blank and no request runs (TT-27-216).
    pub fn can_submit(&self) -> bool {
        !self.processing && !self.username.trim().is_empty() && !self.password.trim().is_empty()
    }

    /// "Loading…" while the request runs, else "Login".
    pub fn button_label(&self) -> &'static str {
        if self.processing {
            "Loading…"
        } else {
            "Login"
        }
    }

    /// Runs the login and records the result on the form. `Ok` means the dialog closes. `Err` carries the
    /// message to show.
    pub async fn submit(&mut self, tracker: &dyn Tracker) -> Result<MsgKey, MsgKey> {
        self.processing = true;
        let result = run_login(tracker, &self.username, &self.password).await;
        self.processing = false;
        self.input_error = result.is_err();
        result
    }
}

/// Logs the tracker in. A failure leaves any session stored before the attempt untouched (T-30a-008). Aniyomi
/// logs out on failure, which wipes a valid session; that defect is not ported.
pub async fn run_login(
    tracker: &dyn Tracker,
    username: &str,
    password: &str,
) -> Result<MsgKey, MsgKey> {
    match tracker.login(username, password).await {
        Ok(()) => Ok(MsgKey::LoginSuccess),
        Err(e) => Err(failure_message(&e)),
    }
}

/// The text of a failure. An empty message shows "Login failed", never the text "null" (defect D14).
pub fn failure_message(e: &TrackError) -> MsgKey {
    if let TrackError::Key(key) = e {
        return key.clone();
    }
    let text = e.to_string();
    if text.trim().is_empty() {
        MsgKey::LoginFailed
    } else {
        MsgKey::Raw(text)
    }
}

/// The toast of a successful login (T-28a-048): "Logged in to AniList" for AniList, "<name> logged in" otherwise.
pub fn login_success_message(tracker: TrackerId, name: &str) -> MsgKey {
    if tracker == TrackerId::ANILIST {
        MsgKey::AniListLoggedIn
    } else {
        MsgKey::TrackerLoggedIn(name.to_owned())
    }
}

/// Title of the logout dialog.
pub fn logout_title(tracker_name: &str) -> String {
    format!("Log out from {tracker_name}?")
}

/// Which button of the logout dialog has the focus when it opens. Cancel, so Enter does not log out (TT-27-217).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButton {
    Cancel,
    LogOut,
}

pub const LOGOUT_DEFAULT_FOCUS: DialogButton = DialogButton::Cancel;

/// Clears the credentials and reports the result. The login state of the tracker goes to `false`, which removes
/// the check mark.
pub fn run_logout(tracker: &dyn Tracker) -> Result<MsgKey, MsgKey> {
    match tracker.logout() {
        Ok(()) => Ok(MsgKey::LogoutSuccess),
        Err(e) => Err(failure_message(&e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeAnime, MemSecrets, new_log};

    fn kitsu() -> FakeAnime {
        FakeAnime::new(TrackerId::KITSU, "Kitsu", MemSecrets::new(), new_log())
    }

    #[test]
    fn tr_27_045_labels_and_title_follow_the_tracker() {
        assert_eq!(username_label(TrackerId::KITSU), "Email address");
        assert_eq!(username_label(TrackerId::MANGAUPDATES), "Username");
        assert_eq!(login_title("Kitsu"), "Log in to Kitsu");
    }

    #[test]
    fn tr_27_045_ok_is_disabled_until_both_fields_have_text() {
        let mut form = LoginForm {
            username: "someone".into(),
            password: "   ".into(),
            processing: false,
            input_error: false,
        };
        assert!(!form.can_submit(), "blank password keeps OK disabled");
        form.password = "pw".into();
        assert!(form.can_submit());
        form.processing = true;
        assert!(!form.can_submit());
        assert_eq!(form.button_label(), "Loading…");
    }

    #[tokio::test]
    async fn tr_27_045_success_logs_in_and_reports_logged_in() {
        let t = kitsu();
        let mut form = LoginForm {
            username: "u".into(),
            password: "p".into(),
            processing: false,
            input_error: false,
        };
        let r = form.submit(&t).await;
        assert_eq!(r, Ok(MsgKey::LoginSuccess));
        assert!(!form.input_error && !form.processing);
    }

    #[tokio::test]
    async fn tr_27_045_failure_marks_the_form_keeps_text_and_the_prior_session() {
        let t = kitsu();
        t.save_credentials("u", "old").unwrap();
        *t.fail.lock().unwrap() = Some("Wrong password".into());
        let mut form = LoginForm {
            username: "u".into(),
            password: "bad".into(),
            processing: false,
            input_error: false,
        };
        let r = form.submit(&t).await;
        assert_eq!(r, Err(MsgKey::Raw("Wrong password".into())));
        assert!(form.input_error);
        assert_eq!(
            (form.username.as_str(), form.password.as_str()),
            ("u", "bad")
        );
        assert!(t.is_logged_in(), "the failed login keeps the prior session");
        assert_eq!((t.username().as_str(), t.password().as_str()), ("u", "old"));
    }

    #[tokio::test]
    async fn tr_27_d14_failure_without_message_shows_login_failed_not_null() {
        let t = kitsu();
        *t.fail.lock().unwrap() = Some(String::new());
        let r = run_login(&t, "u", "p").await;
        assert_eq!(r, Err(MsgKey::LoginFailed));
    }

    #[test]
    fn tr_27_046_a_failure_message_is_shown_verbatim_or_as_login_failed() {
        assert_eq!(
            failure_message(&TrackError::Message("no keyring".into())),
            MsgKey::Raw("no keyring".into())
        );
        assert_eq!(
            failure_message(&TrackError::Message(String::new())),
            MsgKey::LoginFailed
        );
    }

    #[test]
    fn tr_27_046_logout_title_default_focus_and_result() {
        let t = kitsu();
        t.save_credentials("u", "p").unwrap();
        assert_eq!(logout_title("Kitsu"), "Log out from Kitsu?");
        assert_eq!(LOGOUT_DEFAULT_FOCUS, DialogButton::Cancel);
        assert_eq!(run_logout(&t), Ok(MsgKey::LogoutSuccess));
        assert!(!t.is_logged_in());
    }

    #[test]
    fn t_28a_048_login_success_toast_names_anilist_and_others_keep_the_generic_text() {
        assert_eq!(
            login_success_message(TrackerId::ANILIST, "AniList"),
            MsgKey::AniListLoggedIn
        );
        assert_eq!(
            login_success_message(TrackerId::KITSU, "Kitsu"),
            MsgKey::TrackerLoggedIn("Kitsu".into())
        );
    }

    #[test]
    fn t_28a_045_a_keyed_failure_shows_its_own_message() {
        let e = TrackError::Key(MsgKey::AniListLoginFailed);
        assert_eq!(failure_message(&e), MsgKey::AniListLoginFailed);
    }
}
