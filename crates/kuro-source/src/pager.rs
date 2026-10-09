//! Paging state of a remote listing (T-06a-042): which page the next load requests, when an empty page
//! is an error, and what a refresh resets. The source call is the caller's: `append` takes it as a
//! closure, so this type stays free of HTTP. Dropping the pager is what cancels a request in the UI;
//! a synchronous call has nothing to cancel here.

/// Rows the list keeps loaded ahead of the viewport. UI contract for the listing view.
pub const PREFETCH_DISTANCE: usize = 25;

/// One page of a listing: its items and whether the source reports a page after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub has_next_page: bool,
}

/// Why an `append` loaded nothing. `Source` carries the error of the source call unchanged.
#[derive(Debug, PartialEq, Eq)]
pub enum PagerError<E> {
    /// The page had no items. The page number is kept, so a later `append` asks for it again.
    NoResults,
    Source(E),
}

/// Loads one listing page by page. `next_key` is the page the next `append` requests, or `None` once
/// the last page is loaded.
#[derive(Debug)]
pub struct RemotePager<T> {
    next_key: Option<i64>,
    items: Vec<T>,
}

impl<T> RemotePager<T> {
    /// A pager that has loaded nothing and requests page 1 first.
    pub fn new() -> Self {
        Self { next_key: Some(1), items: Vec::new() }
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn next_key(&self) -> Option<i64> {
        self.next_key
    }

    /// Requests `next_key` through `fetch` and appends its items. Does nothing when no page is left.
    /// On a source error or an empty page the page number stays, so the next call retries it.
    pub fn append<E>(&mut self, fetch: impl FnOnce(i64) -> Result<Page<T>, E>) -> Result<(), PagerError<E>> {
        let Some(page) = self.next_key else {
            return Ok(());
        };
        let loaded = fetch(page).map_err(PagerError::Source)?;
        if loaded.items.is_empty() {
            return Err(PagerError::NoResults);
        }
        self.next_key = loaded.has_next_page.then_some(page + 1);
        self.items.extend(loaded.items);
        Ok(())
    }

    /// Drops every loaded item and restarts at page 1.
    pub fn refresh(&mut self) {
        self.items.clear();
        self.next_key = Some(1);
    }
}

impl<T> Default for RemotePager<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(items: &[&'static str], has_next_page: bool) -> Result<Page<&'static str>, ()> {
        Ok(Page { items: items.to_vec(), has_next_page })
    }

    #[test]
    fn the_first_load_requests_page_one() {
        let mut pager = RemotePager::new();
        let mut asked = Vec::new();
        pager
            .append(|p| {
                asked.push(p);
                page(&["a"], true)
            })
            .unwrap();
        assert_eq!(asked, vec![1]);
        assert_eq!(pager.items(), &["a"]);
    }

    #[test]
    fn next_key_moves_on_only_while_the_source_reports_more_pages() {
        let mut pager = RemotePager::new();
        pager.append(|_| page(&["a"], true)).unwrap();
        assert_eq!(pager.next_key(), Some(2));
        pager.append(|_| page(&["b"], false)).unwrap();
        assert_eq!(pager.next_key(), None);
        let mut called = false;
        pager
            .append(|_| {
                called = true;
                page(&["c"], false)
            })
            .unwrap();
        assert!(!called, "no request once the last page is loaded");
        assert_eq!(pager.items(), &["a", "b"]);
    }

    #[test]
    fn an_empty_page_is_no_results_on_any_page_and_keeps_the_page_number() {
        let mut pager = RemotePager::new();
        pager.append(|_| page(&["a"], true)).unwrap();
        assert_eq!(pager.append(|_| page(&[], false)), Err(PagerError::NoResults));
        assert_eq!(pager.next_key(), Some(2));
        assert_eq!(pager.items(), &["a"]);
    }

    #[test]
    fn a_source_error_keeps_next_key_so_the_same_page_is_retried() {
        let mut pager: RemotePager<&str> = RemotePager::new();
        assert_eq!(pager.append(|_| Err::<Page<&str>, _>("offline")), Err(PagerError::Source("offline")));
        assert_eq!(pager.next_key(), Some(1));
        let mut asked = Vec::new();
        pager
            .append(|p| {
                asked.push(p);
                page(&["a"], false)
            })
            .unwrap();
        assert_eq!(asked, vec![1]);
    }

    #[test]
    fn refresh_restarts_at_page_one_and_drops_the_items() {
        let mut pager = RemotePager::new();
        pager.append(|_| page(&["a"], true)).unwrap();
        pager.refresh();
        assert!(pager.items().is_empty());
        assert_eq!(pager.next_key(), Some(1));
    }
}
