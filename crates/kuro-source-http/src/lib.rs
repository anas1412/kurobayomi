//! HTTP layer of the source API (area 15, `p1-rust-design.md` section 8). `kuro-net` does the transport, the
//! interceptors and the cookies. This crate adds the source rules on top: request builders, the success check and
//! error mapping, URL helpers, the jsoup-compatible HTML helpers, and the `HttpSource` and `ParsedHttpSource` traits
//! with the generic `Http<S>` adapter that gives a source the `MangaSource` and `CatalogueSource` traits.

pub mod anime_http;
pub mod anime_parsed;
pub mod error;
pub mod html;
pub mod http_source;
pub mod image;
pub mod local_server;
pub mod parsed;
pub mod request;
pub mod response;
pub mod urls;

pub use anime_http::AnimeHttpBase;
pub use error::map_net_error;
pub use http_source::{Http, HttpCore, HttpSource};
pub use local_server::{LocalHandler, LocalResponse, LocalStreamServer};
pub use parsed::ParsedHttpSource;
pub use response::HttpResponse;

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Arc;

    use kuro_net::NetworkClient;

    /// A client with an in-memory preference store and a temporary cookie file. The directory is returned so that it
    /// lives as long as the test needs it. No request is made with the client.
    pub(crate) fn test_client() -> (NetworkClient, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
        let (toaster, _feed) = kuro_platform::toast::toast_channel();
        let client = NetworkClient::new(
            &prefs,
            dir.path().join("cookies.json"),
            Arc::new(kuro_platform::webview::NoWebView),
            toaster,
        )
        .expect("client");
        (client, dir)
    }
}
