//! What the page needs from outside its own database: the source name, the two fetches, the download states and
//! the download removal. The fetches persist their result (as `updateManga.awaitUpdateFromSource` does), so the
//! page sees the new rows through the store subscriptions. Nothing here imports GTK.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use kuro_data::source::{MangaStubSourceRepo, StubSource};
use kuro_db::handle::Db;
use kuro_downloads::DownloadState;

use crate::entry_core::detail::SourceInfo;
use crate::entry_core::errors::DetailError;

pub type Fetch = Pin<Box<dyn Future<Output = Result<(), DetailError>> + Send>>;

pub trait Backend: Send + Sync + 'static {
    /// The source line of the header. `stub` is true when the extension is not installed.
    fn source_info(&self, source_id: i64) -> SourceInfo;
    /// Fetches the details from the source and stores them.
    fn fetch_details(&self, manga_id: i64) -> Fetch;
    /// Fetches the chapter list from the source and stores it.
    fn fetch_chapters(&self, manga_id: i64) -> Fetch;
    /// The download state of each chapter of the manga that has one.
    fn downloads(&self, _manga_id: i64) -> HashMap<i64, DownloadState> {
        HashMap::new()
    }
    /// Removes the downloads of these chapters (after a mark-read with "remove after read").
    fn delete_downloads(&self, _manga_id: i64, _chapter_ids: &[i64]) {}
    /// Queues these chapters for download. With `start_now` the one chapter goes to the front and the queue starts.
    /// `false` when the downloads layer is not there, or refused: nothing was queued.
    fn enqueue_downloads(&self, _manga_id: i64, _chapter_ids: &[i64], _start_now: bool) -> bool {
        false
    }
    /// Takes one chapter out of the download queue.
    fn cancel_download(&self, _chapter_id: i64) {}
    /// Binds the enhanced trackers of the source to a manga that just joined the library (`bindEnhancedTrackers`).
    fn bind_enhanced(&self, _manga_id: i64) {}
    /// Whether a tracker is logged in: the tracking button opens the tracking sheet then, the tracking settings
    /// otherwise (E-25). The app has no tracker session yet, so the default is no.
    fn has_logged_in_tracker(&self) -> bool {
        false
    }
    /// Whether the source is an HTTP source: only then does the page offer WebView and Share.
    fn is_http_source(&self, _source_id: i64) -> bool {
        false
    }
    /// Deletes the cover files of a manga that just left the library (`removeCovers`), after the write succeeded.
    /// `true` when a file was removed.
    fn remove_cover(&self, _manga_id: i64) -> bool {
        false
    }
}

/// The backend of an app with no source registry yet: every source reads as not installed, the local source reads
/// as installed and has nothing to fetch.
pub struct StubBackend {
    stubs: MangaStubSourceRepo,
}

impl StubBackend {
    pub fn new(db: Arc<Db>) -> Self {
        Self {
            stubs: MangaStubSourceRepo::new(db),
        }
    }
}

const LOCAL_SOURCE_ID: i64 = kuro_source::local::LOCAL_SOURCE_ID;

impl Backend for StubBackend {
    fn source_info(&self, source_id: i64) -> SourceInfo {
        if source_id == LOCAL_SOURCE_ID {
            return SourceInfo {
                name: kuro_source::local::LOCAL_MANGA_NAME.to_owned(),
                stub: false,
            };
        }
        let stub = self
            .stubs
            .get(source_id)
            .ok()
            .flatten()
            .unwrap_or_else(|| StubSource::fallback(source_id));
        SourceInfo {
            name: stub.display_name(),
            stub: true,
        }
    }

    fn fetch_details(&self, manga_id: i64) -> Fetch {
        self.fetch(manga_id)
    }

    fn fetch_chapters(&self, manga_id: i64) -> Fetch {
        self.fetch(manga_id)
    }
}

impl StubBackend {
    fn fetch(&self, _manga_id: i64) -> Fetch {
        Box::pin(async { Err(DetailError::SourceNotInstalled) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_of(path: &std::path::Path) -> Arc<Db> {
        let writer = kuro_db::schema_manga::open(path).expect("schema");
        Arc::new(Db::new(path, writer).expect("db"))
    }

    #[test]
    fn a_known_stub_shows_its_name_and_an_unknown_one_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = db_of(&dir.path().join("manga.db"));
        MangaStubSourceRepo::new(Arc::clone(&db)).upsert(7, "en", "Reader").unwrap();
        let backend = StubBackend::new(db);
        assert_eq!(
            backend.source_info(7),
            SourceInfo {
                name: "Reader (EN)".into(),
                stub: true
            }
        );
        assert_eq!(backend.source_info(99).name, "99");
        assert!(backend.source_info(99).stub);
    }

    #[test]
    fn the_local_source_is_not_a_stub_and_fetches_report_not_installed() {
        let dir = tempfile::tempdir().unwrap();
        let backend = StubBackend::new(db_of(&dir.path().join("manga.db")));
        let local = backend.source_info(LOCAL_SOURCE_ID);
        assert!(!local.stub);
        assert!(!local.name.is_empty());
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        assert_eq!(rt.block_on(backend.fetch_details(1)), Err(DetailError::SourceNotInstalled));
    }
}
