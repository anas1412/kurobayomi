//! `UserAgentInterceptor` (T-12a-006). A request whose last `User-Agent` is missing or empty gets the default one.
//! A blank default inserts no header and logs one warning per process.

use std::sync::OnceLock;

use async_trait::async_trait;
use http::header::{HeaderValue, USER_AGENT};

use super::{Chain, Interceptor};
use crate::cookies::CookieJar;
use crate::error::NetError;
use crate::request::Request;
use crate::response::Response;

type Provider = Box<dyn Fn() -> String + Send + Sync>;

pub struct UserAgentInterceptor {
    provider: Provider,
    /// The jar that holds the user agent a user entered for a domain with a pasted clearance (T-12a-033).
    domains: Option<CookieJar>,
}

impl UserAgentInterceptor {
    /// `provider` is called for each request that needs a user agent.
    pub fn new(provider: impl Fn() -> String + Send + Sync + 'static) -> Self {
        Self {
            provider: Box::new(provider),
            domains: None,
        }
    }

    /// A request for a domain the user pasted a clearance for gets that domain's user agent in place of the default.
    pub fn with_domain_defaults(mut self, jar: CookieJar) -> Self {
        self.domains = Some(jar);
        self
    }
}

static WARNED: OnceLock<()> = OnceLock::new();

fn warn_once(reason: &str) {
    WARNED.get_or_init(|| tracing::warn!("default user agent not applied: {reason}"));
}

#[async_trait]
impl Interceptor for UserAgentInterceptor {
    async fn intercept(&self, chain: Chain<'_>, mut req: Request) -> Result<Response, NetError> {
        let present = req
            .headers
            .get_all(USER_AGENT)
            .iter()
            .last()
            .is_some_and(|v| !v.is_empty());
        if present {
            return chain.proceed(req).await;
        }
        req.headers.remove(USER_AGENT);
        let domain_default = self.domains.as_ref().and_then(|jar| jar.domain_user_agent(&req.url));
        let ua = domain_default.unwrap_or_else(|| (self.provider)());
        let ua = ua.trim();
        if ua.is_empty() {
            warn_once("the setting is blank");
        } else if !ua.is_ascii() {
            warn_once("the setting has characters that are not ASCII");
        } else {
            match HeaderValue::from_str(ua) {
                Ok(value) => {
                    req.headers.insert(USER_AGENT, value);
                }
                Err(_) => warn_once("the setting has characters that are not allowed in a header"),
            }
        }
        chain.proceed(req).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::client::NetworkClient;
    use crate::request;
    use http::HeaderMap;

    /// Every `User-Agent` value the local server received for one request.
    async fn seen_user_agents(provider: &'static str, headers: HeaderMap) -> Vec<String> {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = NetworkClient::for_tests(vec![Arc::new(UserAgentInterceptor::new(move || {
            provider.to_owned()
        }))]);
        let req = request::get(&format!("{}/", server.uri())).unwrap().headers(headers);
        client.execute(req).await.unwrap();
        let received = server.received_requests().await.unwrap();
        received[0]
            .headers
            .get_all("user-agent")
            .iter()
            .map(|v| v.to_str().unwrap().to_owned())
            .collect()
    }

    async fn seen_user_agent(provider: &'static str, headers: HeaderMap) -> Option<String> {
        seen_user_agents(provider, headers).await.last().cloned()
    }

    #[tokio::test]
    async fn a_request_without_a_user_agent_gets_the_trimmed_default() {
        let ua = seen_user_agent("  kuro/1.0 ", HeaderMap::new()).await;
        assert_eq!(ua.as_deref(), Some("kuro/1.0"));
    }

    #[tokio::test]
    async fn a_non_empty_last_user_agent_is_left_alone() {
        let mut headers = HeaderMap::new();
        headers.append(USER_AGENT, HeaderValue::from_static("first"));
        headers.append(USER_AGENT, HeaderValue::from_static("last"));
        let uas = seen_user_agents("kuro/1.0", headers).await;
        assert_eq!(uas, vec!["first".to_owned(), "last".to_owned()]);
    }

    #[tokio::test]
    async fn an_empty_last_user_agent_is_replaced() {
        let mut headers = HeaderMap::new();
        headers.append(USER_AGENT, HeaderValue::from_static("old"));
        headers.append(USER_AGENT, HeaderValue::from_static(""));
        let ua = seen_user_agent("kuro/1.0", headers).await;
        assert_eq!(ua.as_deref(), Some("kuro/1.0"));
    }

    #[tokio::test]
    async fn a_blank_default_inserts_no_header() {
        let ua = seen_user_agent("   ", HeaderMap::new()).await;
        assert_eq!(ua, None);
    }

    #[tokio::test]
    async fn a_default_with_a_non_ascii_character_is_skipped_not_fatal() {
        let ua = seen_user_agent("Mozillaé", HeaderMap::new()).await;
        assert_eq!(ua, None);
    }
}
