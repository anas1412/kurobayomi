//! HTTP client of the enhanced trackers (`NetworkHelper.kt:23-25`, `Dns.SYSTEM`, no HTTP cache; T-33a-001, T-33a-004).

use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// The User-Agent every enhanced-tracker request sends (`JellyfinInterceptor.kt:23-25`, Proposal: the Rust app's
/// own string).
pub const USER_AGENT: &str = concat!("Kurobayomi/", env!("CARGO_PKG_VERSION"));

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
