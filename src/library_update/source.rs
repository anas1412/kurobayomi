//! The source side of a pass: what a source returns for an entry, and the stub used when the source is not
//! installed (`get_or_stub`). Metadata passes call `SourceRegistry::get`, which yields `None` for a missing source.

use std::future::Future;

use kuro_domain::kind::EntryKind;

use crate::library_update::error::UpdateError;

/// One chapter or episode as a source lists it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceItem {
    pub url: String,
    pub name: String,
    /// Chapter or episode number; a negative value is an unrecognised number.
    pub number: f64,
    pub date_upload_ms: i64,
    /// Position on the source, newest first (`0` is the newest).
    pub source_order: i64,
    pub scanlator: Option<String>,
}

/// The metadata a source returns with its item list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceDetails {
    pub title: Option<String>,
    pub status: i64,
    /// The cover url; blank or missing never clears a stored cover (behaviour 5.3).
    pub thumbnail_url: Option<String>,
}

/// What a details refresh asks of a source: the details, the item list, or both (`UpdateAnimeFromRemote`'s
/// `fetch_details` and `fetch_episodes`). A metadata pass asks for details only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteFlags {
    pub fetch_details: bool,
    pub fetch_items: bool,
}

impl RemoteFlags {
    /// The metadata passes: `getMangaDetails`, and for anime `fetch_details = true, fetch_episodes = false`.
    pub const DETAILS_ONLY: Self = Self { fetch_details: true, fetch_items: false };
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fetched {
    pub details: Option<SourceDetails>,
    pub items: Vec<SourceItem>,
}

pub trait Source: Send + Sync {
    /// Local sources may return no items without an error.
    fn is_local(&self) -> bool;
    /// The `UnmeteredSource` marker: exempt from the large-update warning.
    fn is_unmetered(&self) -> bool;
    /// The entry's details (when `with_details`) and its chapter or episode list.
    fn fetch(
        &self,
        kind: EntryKind,
        url: &str,
        with_details: bool,
    ) -> impl Future<Output = Result<Fetched, UpdateError>> + Send;
    /// The metadata call of the manual metadata passes: only the parts `flags` asks for are fetched, so with
    /// `DETAILS_ONLY` the chapter or episode list endpoint is never called. `None` when the source returned no details.
    fn refresh(
        &self,
        kind: EntryKind,
        url: &str,
        flags: RemoteFlags,
    ) -> impl Future<Output = Result<Option<SourceDetails>, UpdateError>> + Send;
}

pub trait SourceRegistry: Send + Sync {
    type Source: Source;

    /// The installed source, or `None`.
    fn get(&self, source_id: i64) -> Option<Self::Source>;

    /// The installed source, or a slot whose calls fail with `SourceNotInstalled`.
    fn get_or_stub(&self, source_id: i64) -> SourceSlot<Self::Source> {
        match self.get(source_id) {
            Some(source) => SourceSlot::Installed(source),
            None => SourceSlot::Missing,
        }
    }
}

pub enum SourceSlot<S> {
    Installed(S),
    Missing,
}

impl<S: Source> SourceSlot<S> {
    pub async fn fetch(&self, kind: EntryKind, url: &str, with_details: bool) -> Result<Fetched, UpdateError> {
        match self {
            Self::Installed(source) => source.fetch(kind, url, with_details).await,
            Self::Missing => Err(UpdateError::SourceNotInstalled),
        }
    }

    pub fn is_local(&self) -> bool {
        match self {
            Self::Installed(source) => source.is_local(),
            Self::Missing => false,
        }
    }

    pub fn is_unmetered(&self) -> bool {
        match self {
            Self::Installed(source) => source.is_unmetered(),
            Self::Missing => false,
        }
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    /// A source that answers from a fixed script and counts its calls.
    #[derive(Clone)]
    pub struct FakeSource {
        pub reply: Result<Fetched, UpdateError>,
        pub local: bool,
        pub calls: Arc<AtomicUsize>,
        pub active: Arc<AtomicUsize>,
        pub max_active: Arc<AtomicUsize>,
        /// The flags of every `refresh` call, in order.
        pub refreshes: Arc<std::sync::Mutex<Vec<RemoteFlags>>>,
        /// What `refresh` answers.
        pub details: Result<Option<SourceDetails>, UpdateError>,
    }

    impl FakeSource {
        pub fn ok(items: Vec<SourceItem>) -> Self {
            Self::new(Ok(Fetched { details: None, items }))
        }

        pub fn err(error: UpdateError) -> Self {
            Self::new(Err(error))
        }

        fn new(reply: Result<Fetched, UpdateError>) -> Self {
            Self {
                reply,
                local: false,
                calls: Arc::default(),
                active: Arc::default(),
                max_active: Arc::default(),
                refreshes: Arc::default(),
                details: Ok(None),
            }
        }

        pub fn with_details(mut self, details: SourceDetails) -> Self {
            self.details = Ok(Some(details));
            self
        }

        pub fn failing_refresh(mut self, error: UpdateError) -> Self {
            self.details = Err(error);
            self
        }
    }

    impl Source for FakeSource {
        fn is_local(&self) -> bool {
            self.local
        }

        fn is_unmetered(&self) -> bool {
            false
        }

        async fn fetch(&self, _kind: EntryKind, _url: &str, _with_details: bool) -> Result<Fetched, UpdateError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(now, Ordering::SeqCst);
            // Yield a few times so overlapping sources are observable.
            for _ in 0..5 {
                tokio::task::yield_now().await;
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            self.reply.clone()
        }

        async fn refresh(&self, _kind: EntryKind, _url: &str, flags: RemoteFlags) -> Result<Option<SourceDetails>, UpdateError> {
            self.refreshes.lock().unwrap().push(flags);
            self.details.clone()
        }
    }

    #[derive(Default)]
    pub struct FakeRegistry {
        pub sources: HashMap<i64, FakeSource>,
    }

    impl SourceRegistry for FakeRegistry {
        type Source = FakeSource;

        fn get(&self, source_id: i64) -> Option<FakeSource> {
            self.sources.get(&source_id).cloned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{FakeRegistry, FakeSource};
    use super::*;

    #[tokio::test]
    async fn a_missing_source_stub_fails_with_source_not_installed() {
        // P-05
        let registry = FakeRegistry::default();
        let slot = registry.get_or_stub(9999);
        let result = slot.fetch(EntryKind::Manga, "/x", false).await;
        assert_eq!(result, Err(UpdateError::SourceNotInstalled));
        assert_eq!(UpdateError::SourceNotInstalled.user_message(), "Source not found");
    }

    #[test]
    fn metadata_lookup_is_none_for_a_missing_source() {
        // T-26a-053 relies on `get` returning None for a missing source.
        let registry = FakeRegistry::default();
        assert!(registry.get(42).is_none());
        assert!(matches!(registry.get_or_stub(42), SourceSlot::Missing));
    }

    #[tokio::test]
    async fn an_installed_source_answers_and_reports_unmetered() {
        // Q-18: the flag is read from the source.
        let mut registry = FakeRegistry::default();
        registry.sources.insert(1001, FakeSource::ok(vec![SourceItem { name: "c1".into(), ..SourceItem::default() }]));
        let slot = registry.get_or_stub(1001);
        assert!(!slot.is_unmetered());
        let fetched = slot.fetch(EntryKind::Manga, "/x", false).await.expect("installed source answers");
        assert_eq!(fetched.items.len(), 1);
        assert!(!SourceSlot::<FakeSource>::Missing.is_local());
    }
}
