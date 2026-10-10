//! Pasted links in global search (T-20a-031, T-15a-043). A pasted URL is first offered to the sources that can resolve
//! links: the first one whose `uri_type` recognises it looks the manga up, and the entry opens. Only when none does, or
//! the lookup finds nothing, the text is searched in the catalogues like any query.

use std::sync::Arc;

use kuro_source::model::SManga;
use kuro_source::{DeepLinkOutcome, ResolvableSource, resolve_deep_link};

use super::listing::{LinkFuture, ListItem};

/// Whether `text` is a link a source could own: an `http` or `https` URL with a host.
pub fn is_link(text: &str) -> bool {
    reqwest::Url::parse(text.trim()).is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

/// The lookup of `uri` over `sources`, in list order (`resolve_deep_link`). `to_item` turns the resolved manga into its
/// library row, given the id of the source that resolved it; `None` from it, or no source recognising the link, is no
/// result.
pub fn resolve(
    sources: Vec<Arc<dyn ResolvableSource>>,
    uri: &str,
    to_item: impl FnOnce(i64, SManga) -> Option<ListItem> + Send + 'static,
) -> LinkFuture {
    let uri = uri.trim().to_owned();
    Box::pin(async move {
        let refs: Vec<&dyn ResolvableSource> = sources.iter().map(|s| s.as_ref()).collect();
        match resolve_deep_link(&refs, &uri).await {
            DeepLinkOutcome::Result { source_id, manga, .. } => to_item(source_id, manga),
            DeepLinkOutcome::NoResults => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use kuro_source::model::{Page, SChapter};
    use kuro_source::{MangaSource, MangaUriType, SourceError};

    use super::*;

    /// A source that logs the order of its calls; it recognises links that contain `/m/`.
    struct Site {
        log: Arc<Mutex<Vec<&'static str>>>,
        lookups: AtomicUsize,
    }

    #[async_trait]
    impl MangaSource for Site {
        fn id(&self) -> i64 {
            7
        }
        fn name(&self) -> &str {
            "Site"
        }
        async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
            Ok(manga.clone())
        }
        async fn get_chapter_list(&self, _manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
            Ok(Vec::new())
        }
        async fn get_page_list(&self, _chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl ResolvableSource for Site {
        fn uri_type(&self, uri: &str) -> MangaUriType {
            self.log.lock().unwrap().push("uri_type");
            if uri.contains("/m/") { MangaUriType::Manga } else { MangaUriType::Unknown }
        }
        async fn manga_from_uri(&self, _uri: &str) -> Result<Option<SManga>, SourceError> {
            self.log.lock().unwrap().push("manga_from_uri");
            self.lookups.fetch_add(1, Ordering::SeqCst);
            Ok(Some(SManga::new("/m/1", "One")))
        }
        async fn chapter_from_uri(&self, _uri: &str) -> Result<Option<SChapter>, SourceError> {
            Ok(None)
        }
    }

    fn site() -> (Arc<Site>, Arc<Mutex<Vec<&'static str>>>) {
        let log: Arc<Mutex<Vec<&'static str>>> = Arc::default();
        (Arc::new(Site { log: Arc::clone(&log), lookups: AtomicUsize::new(0) }), log)
    }

    #[test]
    fn only_http_and_https_urls_with_a_host_are_links() {
        assert!(is_link("https://site.test/m/1"));
        assert!(is_link("  http://site.test  "));
        for text in ["one piece", "site.test/m/1", "ftp://site.test/m/1", "file:///m/1", "https://", ""] {
            assert!(!is_link(text), "{text:?}");
        }
    }

    #[tokio::test]
    async fn the_uri_type_is_asked_before_the_lookup_and_the_entry_row_comes_back() {
        let (source, log) = site();
        let item = resolve(vec![source], "https://site.test/m/1", |source_id, manga| {
            Some(ListItem { id: 41, source_id, url: manga.url, title: manga.title, ..ListItem::default() })
        })
        .await
        .unwrap();
        assert_eq!((item.id, item.source_id, item.url.as_str(), item.title.as_str()), (41, 7, "/m/1", "One"));
        let log = log.lock().unwrap();
        assert_eq!(log.first(), Some(&"uri_type"), "the link is classified before anything is looked up");
        assert!(log.contains(&"manga_from_uri"));
    }

    #[tokio::test]
    async fn a_link_no_source_recognises_resolves_to_nothing_and_looks_nothing_up() {
        let (source, _) = site();
        assert!(resolve(vec![Arc::clone(&source) as Arc<dyn ResolvableSource>], "https://other.test/x", |_, _| unreachable!()).await.is_none());
        assert_eq!(source.lookups.load(Ordering::SeqCst), 0);
    }
}
