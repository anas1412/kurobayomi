//! The aggregate Available list (T-19a-021): every store is fetched with at most four requests in flight. A failed
//! store is reported next to the others. A cancelled refresh returns `Cancelled` and gives the caller nothing to apply.

use futures::stream::{self, StreamExt};
use tokio_util::sync::CancellationToken;

use super::{FetchedStore, StoreError, fetch_store};
use crate::anime::model::{Available, StoreRow};

pub const MAX_CONCURRENT_STORES: usize = 4;

/// A store that could not be read in this refresh. The UI shows it; the other stores still count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreFailure {
    pub index_url: String,
    pub message: String,
}

/// The result of one refresh. `available` keeps store order, then index order. Duplicate package names are all kept,
/// and reconciliation picks the first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Aggregate {
    pub stores: Vec<StoreRow>,
    pub available: Vec<Available>,
    pub failures: Vec<StoreFailure>,
}

/// Fetches every store in `index_urls`. Returns `Err(Cancelled)` when the token fired, so no partial list is returned.
pub async fn fetch_all(
    client: &reqwest::Client,
    index_urls: &[String],
    cancel: &CancellationToken,
) -> Result<Aggregate, StoreError> {
    let results: Vec<(String, Result<FetchedStore, StoreError>)> = stream::iter(index_urls)
        .map(|url| async move { (url.clone(), fetch_store(client, url, cancel).await) })
        .buffered(MAX_CONCURRENT_STORES)
        .collect()
        .await;
    if cancel.is_cancelled() {
        return Err(StoreError::Cancelled);
    }
    let mut aggregate = Aggregate::default();
    for (index_url, result) in results {
        match result {
            Ok(fetched) => {
                aggregate.available.extend(fetched.extensions);
                aggregate.stores.push(fetched.row);
            }
            Err(err) => {
                tracing::error!(index_url = %index_url, "Failed to fetch extensions for store");
                aggregate.failures.push(StoreFailure { index_url, message: err.to_string() });
            }
        }
    }
    Ok(aggregate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::testkit::test_client;
    use crate::anime::testkit::{store_bytes, store_document, store_extension};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn serve_store(server: &MockServer, route: &str, extensions: Vec<serde_json::Value>) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(store_bytes(&store_document("S", "k", extensions))))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn one_failing_store_is_reported_and_the_good_one_is_listed() {
        let server = MockServer::start().await;
        serve_store(&server, "/good.json", vec![store_extension("p.good", 1, None)]).await;
        Mock::given(method("GET")).and(path("/bad.json")).respond_with(ResponseTemplate::new(500)).mount(&server).await;
        let urls = vec![format!("{}/good.json", server.uri()), format!("{}/bad.json", server.uri())];
        let aggregate = fetch_all(&test_client(), &urls, &CancellationToken::new()).await.unwrap();
        assert_eq!(aggregate.available.len(), 1);
        assert_eq!(aggregate.available[0].pkg, "p.good");
        assert_eq!(aggregate.stores.len(), 1);
        assert_eq!(aggregate.failures.len(), 1);
        assert_eq!(aggregate.failures[0].index_url, urls[1]);
        assert_eq!(aggregate.failures[0].message, "the store answered HTTP 500");
    }

    #[tokio::test]
    async fn a_duplicate_package_across_stores_is_kept_in_store_order() {
        let server = MockServer::start().await;
        serve_store(&server, "/a.json", vec![store_extension("p.dup", 2, None)]).await;
        serve_store(&server, "/b.json", vec![store_extension("p.dup", 3, None)]).await;
        let urls = vec![format!("{}/a.json", server.uri()), format!("{}/b.json", server.uri())];
        let aggregate = fetch_all(&test_client(), &urls, &CancellationToken::new()).await.unwrap();
        let codes: Vec<i64> = aggregate.available.iter().map(|ext| ext.version_code).collect();
        assert_eq!(codes, [2, 3]);
    }

    #[tokio::test]
    async fn a_cancelled_refresh_returns_cancelled_and_no_list() {
        let server = MockServer::start().await;
        serve_store(&server, "/a.json", vec![store_extension("p.a", 1, None)]).await;
        let cancel = CancellationToken::new();
        cancel.cancel();
        let urls = vec![format!("{}/a.json", server.uri())];
        let result = fetch_all(&test_client(), &urls, &cancel).await;
        assert_eq!(result, Err(StoreError::Cancelled));
    }

    #[tokio::test]
    async fn no_stores_is_an_empty_aggregate_and_not_an_error() {
        let aggregate = fetch_all(&test_client(), &[], &CancellationToken::new()).await.unwrap();
        assert_eq!(aggregate, Aggregate::default());
    }
}
