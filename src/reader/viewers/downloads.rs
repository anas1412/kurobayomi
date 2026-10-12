//! The download index as the viewers see it (T-43b-032, T-44a-066). A transition card shows whether its destination
//! chapter is downloaded. A local manga is downloaded by definition and is never looked up; every other manga asks the
//! index, off the GTK thread. `Default` is a manga with nothing downloaded, for a viewer whose app has no index.

use std::sync::Arc;

use kuro_data::item::Item;

/// Looks a chapter up in the download index. Cheap to clone, and `Send + Sync` because the lookup runs on a worker.
#[derive(Clone)]
pub struct DownloadCheck {
    pub manga_is_local: bool,
    pub lookup: Arc<dyn Fn(&Item) -> bool + Send + Sync>,
}

impl Default for DownloadCheck {
    fn default() -> Self {
        Self { manga_is_local: false, lookup: Arc::new(|_| false) }
    }
}

impl DownloadCheck {
    pub fn new(manga_is_local: bool, lookup: Arc<dyn Fn(&Item) -> bool + Send + Sync>) -> Self {
        Self { manga_is_local, lookup }
    }
}
