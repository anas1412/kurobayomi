//! Non-library listings (`GetMangaSourcesWithNonLibraryManga.kt`, `GetAnimeSourcesWithNonLibraryAnime.kt`): per source,
//! the number of non-favourite rows, and for anime the ids that a "delete all" action removes.

use kuro_data::DataResult;
use kuro_data::anime::{Anime, AnimeRepo};

use crate::source::model::{Id, Source};

/// One source with the number of its non-library manga (`MangaSourceWithCount.kt`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceWithCount {
    pub source: Source,
    pub count: i64,
}

impl SourceWithCount {
    pub fn id(&self) -> Id {
        self.source.id
    }

    pub fn name(&self) -> &str {
        &self.source.name
    }
}

/// One source with the ids of its non-library anime (`AnimeSourceWithIds.kt`). `orphaned` holds the ids whose parent
/// no longer exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceWithIds {
    pub source: Source,
    pub ids: Vec<Id>,
    pub orphaned: Vec<Id>,
}

impl SourceWithIds {
    /// `ids.len()`.
    pub fn count(&self) -> i64 {
        self.ids.len() as i64
    }

    pub fn id(&self) -> Id {
        self.source.id
    }

    pub fn name(&self) -> &str {
        &self.source.name
    }
}

/// Builds the manga listing from `(source id, non-library count)` rows. A source with a zero count is left out
/// (E-SRC-09). Sources keep the order of `sources`.
pub fn manga_listing(sources: &[Source], counts: &[(Id, i64)]) -> Vec<SourceWithCount> {
    sources
        .iter()
        .filter_map(|source| {
            let count = counts.iter().find(|(id, _)| *id == source.id).map_or(0, |(_, n)| *n);
            (count > 0).then(|| SourceWithCount { source: source.clone(), count })
        })
        .collect()
}

/// Builds the anime listing from `(source id, anime id)` rows of non-library anime and `(source id, anime id)` rows
/// of orphaned anime. A source is listed when it has either kind of row (E-SRC-09). Sources keep the order of
/// `sources`.
pub fn anime_listing(sources: &[Source], ids: &[(Id, Id)], orphaned: &[(Id, Id)]) -> Vec<SourceWithIds> {
    sources
        .iter()
        .filter_map(|source| {
            let of = |rows: &[(Id, Id)]| -> Vec<Id> {
                rows.iter().filter(|(s, _)| *s == source.id).map(|(_, a)| *a).collect()
            };
            let (ids, orphaned) = (of(ids), of(orphaned));
            (!ids.is_empty() || !orphaned.is_empty()).then(|| SourceWithIds { source: source.clone(), ids, orphaned })
        })
        .collect()
}

/// The children of anime `parent_id`, the only rows a "delete children" action touches (`getDeletableChildren`).
pub fn get_deletable_children(repo: &AnimeRepo, parent_id: Id) -> DataResult<Vec<Anime>> {
    repo.children_by_parent(parent_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(id: Id) -> Source {
        Source::new(id, &format!("S{id}"), "en")
    }

    #[test]
    fn manga_listing_counts_non_library_rows_and_omits_zero_counts() {
        let sources = [src(1), src(2), src(3)];
        let rows = manga_listing(&sources, &[(1, 12), (3, 0)]);
        assert_eq!(rows.iter().map(|r| (r.id(), r.count)).collect::<Vec<_>>(), vec![(1, 12)]);
        assert_eq!(rows[0].name(), "S1");
    }

    #[test]
    fn anime_listing_counts_ids_and_keeps_a_zero_count_source_with_orphans() {
        let sources = [src(1), src(2), src(3)];
        let rows = anime_listing(&sources, &[(1, 10), (1, 11)], &[(2, 20)]);
        assert_eq!(rows.len(), 2, "source 3 has no rows at all");
        assert_eq!((rows[0].id(), rows[0].count()), (1, 2));
        assert_eq!((rows[1].id(), rows[1].count(), rows[1].orphaned.as_slice()), (2, 0, &[20][..]), "E-SRC-09");
    }

    #[test]
    fn deletable_children_returns_only_the_children_of_that_parent() {
        use crate::testdb::test_db;
        let (_d, db) = test_db();
        let repo = AnimeRepo::new(db);
        let parent = repo.insert(&Anime { url: "/p".into(), title: "P".into(), source: 1, ..Anime::create() }).unwrap();
        let other = repo.insert(&Anime { url: "/o".into(), title: "O".into(), source: 1, ..Anime::create() }).unwrap();
        let child = repo
            .insert(&Anime {
                url: "/c".into(),
                title: "C".into(),
                source: 1,
                parent_id: Some(parent),
                ..Anime::create()
            })
            .unwrap();
        repo.insert(&Anime {
            url: "/x".into(),
            title: "X".into(),
            source: 1,
            parent_id: Some(other),
            ..Anime::create()
        })
        .unwrap();
        let got = get_deletable_children(&repo, parent).unwrap();
        assert_eq!(got.iter().map(|a| a.id).collect::<Vec<_>>(), vec![child]);
    }
}
