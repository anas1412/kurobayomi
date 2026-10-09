//! The paging contract of a remote listing (T-09a-060): pages are read in order, one `load` per page, and the listing
//! ends when the source names no next page or when a page comes back empty (E-SRC-10). A failed page is returned as it
//! is and the cursor stays on it, so a retry asks for the same page again (E-SRC-04 surfaces as
//! `SourceNotInstalled` on the first page).

use crate::error::DomainError;
use crate::source::repo::PageSource;

/// Reads one listing page by page through a [`PageSource`].
pub struct PageCursor<'a, T> {
    source: &'a dyn PageSource<T>,
    /// The page the next call requests, or `None` once the listing has ended.
    next: Option<i64>,
}

impl<'a, T> PageCursor<'a, T> {
    /// A cursor at page 1 (Kotlin `PagingSource` keys start at 1).
    pub fn new(source: &'a dyn PageSource<T>) -> Self {
        Self { source, next: Some(1) }
    }

    /// The items of the next page, or `None` when the listing is over. An empty page ends the listing even when the
    /// source names a next page, so a misbehaving source cannot loop the caller forever.
    pub fn next_page(&mut self) -> Result<Option<Vec<T>>, DomainError> {
        let Some(page) = self.next else {
            return Ok(None);
        };
        let (items, next) = self.source.load(page)?;
        if items.is_empty() {
            self.next = None;
            return Ok(None);
        }
        self.next = next;
        Ok(Some(items))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    /// Serves fixed pages and records the page numbers asked for.
    struct Pages {
        pages: HashMap<i64, Result<(Vec<&'static str>, Option<i64>), &'static str>>,
        asked: Mutex<Vec<i64>>,
    }

    impl Pages {
        fn new(pages: Vec<(i64, Result<(Vec<&'static str>, Option<i64>), &'static str>)>) -> Self {
            Self { pages: pages.into_iter().collect(), asked: Mutex::new(Vec::new()) }
        }
    }

    impl PageSource<&'static str> for Pages {
        fn load(&self, page: i64) -> Result<(Vec<&'static str>, Option<i64>), DomainError> {
            self.asked.lock().expect("lock").push(page);
            match self.pages.get(&page) {
                Some(Ok(p)) => Ok(p.clone()),
                Some(Err(_)) => Err(DomainError::SourceNotInstalled("Gone (EN)".into())),
                None => Ok((Vec::new(), None)),
            }
        }
    }

    #[test]
    fn pages_are_read_in_order_until_no_next_page_is_named() {
        let source = Pages::new(vec![(1, Ok((vec!["a", "b"], Some(2)))), (2, Ok((vec!["c"], None)))]);
        let mut cursor = PageCursor::new(&source);
        assert_eq!(cursor.next_page().expect("p1"), Some(vec!["a", "b"]));
        assert_eq!(cursor.next_page().expect("p2"), Some(vec!["c"]));
        assert_eq!(cursor.next_page().expect("end"), None);
        assert_eq!(cursor.next_page().expect("still end"), None);
        assert_eq!(*source.asked.lock().expect("lock"), vec![1, 2], "no request after the last page");
    }

    #[test]
    fn an_empty_page_ends_the_listing_even_with_a_next_page() {
        let source = Pages::new(vec![(1, Ok((Vec::new(), Some(2)))), (2, Ok((vec!["never"], None)))]);
        let mut cursor = PageCursor::new(&source);
        assert_eq!(cursor.next_page().expect("empty"), None, "E-SRC-10");
        assert_eq!(cursor.next_page().expect("after"), None);
        assert_eq!(*source.asked.lock().expect("lock"), vec![1], "page 2 is never asked");
    }

    #[test]
    fn a_failing_first_page_is_returned_and_retried_on_the_same_page() {
        let source = Pages::new(vec![(1, Err("uninstalled")), (2, Ok((vec!["x"], None)))]);
        let mut cursor = PageCursor::new(&source);
        assert!(matches!(cursor.next_page(), Err(DomainError::SourceNotInstalled(name)) if name == "Gone (EN)"));
        assert!(matches!(cursor.next_page(), Err(DomainError::SourceNotInstalled(_))));
        assert_eq!(*source.asked.lock().expect("lock"), vec![1, 1], "the failed page is asked again, not skipped");
    }
}
