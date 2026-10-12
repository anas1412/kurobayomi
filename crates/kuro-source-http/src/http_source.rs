//! `HttpCore`, the state every HTTP source carries (`HttpSource.kt`; T-15a-020, T-15a-021).

use std::sync::OnceLock;

use http::HeaderMap;
use http::HeaderValue;
use http::header::USER_AGENT;
use kuro_net::NetworkClient;
use kuro_source::generate_id;

/// The state every HTTP source carries: the client, the id (computed once) and the default headers (computed once).
pub struct HttpCore {
    client: NetworkClient,
    id: OnceLock<i64>,
    headers: OnceLock<HeaderMap>,
}

impl HttpCore {
    pub fn new(client: NetworkClient) -> Self {
        Self { client, id: OnceLock::new(), headers: OnceLock::new() }
    }

    pub fn client(&self) -> &NetworkClient {
        &self.client
    }

    /// Behaviour 1.1: the id from name, language and version, computed on first use and kept.
    pub fn id(&self, name: &str, lang: &str, version_id: i32) -> i64 {
        *self.id.get_or_init(|| generate_id(name, lang, version_id))
    }

    /// Behaviour 1.5: the default headers, built on first use and kept for the life of the source.
    pub fn headers(&self, build: impl FnOnce() -> HeaderMap) -> &HeaderMap {
        self.headers.get_or_init(build)
    }
}

/// The default headers of a source (behaviour 1.5): `User-Agent` from the network preference, trimmed. A blank
/// preference adds no header, so the network layer applies its own default.
pub fn default_headers(core: &HttpCore) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let agent = core.client().default_user_agent();
    let agent = agent.trim();
    if !agent.is_empty()
        && let Ok(value) = HeaderValue::from_str(agent)
    {
        headers.insert(USER_AGENT, value);
    }
    headers
}
