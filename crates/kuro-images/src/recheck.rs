//! Re-checks of cached covers (owner rule, 2026-10-23). A cover can change behind the same address, so a cached one is
//! asked again every [`RECHECK_AFTER_MS`] with a conditional request (`If-None-Match`, `If-Modified-Since`) and only
//! downloaded again when the site says it changed. What the last answer said lives in a small `<md5(url)>.check` file
//! in the image cache folder: the time of the check, then the `ETag`, then the `Last-Modified` value, one per line.

use std::path::{Path, PathBuf};

use http::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use http::{HeaderMap, HeaderValue};

use crate::keys::hash_key;

/// Three days.
pub const RECHECK_AFTER_MS: i64 = 3 * 24 * 60 * 60 * 1000;

/// The last check of one cover address.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Check {
    pub checked_ms: i64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl Check {
    /// A check made at `now_ms` from the response headers of the image.
    pub fn from_headers(headers: &HeaderMap, now_ms: i64) -> Self {
        let text = |name| headers.get(name).and_then(|v: &HeaderValue| v.to_str().ok()).map(str::to_owned);
        Self { checked_ms: now_ms, etag: text(ETAG), last_modified: text(LAST_MODIFIED) }
    }

    /// Whether the cover is due for a check at `now_ms`.
    pub fn due(&self, now_ms: i64) -> bool {
        now_ms - self.checked_ms >= RECHECK_AFTER_MS
    }

    /// `base` with the conditional headers this check allows.
    pub fn conditional(&self, base: &HeaderMap) -> HeaderMap {
        let mut headers = base.clone();
        let pairs = [(IF_NONE_MATCH, &self.etag), (IF_MODIFIED_SINCE, &self.last_modified)];
        for (name, value) in pairs {
            if let Some(value) = value.as_deref().and_then(|v| HeaderValue::from_str(v).ok()) {
                headers.insert(name, value);
            }
        }
        headers
    }
}

fn file(root: &Path, url: &str) -> PathBuf {
    root.join(format!("{}.check", hash_key(url)))
}

/// The last check of `url`, `None` when there was none.
pub fn read(root: &Path, url: &str) -> Option<Check> {
    let text = std::fs::read_to_string(file(root, url)).ok()?;
    let mut lines = text.lines();
    let checked_ms = lines.next()?.trim().parse().ok()?;
    let mut next = || lines.next().map(str::to_owned).filter(|line| !line.is_empty());
    Some(Check { checked_ms, etag: next(), last_modified: next() })
}

/// Stores the check of `url`. A failure only means the cover is asked again sooner.
pub fn write(root: &Path, url: &str, check: &Check) {
    let text = format!("{}\n{}\n{}\n", check.checked_ms, check.etag.as_deref().unwrap_or(""), check.last_modified.as_deref().unwrap_or(""));
    let _ = std::fs::write(file(root, url), text);
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_round_trips_and_is_due_after_three_days() {
        let dir = tempfile::tempdir().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(ETAG, HeaderValue::from_static("\"v1\""));
        let check = Check::from_headers(&headers, 1_000);
        write(dir.path(), "https://c/a.jpg", &check);
        assert_eq!(read(dir.path(), "https://c/a.jpg"), Some(check.clone()));
        assert_eq!(read(dir.path(), "https://c/b.jpg"), None);
        assert!(!check.due(1_000 + RECHECK_AFTER_MS - 1));
        assert!(check.due(1_000 + RECHECK_AFTER_MS));
        let asked = check.conditional(&HeaderMap::new());
        assert_eq!(asked.get(IF_NONE_MATCH).unwrap(), "\"v1\"");
        assert!(asked.get(IF_MODIFIED_SINCE).is_none(), "only the validators the site gave");
    }
}
