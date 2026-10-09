//! The deep link into the store page (T-19a-050). `aniyomi://add-repo?url=...` and `aniyomi://extension-store?url=...`
//! both carry the index URL in the `url` query value. Any other link is refused with one banner.

use url::Url;

/// The banner for a link that is not a store address (K-9).
pub const NOT_A_STORE_ADDRESS: &str = "This link is not an extension store address";

/// The link is not an extension store address. The banner text is the whole message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{NOT_A_STORE_ADDRESS}")]
pub struct NotAStoreAddress;

/// Reads the store index URL from a deep link. The scheme must be `aniyomi`, the host `add-repo` or `extension-store`,
/// and the query must carry a non-blank `url`. The query value comes back decoded, and the caller opens the Confirm
/// dialog with it.
pub fn store_url_from_link(link: &str) -> Result<String, NotAStoreAddress> {
    let parsed = Url::parse(link.trim()).map_err(|_| NotAStoreAddress)?;
    if parsed.scheme() != "aniyomi" || !matches!(parsed.host_str(), Some("add-repo" | "extension-store")) {
        return Err(NotAStoreAddress);
    }
    parsed
        .query_pairs()
        .find(|(key, _)| key == "url")
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or(NotAStoreAddress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_store_hosts_return_the_decoded_url() {
        let url = "https://example.org/repo.json";
        assert_eq!(store_url_from_link("aniyomi://add-repo?url=https%3A%2F%2Fexample.org%2Frepo.json").unwrap(), url);
        assert_eq!(store_url_from_link("aniyomi://extension-store?url=https%3A%2F%2Fexample.org%2Frepo.json").unwrap(), url);
    }

    #[test]
    fn another_scheme_or_host_is_refused_with_the_banner() {
        for link in [
            "https://add-repo?url=https%3A%2F%2Fexample.org%2Frepo.json",
            "aniyomi://other?url=https%3A%2F%2Fexample.org%2Frepo.json",
            "kuro://add-repo?url=https%3A%2F%2Fexample.org%2Frepo.json",
        ] {
            assert_eq!(store_url_from_link(link), Err(NotAStoreAddress), "{link}");
        }
        assert_eq!(NotAStoreAddress.to_string(), "This link is not an extension store address");
    }

    #[test]
    fn a_missing_or_blank_url_and_a_garbage_link_are_refused() {
        assert_eq!(store_url_from_link("aniyomi://add-repo"), Err(NotAStoreAddress));
        assert_eq!(store_url_from_link("aniyomi://add-repo?url=%20%20"), Err(NotAStoreAddress));
        assert_eq!(store_url_from_link("not a link at all"), Err(NotAStoreAddress));
    }
}
