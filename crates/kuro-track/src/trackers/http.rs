//! HTTP client of the enhanced trackers (`NetworkHelper.kt:23-25`, `Dns.SYSTEM`, no HTTP cache; T-33a-001, T-33a-004).

use std::sync::RwLock;
use std::time::Duration;

use kuro_net::UserAgentSource;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// The User-Agent every enhanced-tracker request sends (`JellyfinInterceptor.kt:23-25`, Proposal: the Rust app's
/// own string).
pub const USER_AGENT: &str = concat!("Kurobayomi/", env!("CARGO_PKG_VERSION"));

/// Where the user's default User-Agent preference is read from. The app sets it once at start-up (T-33a-091).
static USER_AGENT_SOURCE: RwLock<Option<UserAgentSource>> = RwLock::new(None);

/// Sets the reader of the user's default User-Agent preference (area 12, `NetworkClient::default_user_agent`). The
/// reader is called on every request, so a change to the preference applies to the next request.
pub fn set_user_agent_source(source: UserAgentSource) {
    *USER_AGENT_SOURCE.write().unwrap_or_else(|p| p.into_inner()) = Some(source);
}

/// The User-Agent for the next request: the user's default preference, trimmed. [`USER_AGENT`] is used only when no
/// source is set or the preference is blank.
pub fn current_user_agent() -> String {
    let source = USER_AGENT_SOURCE.read().unwrap_or_else(|p| p.into_inner()).clone();
    match source.map(|read| read()) {
        Some(agent) if !agent.trim().is_empty() => agent.trim().to_owned(),
        _ => USER_AGENT.to_owned(),
    }
}

/// Serialises the tests that read or set the process-wide source.
#[cfg(test)]
pub(crate) static USER_AGENT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Removes the source again, for tests that set one.
#[cfg(test)]
pub(crate) fn clear_user_agent_source() {
    *USER_AGENT_SOURCE.write().unwrap_or_else(|p| p.into_inner()) = None;
}

/// The tracker client. It resolves with the system resolver (`no_hickory_dns`, because the workspace reqwest enables
/// the hickory feature, which would otherwise be the default). reqwest keeps no HTTP cache, so every GET reaches the
/// server (P-5).
pub fn tracker_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .timeout(CALL_TIMEOUT)
        .no_hickory_dns()
        .build()
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[test]
    fn t_33a_004_the_client_builds_with_the_system_resolver_and_timeouts() {
        assert!(tracker_client().is_ok());
    }

    #[test]
    fn t_33a_001_the_user_agent_names_the_app() {
        assert!(USER_AGENT.starts_with("Kurobayomi/"));
    }

    #[test]
    fn t_33a_091_the_user_preference_is_read_on_each_call_and_the_constant_is_the_fallback() {
        let _guard = USER_AGENT_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let pref = std::sync::Arc::new(std::sync::Mutex::new(String::from("  Mine/1.0  ")));
        let reader = std::sync::Arc::clone(&pref);
        set_user_agent_source(std::sync::Arc::new(move || reader.lock().unwrap().clone()));
        assert_eq!(current_user_agent(), "Mine/1.0");
        *pref.lock().unwrap() = "Other/2.0".into();
        assert_eq!(current_user_agent(), "Other/2.0");
        *pref.lock().unwrap() = "   ".into();
        assert_eq!(current_user_agent(), USER_AGENT);
        clear_user_agent_source();
        assert_eq!(current_user_agent(), USER_AGENT);
    }

    #[tokio::test]
    async fn t_33a_004_two_identical_gets_both_reach_the_server() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ping"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .expect(2)
            .mount(&server)
            .await;
        let client = tracker_client().unwrap();
        let url = format!("{}/ping", server.uri());
        for _ in 0..2 {
            let resp = client.get(&url).send().await.unwrap();
            assert_eq!(resp.status(), 200);
        }
        // `expect(2)` is verified when `server` is dropped.
    }
}
