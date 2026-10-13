//! The tracker search screen (T-38b-035, T-39b-044; p2-rust-design 5.3, E-TRK-8, E-TRK-9, T-TRK-5, T-TRK-6). The states are
//! Idle, Loading, Done (the results and the selection) and Failed (the text). A result is identified by
//! `(tracker id, remote id)`; the private flag is not part of its identity.

/// What a search result needs to offer the screen.
pub trait Hit: Clone {
    /// `(tracker id, remote id)`.
    fn key(&self) -> (i64, i64);
    /// The tracker page. Empty when the tracker set none (`T-TRK-6`: such a result is an error row).
    fn url(&self) -> &str;
    fn set_private(&mut self, private: bool);
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub enum State<R> {
    Idle,
    Loading,
    /// The results, and the key of the selected one. `results` may be empty: the screen then says "No results found".
    Done { results: Vec<R>, selected: Option<(i64, i64)> },
    /// The message. A failure with no message reads "Unknown error".
    Failed(String),
}

#[cfg(test)]
/// How a query ended. A cancelled query is not an error (E-TRK-8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Cancelled,
    Error(String),
}

#[cfg(test)]
/// One search screen for one entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Search<R: Hit> {
    pub query: String,
    /// The link the entry is bound to now. The result with the same URL is preselected (`T-TRK-5`).
    current_url: Option<String>,
    supports_private: bool,
    state: State<R>,
    /// Counts the queries started. Only the latest one may write results (D-6).
    generation: u64,
}

#[cfg(test)]
impl<R: Hit> Search<R> {
    pub fn new(initial_title: &str, current_url: Option<&str>, supports_private: bool) -> Self {
        Self {
            query: initial_title.to_owned(),
            current_url: current_url.filter(|u| !u.trim().is_empty()).map(str::to_owned),
            supports_private,
            state: State::Idle,
            generation: 0,
        }
    }

    /// The query to run on open: the title, when it is not blank.
    pub fn auto_query(&self) -> Option<String> {
        (!self.query.trim().is_empty()).then(|| self.query.clone())
    }

    /// Starts a query. A blank query starts nothing and returns `None`. Otherwise the state goes to Loading and the
    /// generation to pass to [`Search::finish`] is returned.
    pub fn start(&mut self, text: &str) -> Option<u64> {
        if text.trim().is_empty() {
            return None;
        }
        self.generation += 1;
        self.state = State::Loading;
        Some(self.generation)
    }

    /// Applies the result of the query `generation`. A result of an earlier query is dropped (D-6) and `false` is
    /// returned.
    pub fn finish(&mut self, generation: u64, result: Result<Vec<R>, Failure>) -> bool {
        if generation != self.generation {
            return false;
        }
        match result {
            Err(Failure::Cancelled) => {}
            Err(Failure::Error(text)) => {
                self.state = State::Failed(if text.is_empty() { "Unknown error".to_owned() } else { text });
            }
            Ok(results) => {
                let selected = self.current_url.as_deref().and_then(|url| {
                    results.iter().find(|r| r.url() == url).map(|r| r.key())
                });
                self.state = State::Done { results, selected };
            }
        }
        true
    }

    pub fn state(&self) -> &State<R> {
        &self.state
    }

    /// Selects the result with `key`, when the results hold it. An error row (no URL) cannot be selected (T-TRK-6, D-8).
    pub fn select(&mut self, key: (i64, i64)) {
        if let State::Done { results, selected } = &mut self.state {
            if results.iter().any(|r| r.key() == key && !is_error_row(r)) {
                *selected = Some(key);
            }
        }
    }

    pub fn selected(&self) -> Option<&R> {
        match &self.state {
            State::Done { results, selected: Some(key) } => results.iter().find(|r| r.key() == *key),
            _ => None,
        }
        .filter(|hit| !is_error_row(*hit))
    }

    /// Confirm: the selected result, with the private flag written onto it when the tracker supports private tracking.
    pub fn confirm(&self, private: bool) -> Option<R> {
        let mut hit = self.selected()?.clone();
        hit.set_private(self.supports_private && private);
        Some(hit)
    }

    /// The private option shows only when the tracker supports private tracking.
    pub fn private_option_shown(&self) -> bool {
        self.supports_private
    }

    pub fn shows_no_results(&self) -> bool {
        matches!(&self.state, State::Done { results, .. } if results.is_empty())
    }
}

#[cfg(test)]
/// A result with no tracker URL is an error row (`T-TRK-6`, D-8): it cannot be opened, and it is not preselected.
pub fn is_error_row<R: Hit>(hit: &R) -> bool {
    hit.url().trim().is_empty()
}

#[cfg(test)]
/// Two results are the same result when their identities are equal, whatever their private flag says (E-TRK-9).
pub fn same_result<R: Hit>(a: &R, b: &R) -> bool {
    a.key() == b.key()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Hitt {
        tracker: i64,
        remote: i64,
        url: String,
        private: bool,
    }

    impl Hit for Hitt {
        fn key(&self) -> (i64, i64) {
            (self.tracker, self.remote)
        }
        fn url(&self) -> &str {
            &self.url
        }
        fn set_private(&mut self, private: bool) {
            self.private = private;
        }
    }

    fn hit(tracker: i64, remote: i64, url: &str) -> Hitt {
        Hitt { tracker, remote, url: url.into(), private: false }
    }

    #[test]
    fn the_title_runs_on_open_only_when_it_is_not_blank() {
        assert_eq!(Search::<Hitt>::new("Naruto", None, true).auto_query(), Some("Naruto".into()));
        assert_eq!(Search::<Hitt>::new("  ", None, true).auto_query(), None);
    }

    #[test]
    fn preselect_the_result_whose_url_is_the_current_link() {
        let mut s = Search::new("x", Some("https://t/2"), true);
        let g = s.start("x").unwrap();
        s.finish(g, Ok(vec![hit(2, 1, "https://t/1"), hit(2, 2, "https://t/2")]));
        assert_eq!(s.selected().map(Hit::key), Some((2, 2)));
        let mut none = Search::new("x", None, true);
        let g = none.start("x").unwrap();
        none.finish(g, Ok(vec![hit(2, 1, "https://t/1")]));
        assert!(none.selected().is_none(), "no current link, no preselection (T-TRK-5)");
    }

    #[test]
    fn a_blank_query_starts_nothing_and_loading_is_shown_until_the_result_arrives() {
        let mut s = Search::<Hitt>::new("", None, true);
        assert_eq!(s.start("  "), None);
        assert_eq!(*s.state(), State::Idle);
        let g = s.start("ab").unwrap();
        assert_eq!(*s.state(), State::Loading);
        s.finish(g, Ok(vec![]));
        assert!(s.shows_no_results(), "zero results shows the no-results text (E-TRK-8)");
    }

    #[test]
    fn only_the_latest_query_writes_results() {
        let mut s = Search::<Hitt>::new("", None, true);
        let old = s.start("a").unwrap();
        let new = s.start("ab").unwrap();
        assert!(!s.finish(old, Ok(vec![hit(2, 9, "u")])), "an older query is dropped (D-6)");
        assert_eq!(*s.state(), State::Loading);
        assert!(s.finish(new, Ok(vec![])));
    }

    #[test]
    fn cancellation_is_not_an_error_and_an_empty_message_reads_unknown() {
        let mut s = Search::<Hitt>::new("", None, true);
        let g = s.start("a").unwrap();
        s.finish(g, Err(Failure::Cancelled));
        assert_eq!(*s.state(), State::Loading, "a cancel leaves no error behind");
        s.finish(g, Err(Failure::Error(String::new())));
        assert_eq!(*s.state(), State::Failed("Unknown error".into()));
    }

    #[test]
    fn a_result_without_a_url_is_an_error_row() {
        assert!(is_error_row(&hit(2, 1, "")));
        assert!(!is_error_row(&hit(2, 1, "https://t/1")));
    }

    #[test]
    fn an_error_row_can_be_neither_selected_nor_confirmed() {
        let mut s = Search::new("x", Some(""), true);
        let g = s.start("x").unwrap();
        s.finish(g, Ok(vec![hit(2, 1, ""), hit(2, 2, "https://t/2")]));
        s.select((2, 1));
        assert!(s.selected().is_none(), "the row with no URL stays unselected (T-TRK-6)");
        assert!(s.confirm(false).is_none());
        s.select((2, 2));
        assert_eq!(s.confirm(false).map(|h| h.remote), Some(2));
    }

    #[test]
    fn result_identity_ignores_the_private_flag() {
        let a = hit(2, 7, "u");
        let mut b = hit(2, 7, "other");
        b.private = true;
        assert!(same_result(&a, &b), "(tracker, remote) is the identity (E-TRK-9)");
        assert!(!same_result(&a, &hit(3, 7, "u")), "another tracker is another result");
    }

    #[test]
    fn confirm_writes_private_only_when_supported_and_then_closes() {
        let mut s = Search::new("", None, false);
        let g = s.start("a").unwrap();
        s.finish(g, Ok(vec![hit(2, 1, "u")]));
        s.select((2, 1));
        assert_eq!(s.confirm(true).map(|h| h.private), Some(false), "no private option, no private write");
        let mut p = Search::new("", None, true);
        let g = p.start("a").unwrap();
        p.finish(g, Ok(vec![hit(2, 1, "u")]));
        p.select((2, 1));
        assert_eq!(p.confirm(true).map(|h| h.private), Some(true));
        assert!(p.private_option_shown());
    }
}
