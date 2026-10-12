//! One row of the tracking sheet and the assembly of the rows of an entry (`mapToTrackItem`, behaviour 3.4).

use kuro_platform::kind::Kind;
use std::sync::Arc;

use crate::model::TrackerId;
use crate::record::TrackRecord;
use crate::tracker::Tracker;

/// One logged-in tracker of the entry's kind, with its binding when the entry has one. Replaces the Kotlin anime
/// and manga item classes: there is no separate type per kind.
#[derive(Clone)]
pub struct TrackItem {
    pub tracker: Arc<dyn Tracker>,
    /// `None` is the "Add tracking" row.
    pub record: Option<TrackRecord>,
}

impl TrackItem {
    /// The identity the Home model compares: `(tracker id, record)` (behaviour 3.2 `distinctUntilChanged`).
    pub fn same_as(&self, other: &TrackItem) -> bool {
        self.tracker.id() == other.tracker.id() && self.record == other.record
    }
}

/// Builds the rows of an entry. `logged_in` is the trackers in registry order; `rows` are the bindings of the
/// entry; `source` is the qualified source id, or `None` for a stub source.
///
/// A logged-out tracker is not shown, but its row is untouched (E-03). An enhanced tracker is shown only when it
/// accepts `source` (E-06). Rows are bound by tracker id only.
pub fn assemble_items(
    kind: Kind,
    logged_in: &[Arc<dyn Tracker>],
    rows: &[TrackRecord],
    source: Option<&str>,
) -> Vec<TrackItem> {
    logged_in
        .iter()
        .filter(|t| t.is_logged_in())
        .filter(|t| match kind {
            Kind::Anime => t.as_anime().is_some(),
            Kind::Manga => t.as_manga().is_some(),
        })
        .filter(|t| {
            let accepted = match kind {
                Kind::Anime => t
                    .as_enhanced_anime()
                    .map(|e| source.is_some_and(|s| e.accepts(s))),
                Kind::Manga => t
                    .as_enhanced_manga()
                    .map(|e| source.is_some_and(|s| e.accepts(s))),
            };
            accepted != Some(false)
        })
        .map(|t| TrackItem {
            tracker: Arc::clone(t),
            record: find_row(rows, kind, t.id()),
        })
        .collect()
}

fn find_row(rows: &[TrackRecord], kind: Kind, tracker: TrackerId) -> Option<TrackRecord> {
    rows.iter()
        .find(|r| r.kind() == kind && r.tracker_id() == tracker)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeAnime, FakeManga, MemSecrets, anime_binding, manga_binding, new_log};
    use crate::tracker::Tracker;

    fn logged_in_anime(id: TrackerId, name: &'static str) -> Arc<dyn Tracker> {
        let t = FakeAnime::new(id, name, MemSecrets::new(), new_log());
        t.save_credentials("user", "pass").unwrap();
        Arc::new(t)
    }

    fn enhanced_anime(
        id: TrackerId,
        name: &'static str,
        accepted: &'static [&'static str],
    ) -> Arc<dyn Tracker> {
        let mut t = FakeAnime::new(id, name, MemSecrets::new(), new_log());
        t.enhanced = true;
        t.accepted = accepted;
        t.save_credentials("user", "pass").unwrap();
        Arc::new(t)
    }

    #[test]
    fn tr_27b_002_an_item_with_and_without_a_record_compares_by_tracker_and_record() {
        let t = logged_in_anime(TrackerId::KITSU, "Kitsu");
        let row = TrackRecord::from(anime_binding(1, TrackerId::KITSU, 2.0));
        let add = TrackItem {
            tracker: Arc::clone(&t),
            record: None,
        };
        let bound = TrackItem {
            tracker: Arc::clone(&t),
            record: Some(row.clone()),
        };
        assert!(add.record.is_none(), "the Add tracking row has no record");
        assert!(!add.same_as(&bound));
        assert!(bound.same_as(&TrackItem {
            tracker: t,
            record: Some(row)
        }));
    }

    #[test]
    fn tr_27b_003_rows_follow_registry_order_and_bind_by_tracker_id() {
        let logged = vec![
            logged_in_anime(TrackerId::MYANIMELIST, "MyAnimeList"),
            logged_in_anime(TrackerId::ANILIST, "AniList"),
        ];
        let rows = vec![
            TrackRecord::from(anime_binding(1, TrackerId::ANILIST, 3.0)),
            TrackRecord::from(manga_binding(1, TrackerId::MANGAUPDATES, 1.0)),
        ];
        let items = assemble_items(Kind::Anime, &logged, &rows, Some("src"));
        let ids: Vec<_> = items.iter().map(|i| i.tracker.id()).collect();
        assert_eq!(ids, vec![TrackerId::MYANIMELIST, TrackerId::ANILIST]);
        assert!(items[0].record.is_none());
        assert_eq!(items[1].record.as_ref().unwrap().progress(), 3.0);
    }

    #[test]
    fn tr_27b_003_a_logged_out_tracker_is_hidden_and_its_row_is_not_touched() {
        let out = FakeAnime::new(TrackerId::KITSU, "Kitsu", MemSecrets::new(), new_log());
        let logged: Vec<Arc<dyn Tracker>> = vec![Arc::new(out)];
        let rows = vec![TrackRecord::from(anime_binding(1, TrackerId::KITSU, 2.0))];
        let items = assemble_items(Kind::Anime, &logged, &rows, Some("src"));
        assert!(items.is_empty(), "E-03: the logged-out row is not shown");
        assert_eq!(rows.len(), 1, "E-03: the stored row is not removed");
    }

    #[test]
    fn tr_27b_003_only_trackers_of_the_kind_are_listed() {
        let manga: Arc<dyn Tracker> = {
            let m = FakeManga::new(TrackerId::KAVITA, "Kavita", MemSecrets::new(), new_log());
            m.save_credentials("user", "pass").unwrap();
            Arc::new(m)
        };
        let logged = vec![manga, logged_in_anime(TrackerId::ANILIST, "AniList")];
        let items = assemble_items(Kind::Anime, &logged, &[], None);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].tracker.id(), TrackerId::ANILIST);
    }

    #[test]
    fn tr_27b_003_enhanced_tracker_is_dropped_for_a_refused_or_stub_source() {
        let logged = vec![enhanced_anime(
            TrackerId::JELLYFIN,
            "Jellyfin",
            &["src.one"],
        )];
        assert_eq!(
            assemble_items(Kind::Anime, &logged, &[], Some("src.one")).len(),
            1
        );
        assert!(assemble_items(Kind::Anime, &logged, &[], Some("src.two")).is_empty());
        assert!(
            assemble_items(Kind::Anime, &logged, &[], None).is_empty(),
            "E-06: stub source"
        );
    }
}
