//! The worker runtime of the anime extensions (T-19a-059, `p1-rust-design.md` section 10).
//!
//! The GTK main thread owns the state. Background work runs on a tokio runtime with two worker threads and reports
//! back through an `async_channel` of [`Event`](crate::anime::manager::Event)s. Background tasks own clones of what
//! they need and never touch the state. Store refresh fetches at most
//! `MAX_CONCURRENT_STORES` (four) stores at once.
//! The main thread never waits on a future.

use std::io;

use tokio::runtime::{Builder, Runtime};

/// Worker threads of the extension runtime: enough for two concurrent downloads plus a store fetch. Aniyomi uses an
/// unbounded pool; this bound is a Kurobayomi decision.
pub const WORKER_THREADS: usize = 2;

/// Builds the runtime the caller keeps alive for the life of the process.
pub fn build_runtime() -> io::Result<Runtime> {
    Builder::new_multi_thread()
        .worker_threads(WORKER_THREADS)
        .thread_name("kuro-extension-worker")
        .enable_all()
        .build()
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::anime::manager::Event;
    use crate::anime::store::refresh::{Aggregate, fetch_all};
    use crate::anime::testkit::{store_bytes, store_document, test_client};

    fn assert_send<T: Send + 'static>() {}

    #[test]
    fn the_runtime_has_exactly_two_workers() {
        let runtime = build_runtime().expect("the runtime builds");
        assert_eq!(runtime.handle().metrics().num_workers(), WORKER_THREADS);
    }

    #[test]
    fn what_crosses_threads_is_send_and_static() {
        assert_send::<Event>();
        assert_send::<Aggregate>();
        assert_send::<async_channel::Sender<Event>>();
        assert_send::<async_channel::Receiver<Event>>();
    }

    #[tokio::test]
    async fn a_refresh_of_eight_slow_stores_takes_two_rounds_of_four() {
        let server = MockServer::start().await;
        let mut urls = Vec::new();
        for index in 0..8 {
            let route = format!("/store{index}/index.json");
            let body = store_bytes(&store_document("S", "k", Vec::new()));
            Mock::given(method("GET"))
                .and(path(route.as_str()))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_bytes(body)
                        .set_delay(Duration::from_millis(200)),
                )
                .mount(&server)
                .await;
            urls.push(format!("{}{route}", server.uri()));
        }
        let started = Instant::now();
        let aggregate = fetch_all(
            &test_client(),
            &urls,
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("the refresh completes");
        assert_eq!(aggregate.stores.len(), 8);
        // Eight 200 ms requests with at most four in flight need two rounds, so at least 400 ms. Unlimited
        // concurrency would finish in about 200 ms.
        assert!(
            started.elapsed() >= Duration::from_millis(380),
            "took {:?}",
            started.elapsed()
        );
    }
}
