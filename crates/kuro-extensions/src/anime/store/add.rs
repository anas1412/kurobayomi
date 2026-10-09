//! Adding a store (T-19a-047). The typed or linked URL is trimmed, and a URL that is already stored is refused before
//! any request is made. The store is then fetched, and its final URL is compared with the stored ones, because a legacy
//! `index.min.json` link resolves to its `repo.json`, and a redirect can end on a stored store (TST-19-118).

use tokio_util::sync::CancellationToken;

use super::{FetchedStore, StoreError, fetch_store};
use crate::anime::logs;
use crate::anime::model::StoreRow;

/// Fetches the store at `typed` and returns it for saving. `AlreadyExists` is returned when the trimmed URL, or the URL
/// the fetch ends on, is already stored. Any other failure is logged with the fixed text and returned.
pub async fn add_store(
    client: &reqwest::Client,
    typed: &str,
    stored: &[StoreRow],
    cancel: &CancellationToken,
) -> Result<FetchedStore, StoreError> {
    let url = typed.trim();
    if stored.iter().any(|row| row.index_url == url) {
        return Err(StoreError::AlreadyExists);
    }
    let fetched = match fetch_store(client, url, cancel).await {
        Ok(fetched) => fetched,
        Err(err) => {
            tracing::error!(store = %url, "{}", logs::add_store_failed(url));
            return Err(err);
        }
    };
    if stored
        .iter()
        .any(|row| row.index_url == fetched.row.index_url)
    {
        return Err(StoreError::AlreadyExists);
    }
    Ok(fetched)
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::anime::testkit::{store_bytes, store_document, test_client};

    fn stored(index_url: &str) -> StoreRow {
        StoreRow {
            index_url: index_url.into(),
            name: "S".into(),
            badge_label: "S".into(),
            signing_key: "ab".repeat(32),
            contact_website: String::new(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
        }
    }

    async fn serve(server: &MockServer, route: &str, body: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn tst_19_116_a_stored_url_with_spaces_is_refused_before_any_request() {
        let server = MockServer::start().await;
        // No mock is mounted, so any request would answer 404 and fail with a different error.
        let url = format!("{}/index.json", server.uri());
        let result = add_store(
            &test_client(),
            &format!("  {url} "),
            &[stored(&url)],
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(result.unwrap_err(), StoreError::AlreadyExists);
        assert!(
            server
                .received_requests()
                .await
                .unwrap_or_default()
                .is_empty(),
            "no request was made"
        );
    }

    #[tokio::test]
    async fn a_new_url_is_fetched_with_its_spaces_trimmed() {
        let server = MockServer::start().await;
        serve(
            &server,
            "/index.json",
            store_bytes(&store_document("New", "k", Vec::new())),
        )
        .await;
        let url = format!("{}/index.json", server.uri());
        let fetched = add_store(
            &test_client(),
            &format!(" {url} "),
            &[],
            &CancellationToken::new(),
        )
        .await
        .expect("the store is added");
        assert_eq!(fetched.row.index_url, url);
        assert_eq!(fetched.row.name, "New");
    }

    #[tokio::test]
    async fn tst_19_118_a_legacy_list_link_to_a_stored_repo_is_a_duplicate() {
        let server = MockServer::start().await;
        let repo = format!("{}/repo.json", server.uri());
        serve(&server, "/index.min.json", br#"[]"#.to_vec()).await;
        serve(&server, "/repo.json", br#"{"meta":{"name":"Legacy","shortName":null,"website":"https://x","signingKeyFingerprint":"ab"}}"#.to_vec()).await;
        let link = format!("{}/index.min.json", server.uri());
        let result = add_store(
            &test_client(),
            &link,
            &[stored(&repo)],
            &CancellationToken::new(),
        )
        .await;
        assert_eq!(result.unwrap_err(), StoreError::AlreadyExists);
    }

    #[tokio::test]
    async fn a_failed_add_is_logged_with_the_fixed_text_and_returned() {
        let server = MockServer::start().await;
        let url = format!("{}/missing.json", server.uri());
        let recording = crate::anime::testkit::start_capture();
        let result = add_store(&test_client(), &url, &[], &CancellationToken::new()).await;
        assert!(result.is_err());
        let logged = recording.events();
        assert!(
            logged
                .iter()
                .any(|event| event.message == logs::add_store_failed(&url)),
            "{logged:?}"
        );
    }
}
