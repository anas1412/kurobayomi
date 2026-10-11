//! Removes the HTTP cookies of an extension's urls from the app's cookie jar (T-41a-021). The details page asks once per
//! distinct base url (`ext_details::clear_cookies`) and sums the counts. A url that cannot be cleared counts as 0 and is
//! logged (T25), so one bad url never hides the others. The jar is the one the running clients and the settings page
//! use (`net_client::app_jar`), so the removal takes effect at once and no older copy writes the cookies back.

use kuro_net::cookies::CookieJar;

/// The cookies matching `url` are removed from `jar`, and the jar is written. Returns how many were removed: 0 for a
/// url that does not parse, a jar that cannot be saved, or no match.
pub fn remove_at(jar: &CookieJar, url: &str) -> usize {
    let url = match reqwest::Url::parse(url) {
        Ok(url) => url,
        Err(error) => {
            tracing::warn!(%error, url, "the cookies of a url could not be cleared");
            return 0;
        }
    };
    let removed = jar.remove(&url, None);
    match jar.flush() {
        Ok(()) => removed,
        Err(error) => {
            tracing::warn!(%error, %url, "the cookie jar could not be saved after clearing");
            0
        }
    }
}

/// The remover the details page uses: the app's jar, or 0 for every url when it cannot be found.
pub fn remover() -> impl Fn(&str) -> usize {
    move |url| crate::net_client::app_jar().map_or(0, |jar| remove_at(&jar, url))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_cookie(jar: &CookieJar, url: &str) {
        let url = reqwest::Url::parse(url).expect("url");
        reqwest::cookie::CookieStore::set_cookies(
            &*jar.provider(),
            &mut std::iter::once(&reqwest::header::HeaderValue::from_str("k=v; Max-Age=3600").expect("header")),
            &url,
        );
    }

    #[test]
    fn the_cookies_of_one_url_are_removed_and_counted_and_the_file_is_saved() {
        let dir = tempfile::tempdir().expect("dir");
        let file = dir.path().join("cookies.json");
        let jar = CookieJar::load(file.clone());
        store_cookie(&jar, "https://a.test/");
        store_cookie(&jar, "https://b.test/");
        jar.flush().expect("flush");
        assert_eq!(remove_at(&jar, "https://a.test/"), 1, "T25: the url's own cookie goes");
        assert_eq!(remove_at(&jar, "https://a.test/"), 0, "nothing is left to remove");
        let reloaded = CookieJar::load(file);
        assert!(reloaded.get(&reqwest::Url::parse("https://a.test/").expect("url")).is_empty(), "the removal was saved");
        assert_eq!(reloaded.get(&reqwest::Url::parse("https://b.test/").expect("url")).len(), 1, "other urls keep theirs");
    }

    #[test]
    fn a_url_that_cannot_be_parsed_counts_zero() {
        let dir = tempfile::tempdir().expect("dir");
        let jar = CookieJar::load(dir.path().join("cookies.json"));
        assert_eq!(remove_at(&jar, "not a url"), 0, "per-url failure is 0 and logged");
    }
}
