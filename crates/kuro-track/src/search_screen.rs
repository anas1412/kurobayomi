//! Logic of the track search screen (`AnimeTrackerSearch.kt`, `MangaTrackerSearch.kt`, behaviour section 15). The
//! GUI draws it; this holds the query rules, the body state, the selection, the bottom bar and the result card.

use crate::error::TrackError;
use crate::messages::MsgKey;
use crate::model::TrackerId;
use crate::model::search::{AnimeTrackSearch, MangaTrackSearch};

/// What the screen needs from a search result. Both kinds implement it.
pub trait SearchHit: Clone {
    /// The list key `(tracker, remote id)`. Two results of different trackers never collide (D10).
    fn list_key(&self) -> (TrackerId, i64);
    fn title(&self) -> &str;
    /// Empty when the tracker did not set one (D9). The open action is then disabled.
    fn tracking_url(&self) -> &str;
    fn card(&self) -> CardView;
    fn set_private(&mut self, private: bool);
}

/// One detail line of a card: a label key with its English text, and the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailLine {
    pub label_key: &'static str,
    pub label: &'static str,
    pub value: String,
}

/// Everything a result card shows. Empty optional parts are `None` or absent (behaviour 15.4).
#[derive(Debug, Clone, PartialEq)]
pub struct CardView {
    pub title: String,
    /// Authors and artists, distinct, joined with ", ". `None` when both lists are empty.
    pub byline: Option<String>,
    /// Type, Started, Status and Score, in that order. Only the lines with a value are present.
    pub details: Vec<DetailLine>,
    /// The summary trimmed, at most drawn as four lines by the GUI. `None` when blank.
    pub summary: Option<String>,
    pub can_open: bool,
}

/// Lower-cases the text and capitalises its first character ("ONGOING" becomes "Ongoing").
pub fn capitalise_lower(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// A score as a whole number (E-56, a deliberate change from the Double text of the source). Truncates like `toInt()`.
pub fn score_text(score: f64) -> String {
    (score.trunc() as i64).to_string()
}

/// Fields of one result that the card reads. Anime and manga results both map to it.
struct CardFields<'a> {
    title: &'a str,
    authors: &'a [String],
    artists: &'a [String],
    publishing_type: &'a str,
    start_date: &'a str,
    publishing_status: &'a str,
    score: Option<f64>,
    summary: &'a str,
    tracking_url: &'a str,
}

fn card(f: CardFields<'_>) -> CardView {
    let mut names: Vec<&str> = Vec::new();
    for name in f.authors.iter().chain(f.artists.iter()) {
        if !names.contains(&name.as_str()) {
            names.push(name);
        }
    }
    let mut details = Vec::new();
    let kind = capitalise_lower(f.publishing_type);
    if !kind.is_empty() {
        details.push(DetailLine {
            label_key: "track_type",
            label: "Type",
            value: kind,
        });
    }
    if !f.start_date.is_empty() {
        details.push(DetailLine {
            label_key: "label_started",
            label: "Started",
            value: f.start_date.to_owned(),
        });
    }
    // E-37: the API's underscores read as spaces ("ON_HOLD" is "On hold").
    let status = capitalise_lower(&f.publishing_status.replace('_', " "));
    if !status.is_empty() {
        details.push(DetailLine {
            label_key: "track_status",
            label: "Status",
            value: status,
        });
    }
    if let Some(score) = f.score {
        details.push(DetailLine {
            label_key: "score",
            label: "Score",
            value: score_text(score),
        });
    }
    let summary = Some(f.summary.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    CardView {
        title: f.title.to_owned(),
        byline: (!names.is_empty()).then(|| names.join(", ")),
        details,
        summary,
        can_open: !f.tracking_url.is_empty(),
    }
}

impl SearchHit for AnimeTrackSearch {
    fn list_key(&self) -> (TrackerId, i64) {
        AnimeTrackSearch::list_key(self)
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn tracking_url(&self) -> &str {
        &self.tracking_url
    }
    fn card(&self) -> CardView {
        card(CardFields {
            title: &self.title,
            authors: &self.authors,
            artists: &self.artists,
            publishing_type: &self.publishing_type,
            start_date: &self.start_date,
            publishing_status: &self.publishing_status,
            score: self.score,
            summary: &self.summary,
            tracking_url: &self.tracking_url,
        })
    }
    fn set_private(&mut self, private: bool) {
        self.private = private;
    }
}

impl SearchHit for MangaTrackSearch {
    fn list_key(&self) -> (TrackerId, i64) {
        MangaTrackSearch::list_key(self)
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn tracking_url(&self) -> &str {
        &self.tracking_url
    }
    fn card(&self) -> CardView {
        card(CardFields {
            title: &self.title,
            authors: &self.authors,
            artists: &self.artists,
            publishing_type: &self.publishing_type,
            start_date: &self.start_date,
            publishing_status: &self.publishing_status,
            score: self.score,
            summary: &self.summary,
            tracking_url: &self.tracking_url,
        })
    }
    fn set_private(&mut self, private: bool) {
        self.private = private;
    }
}

/// Body state of the screen (behaviour 15.3). `Idle` is the screen before any query ran, which the source leaves
/// as a spinner forever when the title is blank.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchState<R> {
    Idle,
    Loading,
    Empty,
    /// The message, or "Unknown error" when the failure carries none.
    Error(String),
    Results(Vec<R>),
}

/// One search screen for one entry and kind.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchScreen<R: SearchHit> {
    pub query: String,
    supports_private: bool,
    /// The `(tracker, remote id)` of the binding being edited; its result is preselected (E-57).
    bound: Option<(TrackerId, i64)>,
    state: SearchState<R>,
    selected: Option<(TrackerId, i64)>,
    /// Counts the queries started. Only the latest one may write results (D-6, E-32, E-33).
    generation: u64,
    /// True while a bind is being written. Both Track buttons are disabled then (T-28a-057).
    in_flight: bool,
}

impl<R: SearchHit> SearchScreen<R> {
    /// The screen opens with the title in the field. The returned query runs once when the title is not blank.
    /// `bound` is the `(tracker, remote id)` of the binding being edited, if any (076, E-57).
    pub fn open(
        initial_title: &str,
        bound: Option<(TrackerId, i64)>,
        supports_private: bool,
    ) -> (Self, Option<String>) {
        let mut screen = Self {
            query: initial_title.to_owned(),
            supports_private,
            bound,
            state: SearchState::Idle,
            selected: None,
            generation: 0,
            in_flight: false,
        };
        let first = screen.begin(initial_title);
        (screen, first)
    }

    /// Starts a query. A blank query is not sent: the function returns `None` and nothing changes (D11). Otherwise
    /// the state goes to loading and the selection is cleared (076). The caller runs the tracker search with the
    /// returned text.
    pub fn begin(&mut self, text: &str) -> Option<String> {
        if text.trim().is_empty() {
            return None;
        }
        self.generation += 1;
        self.start_loading();
        Some(text.to_owned())
    }

    /// Runs the query in the field again (the Retry button of the error state).
    pub fn retry(&mut self) -> Option<String> {
        let query = self.query.clone();
        self.begin(&query)
    }

    /// The generation of the latest query. The caller keeps it to pass to [`SearchScreen::finish_for`].
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Applies the result of the latest query. Same as [`SearchScreen::finish_for`] with the current generation.
    pub fn finish(&mut self, result: Result<Vec<R>, TrackError>) {
        let generation = self.generation;
        self.finish_for(generation, result);
    }

    /// Applies the result of the query `generation`. A result of an earlier query is dropped and `false` is
    /// returned (D-6). Otherwise the result whose key equals `bound` is selected (076, E-57).
    pub fn finish_for(&mut self, generation: u64, result: Result<Vec<R>, TrackError>) -> bool {
        if generation != self.generation {
            return false;
        }
        self.selected = None;
        self.state = match result {
            Ok(hits) if hits.is_empty() => SearchState::Empty,
            Ok(hits) => {
                self.selected = self.auto_select(&hits);
                SearchState::Results(hits)
            }
            Err(e) => {
                let text = e.to_string();
                SearchState::Error(if text.trim().is_empty() {
                    crate::messages::english(&MsgKey::UnknownError)
                } else {
                    text
                })
            }
        };
        true
    }

    /// Marks a search as running: loading, nothing selected. [`SearchScreen::begin`] already calls it.
    pub fn start_loading(&mut self) {
        self.selected = None;
        self.state = SearchState::Loading;
    }

    fn auto_select(&self, hits: &[R]) -> Option<(TrackerId, i64)> {
        let bound = self.bound?;
        hits.iter().map(SearchHit::list_key).find(|k| *k == bound)
    }

    pub fn state(&self) -> &SearchState<R> {
        &self.state
    }

    /// A click on a card selects it. Clicking the selected card keeps it selected.
    pub fn select(&mut self, key: (TrackerId, i64)) {
        let known = match &self.state {
            SearchState::Results(hits) => hits.iter().any(|h| h.list_key() == key),
            _ => false,
        };
        if known {
            self.selected = Some(key);
        }
    }

    pub fn selected(&self) -> Option<&R> {
        match (&self.state, self.selected) {
            (SearchState::Results(hits), Some(key)) => hits.iter().find(|h| h.list_key() == key),
            _ => None,
        }
    }

    /// The bottom bar is shown only while a card is selected (TT-27-209).
    pub fn bottom_bar_shown(&self) -> bool {
        self.selected().is_some()
    }

    /// The private button appears only for trackers with private support (TT-27-210).
    pub fn private_button_shown(&self) -> bool {
        self.bottom_bar_shown() && self.supports_private
    }

    /// Marks a bind as being written (`true`) or finished (`false`). The GUI calls it around the bind.
    pub fn set_in_flight(&mut self, in_flight: bool) {
        self.in_flight = in_flight;
    }

    /// Both Track buttons are enabled: a card is selected and no bind is in flight (T-28a-057).
    pub fn track_buttons_enabled(&self) -> bool {
        self.bottom_bar_shown() && !self.in_flight
    }

    /// "Track" (`private` false) or the private button (`true`). Returns the result to register. Private is set
    /// only when the tracker supports it. `None` while a bind is in flight.
    pub fn confirm(&self, private: bool) -> Option<R> {
        if self.in_flight {
            return None;
        }
        let mut hit = self.selected()?.clone();
        hit.set_private(private && self.supports_private);
        Some(hit)
    }
}

/// Items of the long-press menu of a card, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardMenuItem {
    CopyToClipboard,
    OpenInBrowser,
}

/// What a card menu item does. The menu closes after every item (D15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardEffect {
    /// Copies the title and shows the copy toast.
    Copy { text: String, toast: MsgKey },
    /// Opens the tracker page in the browser.
    Open(String),
}

/// Runs a menu item. `None` when the item is disabled: open with no URL (D9).
pub fn card_action(item: CardMenuItem, hit: &impl SearchHit) -> Option<CardEffect> {
    match item {
        CardMenuItem::CopyToClipboard => Some(CardEffect::Copy {
            text: hit.title().to_owned(),
            toast: crate::sheet::copy_toast(hit.title()),
        }),
        CardMenuItem::OpenInBrowser => {
            let url = hit.tracking_url();
            (!url.is_empty()).then(|| CardEffect::Open(url.to_owned()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::search::AnimeTrackSearch;

    fn hit(tracker: TrackerId, remote: i64, url: &str) -> AnimeTrackSearch {
        AnimeTrackSearch::new(tracker, remote, format!("Title {remote}"), url)
    }

    fn results() -> Vec<AnimeTrackSearch> {
        vec![
            hit(TrackerId::ANILIST, 1, "https://a.test/1"),
            hit(TrackerId::ANILIST, 2, "https://a.test/2"),
            hit(TrackerId::KITSU, 1, "https://k.test/1"),
        ]
    }

    #[test]
    fn tr_27_d11_a_blank_query_is_not_sent_and_changes_nothing() {
        let (mut screen, first) = SearchScreen::<AnimeTrackSearch>::open("  ", None, true);
        assert_eq!(first, None);
        assert_eq!(screen.state(), &SearchState::Idle);
        screen.finish(Ok(results()));
        assert_eq!(screen.begin(""), None, "defect D11");
        assert!(
            matches!(screen.state(), SearchState::Results(_)),
            "results stay"
        );
    }

    #[test]
    fn tr_27_056_open_with_a_title_runs_one_search() {
        let (screen, first) = SearchScreen::<AnimeTrackSearch>::open("Frieren", None, false);
        assert_eq!(first.as_deref(), Some("Frieren"));
        assert_eq!(screen.query, "Frieren");
        assert_eq!(screen.state(), &SearchState::Loading);
        assert!(screen.selected().is_none());
    }

    #[test]
    fn tr_27b_012_a_slow_first_response_cannot_overwrite_the_second_query() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("a", None, true);
        let first = screen.generation();
        screen.begin("b").unwrap();
        let second = screen.generation();
        assert!(screen.finish_for(second, Ok(vec![hit(TrackerId::ANILIST, 2, "")])));
        assert!(
            !screen.finish_for(first, Ok(vec![hit(TrackerId::ANILIST, 1, "")])),
            "stale"
        );
        match screen.state() {
            SearchState::Results(hits) => {
                assert_eq!(hits[0].remote_id, 2, "only the latest query writes")
            }
            other => panic!("expected results, got {other:?}"),
        }
    }

    #[test]
    fn tr_27b_012_retry_reruns_the_field_and_a_blank_field_does_nothing() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, true);
        screen.finish(Err(TrackError::Message(String::new())));
        assert_eq!(screen.state(), &SearchState::Error("Unknown error".into()));
        assert_eq!(screen.retry().as_deref(), Some("x"));
        assert_eq!(screen.state(), &SearchState::Loading);
        screen.query = "  ".into();
        assert_eq!(screen.retry(), None);
    }

    #[test]
    fn tr_27_076_a_second_begin_clears_the_selection_and_shows_loading() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, true);
        screen.finish(Ok(results()));
        screen.select((TrackerId::ANILIST, 2));
        assert!(screen.selected().is_some());
        assert_eq!(screen.begin("y").as_deref(), Some("y"));
        assert_eq!(screen.state(), &SearchState::Loading);
        assert!(screen.selected().is_none());
        screen.finish(Ok(results()));
        assert!(
            screen.selected().is_none(),
            "the old selection did not survive"
        );
    }

    #[test]
    fn tr_27_208_states_loading_empty_error_and_results() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, false);
        screen.start_loading();
        assert_eq!(screen.state(), &SearchState::Loading);
        screen.finish(Ok(vec![]));
        assert_eq!(screen.state(), &SearchState::Empty);
        screen.start_loading();
        screen.finish(Err(TrackError::Message("timeout".into())));
        assert_eq!(screen.state(), &SearchState::Error("timeout".into()));
        screen.start_loading();
        screen.finish(Err(TrackError::Message(String::new())));
        assert_eq!(screen.state(), &SearchState::Error("Unknown error".into()));
        screen.start_loading();
        screen.finish(Ok(results()));
        assert!(matches!(screen.state(), SearchState::Results(r) if r.len() == 3));
    }

    #[test]
    fn tr_27_209_one_selection_then_a_second_click_changes_it() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, true);
        screen.finish(Ok(results()));
        assert!(!screen.bottom_bar_shown());
        screen.select((TrackerId::ANILIST, 1));
        assert!(screen.bottom_bar_shown());
        screen.select((TrackerId::ANILIST, 1));
        assert_eq!(screen.selected().map(|h| h.remote_id), Some(1), "no toggle");
        screen.select((TrackerId::KITSU, 1));
        assert_eq!(
            screen.selected().map(|h| h.tracker_id),
            Some(TrackerId::KITSU)
        );
        screen.select((TrackerId::KITSU, 99));
        assert_eq!(
            screen.selected().map(|h| h.tracker_id),
            Some(TrackerId::KITSU),
            "unknown key ignored"
        );
    }

    #[test]
    fn tr_27_076_bound_remote_id_auto_selects_and_a_new_query_clears_it() {
        let (mut screen, _) =
            SearchScreen::<AnimeTrackSearch>::open("x", Some((TrackerId::ANILIST, 2)), true);
        screen.finish(Ok(results()));
        assert_eq!(screen.selected().map(|h| h.remote_id), Some(2));
        screen.start_loading();
        assert!(screen.selected().is_none());
        assert_eq!(screen.state(), &SearchState::Loading);
    }

    #[test]
    fn tr_27_076_no_bound_id_means_no_selection() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, true);
        screen.finish(Ok(results()));
        assert!(screen.selected().is_none());
        let (mut other, _) =
            SearchScreen::<AnimeTrackSearch>::open("x", Some((TrackerId::ANILIST, 5)), true);
        other.finish(Ok(results()));
        assert!(other.selected().is_none(), "an id absent from the results never matches");
        let (mut kitsu, _) =
            SearchScreen::<AnimeTrackSearch>::open("x", Some((TrackerId::KITSU, 1)), true);
        kitsu.finish(Ok(results()));
        assert_eq!(
            kitsu.selected().map(|h| h.list_key()),
            Some((TrackerId::KITSU, 1)),
            "the same remote id of another tracker is not chosen"
        );
        let (mut empty_url, _) =
            SearchScreen::<AnimeTrackSearch>::open("x", Some((TrackerId::ANILIST, 5)), true);
        empty_url.finish(Ok(vec![hit(TrackerId::ANILIST, 5, "")]));
        assert!(empty_url.selected().is_some(), "selection does not depend on the URL");
    }

    #[test]
    fn tr_27_058_track_is_public_and_private_only_when_supported() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, true);
        screen.finish(Ok(results()));
        screen.select((TrackerId::ANILIST, 1));
        assert!(screen.private_button_shown());
        let public = screen.confirm(false).unwrap();
        assert!(!public.private);
        let private = screen.confirm(true).unwrap();
        assert!(private.private);

        let (mut plain, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, false);
        plain.finish(Ok(results()));
        plain.select((TrackerId::ANILIST, 1));
        assert!(plain.bottom_bar_shown());
        assert!(!plain.private_button_shown(), "TT-27-210");
        assert!(
            !plain.confirm(true).unwrap().private,
            "private needs support"
        );
    }

    #[test]
    fn tr_27_d10_results_with_the_same_remote_id_but_other_trackers_are_distinct() {
        let list = results();
        assert_ne!(list[0].list_key(), list[2].list_key());
        assert_eq!(list[0].list_key(), (TrackerId::ANILIST, 1));
    }

    #[test]
    fn tr_27_057_card_lines_follow_the_source_rules() {
        let mut h = hit(TrackerId::ANILIST, 7, "https://a.test/7");
        h.authors = vec!["Ann".into(), "Bo".into()];
        h.artists = vec!["Ann".into(), "Cy".into()];
        h.publishing_type = "OVA".into();
        h.publishing_status = "ONGOING".into();
        h.start_date = "2026-01-02".into();
        h.score = Some(8.0);
        h.summary = "  A story.  ".into();
        let card = h.card();
        assert_eq!(card.byline.as_deref(), Some("Ann, Bo, Cy"));
        let lines: Vec<(&str, &str)> = card
            .details
            .iter()
            .map(|d| (d.label, d.value.as_str()))
            .collect();
        assert_eq!(
            lines,
            [
                ("Type", "Ova"),
                ("Started", "2026-01-02"),
                ("Status", "Ongoing"),
                ("Score", "8"),
            ]
        );
        assert_eq!(card.summary.as_deref(), Some("A story."));
        assert!(card.can_open);
    }

    #[test]
    fn tr_27_057_empty_detail_lines_and_score_are_hidden() {
        let h = hit(TrackerId::ANILIST, 8, "https://a.test/8");
        let card = h.card();
        assert!(card.details.is_empty());
        assert_eq!(card.byline, None);
        assert_eq!(card.summary, None);
        assert_eq!(score_text(7.5), "7");
    }

    #[test]
    fn tr_27_d09_a_result_without_a_url_has_no_open_action() {
        let h = hit(TrackerId::ANILIST, 9, "");
        assert!(!h.card().can_open);
        assert_eq!(card_action(CardMenuItem::OpenInBrowser, &h), None);
        assert!(card_action(CardMenuItem::CopyToClipboard, &h).is_some());
    }

    #[test]
    fn tr_27_057_d15_menu_actions_copy_the_title_and_open_the_url() {
        let h = hit(TrackerId::ANILIST, 3, "https://a.test/3");
        match card_action(CardMenuItem::CopyToClipboard, &h) {
            Some(CardEffect::Copy { text, toast }) => {
                assert_eq!(text, "Title 3");
                assert!(crate::messages::english(&toast).starts_with("Copied to clipboard:"));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(
            card_action(CardMenuItem::OpenInBrowser, &h),
            Some(CardEffect::Open("https://a.test/3".into()))
        );
    }

    #[test]
    fn t_28a_057_track_buttons_are_disabled_while_a_bind_is_in_flight() {
        let (mut screen, _) = SearchScreen::<AnimeTrackSearch>::open("x", None, true);
        screen.finish(Ok(results()));
        screen.select((TrackerId::ANILIST, 1));
        assert!(screen.track_buttons_enabled());
        assert!(screen.confirm(false).is_some());
        screen.set_in_flight(true);
        assert!(!screen.track_buttons_enabled());
        assert!(screen.confirm(true).is_none(), "no second bind while one is written");
        screen.set_in_flight(false);
        assert!(screen.confirm(true).is_some());
    }

    #[test]
    fn t_28a_030_status_underscores_read_as_spaces_and_the_score_is_whole() {
        let mut hit = hit(TrackerId::ANILIST, 5, "https://a.test/5");
        hit.publishing_status = "ON_HOLD".into();
        hit.score = Some(85.9);
        let card = hit.card();
        let status = card.details.iter().find(|d| d.label == "Status").map(|d| d.value.clone());
        assert_eq!(status.as_deref(), Some("On hold"));
        let score = card.details.iter().find(|d| d.label == "Score").map(|d| d.value.clone());
        assert_eq!(score.as_deref(), Some("85"));
    }
}
