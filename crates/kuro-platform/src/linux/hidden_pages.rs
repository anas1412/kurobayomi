//! The WebKitGTK engine of [`crate::hidden_pages`]: one unshown `WebView` per site, on the GTK thread.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use webkit6::glib;
use webkit6::prelude::*;

use crate::hidden_pages::{ANSWER_TIMEOUT, FETCH_JS, HiddenPages, IDLE_CLOSE, READY_TIMEOUT, idle_sites, is_check_page, origin_of, still_challenged};

/// The pages, with the folders their cookies and cache are kept in.
pub struct WebKitPages {
    data: PathBuf,
    cache: PathBuf,
}

impl WebKitPages {
    pub fn new(data: PathBuf, cache: PathBuf) -> Self {
        Self { data, cache }
    }
}

impl HiddenPages for WebKitPages {
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
    /// The page of each site (`scheme://host[:port]`), the session they share, when each was last used, and whether the
    /// sweep that closes idle pages runs.
    static VIEWS: RefCell<HashMap<String, webkit6::WebView>> = RefCell::new(HashMap::new());
    static SESSION: RefCell<Option<webkit6::NetworkSession>> = const { RefCell::new(None) };
    static USED: RefCell<HashMap<String, Instant>> = RefCell::new(HashMap::new());
    static SWEEPING: Cell<bool> = const { Cell::new(false) };
}

/// Notes a use of `origin`'s page and keeps a once-a-minute sweep running while any page is open.
fn touch(origin: &str) {
    USED.with(|used| used.borrow_mut().insert(origin.to_owned(), Instant::now()));
    if SWEEPING.replace(true) {
        return;
    }
    glib::timeout_add_seconds_local(60, || {
        let idle = USED.with(|used| idle_sites(&used.borrow(), Instant::now(), IDLE_CLOSE));
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

async fn fetch_in_page(data: &Path, cache: &Path, request_json: &str) -> Result<String, String> {
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

/// The page of `origin`, past the site's check. A new page, or one asked to `reload`, loads the site's front page and
/// waits until it is no longer the check.
async fn page_for(origin: &str, data: &Path, cache: &Path, reload: bool) -> Result<webkit6::WebView, String> {
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
    let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
    let ready = tx.clone();
    let loaded = view.connect_load_changed(move |view, event| {
        if event == webkit6::LoadEvent::Finished && !view.title().is_some_and(|t| is_check_page(&t)) {
            let _ = ready.try_send(Ok(()));
        }
    });
    let failed = view.connect_load_failed(move |_, _, uri, error| {
        let _ = tx.try_send(Err(format!("the site could not be opened ({uri}): {error}")));
        false
    });
    view.load_uri(&format!("{origin}/"));
    let outcome = match glib::future_with_timeout(READY_TIMEOUT, rx.recv()).await {
        Ok(Ok(result)) => result,
        _ => Err("the site's browser check did not clear".to_owned()),
    };
    view.disconnect(loaded);
    view.disconnect(failed);
    outcome.map(|()| view)
}
