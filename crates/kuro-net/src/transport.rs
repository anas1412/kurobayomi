//! The terminal stage: reqwest with the Aniyomi transport limits (T-12a-004) and the dispatcher emulation.
//! A request waits for its host slot and then for a global slot, both before its call timeout starts, so a
//! queued request never times out while it waits (`p1-rust-design.md` section 2.6 step 3).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use reqwest::redirect::Policy;
use tokio::sync::Semaphore;

use crate::doh::DohResolver;
use crate::error::NetError;
use crate::logging;
use crate::progress;
use crate::request::{Body, Method, Request};
use crate::response::{BodyStream, Response};

/// The Aniyomi `NetworkHelper` values. Every field is checked by a unit test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportLimits {
    pub connect_timeout: Duration,
    /// reqwest's read timeout is an inactivity timeout, the same as OkHttp `readTimeout`.
    pub read_timeout: Duration,
    /// The whole call, from dispatch to the end of the body.
    pub call_timeout: Duration,
    pub max_redirects: usize,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    /// Dispatcher: calls in flight across all hosts.
    pub max_requests: usize,
    /// Dispatcher: calls in flight to one host.
    pub max_requests_per_host: usize,
}

impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(30),
            call_timeout: Duration::from_secs(120),
            max_redirects: 20,
            pool_idle_timeout: Duration::from_secs(300),
            pool_max_idle_per_host: 5,
            max_requests: 64,
            max_requests_per_host: 5,
        }
    }
}

/// The reqwest client builder for `limits` and `cookies`. Cookies are the jar's store.
pub fn client_builder<C: reqwest::cookie::CookieStore + 'static>(
    limits: &TransportLimits,
    cookies: Arc<C>,
) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .connect_timeout(limits.connect_timeout)
        .read_timeout(limits.read_timeout)
        .timeout(limits.call_timeout)
        .redirect(Policy::limited(limits.max_redirects))
        .pool_idle_timeout(limits.pool_idle_timeout)
        .pool_max_idle_per_host(limits.pool_max_idle_per_host)
        .cookie_provider(cookies)
        .use_rustls_tls()
}

/// Sends one request. Holds the dispatcher slots until the body is dropped.
pub struct Transport {
    client: reqwest::Client,
    limits: TransportLimits,
    all: Arc<Semaphore>,
    per_host: Mutex<HashMap<String, Arc<Semaphore>>>,
    /// Logs every request and response at `kuro_net::http` (T-12a-026). Set once, at construction.
    verbose: bool,
}

impl Transport {
    pub fn new<C: reqwest::cookie::CookieStore + 'static>(
        limits: TransportLimits,
        cookies: Arc<C>,
    ) -> Result<Self, NetError> {
        Self::with_doh(limits, cookies, None)
    }

    /// Like [`Transport::new`], with DNS over HTTPS answering every host name when `doh` is set (T-12a-025).
    pub fn with_doh<C: reqwest::cookie::CookieStore + 'static>(
        limits: TransportLimits,
        cookies: Arc<C>,
        doh: Option<DohResolver>,
    ) -> Result<Self, NetError> {
        let mut builder = client_builder(&limits, cookies);
        if let Some(resolver) = doh {
            builder = builder.dns_resolver(resolver);
        }
        let client = builder
            .build()
            .map_err(|e| NetError::Other(format!("cannot build HTTP client: {e}")))?;
        Ok(Self {
            client,
            all: Arc::new(Semaphore::new(limits.max_requests)),
            limits,
            per_host: Mutex::new(HashMap::new()),
            verbose: false,
        })
    }

    /// Turns verbose logging on or off for this transport.
    pub fn with_verbose_logging(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    fn host_semaphore(&self, host: &str) -> Arc<Semaphore> {
        let mut map = self.per_host.lock().unwrap_or_else(|p| p.into_inner());
        map.entry(host.to_owned())
            .or_insert_with(|| Arc::new(Semaphore::new(self.limits.max_requests_per_host)))
            .clone()
    }

    /// Runs one request through reqwest. The per-host slot is taken before the global one.
    pub async fn send(&self, req: Request) -> Result<Response, NetError> {
        if self.verbose {
            logging::log_request(&req);
        }
        let host = req.url.host_str().unwrap_or_default().to_owned();
        let host_permit = self.host_semaphore(&host).acquire_owned().await.map_err(closed)?;
        let all_permit = self.all.clone().acquire_owned().await.map_err(closed)?;

        let mut builder = self.client.request(to_reqwest_method(req.method), req.url.clone());
        builder = builder.headers(req.headers.clone());
        builder = match &req.body {
            Body::Empty => builder,
            Body::Form(pairs) => builder.form(pairs),
            Body::Bytes { content_type, data } => builder
                .header(http::header::CONTENT_TYPE, content_type.as_str())
                .body(data.clone()),
        };
        if let Some(timeout) = req.opts.timeout {
            builder = builder.timeout(timeout);
        }
        let resp = builder.send().await?;

        let status = resp.status();
        let headers = resp.headers().clone();
        let url = resp.url().clone();
        let mut body: BodyStream = resp.bytes_stream().map(|c| c.map_err(NetError::from)).boxed();
        if let Some(listener) = req.opts.progress.clone() {
            let length = progress::content_length_of(&headers);
            body = progress::wrap_body(body, length, listener);
        }
        // The permits live as long as the body stream, so a slow download keeps its slot.
        let held = (host_permit, all_permit);
        let body = body.map(move |item| {
            let _ = &held;
            item
        });
        let response = Response::new(status, headers, url, body.boxed());
        if self.verbose {
            logging::log_response(&response);
        }
        Ok(response)
    }
}

fn closed(_: tokio::sync::AcquireError) -> NetError {
    NetError::Other("dispatcher closed".to_owned())
}

fn to_reqwest_method(method: Method) -> reqwest::Method {
    match method {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Delete => reqwest::Method::DELETE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::UserMessage;

    #[test]
    fn the_limits_are_the_aniyomi_values() {
        let l = TransportLimits::default();
        assert_eq!(l.connect_timeout, Duration::from_secs(30));
        assert_eq!(l.read_timeout, Duration::from_secs(30));
        assert_eq!(l.call_timeout, Duration::from_secs(120));
        assert_eq!(l.max_redirects, 20);
        assert_eq!(l.pool_idle_timeout, Duration::from_secs(300));
        assert_eq!(l.pool_max_idle_per_host, 5);
        assert_eq!(l.max_requests, 64);
        assert_eq!(l.max_requests_per_host, 5);
    }

    #[test]
    fn the_global_slot_count_is_the_configured_maximum() {
        let store = Arc::new(reqwest_cookie_store::CookieStoreMutex::new(
            cookie_store::CookieStore::new(),
        ));
        let limits = TransportLimits::default();
        let transport = Transport::new(limits.clone(), store).unwrap();
        assert_eq!(transport.all.available_permits(), limits.max_requests);
        assert_eq!(transport.all.available_permits(), 64);
    }

    #[test]
    fn the_host_slot_is_shared_per_host() {
        let store = Arc::new(reqwest_cookie_store::CookieStoreMutex::new(
            cookie_store::CookieStore::new(),
        ));
        let transport = Transport::new(TransportLimits::default(), store).unwrap();
        let a = transport.host_semaphore("a.test");
        let again = transport.host_semaphore("a.test");
        let other = transport.host_semaphore("b.test");
        assert!(Arc::ptr_eq(&a, &again));
        assert!(!Arc::ptr_eq(&a, &other));
    }

    /// A name the resolver rejects (a label over 63 bytes, so no lookup is sent) is an unknown host, online or not.
    /// The sandbox resolves real `.invalid` names, so this uses the length rule, which fails the same way.
    #[tokio::test]
    async fn an_unresolvable_host_is_an_unknown_host() {
        let store = Arc::new(reqwest_cookie_store::CookieStoreMutex::new(
            cookie_store::CookieStore::new(),
        ));
        let transport = Transport::new(TransportLimits::default(), store).unwrap();
        let host = format!("{}.invalid", "a".repeat(64));
        let req = crate::request::get(&format!("http://{host}/")).unwrap();
        let err = transport.send(req).await.err().expect("an unresolvable host fails");
        assert!(
            matches!(&err, NetError::UnknownHost { host: named } if *named == host),
            "{err:?}"
        );
        assert_eq!(err.user_message(true), UserMessage::KeyArg("exception_unknown_host", host.clone()));
        assert_eq!(err.user_message(false), UserMessage::Key("exception_offline"));
    }

    /// A request queued behind the per-host limit does not start its call timeout while it waits.
    #[tokio::test]
    async fn a_queued_request_is_not_timed_out_while_it_waits_for_a_host_slot() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_delay(Duration::from_millis(300)),
            )
            .mount(&server)
            .await;
        let limits = TransportLimits {
            max_requests_per_host: 1,
            call_timeout: Duration::from_millis(500),
            ..TransportLimits::default()
        };
        let store = Arc::new(reqwest_cookie_store::CookieStoreMutex::new(
            cookie_store::CookieStore::new(),
        ));
        let transport = Arc::new(Transport::new(limits, store).unwrap());
        let url = format!("{}/", server.uri());
        let mut calls = Vec::new();
        for _ in 0..2 {
            let transport = transport.clone();
            let url = url.clone();
            calls.push(tokio::spawn(async move {
                let req = crate::request::get(&url).unwrap();
                transport.send(req).await.map(|r| r.status)
            }));
        }
        for call in calls {
            assert_eq!(call.await.unwrap().unwrap(), http::StatusCode::OK);
        }
    }
}
