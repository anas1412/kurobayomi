//! The pages of an online chapter, from the manga's source (the built-in Weeb Central and Comix, and any installed
//! source the directory answers for). The page list comes from `MangaSource::get_page_list`; the images go over the
//! built-in sources' network client with the source's image headers (`Builtin::image_headers`: the Weeb Central
//! `Referer`, none for Comix), and a Comix image goes through its descrambler. The reader's HTTP loader keeps both in
//! the chapter cache, the folder the cache settings clear.
//!
//! The loader calls a source from its worker thread, outside the runtime, so every call is spawned on the app's runtime
//! and waited for. The GTK thread never asks the source: [`Prefetch`] fetches page lists on threads, and the reader
//! reads them from the cache.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use kuro_core::sources::SourceGate;
use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;
use kuro_source::MangaSource;
use kuro_source::model::SChapter;
use reqwest::Url;
use reqwest::header::HeaderMap;
use tokio::runtime::Handle;
use tokio::sync::watch;

use super::cache::{CACHE_LIMIT_BYTES, DiskCache};
use super::loader::PageEntry;
use super::loader::http::HttpPageSource;

/// A source's image fix-up: `Ok(None)` shows the body as it came, `Ok(Some)` shows these bytes instead.
pub type Unscramble = fn(&[u8], &HeaderMap) -> Result<Option<Vec<u8>>, String>;

/// Kotlin `altHostFallback` waits this long before it tries the next host.
const ALT_HOST_GAP: Duration = Duration::from_secs(2);
/// The statuses a Comix image host answers a passing failure with; the same url is asked again (Kotlin `proceedWithRetry`).
const RETRY_STATUS: [u16; 5] = [502, 503, 522, 523, 404];
const RETRY_GAP: Duration = Duration::from_millis(1500);
const RETRIES: u32 = 10;
/// The path segments a Comix image host serves the same file under, tried after the alt hosts.
const PATH_VARIANTS: [&str; 7] = ["hi", "fcf", "i5", "si", "i", "sii", "ii"];

/// What the reader's loader reports for a chapter whose page list is still being fetched. It is a wait, not a failure:
/// the reader shows its spinner and asks again (see [`is_pending`]).
pub const STILL_LOADING: &str = "The page list is still loading. Try again in a moment.";

/// Whether a load error is the wait for a page list that is still being fetched.
pub fn is_pending(message: &str) -> bool {
    message == STILL_LOADING
}

pub struct SourcePages {
    source: Arc<dyn MangaSource>,
    client: NetworkClient,
    headers: HeaderMap,
    /// Set for Comix: its descrambler; its images also get the retries and fallback urls its hosts need.
    unscramble: Option<Unscramble>,
    runtime: Handle,
}

impl SourcePages {
    pub fn new(source: Arc<dyn MangaSource>, client: NetworkClient, headers: HeaderMap, runtime: Handle) -> Self {
        let unscramble: Option<Unscramble> = None;
        Self { source, client, headers, unscramble, runtime }
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
        let pages = self.run(async move { kuro_source::isolate(source.get_page_list(&chapter)).await })?.map_err(|e| e.to_string())?;
        Ok(pages.into_iter().map(|page| PageEntry { url: page.url, image_url: page.image_url }).collect())
    }

    fn image_url(&self, page_url: &str) -> Result<String, String> {
        // `MangaSource` has no image-url call; the sources this build has give every image url in the list
        Err(format!("the source gave no image for page {page_url}"))
    }

    fn image(&self, image_url: &str) -> Result<Vec<u8>, String> {
        let comix = self.unscramble.is_some();
        let (client, headers, tries) = (self.client.clone(), self.headers.clone(), attempts(image_url, comix));
        let retries = if comix { RETRIES } else { 0 };
        // ponytail: a page every host refuses takes minutes to fail (each url retried 10 times, 1.5 s apart, as Kotlin
        // does), and the loader's one worker waits with it. Cap the retries if the live check shows such pages.
        let (body, got) = self.run(async move {
            let mut last = String::new();
            for (n, url) in tries.iter().enumerate() {
                if n > 0 {
                    tokio::time::sleep(ALT_HOST_GAP).await;
                }
                for attempt in 0..=retries {
                    if attempt > 0 {
                        tokio::time::sleep(RETRY_GAP).await;
                    }
                    let target = if attempt == 0 { url.clone() } else { with_retry(url, attempt) };
                    match fetch(&client, &target, &headers).await {
                        Ok(got) => return Ok(got),
                        Err((status, error)) => {
                            last = error;
                            if !status.is_some_and(|s| RETRY_STATUS.contains(&s)) {
                                break;
                            }
                        }
                    }
                }
            }
            Err(last)
        })??;
        // the descrambler decodes and re-encodes the page, so it runs here on the loader's thread, not on the runtime
        shown(body, &got, self.unscramble)
    }
}

/// The GET of one image with the source's headers.
fn request(url: &str, headers: &HeaderMap) -> Result<kuro_net::request::Request, String> {
    Ok(kuro_net::get(url).map_err(|e| e.to_string())?.headers(headers.clone()))
}

/// The body and the response headers of one image. A status other than 2xx or 304 is an error with that status.
async fn fetch(client: &NetworkClient, url: &str, headers: &HeaderMap) -> Result<(Vec<u8>, HeaderMap), (Option<u16>, String)> {
    let response = client.execute(request(url, headers).map_err(|e| (None, e))?).await.map_err(|e| (None, e.to_string()))?;
    kuro_images::http::check_status(response.status).map_err(|e| (Some(response.status.as_u16()), e.to_string()))?;
    let got = response.headers.clone();
    Ok((response.bytes().await.map_err(|e| (None, e.to_string()))?.to_vec(), got))
}

/// The bytes the reader shows: the body, or what the source's descrambler makes of it.
fn shown(body: Vec<u8>, headers: &HeaderMap, unscramble: Option<Unscramble>) -> Result<Vec<u8>, String> {
    match unscramble {
        Some(unscramble) => Ok(unscramble(&body, headers)?.unwrap_or(body)),
        None => Ok(body),
    }
}

/// The urls an image is tried at, in order: the url without its fragment, then each host a Comix `#alt=h1,h2` fragment
/// lists, with `r=1`, `r=2`, ... (Kotlin `altHostFallback`). For Comix, then the url with each other path variant in
/// place of its first variant segment (`/hi/`, `/i5/`, ... or any `/s?i+/`).
fn attempts(image_url: &str, comix: bool) -> Vec<String> {
    let Ok(mut url) = Url::parse(image_url) else { return vec![image_url.to_owned()] };
    let hosts: Vec<String> = url.fragment().and_then(|f| f.strip_prefix("alt=")).map(|list| list.split(',').filter(|h| !h.is_empty()).map(str::to_owned).collect()).unwrap_or_default();
    url.set_fragment(None);
    let mut out = vec![url.to_string()];
    for (n, host) in hosts.iter().enumerate() {
        let mut next = url.clone();
        if next.set_host(Some(host)).is_ok() {
            next.query_pairs_mut().append_pair("r", &(n + 1).to_string());
            out.push(next.into());
        }
    }
    let segments: Vec<String> = url.path().split('/').map(str::to_owned).collect();
    // the last segment is the file name
    let at = segments.iter().take(segments.len().saturating_sub(1)).position(|s| is_variant_segment(s));
    if let (true, Some(at)) = (comix, at) {
        for variant in PATH_VARIANTS.iter().filter(|v| **v != segments[at]) {
            let mut path = segments.clone();
            path[at] = (*variant).to_owned();
            let mut next = url.clone();
            next.set_path(&path.join("/"));
            out.push(next.into());
        }
    }
    out
}

/// A Comix image path segment that has variants: one of [`PATH_VARIANTS`], or `s?i+`.
fn is_variant_segment(segment: &str) -> bool {
    let rest = segment.strip_prefix('s').unwrap_or(segment);
    PATH_VARIANTS.contains(&segment) || (!rest.is_empty() && rest.chars().all(|c| c == 'i'))
}

/// `url` with its `r` query parameter set to `attempt`, the rest of the query kept as it is.
fn with_retry(url: &str, attempt: u32) -> String {
    let Ok(mut parsed) = Url::parse(url) else { return url.to_owned() };
    let mut query: Vec<String> = parsed.query().unwrap_or("").split('&').filter(|p| !p.is_empty() && !p.starts_with("r=")).map(str::to_owned).collect();
    query.push(format!("r={attempt}"));
    parsed.set_query(Some(&query.join("&")));
    parsed.into()
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
                me.cache.put_page_list(me.manga_id, &url, &list).map_err(|e| format!("the page list could not be cached: {e}"))
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

    /// The reader's error for a chapter whose list is not cached: the last fetch's error, or [`STILL_LOADING`]. A fetch
    /// is started again when none runs, so the next try can read the list.
    pub fn missing(self: &Arc<Self>, chapter_url: &str) -> String {
        let failed = match self.lock().get(chapter_url) {
            Some(Fetch::Failed(error)) => Some(error.clone()),
            _ => None,
        };
        self.fetch(chapter_url);
        failed.unwrap_or_else(|| STILL_LOADING.to_owned())
    }
}

/// The pages of `source` over the built-in sources' client (one client, so one cookie jar), with the source's image
/// headers.
fn builtin_pages(builtin: &crate::browse::builtin::Builtin, source: Arc<dyn MangaSource>, runtime: Handle) -> Option<SourcePages> {
    let headers = builtin.image_headers(source.id()).unwrap_or_default();
    Some(SourcePages::new(source, builtin.client()?, headers, runtime))
}

/// The page lists and pages of the manga `manga_id` of source `source_id`, when the directory knows that source and
/// the app's runtime runs; `None` for a local or unknown source.
pub fn online(source_id: i64, manga_id: i64) -> Option<Arc<Prefetch>> {
    let source = crate::browse::directory::current()?.manga_source(source_id)?;
    let runtime = Handle::try_current().ok()?;
    let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
    let cache = DiskCache::open(paths.chapter_cache_dir(), CACHE_LIMIT_BYTES).map_err(|e| tracing::warn!(error = %e, "no chapter cache")).ok()?;
    let pages = builtin_pages(&*crate::browse::builtin::current()?, source, runtime)?;
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

    use super::*;
    use crate::reader::loader::http::HttpLoader;

    /// A source whose chapters have three pages, each with its image url; it logs the chapter urls it was asked for.
    struct ThreePages(Mutex<Vec<String>>);

    #[async_trait]
    impl MangaSource for ThreePages {
        fn id(&self) -> i64 {
            2131019126180322627
        }
        fn name(&self) -> &str {
            "Weeb Central"
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
        assert!(pages.unscramble.is_none(), "Weeb Central pages go as they are");
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
        assert!(is_pending(STILL_LOADING));
        assert!(!is_pending("Failed to load pages: 404"));
    }

    #[test]
    fn an_image_is_tried_without_its_fragment_then_on_each_alt_host_then_on_each_path_variant() {
        assert_eq!(attempts("https://img.example/a/1.png", false), ["https://img.example/a/1.png"]);
        assert_eq!(attempts("https://img.example/i/1.png", false), ["https://img.example/i/1.png"], "only Comix has path variants");
        assert_eq!(
            attempts("https://s1.comix.to/i5/x/1.webp?v3#alt=s1.cdn1.net,s2.cdn2.net", true),
            [
                "https://s1.comix.to/i5/x/1.webp?v3",
                "https://s1.cdn1.net/i5/x/1.webp?v3&r=1",
                "https://s2.cdn2.net/i5/x/1.webp?v3&r=2",
                "https://s1.comix.to/hi/x/1.webp?v3",
                "https://s1.comix.to/fcf/x/1.webp?v3",
                "https://s1.comix.to/si/x/1.webp?v3",
                "https://s1.comix.to/i/x/1.webp?v3",
                "https://s1.comix.to/sii/x/1.webp?v3",
                "https://s1.comix.to/ii/x/1.webp?v3",
            ]
        );
        assert_eq!(attempts("https://h.example/x/i.webp", true), ["https://h.example/x/i.webp"], "the file name is not a variant segment");
        assert!(is_variant_segment("siii") && !is_variant_segment("s") && !is_variant_segment("x"));
    }

    #[test]
    fn a_retry_sets_r_and_keeps_the_rest_of_the_query() {
        assert_eq!(with_retry("https://s1.cdn1.net/i5/x/1.webp?v3&r=1", 4), "https://s1.cdn1.net/i5/x/1.webp?v3&r=4");
        assert_eq!(with_retry("https://s1.comix.to/i5/x/1.webp", 1), "https://s1.comix.to/i5/x/1.webp?r=1");
    }

    #[test]
    fn the_image_request_carries_the_source_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("referer", HeaderValue::from_static("https://weebcentral.com/"));
        let request = request("https://temp.compsci88.com/manga/x/1.png", &headers).unwrap();
        assert_eq!(request.headers.get("referer").unwrap(), "https://weebcentral.com/");
        assert_eq!(request.url.as_str(), "https://temp.compsci88.com/manga/x/1.png");
    }

    const GRID: u32 = 5;
    const TILE: u32 = 8;

    fn colour(tile: u32) -> image::Rgb<u8> {
        image::Rgb([(tile * 10) as u8, 255 - (tile * 10) as u8, ((tile % 5) * 60) as u8])
    }

    #[test]
    fn a_scrambled_comix_page_comes_out_with_its_tiles_in_place() {
        let comix = unscrambler(kuro_source::generate_id("Comix", "en", 1));
        assert!(comix.is_some() && unscrambler(2131019126180322627).is_none());
        let (body, headers) = scrambled_fixture(12345, 987_654_321);
        assert!(image::load_from_memory(&body).is_err(), "the body is not an image before the descrambler");
        let shown = shown(body, &headers, comix).unwrap();
        let page = image::load_from_memory(&shown).expect("the shown bytes decode").to_rgb8();
        assert_eq!(page.dimensions(), (GRID * TILE, GRID * TILE));
        for tile in 0..GRID * GRID {
            let (x, y) = ((tile % GRID) * TILE + TILE / 2, (tile / GRID) * TILE + TILE / 2);
            let (got, want) = (page.get_pixel(x, y).0, colour(tile).0);
            assert!(got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 12), "tile {tile}: {got:?} against {want:?} (JPEG 90)");
        }
        let plain = b"\x89PNG plain".to_vec();
        assert_eq!(super::shown(plain.clone(), &HeaderMap::new(), comix).unwrap(), plain, "an unscrambled page goes as it is");
        assert_eq!(super::shown(plain.clone(), &headers, None).unwrap(), plain, "other sources never descramble");
    }
}
