//! The pages of an online chapter, from the manga's source (any source the directory answers for). The page list comes
//! from `MangaSource::get_page_list`; the images go over the shared client (`recipe_sources::client`). A source that
//! declares request headers (ABI 1.5) sends its own; each image then goes through the source's `unscramble` fix-up with
//! the response headers. The reader's HTTP loader keeps both in the chapter cache, the folder the cache settings clear.
//!
//! The loader calls a source from its worker thread, outside the runtime, so every call is spawned on the app's runtime
//! and waited for. The GTK thread never asks the source: [`Prefetch`] fetches page lists on threads, and the reader
//! reads them from the cache.

use kuro_i18n::{MsgKey, tr};
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use kuro_core::sources::SourceGate;
use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;
use kuro_source::MangaSource;
use kuro_source::model::SChapter;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio::runtime::Handle;
use tokio::sync::watch;

use super::cache::{CACHE_LIMIT_BYTES, DiskCache};
use super::loader::PageEntry;
use super::loader::http::HttpPageSource;

/// What the reader's loader reports for a chapter whose page list is still being fetched. It is a wait, not a failure:
/// the reader shows its spinner and asks again (see [`is_pending`]).
pub fn still_loading() -> String {
    tr(MsgKey::reader_still_loading)
}

/// Whether a load error is the wait for a page list that is still being fetched.
pub fn is_pending(message: &str) -> bool {
    message == still_loading()
}

pub struct SourcePages {
    source: Arc<dyn MangaSource>,
    client: NetworkClient,
    headers: HeaderMap,
    runtime: Handle,
}

impl SourcePages {
    pub fn new(source: Arc<dyn MangaSource>, client: NetworkClient, headers: HeaderMap, runtime: Handle) -> Self {
        Self { source, client, headers, runtime }
    }

    /// Runs `work` on the runtime and waits for it. Never call it on a runtime worker.
    fn run<T: Send + 'static>(&self, work: impl Future<Output = T> + Send + 'static) -> Result<T, String> {
        let (done, answer) = std::sync::mpsc::channel();
        self.runtime.spawn(async move {
            let _ = done.send(work.await);
        });
        answer.recv().map_err(|_| "the source call did not finish".to_owned())
    }
}

impl HttpPageSource for SourcePages {
    fn page_list(&self, chapter_url: &str) -> Result<Vec<PageEntry>, String> {
        let (source, chapter) = (Arc::clone(&self.source), SChapter::new(chapter_url, ""));
        // the answer is the sentence the reader's error card shows; a source that stays silent ends after 30 seconds
        let pages = self.run(async move { crate::browse::errors::plain(source.name(), source.get_page_list(&chapter)).await })??;
        Ok(pages.into_iter().map(|page| PageEntry { url: page.url, image_url: page.image_url }).collect())
    }

    fn image_url(&self, page_url: &str) -> Result<String, String> {
        // `MangaSource` has no image-url call; the sources this build has give every image url in the list
        Err(format!("the source gave no image for page {page_url}"))
    }

    fn image(&self, image_url: &str) -> Result<Vec<u8>, String> {
        let (source, client, headers, url) = (Arc::clone(&self.source), self.client.clone(), self.headers.clone(), image_url.to_owned());
        self.run(async move {
            let headers = page_image_headers(source.as_ref(), headers).await;
            let fetched = fetch(&client, &url, &headers).await?;
            shown(source.as_ref(), fetched).await
        })?
    }
}

/// The headers of one page image request: the source's own set when it declares one (ABI 1.5), else the default.
pub(crate) async fn page_image_headers(source: &dyn MangaSource, default: HeaderMap) -> HeaderMap {
    match crate::browse::errors::limited(source.request_headers()).await {
        Ok(Some(pairs)) => header_map(&pairs),
        Ok(None) => default,
        Err(error) => {
            tracing::warn!(%error, "the source gave no request headers, so the default set is sent");
            default
        }
    }
}

/// The source's pairs as a header map. A pair that is not a valid header is left out.
pub(crate) fn header_map(pairs: &[(String, String)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
            map.append(name, value);
        }
    }
    map
}

/// The GET of one image with the source's headers.
fn request(url: &str, headers: &HeaderMap) -> Result<kuro_net::request::Request, String> {
    Ok(kuro_net::get(url).map_err(|e| e.to_string())?.headers(headers.clone()))
}

/// One fetched page image: its body and the response headers, which the source's fix-up reads.
struct Fetched {
    body: Vec<u8>,
    headers: Vec<(String, String)>,
}

/// The body and headers of one image. A status other than 2xx or 304 is an error with that status.
async fn fetch(client: &NetworkClient, url: &str, headers: &HeaderMap) -> Result<Fetched, String> {
    let response = client.execute(request(url, headers)?).await.map_err(|e| e.to_string())?;
    kuro_images::http::check_status(response.status).map_err(|e| e.to_string())?;
    let headers = response
        .headers
        .iter()
        .filter_map(|(name, value)| value.to_str().ok().map(|v| (name.as_str().to_owned(), v.to_owned())))
        .collect();
    let body = response.bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok(Fetched { body, headers })
}

/// The bytes to show for a fetched image: the source's fix-up of them, or the bytes as fetched. A source without the
/// fix-up, or with one that answers no change, shows them as fetched; a fix-up error fails the page.
async fn shown(source: &dyn MangaSource, fetched: Fetched) -> Result<Vec<u8>, String> {
    let outcome = source.unscramble(&fetched.body, &fetched.headers).await;
    match outcome {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => Ok(fetched.body),
        Err(error) => Err(error.to_string()),
    }
}

/// A page-list fetch that has not reached the cache.
enum Fetch {
    /// Running; its sender opens the gates handed out for it.
    Running(watch::Sender<bool>),
    /// Over, with this error. The next ask fetches again.
    Failed(String),
}

/// The page lists of one manga's chapters, fetched on threads into the chapter cache. The reader builds a chapter's
/// loader only once its list is cached, so the GTK thread never waits on the source, which sends one request at a time
/// and is shared with the entry pages and library updates.
pub struct Prefetch {
    pages: Arc<dyn HttpPageSource>,
    cache: Arc<DiskCache>,
    manga_id: i64,
    fetches: Mutex<HashMap<String, Fetch>>,
}

impl Prefetch {
    pub fn new(pages: Arc<dyn HttpPageSource>, cache: Arc<DiskCache>, manga_id: i64) -> Arc<Self> {
        Arc::new(Self { pages, cache, manga_id, fetches: Mutex::default() })
    }

    pub fn pages(&self) -> Arc<dyn HttpPageSource> {
        Arc::clone(&self.pages)
    }

    pub fn cache(&self) -> Arc<DiskCache> {
        Arc::clone(&self.cache)
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Fetch>> {
        self.fetches.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    pub fn is_cached(&self, chapter_url: &str) -> bool {
        self.cache.get_page_list(self.manga_id, chapter_url).is_some()
    }

    /// Fetches the chapter's page list on a thread, unless it is cached or a fetch of it runs. The gate opens once the
    /// list is cached or the fetch has failed.
    pub fn fetch(self: &Arc<Self>, chapter_url: &str) -> SourceGate {
        if self.is_cached(chapter_url) {
            return SourceGate::new(watch::channel(true).1);
        }
        let mut fetches = self.lock();
        if let Some(Fetch::Running(open)) = fetches.get(chapter_url) {
            return SourceGate::new(open.subscribe());
        }
        let (open, ready) = watch::channel(false);
        fetches.insert(chapter_url.to_owned(), Fetch::Running(open));
        let (me, url) = (Arc::clone(self), chapter_url.to_owned());
        std::thread::spawn(move || {
            let outcome = me.pages.page_list(&url).and_then(|list| {
                me.cache.put_page_list(me.manga_id, &url, &list).map_err(|e| {
                    tracing::warn!(error = %e, "the page list could not be cached");
                    crate::browse::errors::generic().to_owned()
                })
            });
            let mut fetches = me.lock();
            let done = match outcome {
                Ok(()) => fetches.remove(&url),
                Err(error) => {
                    tracing::warn!(%error, "the page list did not load ahead");
                    fetches.insert(url, Fetch::Failed(error))
                }
            };
            if let Some(Fetch::Running(open)) = done {
                open.send_replace(true);
            }
        });
        SourceGate::new(ready)
    }

    /// The reader's error for a chapter whose list is not cached: the last fetch's error, or [`still_loading()`]. A fetch
    /// is started again when none runs, so the next try can read the list.
    pub fn missing(self: &Arc<Self>, chapter_url: &str) -> String {
        let failed = match self.lock().get(chapter_url) {
            Some(Fetch::Failed(error)) => Some(error.clone()),
            _ => None,
        };
        self.fetch(chapter_url);
        failed.unwrap_or_else(|| still_loading().to_owned())
    }
}

/// The page lists and pages of the manga `manga_id` of source `source_id`, when the directory knows that source and
/// the app's runtime runs; `None` for a local or unknown source.
pub fn online(source_id: i64, manga_id: i64) -> Option<Arc<Prefetch>> {
    let source = crate::browse::directory::current(crate::browse::route::BrowseKind::Manga).catalogue(source_id)?.manga()?;
    let runtime = Handle::try_current().ok()?;
    let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
    let cache = DiskCache::open(paths.chapter_cache_dir(), CACHE_LIMIT_BYTES).map_err(|e| tracing::warn!(error = %e, "no chapter cache")).ok()?;
    let pages = SourcePages::new(source, crate::recipe_sources::client()?, crate::recipe_sources::image_headers(source_id), runtime);
    Some(Prefetch::new(Arc::new(pages), Arc::new(cache), manga_id))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use kuro_source::SourceError;
    use kuro_prefs::network::NetworkPrefs;
    use kuro_prefs::store::PrefStore;
    use kuro_source::model::{Page, SManga};
    use reqwest::header::HeaderValue;
    use std::time::Duration;

    use super::*;
    use crate::reader::loader::http::HttpLoader;

    /// A source whose chapters have three pages, each with its image url; it logs the chapter urls it was asked for.
    struct ThreePages(Mutex<Vec<String>>);

    #[async_trait]
    impl MangaSource for ThreePages {
        fn id(&self) -> i64 {
            kuro_source::generate_id("Fixture", "en", 1)
        }
        fn name(&self) -> &str {
            "Fixture"
        }
        async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
            Ok(manga.clone())
        }
        async fn get_chapter_list(&self, _manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
            Ok(Vec::new())
        }
        async fn get_page_list(&self, chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
            self.0.lock().unwrap().push(chapter.url.clone());
            Ok((0..3).map(|i| Page { image_url: Some(format!("https://img.example{}/{i}.png", chapter.url)), ..Page::new(i, "") }).collect())
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap()
    }

    fn pages_of(source: Arc<ThreePages>, runtime: &tokio::runtime::Runtime) -> (kuro_testkit::TempPaths, SourcePages) {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        let client = crate::net_client::build(&NetworkPrefs::new(PrefStore::in_memory()), &paths).unwrap();
        (paths, SourcePages::new(source, client, HeaderMap::new(), runtime.handle().clone()))
    }

    #[test]
    fn the_page_list_comes_from_the_source_with_its_image_urls() {
        let rt = runtime();
        let source = Arc::new(ThreePages(Mutex::default()));
        let (_paths, pages) = pages_of(Arc::clone(&source), &rt);
        let list = pages.page_list("/chapters/01J").unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[2].image_url.as_deref(), Some("https://img.example/chapters/01J/2.png"));
        assert_eq!(*source.0.lock().unwrap(), ["/chapters/01J"]);
        assert!(pages.image_url("p0").is_err(), "a page without an image url has nothing to ask for");
    }

    /// The gate opens once the list is cached, and the reader's loader then reads it without asking the source again.
    #[test]
    fn the_opened_chapter_list_is_cached_ahead_and_the_loader_reads_it_from_there() {
        let rt = runtime();
        let source = Arc::new(ThreePages(Mutex::default()));
        let (paths, pages) = pages_of(Arc::clone(&source), &rt);
        let pages: Arc<dyn HttpPageSource> = Arc::new(pages);
        let cache = Arc::new(DiskCache::open(paths.chapter_cache_dir(), CACHE_LIMIT_BYTES).unwrap());
        let prefetch = Prefetch::new(Arc::clone(&pages), Arc::clone(&cache), 7);
        let gate = prefetch.fetch("/c/1");
        let again = prefetch.fetch("/c/1");
        assert!(rt.block_on(crate::reader::screen::wait_ready(gate.ready_watch())));
        assert!(rt.block_on(crate::reader::screen::wait_ready(again.ready_watch())), "a second ask waits on the same fetch");
        assert!(prefetch.is_cached("/c/1"));
        let loader = HttpLoader::new(Arc::clone(&pages), 7, "/c/1".into(), Arc::clone(&cache));
        assert_eq!(loader.get_pages().ok().unwrap().len(), 3);
        assert_eq!(source.0.lock().unwrap().len(), 1, "one fetch, and the loader asked the source for nothing");
        assert!(*prefetch.fetch("/c/1").ready_watch().borrow(), "a cached list opens the gate at once");
        assert_eq!(source.0.lock().unwrap().len(), 1);
    }

    struct Offline(AtomicUsize);

    impl HttpPageSource for Offline {
        fn page_list(&self, _: &str) -> Result<Vec<PageEntry>, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err("offline".into())
        }
        fn image_url(&self, _: &str) -> Result<String, String> {
            Err("offline".into())
        }
        fn image(&self, _: &str) -> Result<Vec<u8>, String> {
            Err("offline".into())
        }
    }

    /// A failed fetch opens the gate with nothing cached and keeps its error for the reader; asking for it starts a new
    /// fetch, which a retry waits on.
    #[test]
    fn a_failed_fetch_opens_the_gate_and_hands_its_error_back_once() {
        let rt = runtime();
        let dir = tempfile::tempdir().unwrap();
        let cache = Arc::new(DiskCache::open(dir.path(), CACHE_LIMIT_BYTES).unwrap());
        let source = Arc::new(Offline(AtomicUsize::new(0)));
        let prefetch = Prefetch::new(source.clone(), Arc::clone(&cache), 7);
        assert!(rt.block_on(crate::reader::screen::wait_ready(prefetch.fetch("/c/1").ready_watch())));
        assert!(!prefetch.is_cached("/c/1"));
        assert_eq!(prefetch.missing("/c/1"), "offline");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !matches!(prefetch.lock().get("/c/1"), Some(Fetch::Failed(_))) {
            assert!(std::time::Instant::now() < deadline, "the second fetch never ended");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(source.0.load(Ordering::SeqCst), 2, "asking for the error started one more fetch");
        assert_eq!(prefetch.missing("/c/1"), "offline");
    }

    #[test]
    fn only_the_still_loading_message_is_a_wait() {
        assert!(is_pending(&still_loading()));
        assert!(!is_pending("Failed to load pages: 404"));
    }

    /// A source whose images are fixed up by XOR with 0xff. A body that starts with 0 is an image it cannot decode.
    struct Scrambled;

    #[async_trait]
    impl MangaSource for Scrambled {
        fn id(&self) -> i64 {
            1
        }
        fn name(&self) -> &str {
            "Scrambled"
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
        async fn unscramble(&self, body: &[u8], _headers: &[(String, String)]) -> Result<Option<Vec<u8>>, SourceError> {
            if body.first() == Some(&0) {
                return Err(SourceError::Transport("Failed to decode image".into()));
            }
            Ok(Some(body.iter().map(|byte| byte ^ 0xff).collect()))
        }
    }

    #[test]
    fn a_fetched_image_is_shown_as_the_source_fixes_it_or_as_fetched() {
        let rt = runtime();
        let fetched = |body: &[u8]| Fetched { body: body.to_vec(), headers: vec![("x-enc-seed".into(), "1".into())] };
        assert_eq!(rt.block_on(shown(&Scrambled, fetched(&[0x0f]))), Ok(vec![0xf0]), "the fix-up's bytes are shown");
        assert_eq!(rt.block_on(shown(&Scrambled, fetched(&[0]))), Err("Failed to decode image".to_owned()));
        let plain = ThreePages(Mutex::default());
        assert_eq!(rt.block_on(shown(&plain, fetched(&[1, 2]))), Ok(vec![1, 2]), "no fix-up: the bytes as fetched");
    }

    #[test]
    fn the_source_request_headers_are_sent_and_an_invalid_pair_is_left_out() {
        let map = header_map(&[("Accept".into(), "image/png".into()), ("bad name".into(), "x".into())]);
        assert_eq!(map.get("accept").unwrap(), "image/png");
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn the_image_request_carries_the_source_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("referer", HeaderValue::from_static("https://source.example.org/"));
        let request = request("https://img.example.org/manga/x/1.png", &headers).unwrap();
        assert_eq!(request.headers.get("referer").unwrap(), "https://source.example.org/");
        assert_eq!(request.url.as_str(), "https://img.example.org/manga/x/1.png");
    }

}
