//! The cookie jar (T-12a-009, T-12a-018 jar side). One store is shared by reqwest and by the app. It is saved as
//! JSON with persistent cookies only, after a 2 s quiet period, through a temp file and a rename.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cookie::SameSite as RawSameSite;
use cookie_store::serde::json;
use cookie_store::{Cookie, CookieDomain, CookieExpiration, CookieStore};
use kuro_platform::toast::{ToastLength, Toaster};
use kuro_platform::webview::{SameSite, WebCookie};
use reqwest_cookie_store::CookieStoreMutex;
use url::Url;

/// Writes happen this long after the last change.
pub const SAVE_DEBOUNCE: Duration = Duration::from_secs(2);
/// How long a pasted `cf_clearance` stays in the jar (what Cloudflare sets).
const CLEARANCE_MAX_AGE_SECS: i64 = 365 * 24 * 60 * 60;
/// How often the writer checks for changes.
const POLL: Duration = Duration::from_millis(500);

struct Inner {
    store: CookieStoreMutex,
    path: PathBuf,
    /// Counts changes. `saved` is the version of the last successful write, so `version != saved` means unsaved.
    version: AtomicU64,
    saved: AtomicU64,
    /// The browser user agent the user entered with a pasted clearance, by registrable domain (T-12a-033). It is
    /// the default `User-Agent` for that domain, because a clearance only works with the agent that earned it.
    agents: Mutex<BTreeMap<String, String>>,
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, CookieStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn agents(&self) -> MutexGuard<'_, BTreeMap<String, String>> {
        self.agents.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn changed(&self) {
        self.version.fetch_add(1, Ordering::SeqCst);
    }

    fn is_dirty(&self) -> bool {
        self.version.load(Ordering::SeqCst) != self.saved.load(Ordering::SeqCst)
    }

    /// Writes the store now. The version is read under the store lock, so the file holds at least that version.
    fn write_now(&self) -> io::Result<()> {
        let guard = self.lock();
        let version = self.version.load(Ordering::SeqCst);
        write_store(&guard, &self.path)?;
        drop(guard);
        self.saved.store(version, Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        if self.is_dirty() {
            if let Err(e) = self.write_now() {
                tracing::warn!("cannot save cookies on exit: {e}");
            }
        }
    }
}

/// The app's cookie jar. Clones share one store.
#[derive(Clone)]
pub struct CookieJar {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for CookieJar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CookieJar").field("path", &self.inner.path).finish()
    }
}

impl CookieJar {
    /// Opens the jar at `path`. A missing file gives an empty jar. A corrupt file is renamed to `<path>.corrupt`
    /// and the jar starts empty. The debounced writer needs a tokio runtime; without one, changes are saved by
    /// `flush` and when the last clone is dropped.
    pub fn load(path: PathBuf) -> Self {
        let store = match open_store(&path) {
            Ok(store) => store,
            Err(e) => {
                tracing::warn!("cookie file {} is corrupt, starting empty: {e}", path.display());
                let mut corrupt = path.as_os_str().to_owned();
                corrupt.push(".corrupt");
                if let Err(rename) = fs::rename(&path, &corrupt) {
                    tracing::warn!("cannot move the corrupt cookie file aside: {rename}");
                }
                CookieStore::new()
            }
        };
        let agents_path = agents_path(&path);
        let inner = Arc::new(Inner {
            store: CookieStoreMutex::new(store),
            path,
            version: AtomicU64::new(0),
            saved: AtomicU64::new(0),
            agents: Mutex::new(read_agents(&agents_path)),
        });
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(writer(Arc::downgrade(&inner)));
        }
        Self { inner }
    }

    /// The file the jar saves to.
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// The store as reqwest's cookie provider. Cookies from responses mark the jar changed.
    pub fn provider(&self) -> Arc<CookieProvider> {
        Arc::new(CookieProvider {
            inner: self.inner.clone(),
        })
    }

    /// Unexpired cookies that match `url`.
    pub fn get(&self, url: &Url) -> Vec<Cookie<'static>> {
        self.inner.lock().matches(url).into_iter().cloned().collect()
    }

    /// Deletes the cookies that match `url` and have one of `names` (all of them when `None`). Domain-scoped
    /// cookies are deleted too. Returns how many were deleted.
    pub fn remove(&self, url: &Url, names: Option<&[&str]>) -> usize {
        let mut store = self.inner.lock();
        let targets: Vec<(String, String, String)> = store
            .matches(url)
            .into_iter()
            .filter(|c| names.is_none_or(|ns| ns.contains(&c.name().trim())))
            .map(|c| {
                let domain = c.domain.as_cow().map(|d| d.into_owned()).unwrap_or_default();
                let path: &str = c.path.as_ref();
                (domain, path.to_owned(), c.name().to_owned())
            })
            .collect();
        for (domain, path, name) in &targets {
            store.remove(domain, path, name);
        }
        drop(store);
        if !targets.is_empty() {
            self.inner.changed();
        }
        targets.len()
    }

    /// Empties the jar and writes the empty file at once.
    pub fn remove_all(&self) -> io::Result<()> {
        self.inner.lock().clear();
        self.inner.changed();
        self.inner.write_now()?;
        self.inner.agents().clear();
        match fs::remove_file(agents_path(&self.inner.path)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Saves now when there are unsaved changes.
    pub fn flush(&self) -> io::Result<()> {
        if self.inner.is_dirty() {
            self.inner.write_now()?;
        }
        Ok(())
    }

    /// Adds cookies received from the WebView. A cookie with the same name, domain and path replaces the old one.
    pub fn import(&self, url: &Url, cookies: &[WebCookie]) {
        let mut store = self.inner.lock();
        let mut changed = false;
        for web in cookies {
            if let Some(cookie) = from_web(url, web) {
                changed |= store.insert(cookie, url).is_ok();
            }
        }
        drop(store);
        if changed {
            self.inner.changed();
        }
    }

    /// Stores a `cf_clearance` value that the user pasted for `url`, and the browser user agent that earned it as
    /// the default `User-Agent` for the domain (T-12a-033). The value must be non-empty and hold no `;` and no
    /// whitespace, so it cannot break the cookie header it is sent in; the user agent must be a usable header
    /// value. Nothing is stored when either is refused.
    pub fn import_clearance(&self, url: &Url, value: &str, user_agent: &str) -> Result<(), ClearanceImportError> {
        if value.is_empty() {
            return Err(ClearanceImportError::Empty);
        }
        if value.contains(';') || value.chars().any(char::is_whitespace) {
            return Err(ClearanceImportError::Malformed);
        }
        let user_agent = user_agent.trim();
        if user_agent.is_empty() || http::HeaderValue::from_str(user_agent).is_err() {
            return Err(ClearanceImportError::InvalidUserAgent);
        }
        // Cloudflare scopes the cookie to the registrable domain (`.site.test`), so `www.site.test` and `site.test`
        // both send it. An address or a name with no dot (`localhost`) stays host-only.
        let key = crate::interceptors::cloudflare::solve_key(url);
        let domain = match url.host() {
            Some(url::Host::Domain(_)) if key.contains('.') => format!(".{key}"),
            _ => url.host_str().unwrap_or_default().to_owned(),
        };
        let cookie = WebCookie {
            name: "cf_clearance".into(),
            value: value.into(),
            domain,
            path: "/".into(),
            secure: url.scheme() == "https",
            http_only: false,
            // Cloudflare sets `cf_clearance` for a year. A pasted value has no expiry of its own, and a session
            // cookie would not reach the jar file, so it gets the same year.
            expires: Some(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64) + CLEARANCE_MAX_AGE_SECS),
            same_site: None,
        };
        self.import(url, &[cookie]);
        let mut agents = self.inner.agents();
        agents.insert(crate::interceptors::cloudflare::solve_key(url), user_agent.to_owned());
        if let Err(e) = write_agents(&agents, &agents_path(&self.inner.path)) {
            tracing::warn!("cannot save the domain user agents: {e}");
        }
        Ok(())
    }

    /// The default `User-Agent` the user entered for `url`'s registrable domain with a pasted clearance.
    pub fn domain_user_agent(&self, url: &Url) -> Option<String> {
        self.inner.agents().get(&crate::interceptors::cloudflare::solve_key(url)).cloned()
    }

    /// The cookies to hand to the WebView before a load of `url`.
    pub fn export_for(&self, url: &Url) -> Vec<WebCookie> {
        self.get(url).iter().map(to_web).collect()
    }
}

const COOKIES_CLEARED_TEXT: &str = "Cookies cleared";

/// Settings > Networking > Clear cookies (T-12a-035): empties the jar, writes the empty file, then shows the toast.
/// When the file cannot be written the error is returned and no toast is shown.
/// Why a pasted `cf_clearance` value was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearanceImportError {
    Empty,
    Malformed,
    /// The browser user agent is blank or cannot be sent as a header.
    InvalidUserAgent,
}

pub fn clear_cookies(jar: &CookieJar, toaster: &Toaster) -> io::Result<()> {
    jar.remove_all()?;
    toaster.show(COOKIES_CLEARED_TEXT, ToastLength::Short);
    Ok(())
}

/// reqwest's view of the jar: the store, with a change recorded on every write.
pub struct CookieProvider {
    inner: Arc<Inner>,
}

impl reqwest::cookie::CookieStore for CookieProvider {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &http::HeaderValue>, url: &Url) {
        reqwest::cookie::CookieStore::set_cookies(&self.inner.store, cookie_headers, url);
        self.inner.changed();
    }

    fn cookies(&self, url: &Url) -> Option<http::HeaderValue> {
        reqwest::cookie::CookieStore::cookies(&self.inner.store, url)
    }
}

/// The debounced writer. It holds a weak reference, so dropping the last jar ends it (and the drop saves).
async fn writer(inner: std::sync::Weak<Inner>) {
    let mut seen = 0u64;
    let mut last_change: Option<tokio::time::Instant> = None;
    loop {
        tokio::time::sleep(POLL).await;
        let Some(jar) = inner.upgrade() else { return };
        let version = jar.version.load(Ordering::SeqCst);
        if version != seen {
            seen = version;
            last_change = Some(tokio::time::Instant::now());
        }
        if let Some(at) = last_change {
            if at.elapsed() >= SAVE_DEBOUNCE {
                if let Err(e) = jar.write_now() {
                    tracing::warn!("cannot save cookies: {e}");
                }
                last_change = None;
            }
        }
    }
}

/// The file that holds the domain user agents, next to the jar.
fn agents_path(jar_path: &Path) -> PathBuf {
    let mut path = jar_path.as_os_str().to_owned();
    path.push(".user_agents.json");
    PathBuf::from(path)
}

/// A missing or unreadable file gives no agents.
fn read_agents(path: &Path) -> BTreeMap<String, String> {
    File::open(path)
        .ok()
        .and_then(|file| serde_json::from_reader(BufReader::new(file)).ok())
        .unwrap_or_default()
}

fn write_agents(agents: &BTreeMap<String, String>, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, serde_json::to_vec(agents).map_err(io::Error::other)?)?;
    fs::rename(&tmp, path)
}

fn open_store(path: &Path) -> io::Result<CookieStore> {
    match File::open(path) {
        Ok(file) => json::load(BufReader::new(file)).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(CookieStore::new()),
        Err(e) => Err(e),
    }
}

/// Writes the persistent cookies to `path`: a temp file (mode 0600 on Unix), fsync, then a rename.
fn write_store(store: &CookieStore, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(&tmp)?;
    let mut writer = BufWriter::new(file);
    json::save(store, &mut writer).map_err(io::Error::other)?;
    writer.flush()?;
    let file = writer.into_inner().map_err(|e| e.into_error())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

/// `cookie_store` cookie to the WebView shape. A leading dot on `domain` means the cookie is not host-only.
pub fn to_web(cookie: &Cookie<'static>) -> WebCookie {
    let domain = match &cookie.domain {
        CookieDomain::HostOnly(d) => d.clone(),
        CookieDomain::Suffix(d) => format!(".{d}"),
        CookieDomain::NotPresent | CookieDomain::Empty => String::new(),
    };
    let expires = match cookie.expires {
        CookieExpiration::AtUtc(at) => Some(at.unix_timestamp()),
        CookieExpiration::SessionEnd => None,
    };
    let same_site = cookie.same_site().map(|s| match s {
        RawSameSite::Strict => SameSite::Strict,
        RawSameSite::Lax => SameSite::Lax,
        RawSameSite::None => SameSite::None,
    });
    let path: &str = cookie.path.as_ref();
    WebCookie {
        name: cookie.name().to_owned(),
        value: cookie.value().to_owned(),
        domain,
        path: path.to_owned(),
        secure: cookie.secure().unwrap_or(false),
        http_only: cookie.http_only().unwrap_or(false),
        expires,
        same_site,
    }
}

/// The WebView cookie as a `cookie_store` cookie for `url`, built from a Set-Cookie line so the store's own parser
/// applies its rules. Returns `None` for a cookie that has already expired.
pub fn from_web(url: &Url, web: &WebCookie) -> Option<Cookie<'static>> {
    let mut line = format!("{}={}; Path={}", web.name, web.value, web.path);
    if let Some(domain) = web.domain.strip_prefix('.') {
        line.push_str(&format!("; Domain={domain}"));
    }
    if web.secure {
        line.push_str("; Secure");
    }
    if web.http_only {
        line.push_str("; HttpOnly");
    }
    if let Some(expires) = web.expires {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
        let remaining = expires - now;
        if remaining <= 0 {
            return None;
        }
        line.push_str(&format!("; Max-Age={remaining}"));
    }
    match web.same_site {
        Some(SameSite::Strict) => line.push_str("; SameSite=Strict"),
        Some(SameSite::Lax) => line.push_str("; SameSite=Lax"),
        Some(SameSite::None) => line.push_str("; SameSite=None"),
        None => {}
    }
    Cookie::parse(line, url).ok().map(|c| c.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use reqwest::cookie::CookieStore as _;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn web(name: &str, value: &str, domain: &str, path: &str, expires: Option<i64>) -> WebCookie {
        WebCookie {
            name: name.into(),
            value: value.into(),
            domain: domain.into(),
            path: path.into(),
            secure: false,
            http_only: false,
            expires,
            same_site: None,
        }
    }

    fn future() -> Option<i64> {
        Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64 + 3600)
    }

    fn names(jar: &CookieJar, u: &Url) -> Vec<String> {
        let mut out: Vec<String> = jar.get(u).iter().map(|c| c.name().to_owned()).collect();
        out.sort();
        out
    }

    #[test]
    fn cookie_remove_matches_first_and_second_cookie() {
        let dir = tempfile::tempdir().unwrap();
        let jar = CookieJar::load(dir.path().join("cookies.json"));
        let u = url("https://example.test/");
        let headers = [HeaderValue::from_static("a=1"), HeaderValue::from_static("cf_clearance=x")];
        jar.provider().set_cookies(&mut headers.iter(), &u);
        assert_eq!(jar.remove(&u, Some(&["cf_clearance"])), 1);
        assert_eq!(names(&jar, &u), vec!["a".to_owned()]);
    }

    #[test]
    fn cookie_remove_deletes_a_domain_scoped_clearance() {
        let dir = tempfile::tempdir().unwrap();
        let jar = CookieJar::load(dir.path().join("cookies.json"));
        let u = url("https://example.test/");
        jar.import(&u, &[web("cf_clearance", "v", ".example.test", "/", None)]);
        assert_eq!(jar.remove(&u, Some(&["cf_clearance"])), 1);
        assert!(jar.get(&u).is_empty());
    }

    #[test]
    fn a_corrupt_file_is_renamed_and_the_jar_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookies.json");
        std::fs::write(&path, r#"{"not": "a cookie store""#).unwrap();
        let jar = CookieJar::load(path.clone());
        assert!(jar.get(&url("https://example.test/")).is_empty());
        assert!(!path.exists());
        assert!(dir.path().join("cookies.json.corrupt").exists());
    }

    #[test]
    fn a_save_is_atomic_mode_0600_and_keeps_only_persistent_cookies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookies.json");
        let jar = CookieJar::load(path.clone());
        let u = url("https://example.test/");
        jar.import(&u, &[web("keep", "1", "example.test", "/", future())]);
        jar.provider().set_cookies(
            &mut [HeaderValue::from_static("session=2")].iter(),
            &u,
        );
        jar.flush().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("keep"));
        assert!(!text.contains("session"));
        assert!(!dir.path().join("cookies.json.tmp").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn a_saved_jar_loads_back_with_the_same_cookies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookies.json");
        let u = url("https://example.test/");
        {
            let jar = CookieJar::load(path.clone());
            jar.import(&u, &[web("keep", "1", "example.test", "/", future())]);
            jar.flush().unwrap();
        }
        let jar = CookieJar::load(path);
        assert_eq!(names(&jar, &u), vec!["keep".to_owned()]);
    }

    #[test]
    fn remove_all_writes_the_empty_store_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookies.json");
        let jar = CookieJar::load(path.clone());
        let u = url("https://example.test/");
        jar.import(&u, &[web("keep", "1", "example.test", "/", future())]);
        jar.remove_all().unwrap();
        assert!(jar.get(&u).is_empty());
        let reloaded = CookieJar::load(path);
        assert!(reloaded.get(&u).is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn the_writer_saves_once_after_the_quiet_period() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookies.json");
        let jar = CookieJar::load(path.clone());
        let u = url("https://example.test/");
        for i in 0..10 {
            jar.import(&u, &[web(&format!("c{i}"), "1", "example.test", "/", future())]);
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
        assert!(!path.exists(), "still inside the quiet period");
        tokio::time::sleep(Duration::from_secs(3)).await;
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("c9"));
    }

    #[test]
    fn a_replacement_with_the_same_name_domain_and_path_replaces_the_entry() {
        let dir = tempfile::tempdir().unwrap();
        let jar = CookieJar::load(dir.path().join("cookies.json"));
        let u = url("https://example.test/");
        jar.import(&u, &[web("cf_clearance", "old", "example.test", "/", future())]);
        jar.import(&u, &[web("cf_clearance", "new", "example.test", "/", future())]);
        let values: Vec<String> = jar.get(&u).iter().map(|c| c.value().to_owned()).collect();
        assert_eq!(values, vec!["new".to_owned()]);
    }

    #[test]
    fn every_web_field_survives_the_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let jar = CookieJar::load(dir.path().join("cookies.json"));
        let u = url("https://example.test/x/y");
        let original = WebCookie {
            name: "cf_clearance".into(),
            value: "abc".into(),
            domain: ".example.test".into(),
            path: "/x".into(),
            secure: true,
            http_only: true,
            expires: future(),
            same_site: Some(SameSite::Lax),
        };
        jar.import(&u, std::slice::from_ref(&original));
        let exported = jar.export_for(&u);
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].name, original.name);
        assert_eq!(exported[0].value, original.value);
        assert_eq!(exported[0].domain, original.domain);
        assert_eq!(exported[0].path, original.path);
        assert_eq!(exported[0].secure, original.secure);
        assert_eq!(exported[0].http_only, original.http_only);
        assert_eq!(exported[0].same_site, original.same_site);
        let within = exported[0].expires.unwrap() - original.expires.unwrap();
        assert!(within.abs() <= 2, "expiry drifted by {within}s");
    }

    #[test]
    fn a_host_only_cookie_has_no_leading_dot_and_an_expired_one_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let jar = CookieJar::load(dir.path().join("cookies.json"));
        let u = url("https://example.test/");
        jar.import(&u, &[web("h", "1", "example.test", "/", future())]);
        jar.import(&u, &[web("gone", "1", "example.test", "/", Some(1))]);
        let exported = jar.export_for(&u);
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].domain, "example.test");
    }

    /// A client on `jar`, the way the app builds one: the jar is the transport's cookie store.
    fn client_on(jar: &CookieJar) -> crate::NetworkClient {
        let transport = std::sync::Arc::new(
            crate::transport::Transport::new(crate::transport::TransportLimits::default(), jar.provider()).unwrap(),
        );
        crate::client::ClientBuilder {
            chain: vec![],
            cloudflare: false,
            transport,
            cookies: jar.clone(),
            user_agent: crate::client::fixed_user_agent(""),
        }
        .build()
    }

    #[tokio::test]
    async fn clear_cookies_empties_the_jar_and_shows_the_short_toast() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cookies.json");
        let jar = CookieJar::load(path.clone());
        let u = url("https://example.test/");
        jar.import(&u, &[web("keep", "1", "example.test", "/", future())]);
        let (toaster, feed) = kuro_platform::toast::toast_channel();
        clear_cookies(&jar, &toaster).unwrap();
        assert!(jar.get(&u).is_empty());
        assert!(!std::fs::read_to_string(&path).unwrap().contains("keep"));
        let message = feed.next().await.unwrap();
        assert_eq!(message.text, "Cookies cleared");
        assert_eq!(message.length, ToastLength::Short);
    }

    #[tokio::test]
    async fn clear_cookies_shows_no_toast_when_the_file_cannot_be_written() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, "a file where the directory should be").unwrap();
        let jar = CookieJar::load(blocker.join("cookies.json"));
        let (toaster, feed) = kuro_platform::toast::toast_channel();
        assert!(clear_cookies(&jar, &toaster).is_err());
        drop(toaster);
        assert!(feed.next().await.is_none(), "no toast was queued");
    }

    #[tokio::test]
    async fn after_clear_cookies_no_request_sends_a_cookie_header() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::path("/login"))
            .respond_with(wiremock::ResponseTemplate::new(200).insert_header("set-cookie", "sid=1; Path=/"))
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::path("/page"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let dir = tempfile::tempdir().unwrap();
        let jar = CookieJar::load(dir.path().join("cookies.json"));
        let client = client_on(&jar);
        client.get(&format!("{}/login", server.uri())).await.unwrap();
        client.get(&format!("{}/page", server.uri())).await.unwrap();
        let before = server.received_requests().await.unwrap();
        assert!(before[1].headers.get("cookie").is_some(), "the cookie is sent before the clear");
        jar.remove_all().unwrap();
        client.get(&format!("{}/page", server.uri())).await.unwrap();
        let after = server.received_requests().await.unwrap();
        assert!(after[2].headers.get("cookie").is_none(), "no cookie is sent after the clear");
    }
}
