//! `Page`: one image of a chapter, with its live state and progress as `watch` channels (`Page.kt`, T-15a-005).

use std::path::PathBuf;

use tokio::sync::watch;

/// Where a page is in its life cycle. The ordinals are the declaration order, 0 to 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum PageState {
    Queue = 0,
    LoadPage = 1,
    DownloadImage = 2,
    Ready = 3,
    Error = 4,
}

/// The observable state of one page. A receiver made later sees the latest value.
#[derive(Debug)]
pub struct PageStatus {
    pub state: watch::Sender<PageState>,
    /// Percent of the image read, or -1 when the total is unknown.
    pub progress: watch::Sender<i32>,
}

#[derive(Debug)]
pub struct Page {
    /// Assigned by the app from the position in the list; a source's value is never trusted.
    pub index: i32,
    pub url: String,
    pub image_url: Option<String>,
    /// A local file for a downloaded or local page. Not part of the cache shape.
    pub local_path: Option<PathBuf>,
    pub status: PageStatus,
}

impl Page {
    /// A page in the `Queue` state with progress 0.
    pub fn new(index: i32, url: impl Into<String>) -> Self {
        Self {
            index,
            url: url.into(),
            image_url: None,
            local_path: None,
            status: PageStatus { state: watch::channel(PageState::Queue).0, progress: watch::channel(0).0 },
        }
    }

    /// The 1-based page number shown to the user.
    pub fn number(&self) -> i32 {
        self.index + 1
    }

    /// Progress in percent: `100 * read / total` with integer division, computed in u128 and capped at 100.
    /// An absent or zero total gives -1 (EC-MD-06..09).
    pub fn update_progress(&self, bytes_read: u64, content_length: Option<u64>) -> i32 {
        match content_length {
            Some(total) if total > 0 => {
                let percent = u128::from(bytes_read) * 100 / u128::from(total);
                i32::try_from(percent.min(100)).unwrap_or(100)
            }
            _ => -1,
        }
    }

    /// Sets the state. Works with no receiver attached.
    pub fn set_state(&self, state: PageState) {
        self.status.state.send_replace(state);
    }

    /// Sets the progress value. Works with no receiver attached.
    pub fn set_progress(&self, progress: i32) {
        self.status.progress.send_replace(progress);
    }
}

/// The page cache shape (`ChapterCache`, T-15a-038): only `index`, `url` and `imageUrl` are written. The live watch
/// channels and the local file are never cached.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageCacheEntry {
    pub index: i32,
    #[serde(default)]
    pub url: String,
    #[serde(rename = "imageUrl", default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

impl From<&Page> for PageCacheEntry {
    fn from(page: &Page) -> Self {
        Self { index: page.index, url: page.url.clone(), image_url: page.image_url.clone() }
    }
}

impl PageCacheEntry {
    /// A fresh page in the `Queue` state with the cached URLs.
    pub fn into_page(self) -> Page {
        Page { image_url: self.image_url, ..Page::new(self.index, self.url) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_page_is_queued_at_zero_and_numbered_from_one() {
        let page = Page::new(4, "/p/5");
        assert_eq!(*page.status.state.borrow(), PageState::Queue);
        assert_eq!(*page.status.progress.borrow(), 0);
        assert_eq!(page.number(), 5);
        assert!(page.image_url.is_none() && page.local_path.is_none());
    }

    #[test]
    fn progress_follows_the_integer_division_rules() {
        let page = Page::new(0, "");
        assert_eq!(page.update_progress(50, Some(0)), -1);
        assert_eq!(page.update_progress(50, None), -1);
        assert_eq!(page.update_progress(50, Some(200)), 25);
        assert_eq!(page.update_progress(1, Some(3)), 33);
        assert_eq!(page.update_progress(u64::MAX, Some(u64::MAX)), 100);
        assert_eq!(page.update_progress(u64::MAX, Some(1)), 100);
    }

    #[test]
    fn a_receiver_created_after_a_change_sees_the_latest_value() {
        let page = Page::new(0, "");
        page.set_state(PageState::Ready);
        page.set_progress(100);
        let state = page.status.state.subscribe();
        let progress = page.status.progress.subscribe();
        assert_eq!(*state.borrow(), PageState::Ready);
        assert_eq!(*progress.borrow(), 100);
    }

    #[test]
    fn an_existing_receiver_is_woken_by_a_change() {
        let page = Page::new(0, "");
        let mut rx = page.status.state.subscribe();
        page.set_state(PageState::LoadPage);
        assert!(rx.has_changed().unwrap());
        assert_eq!(*rx.borrow_and_update(), PageState::LoadPage);
    }

    #[test]
    fn page_state_ordinals_follow_the_declaration_order() {
        let ordinals =
            [PageState::Queue, PageState::LoadPage, PageState::DownloadImage, PageState::Ready, PageState::Error]
                .map(|s| s as i32);
        assert_eq!(ordinals, [0, 1, 2, 3, 4]);
    }

    #[test]
    fn the_cache_shape_keeps_only_index_url_and_image_url() {
        let mut page = Page::new(3, "/ch/4/p/3");
        page.image_url = Some("https://img.test/3.jpg".into());
        page.local_path = Some(PathBuf::from("/tmp/3.jpg"));
        let json = serde_json::to_string(&PageCacheEntry::from(&page)).unwrap();
        assert_eq!(json, r#"{"index":3,"url":"/ch/4/p/3","imageUrl":"https://img.test/3.jpg"}"#);
    }

    #[test]
    fn a_missing_image_url_is_not_written_and_reads_back_as_none() {
        let json = serde_json::to_string(&PageCacheEntry::from(&Page::new(0, "/p"))).unwrap();
        assert_eq!(json, r#"{"index":0,"url":"/p"}"#);
        let entry: PageCacheEntry = serde_json::from_str(r#"{"index":2}"#).unwrap();
        assert_eq!(entry.url, "");
        assert_eq!(entry.image_url, None);
    }

    #[test]
    fn a_cache_entry_becomes_a_queued_page_again() {
        let entry: PageCacheEntry = serde_json::from_str(r#"{"index":1,"url":"u","imageUrl":"i"}"#).unwrap();
        let page = entry.into_page();
        assert_eq!((page.index, page.url.as_str(), page.image_url.as_deref()), (1, "u", Some("i")));
        assert_eq!(*page.status.state.borrow(), PageState::Queue);
    }
}
