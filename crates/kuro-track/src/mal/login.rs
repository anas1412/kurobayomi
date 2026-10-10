//! Login dialog model (T-29a-039, T-29a-052). It starts the loopback listener and falls back to the paste field
//! when the port cannot be bound (E-LOGIN-6). The GTK dialog draws it: the authorise URL with Copy and Open, a
//! paste field, and Cancel. The redirect wait and the paste submit may run side by side; whichever delivers a
//! code first completes the login, and dropping the other future stops its listener.

use tokio::net::TcpListener;

use super::error::MalError;
use super::messages as text;
use super::redirect::{CodeOutcome, LOGIN_TIMEOUT, bind_loopback, parse_pasted, wait_for_code};
use super::tracker::MalTracker;

/// Label of the paste field.
pub const PASTE_LABEL: &str = "Paste the redirect URL or code";

pub struct LoginDialog {
    authorize_url: String,
    port: u16,
    /// `None` when the port was not free, or once the wait for the redirect has started.
    listener: Option<TcpListener>,
    redirect_listening: bool,
    error: Option<String>,
}

impl LoginDialog {
    /// Starts a login: a new verifier, the authorise URL, and the listener on `port`. A bind failure is logged at
    /// WARN and leaves the paste field as the only way in. A missing client id fails before anything starts.
    pub async fn open(tracker: &MalTracker, port: u16) -> Result<Self, MalError> {
        let authorize_url = tracker.begin_login()?.to_string();
        let (listener, port) = match bind_loopback(port).await {
            Ok(listener) => {
                let bound = listener.local_addr().map_or(port, |a| a.port());
                (Some(listener), bound)
            }
            Err(e) => {
                tracing::warn!(port, error = %e, "login port not available, paste only");
                (None, port)
            }
        };
        Ok(Self {
            authorize_url,
            port,
            redirect_listening: listener.is_some(),
            listener,
            error: None,
        })
    }

    /// The URL shown in the dialog. The Copy button copies it and the Open button opens it.
    pub fn authorize_url(&self) -> &str {
        &self.authorize_url
    }

    /// The loopback port in use, or the one that could not be bound.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// False when only the paste field can finish the login.
    pub fn redirect_listening(&self) -> bool {
        self.redirect_listening
    }

    /// The text under the paste field, if the last input was refused or the redirect did not arrive.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The pasted text. A valid redirect or code completes the login. Invalid input sets the error text and keeps
    /// the login pending (E-LOGIN-9).
    pub async fn submit_paste(
        &mut self,
        tracker: &MalTracker,
        input: &str,
    ) -> Option<Result<(), MalError>> {
        match parse_pasted(input) {
            Ok(outcome) => Some(self.finish(tracker, outcome).await),
            Err(e) => {
                self.error = Some(e.to_string());
                None
            }
        }
    }

    /// Waits for the browser redirect and finishes the login with it. `None` when no listener runs (paste only,
    /// or the wait already took it). After the timeout the verifier stays, so the paste field still works
    /// (E-LOGIN-5); the error is returned and shown.
    pub async fn wait_redirect(&mut self, tracker: &MalTracker) -> Option<Result<(), MalError>> {
        let listener = self.listener.take()?;
        match wait_for_code(listener, LOGIN_TIMEOUT).await {
            CodeOutcome::TimedOut => {
                self.redirect_listening = false;
                self.error = Some(text::LOGIN_TIMED_OUT.to_owned());
                Some(Err(MalError::LoginTimedOut))
            }
            outcome => Some(self.finish(tracker, outcome).await),
        }
    }

    async fn finish(&mut self, tracker: &MalTracker, outcome: CodeOutcome) -> Result<(), MalError> {
        self.listener = None;
        self.redirect_listening = false;
        let result = tracker.finish_login(outcome).await;
        self.error = result.as_ref().err().map(ToString::to_string);
        result
    }

    /// Cancel: drops the listener and the pending verifier.
    pub fn cancel(self, tracker: &MalTracker) {
        tracker.cancel_login();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::mal::testkit::{MemorySecrets, tracker_for};

    async fn mount_exchange(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/v1/oauth2/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "token_type":"Bearer","expires_in":2592000,
                "access_token":"a","refresh_token":"r"
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v2/users/@me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name":"fixture_user"})))
            .mount(server)
            .await;
    }

    async fn setup() -> (MockServer, tempfile::TempDir, MalTracker) {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let tracker = tracker_for(&server, dir.path(), Arc::new(MemorySecrets::default()));
        mount_exchange(&server).await;
        (server, dir, tracker)
    }

    #[tokio::test]
    async fn a_bound_port_is_listening_and_the_redirect_completes_the_login() {
        let (_server, _dir, tracker) = setup().await;
        let mut dialog = LoginDialog::open(&tracker, 0).await.unwrap();
        assert!(dialog.redirect_listening());
        assert!(dialog.authorize_url().contains("client_id=test-client"));

        let port = dialog.port();
        let browser = async move {
            let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            client
                .write_all(
                    b"GET /myanimelist-auth?code=CODE123456789012 HTTP/1.1\r\nHost: x\r\n\r\n",
                )
                .await
                .unwrap();
            let mut page = Vec::new();
            client.read_to_end(&mut page).await.unwrap();
        };
        let (result, ()) = tokio::join!(dialog.wait_redirect(&tracker), browser);
        result.expect("a listener ran").expect("login completes");
        assert!(tracker.is_logged_in());
        assert!(!tracker.login_in_progress());
    }

    #[tokio::test]
    async fn a_port_that_cannot_be_bound_leaves_the_paste_path() {
        let (_server, _dir, tracker) = setup().await;
        let taken = bind_loopback(0).await.unwrap();
        let port = taken.local_addr().unwrap().port();

        let mut dialog = LoginDialog::open(&tracker, port)
            .await
            .expect("a bind failure does not fail the login");
        assert!(!dialog.redirect_listening());
        assert_eq!(dialog.port(), port);
        assert!(dialog.wait_redirect(&tracker).await.is_none());
        assert!(tracker.login_in_progress(), "the login waits for the paste");

        let result = dialog
            .submit_paste(
                &tracker,
                "http://localhost:8765/myanimelist-auth?code=CODE123456789012",
            )
            .await;
        result.expect("paste is parsed").expect("login completes");
        assert!(tracker.is_logged_in());
        assert_eq!(dialog.error(), None);
    }

    #[tokio::test]
    async fn invalid_paste_shows_the_hint_and_keeps_the_login_pending() {
        let (_server, _dir, tracker) = setup().await;
        let mut dialog = LoginDialog::open(&tracker, 0).await.unwrap();
        assert!(dialog.submit_paste(&tracker, "hello").await.is_none());
        assert_eq!(
            dialog.error(),
            Some("Paste the full redirect URL or the code")
        );
        assert!(tracker.login_in_progress());
        assert!(!tracker.is_logged_in());
    }

    #[tokio::test]
    async fn a_denied_paste_is_not_completed_and_cancel_drops_the_verifier() {
        let (_server, _dir, tracker) = setup().await;
        let mut dialog = LoginDialog::open(&tracker, 0).await.unwrap();
        let err = dialog
            .submit_paste(
                &tracker,
                "http://localhost/myanimelist-auth?error=access_denied",
            )
            .await
            .expect("a denial is an outcome")
            .unwrap_err();
        assert!(matches!(err, MalError::LoginNotCompleted));
        assert_eq!(dialog.error(), Some("Login was not completed"));

        let dialog = LoginDialog::open(&tracker, 0).await.unwrap();
        assert!(tracker.login_in_progress());
        dialog.cancel(&tracker);
        assert!(!tracker.login_in_progress());
    }

    #[tokio::test]
    async fn without_a_client_id_nothing_starts() {
        let dir = tempfile::tempdir().unwrap();
        let tokens: Arc<dyn crate::mal::store::TokenStore> = Arc::new(
            crate::mal::store::SecretTokenStore::new(Arc::new(MemorySecrets::default())),
        );
        let tracker = MalTracker::new(dir.path(), tokens, None).unwrap();
        let err = LoginDialog::open(&tracker, 0).await.err().expect("refused");
        assert!(matches!(err, MalError::NoClientId));
        assert!(!tracker.login_in_progress());
    }
}
