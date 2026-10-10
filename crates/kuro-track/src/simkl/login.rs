//! The Simkl login dialog (T-32a-048, U-3): what the dialog shows and decides. The row tap opens the browser at the
//! constant authorise URL and the dialog waits. The code arrives by a delivered redirect (`aniyomi://simkl-auth`) or by
//! the paste field, which takes a redirect or a bare code. Failures are shown in the dialog text (a deliberate change
//! from the source, which shows nothing). Opening the browser and drawing the widgets are GUI work.

use super::SimklTracker;
use super::oauth::{authorize_url, code_from_paste};
use crate::dialog::failure_message;
use crate::error::TrackError;
use crate::messages::{MsgKey, english};

/// Where the dialog is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginState {
    /// The browser is open and the dialog waits for the redirect or a paste.
    Waiting,
    /// A code is being exchanged. The paste field and OK are disabled.
    Exchanging,
    /// The last attempt failed. The dialog stays open and shows the text; the pasted text is kept.
    Failed(MsgKey),
}

/// Model of the Simkl login dialog.
#[derive(Debug, Clone)]
pub struct SimklLogin {
    state: LoginState,
    /// The text of the paste field.
    pub paste: String,
}

impl SimklLogin {
    /// Starts a login: the URL to open in the default browser, and the dialog in its waiting state. The URL has no
    /// `state` parameter and needs no configured secret (the client constants are the source's, B-3.10).
    pub fn start() -> (Self, String) {
        let dialog = Self {
            state: LoginState::Waiting,
            paste: String::new(),
        };
        (dialog, authorize_url())
    }

    pub fn state(&self) -> &LoginState {
        &self.state
    }

    /// The error line of the dialog, `None` unless the last attempt failed.
    pub fn error_text(&self) -> Option<String> {
        match &self.state {
            LoginState::Failed(key) => Some(english(key)),
            _ => None,
        }
    }

    /// "Waiting for Simkl..." while no code has arrived. Not shown while exchanging.
    pub fn is_waiting(&self) -> bool {
        self.state == LoginState::Waiting
    }

    /// OK is enabled when the paste field has text and no exchange runs.
    pub fn can_submit(&self) -> bool {
        self.state != LoginState::Exchanging && !self.paste.trim().is_empty()
    }

    /// Takes the pasted redirect or code. `true` means the login worked and the dialog closes. Text that is no
    /// redirect of the Simkl host and no plausible code gives "Unrecognized login link" and no request.
    pub async fn submit_paste(&mut self, tracker: &SimklTracker) -> bool {
        let Some(code) = code_from_paste(&self.paste) else {
            self.state = LoginState::Failed(MsgKey::UnrecognizedLoginLink);
            return false;
        };
        self.state = LoginState::Exchanging;
        let result = tracker.login_code(&code).await;
        self.finish(result)
    }

    /// A launch argument delivered while the dialog is open. `true` closes the dialog: the login worked, or the
    /// redirect had no code, which logs out and reports nothing else (TST-57). `false` for another link (the dialog
    /// is untouched) or a failed login (the dialog shows the text).
    pub async fn on_launch_uri(&mut self, tracker: &SimklTracker, uri: &str) -> bool {
        if !super::oauth::is_simkl_redirect(uri) {
            return false;
        }
        self.state = LoginState::Exchanging;
        let result = tracker.handle_redirect(uri).await;
        self.finish(result)
    }

    fn finish(&mut self, result: Result<(), TrackError>) -> bool {
        match result {
            Ok(()) => {
                self.state = LoginState::Waiting;
                true
            }
            Err(e) => {
                self.state = LoginState::Failed(failure_message(&e));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_platform::secrets::SecretStore;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::EventBus;
    use crate::fakes::MemSecrets;
    use crate::tracker::Tracker;

    fn tracker(server: &MockServer) -> SimklTracker {
        let secrets: Arc<dyn SecretStore> = MemSecrets::new();
        SimklTracker::with_base_url(&server.uri(), secrets, EventBus::default()).unwrap()
    }

    async fn healthy(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"access_token":"AT"}"#))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/users/settings"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"account":{"id":7}}"#))
            .mount(server)
            .await;
    }

    #[test]
    fn t_32a_048_start_gives_the_byte_equal_url_and_a_waiting_dialog() {
        let (dialog, url) = SimklLogin::start();
        assert_eq!(
            url,
            "https://simkl.com/oauth/authorize?response_type=code&client_id=aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth"
        );
        assert!(dialog.is_waiting());
        assert_eq!(dialog.error_text(), None);
        assert!(!dialog.can_submit(), "nothing pasted yet");
    }

    #[tokio::test]
    async fn t_32a_048_a_delivered_redirect_logs_in_and_closes_the_dialog() {
        let server = MockServer::start().await;
        healthy(&server).await;
        let t = tracker(&server);
        let (mut dialog, _) = SimklLogin::start();
        assert!(
            !dialog
                .on_launch_uri(&t, "aniyomi://add-repo?url=x")
                .await,
            "another link leaves the dialog alone"
        );
        assert!(dialog.is_waiting() && !t.is_logged_in());
        assert!(
            dialog
                .on_launch_uri(&t, "aniyomi://simkl-auth?code=C1")
                .await
        );
        assert!(t.is_logged_in());
        assert_eq!(t.username(), "7");
    }

    #[tokio::test]
    async fn t_32a_048_a_pasted_redirect_or_bare_code_reaches_login_code() {
        for pasted in [
            "aniyomi://simkl-auth?code=C1",
            "  a1.b2_c3-d4e5f6g7h8i9j0k1  ",
        ] {
            let server = MockServer::start().await;
            healthy(&server).await;
            let t = tracker(&server);
            let (mut dialog, _) = SimklLogin::start();
            dialog.paste = pasted.into();
            assert!(dialog.can_submit());
            assert!(dialog.submit_paste(&t).await, "{pasted}");
            assert!(t.is_logged_in());
            let body = server.received_requests().await.unwrap()[0].body.clone();
            let body = String::from_utf8(body).unwrap();
            let code = if pasted.contains("C1") {
                "C1"
            } else {
                "a1.b2_c3-d4e5f6g7h8i9j0k1"
            };
            assert!(body.contains(&format!(r#""code":"{code}""#)), "{body}");
        }
    }

    #[tokio::test]
    async fn t_32a_048_a_paste_from_another_host_shows_an_error_and_sends_nothing() {
        let server = MockServer::start().await;
        let t = tracker(&server);
        let (mut dialog, _) = SimklLogin::start();
        dialog.paste = "https://example.com/?code=abc".into();
        assert!(!dialog.submit_paste(&t).await);
        assert_eq!(dialog.error_text().as_deref(), Some("Unrecognized login link"));
        assert_eq!(dialog.paste, "https://example.com/?code=abc", "text is kept");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn t_32a_048_a_rejected_code_shows_the_error_in_the_dialog_and_stays_open() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/oauth/token"))
            .respond_with(ResponseTemplate::new(400).set_body_string("bad"))
            .mount(&server)
            .await;
        let t = tracker(&server);
        let (mut dialog, _) = SimklLogin::start();
        assert!(
            !dialog
                .on_launch_uri(&t, "aniyomi://simkl-auth?code=C1")
                .await
        );
        assert_eq!(dialog.error_text().as_deref(), Some("HTTP 400"));
        assert!(!t.is_logged_in());
    }

    #[tokio::test]
    async fn t_32a_031_a_redirect_without_a_code_logs_out_and_closes_with_no_error() {
        let server = MockServer::start().await;
        let t = tracker(&server);
        t.save_credentials("7", "AT").unwrap();
        let (mut dialog, _) = SimklLogin::start();
        assert!(dialog.on_launch_uri(&t, "aniyomi://simkl-auth").await);
        assert_eq!(dialog.error_text(), None);
        assert!(!t.is_logged_in());
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
