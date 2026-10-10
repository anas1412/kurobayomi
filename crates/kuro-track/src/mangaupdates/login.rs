//! The MangaUpdates login dialog remembers the last typed login name (T-32a-068, U-2). The stored uid is a number, not
//! what the user typed, and the stored password is a session token that is never shown, so the form takes its name from
//! the preference `track_login_name_7` and starts with an empty password.

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;

use crate::dialog::LoginForm;
use crate::messages::MsgKey;
use crate::tracker::Tracker;

/// Preference key (default empty). It holds a login name only, never a secret.
pub const LOGIN_NAME_KEY: &str = "track_login_name_7";

/// The remembered login name.
pub struct LoginName(Pref<String>);

impl LoginName {
    pub fn new(store: PrefStore) -> Self {
        Self(Pref::new(store, LOGIN_NAME_KEY, String::new()))
    }

    /// The form the dialog opens with: the remembered name and an empty password.
    pub fn form(&self) -> LoginForm {
        LoginForm {
            username: self.0.get(),
            password: String::new(),
            processing: false,
            input_error: false,
        }
    }

    /// Runs the login. The typed name is remembered on every attempt, before the request, so a failed attempt keeps it
    /// for the next try.
    pub async fn submit(
        &self,
        form: &mut LoginForm,
        tracker: &dyn Tracker,
    ) -> Result<MsgKey, MsgKey> {
        if let Err(e) = self.0.set(form.username.clone()) {
            tracing::warn!("could not remember the MangaUpdates login name: {e}");
        }
        form.submit(tracker).await
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
    use crate::mangaupdates::MangaUpdatesTracker;

    fn tracker(server: &MockServer) -> MangaUpdatesTracker {
        let secrets: Arc<dyn SecretStore> = MemSecrets::new();
        MangaUpdatesTracker::with_base_url(&server.uri(), secrets, EventBus::default()).unwrap()
    }

    #[tokio::test]
    async fn t_32a_068_a_logged_in_form_has_the_saved_name_and_an_empty_password() {
        let server = MockServer::start().await;
        let t = tracker(&server);
        t.save_credentials("42", "SESSION-TOKEN").unwrap();
        let store = PrefStore::in_memory();
        let names = LoginName::new(store.clone());
        assert_eq!(names.form().username, "", "default is empty");
        Pref::new(store, LOGIN_NAME_KEY, String::new())
            .set("ann".into())
            .unwrap();
        let form = names.form();
        assert_eq!(form.username, "ann", "not the stored uid 42");
        assert_eq!(form.password, "", "never the session token");
        assert_eq!(t.password(), "SESSION-TOKEN", "the token is still stored");
        assert!(!form.can_submit(), "OK waits for a typed password");
    }

    #[tokio::test]
    async fn t_32a_068_the_name_is_written_on_each_attempt_even_when_it_fails() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/v1/account/login"))
            .respond_with(ResponseTemplate::new(401).set_body_string("no"))
            .mount(&server)
            .await;
        let t = tracker(&server);
        let names = LoginName::new(PrefStore::in_memory());
        let mut form = names.form();
        form.username = "ann".into();
        form.password = "bad".into();
        assert!(names.submit(&mut form, &t).await.is_err());
        assert_eq!(names.form().username, "ann");
        assert!(!t.is_logged_in());
    }

    #[test]
    fn t_32a_068_the_key_is_the_one_the_plan_names() {
        assert_eq!(LOGIN_NAME_KEY, "track_login_name_7");
    }
}
