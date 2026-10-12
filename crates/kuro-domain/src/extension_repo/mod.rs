//! The repository links the app reads sources from (`docs/design/app-flow.md`), and the import of the old repo
//! addresses of a backup into them.
//!
//! An old address (a backup's `extension_repos` or `anime_extension_repos` list, or a row of the old repo or store
//! table) becomes a link of the one repository list when it is an https address ending in `index.min.json`, or when
//! `<address>/index.min.json` answers. Anything else is ignored without error. The one network call is a GET, behind
//! [`HttpGet`]; the HTTP client lives in the app.

use std::collections::BTreeSet;

use kuro_prefs::pref::Pref;

/// The one network call of the import: a GET whose body is returned as text.
pub trait HttpGet: Sync {
    fn get_text(&self, url: &str) -> Result<String, String>;
}

/// The file name every repository link ends with.
pub const INDEX_FILE: &str = "index.min.json";

/// The repository link an old address stands for, or `None` when it is not one.
fn link_for(address: &str, repo_http: &dyn HttpGet) -> Option<String> {
    let address = address.trim();
    if address.starts_with("https://") && address.ends_with(INDEX_FILE) {
        return Some(address.to_owned());
    }
    let candidate = format!("{}/{INDEX_FILE}", address.trim_end_matches('/'));
    (candidate.starts_with("https://") && repo_http.get_text(&candidate).is_ok()).then_some(candidate)
}

/// Adds the links of the old `addresses` to the one repository list, once each. Returns the links that were added, in
/// order. A save that fails adds nothing and is logged.
pub fn import_legacy_repos(links: &Pref<BTreeSet<String>>, addresses: &BTreeSet<String>, repo_http: &dyn HttpGet) -> Vec<String> {
    let mut known = links.get();
    let mut added = Vec::new();
    for address in addresses {
        match link_for(address, repo_http) {
            Some(link) => {
                if known.insert(link.clone()) {
                    added.push(link);
                }
            }
            None => tracing::debug!(address = address.as_str(), "an old repo address is not a repository link; ignored"),
        }
    }
    if added.is_empty() {
        return added;
    }
    if let Err(e) = links.set(known) {
        tracing::error!(error = %e, "could not save the imported repo links");
        return Vec::new();
    }
    added
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use kuro_prefs::source::SourcePrefs;
    use kuro_prefs::store::PrefStore;

    use super::*;

    /// Serves bodies by URL; any other URL is a 404.
    #[derive(Default)]
    struct FakeHttp(HashMap<String, String>);

    impl HttpGet for FakeHttp {
        fn get_text(&self, url: &str) -> Result<String, String> {
            self.0.get(url).cloned().ok_or_else(|| format!("404 {url}"))
        }
    }

    fn set(urls: &[&str]) -> BTreeSet<String> {
        urls.iter().map(|u| (*u).to_owned()).collect()
    }

    #[test]
    fn an_address_ending_in_the_index_file_becomes_a_link_without_a_fetch() {
        let links = SourcePrefs::new(PrefStore::in_memory()).manga_extension_repos;
        let added = import_legacy_repos(&links, &set(&["https://r.test/x/index.min.json"]), &FakeHttp::default());
        assert_eq!(added, ["https://r.test/x/index.min.json"]);
        assert!(links.get().contains("https://r.test/x/index.min.json"));
    }

    #[test]
    fn a_base_address_becomes_a_link_when_its_index_answers() {
        let links = SourcePrefs::new(PrefStore::in_memory()).manga_extension_repos;
        let mut http = FakeHttp::default();
        http.0.insert("https://r.test/base/index.min.json".into(), "[]".into());
        let added = import_legacy_repos(&links, &set(&["https://r.test/base/"]), &http);
        assert_eq!(added, ["https://r.test/base/index.min.json"]);
    }

    #[test]
    fn other_addresses_are_ignored_without_error() {
        let links = SourcePrefs::new(PrefStore::in_memory()).manga_extension_repos;
        let added = import_legacy_repos(&links, &set(&["http://r.test/index.min.json", "https://gone.test", "user/repo"]), &FakeHttp::default());
        assert!(added.is_empty());
        assert!(links.get().is_empty());
    }

    #[test]
    fn a_link_already_in_the_list_is_not_added_twice() {
        let links = SourcePrefs::new(PrefStore::in_memory()).manga_extension_repos;
        links.set(set(&["https://r.test/index.min.json"])).unwrap();
        let added = import_legacy_repos(&links, &set(&["https://r.test/index.min.json"]), &FakeHttp::default());
        assert!(added.is_empty());
        assert_eq!(links.get().len(), 1);
    }
}
