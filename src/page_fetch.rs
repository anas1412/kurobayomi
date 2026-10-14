//! The hidden browser pages behind `kuro_source_wasm::bridge::PageFetch`. A site that answers the plug-ins with
//! Cloudflare's browser check ("Just a moment...") is opened once in an unshown WebKit view, which passes the check by
//! itself, and the plug-in's requests to it are then sent with `fetch()` from inside that page: the pass only holds for
//! the browser that earned it, so replaying its cookie from the app's own client is refused. One view per site, kept
//! for the run. Its cookies live in the app's WebKit folder, so a later run usually starts past the check.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use gtk::glib;
use serde::Deserialize;
use webkit6::prelude::*;

/// How long a site's check may take to clear in the hidden page.
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a plug-in waits for one answer, the check included.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(90);
/// A site's hidden page is closed after this long unused: each holds a web process. Its cookies stay, so reopening it
/// rarely meets the check again.
const IDLE_CLOSE: Duration = Duration::from_secs(10 * 60);

/// Runs in the page: sends the plug-in's request (`req`, its JSON) with the page's own `fetch` and returns the plug-in's
/// response shape. A Referer cannot be set as a header from a page, so it becomes the request's referrer.
const FETCH_JS: &str = r#"
const q = JSON.parse(req);
const headers = {};
let referrer;
for (const [name, value] of (q.headers || [])) {
  if (name.toLowerCase() === 'referer') referrer = value; else headers[name] = value;
}
const init = { method: q.method || 'GET', headers, credentials: 'include' };
if (referrer) init.referrer = referrer;
if (q.body != null) init.body = q.body;
const r = await fetch(q.url, init);
const bytes = new Uint8Array(await r.arrayBuffer());
let body, bodyEncoding;
try {
  body = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
} catch (e) {
  let s = '';
  for (let i = 0; i < bytes.length; i += 32768) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 32768));
  body = btoa(s);
  bodyEncoding = 'base64';
}
return JSON.stringify({ status: r.status, headers: [...r.headers.entries()], url: r.url, body, bodyEncoding });
"#;

/// The WebKit pages, with the folders their cookies and cache are kept in.
pub struct WebKitPages {
    data: PathBuf,
    cache: PathBuf,
}

impl WebKitPages {
    pub fn new(data: PathBuf, cache: PathBuf) -> Self {
        Self { data, cache }
    }
}

impl kuro_source_wasm::bridge::PageFetch for WebKitPages {
    /// Called on a plug-in's worker thread, never the window's: the page work runs on the window's thread and this
    /// waits for its answer.
    fn fetch(&self, request_json: &str) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        let (json, data, cache) = (request_json.to_owned(), self.data.clone(), self.cache.clone());
        glib::MainContext::default().invoke(move || {
            glib::spawn_future_local(async move {
                let _ = tx.send(fetch_in_page(&data, &cache, &json).await);
            });
        });
        rx.recv_timeout(ANSWER_TIMEOUT).map_err(|_| "the site's hidden page did not answer in time".to_owned())?
    }
}

thread_local! {
    /// The hidden page of each site (`scheme://host[:port]`), and the session they share.
    static VIEWS: RefCell<HashMap<String, webkit6::WebView>> = RefCell::new(HashMap::new());
    /// When each site's page was last used, and whether the sweep that closes idle pages is running.
    static USED: RefCell<HashMap<String, std::time::Instant>> = RefCell::new(HashMap::new());
    static SWEEPING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SESSION: RefCell<Option<webkit6::NetworkSession>> = const { RefCell::new(None) };
}

#[derive(Deserialize)]
struct Target {
    url: String,
}

/// The site a request goes to: its scheme, host and port.
fn origin_of(request_json: &str) -> Result<String, String> {
    let target: Target = serde_json::from_str(request_json).map_err(|e| format!("invalid request: {e}"))?;
    let url = url::Url::parse(&target.url).map_err(|e| format!("invalid url: {e}"))?;
    Ok(url.origin().ascii_serialization())
}

/// Whether the page's answer is still the browser check (the pass ran out): the page is reloaded once and asked again.
fn still_challenged(answer: &str) -> bool {
    #[derive(Deserialize)]
    struct Answer {
        status: u16,
        headers: Vec<(String, String)>,
    }
    serde_json::from_str::<Answer>(answer).is_ok_and(|a| {
        matches!(a.status, 403 | 503) && a.headers.iter().any(|(n, v)| n.eq_ignore_ascii_case("cf-mitigated") && v.eq_ignore_ascii_case("challenge"))
    })
}

/// The sites whose pages have been unused for `idle` or more at `now`.
fn idle_sites(used: &HashMap<String, std::time::Instant>, now: std::time::Instant, idle: Duration) -> Vec<String> {
    used.iter().filter(|(_, at)| now.duration_since(**at) >= idle).map(|(site, _)| site.clone()).collect()
}

/// Notes a use of `origin`'s page and keeps a once-a-minute sweep running while any page is open.
fn touch(origin: &str) {
    USED.with(|used| used.borrow_mut().insert(origin.to_owned(), std::time::Instant::now()));
    if SWEEPING.replace(true) {
        return;
    }
    glib::timeout_add_seconds_local(60, || {
        let idle = USED.with(|used| idle_sites(&used.borrow(), std::time::Instant::now(), IDLE_CLOSE));
        for site in idle {
            USED.with(|used| used.borrow_mut().remove(&site));
            if let Some(view) = VIEWS.with(|views| views.borrow_mut().remove(&site)) {
                view.terminate_web_process();
            }
        }
        let open = VIEWS.with(|views| !views.borrow().is_empty());
        SWEEPING.set(open);
        if open { glib::ControlFlow::Continue } else { glib::ControlFlow::Break }
    });
}

async fn fetch_in_page(data: &std::path::Path, cache: &std::path::Path, request_json: &str) -> Result<String, String> {
    let origin = origin_of(request_json)?;
    touch(&origin);
    let mut answer = String::new();
    for reload in [false, true] {
        let view = page_for(&origin, data, cache, reload).await?;
        let args = glib::VariantDict::new(None);
        args.insert("req", request_json);
        let value = view
            .call_async_javascript_function_future(FETCH_JS, Some(&args.end()), None, None)
            .await
            .map_err(|e| format!("the site's hidden page failed the request: {e}"))?;
        answer = value.to_str().to_string();
        if !still_challenged(&answer) {
            break;
        }
    }
    Ok(answer)
}

/// The hidden page of `origin`, past the site's check. A new page, or one asked to `reload`, loads the site's front page
/// and waits until it is no longer the check.
async fn page_for(origin: &str, data: &std::path::Path, cache: &std::path::Path, reload: bool) -> Result<webkit6::WebView, String> {
    if let Some(view) = VIEWS.with(|views| views.borrow().get(origin).cloned()).filter(|_| !reload) {
        return Ok(view);
    }
    let view = VIEWS.with(|views| views.borrow().get(origin).cloned()).unwrap_or_else(|| {
        let session = SESSION.with(|s| {
            s.borrow_mut()
                .get_or_insert_with(|| webkit6::NetworkSession::new(data.to_str(), cache.to_str()))
                .clone()
        });
        let view = webkit6::WebView::builder().network_session(&session).build();
        VIEWS.with(|views| views.borrow_mut().insert(origin.to_owned(), view.clone()));
        view
    });
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), String>>();
    let tx = Rc::new(RefCell::new(Some(tx)));
    let ready = Rc::clone(&tx);
    let loaded = view.connect_load_changed(move |view, event| {
        let checking = view.title().is_some_and(|t| t.to_lowercase().contains("just a moment"));
        if event == webkit6::LoadEvent::Finished && !checking {
            if let Some(tx) = ready.borrow_mut().take() {
                let _ = tx.send(Ok(()));
            }
        }
    });
    let failed = view.connect_load_failed(move |_, _, uri, error| {
        if let Some(tx) = tx.borrow_mut().take() {
            let _ = tx.send(Err(format!("the site could not be opened ({uri}): {error}")));
        }
        false
    });
    view.load_uri(&format!("{origin}/"));
    let outcome = match glib::future_with_timeout(READY_TIMEOUT, rx).await {
        Ok(Ok(result)) => result,
        _ => Err("the site's browser check did not clear".to_owned()),
    };
    view.disconnect(loaded);
    view.disconnect(failed);
    outcome.map(|()| view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_goes_to_the_page_of_its_site() {
        assert_eq!(origin_of(r#"{"url":"https://animepahe.pw/api?m=airing"}"#).unwrap(), "https://animepahe.pw");
        assert_eq!(origin_of(r#"{"url":"http://a.test:8080/x"}"#).unwrap(), "http://a.test:8080");
        assert!(origin_of("nope").is_err());
    }

    #[test]
    fn only_pages_unused_long_enough_are_closed() {
        let now = std::time::Instant::now();
        let used = HashMap::from([("https://old.test".to_owned(), now - Duration::from_secs(700)), ("https://new.test".to_owned(), now)]);
        assert_eq!(idle_sites(&used, now, IDLE_CLOSE), ["https://old.test"]);
    }

    #[test]
    fn only_a_check_answer_reloads_the_page() {
        assert!(still_challenged(r#"{"status":403,"headers":[["cf-mitigated","challenge"]],"url":"u","body":""}"#));
        assert!(!still_challenged(r#"{"status":200,"headers":[],"url":"u","body":"{}"}"#));
        assert!(!still_challenged(r#"{"status":403,"headers":[],"url":"u","body":""}"#));
    }
}
