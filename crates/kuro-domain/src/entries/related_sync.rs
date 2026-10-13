//! `SyncRelatedAnimeWithSource` (T-10a-011): fetches the related anime of one entry from its source and stores the
//! groups with `replace_relations`. A fresh stored fetch is reused, and a source that does not list related anime
//! costs no database access at all.

use std::collections::BTreeSet;

use kuro_data::DataResult;
use kuro_data::anime_relation::{AnimeRelationGroup, AnimeRelationRepo};
use kuro_data::entry::{Entry, EntryRepo};
use kuro_platform::kind::Kind;
use kuro_source::anime::{AnimeSource, SAnime, related_animes};

use crate::entries::network_to_local::network_to_local;
use crate::error::DomainError;

/// Seven days in milliseconds. A stored fetch younger than this is reused unless the caller forces a refresh.
pub const RELATED_TTL_MS: i64 = 604_800_000;

/// Related anime kept per group.
pub const MAX_PER_GROUP: usize = 10;

/// Refreshes the related groups of `anime` from `source`. Returns `true` when the source was asked and the groups were
/// replaced, `false` when nothing was done (no source, no related support, or a fresh stored fetch).
///
/// Each group keeps the first occurrence of each url, drops the entry's own url, keeps at most
/// [`MAX_PER_GROUP`] anime, maps them to local rows of the entry's source, and is dropped when empty.
pub async fn sync_related_anime_with_source(
    source: Option<&dyn AnimeSource>,
    entries: &EntryRepo,
    relations: &AnimeRelationRepo,
    anime: &Entry,
    force_refresh: bool,
    now_ms: i64,
) -> Result<bool, DomainError> {
    let Some(source) = source.filter(|s| s.supports_related_anime()) else {
        return Ok(false);
    };
    if !force_refresh {
        if let Some(fetched_at) = relations.last_fetched_at(anime.id)? {
            if now_ms - fetched_at < RELATED_TTL_MS {
                return Ok(false);
            }
        }
    }
    let parent = SAnime { url: anime.url.clone(), title: anime.title.clone(), ..SAnime::default() };
    let raw = related_animes(source, &parent).await?;

    let mut groups: Vec<AnimeRelationGroup> = Vec::new();
    for relation in raw {
        let mut seen = BTreeSet::new();
        let kept: Vec<&SAnime> = relation
            .animes
            .iter()
            .filter(|s| s.url != anime.url && seen.insert(s.url.as_str()))
            .take(MAX_PER_GROUP)
            .collect();
        let mapped: Vec<Entry> = kept
            .into_iter()
            .map(|s| {
                let wanted = Entry {
                    url: s.url.clone(),
                    title: s.title.clone(),
                    source: anime.source,
                    thumbnail_url: s.thumbnail_url.clone(),
                    ..Entry::create(Kind::Anime)
                };
                network_to_local(entries, &wanted)
            })
            .collect::<DataResult<_>>()?;
        if !mapped.is_empty() {
            groups.push(AnimeRelationGroup { name: relation.name, anime: mapped });
        }
    }
    relations.replace_relations(anime.id, &groups, now_ms)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use kuro_source::SourceError;
    use kuro_source::anime::{AnimeRelation, AnimesPage};
    use kuro_source::filter_list::FilterList;

    use super::*;
    use crate::testdb::test_db;

    /// A source that lists one group of related anime and counts its calls.
    struct Related {
        supported: bool,
        calls: Arc<AtomicUsize>,
        groups: Vec<AnimeRelation>,
    }

    #[async_trait]
    impl AnimeSource for Related {
        fn id(&self) -> i64 {
            42
        }
        fn name(&self) -> &str {
            "Related"
        }
        fn supports_latest(&self) -> bool {
            false
        }
        fn supports_related_anime(&self) -> bool {
            self.supported
        }
        async fn related_anime(&self, _anime: &SAnime) -> Result<Vec<AnimeRelation>, SourceError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.groups.clone())
        }
        async fn popular(&self, _: i32) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("popular"))
        }
        async fn latest(&self, _: i32) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("latest"))
        }
        async fn search(&self, _: i32, _: &str, _: &FilterList) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("search"))
        }
        fn provides_hosters(&self) -> bool {
            false
        }
    }

    fn sanime(url: &str) -> SAnime {
        SAnime { url: url.into(), title: url.into(), ..SAnime::default() }
    }

    fn setup() -> (tempfile::TempDir, Arc<kuro_db::handle::Db>, Entry) {
        let (dir, db) = test_db();
        let entries = EntryRepo::new(db.clone());
        let id = entries.insert(&Entry { url: "/parent".into(), title: "Parent".into(), source: 42, status: 1, ..Entry::create(Kind::Anime) }).unwrap();
        let parent = entries.by_id(id).unwrap();
        (dir, db, parent)
    }

    fn fake(supported: bool, groups: Vec<AnimeRelation>) -> (Related, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (Related { supported, calls: calls.clone(), groups }, calls)
    }

    #[tokio::test]
    async fn no_source_or_no_support_returns_without_touching_the_database() {
        let (_dir, db, parent) = setup();
        let animes = EntryRepo::new(db.clone());
        let relations = AnimeRelationRepo::new(db);
        assert!(!sync_related_anime_with_source(None, &animes, &relations, &parent, true, 1).await.unwrap());
        let (src, calls) = fake(false, Vec::new());
        assert!(!sync_related_anime_with_source(Some(&src), &animes, &relations, &parent, true, 1).await.unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(relations.last_fetched_at(parent.id).unwrap(), None, "no fetch was stored");
    }

    #[tokio::test]
    async fn a_fresh_fetch_is_reused_and_a_week_old_one_refreshes() {
        let (_dir, db, parent) = setup();
        let animes = EntryRepo::new(db.clone());
        let relations = AnimeRelationRepo::new(db);
        let (src, calls) = fake(true, vec![AnimeRelation { name: "Sequels".into(), animes: vec![sanime("/s1")] }]);
        assert!(sync_related_anime_with_source(Some(&src), &animes, &relations, &parent, false, 1_000).await.unwrap());
        assert!(!sync_related_anime_with_source(Some(&src), &animes, &relations, &parent, false, 1_000 + RELATED_TTL_MS - 1).await.unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 1, "younger than seven days: reused");
        assert!(sync_related_anime_with_source(Some(&src), &animes, &relations, &parent, false, 1_000 + RELATED_TTL_MS).await.unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 2, "exactly seven days old refreshes");
        assert!(sync_related_anime_with_source(Some(&src), &animes, &relations, &parent, true, 1_001).await.unwrap());
        assert_eq!(calls.load(Ordering::SeqCst), 3, "a forced refresh ignores the stored time");
    }

    #[tokio::test]
    async fn groups_are_deduplicated_capped_without_the_parent_and_empty_groups_are_dropped() {
        let (_dir, db, parent) = setup();
        let animes = EntryRepo::new(db.clone());
        let relations = AnimeRelationRepo::new(db);
        let mut many: Vec<SAnime> = vec![sanime("/parent"), sanime("/dup"), sanime("/dup")];
        many.extend((0..12).map(|i| sanime(&format!("/n{i}"))));
        let (src, _) = fake(
            true,
            vec![
                AnimeRelation { name: "Many".into(), animes: many },
                AnimeRelation { name: "Nothing".into(), animes: vec![sanime("/parent")] },
            ],
        );
        sync_related_anime_with_source(Some(&src), &animes, &relations, &parent, true, 5).await.unwrap();
        let mut sub = relations.subscribe_related(parent.id);
        let groups = tokio::time::timeout(std::time::Duration::from_millis(500), sub.changed()).await.unwrap().unwrap().unwrap();
        assert_eq!(groups.len(), 1, "the empty group is dropped");
        assert_eq!(groups[0].name, "Many");
        let urls: Vec<&str> = groups[0].anime.iter().map(|a| a.url.as_str()).collect();
        assert_eq!(urls.len(), MAX_PER_GROUP);
        assert_eq!(urls[0], "/dup", "the first duplicate is kept once");
        assert!(!urls.contains(&"/parent"));
        assert_eq!(relations.last_fetched_at(parent.id).unwrap(), Some(5));
    }
}
