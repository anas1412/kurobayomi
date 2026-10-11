//! Delete interactors for downloaded chapters and episodes (port of DeleteChapterDownload.kt and
//! DeleteEpisodeDownload.kt; T-25a-010, T-25a-011, T-25a-006). The settings diet cut the category exclusions of the
//! delete rules, so a delete keeps no category: [`DeleteSettings::default`] is the only settings value.

use std::collections::BTreeSet;
use std::sync::Arc;

use tokio::sync::oneshot;
use tracing::warn;

use crate::anime::manager::{AnimeQueue, Command as AnimeCommand};
use crate::anime::model::{AnimeRef, AnimeSource, EpisodeRef};
use crate::error::DownloadError;
use crate::manager::{DeleteRules, DownloadManager};
use crate::model::{ChapterRef, MangaRef};
use crate::source::ChapterSource;

/// Errors of the delete path that the caller logs.
#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    #[error(transparent)]
    Download(#[from] DownloadError),
}

/// The delete settings read from the store for one kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteSettings {
    /// Categories whose watched or read items are kept.
    pub exclude_categories: BTreeSet<i64>,
    /// `pref_remove_bookmarked`: allow deleting bookmarked items.
    pub remove_bookmarked: bool,
}

/// Looks up an installed manga or novel source by id (`sourceManager.get`). `None` means the source is not installed.
pub trait ChapterSourceResolver: Send + Sync {
    fn chapter_source(&self, source_id: i64) -> Option<Arc<dyn ChapterSource>>;
}

/// Looks up an installed anime source by id. `None` means the source is not installed.
pub trait AnimeSourceResolver: Send + Sync {
    fn anime_source(&self, source_id: i64) -> Option<AnimeSource>;
}

/// What the delete filter reads from one downloaded item: a chapter (`read`, `bookmark`) or an episode (`seen`,
/// `bookmark`).
pub trait DeleteCandidate {
    fn watched(&self) -> bool;
    fn bookmarked(&self) -> bool;
}

impl DeleteCandidate for ChapterRef {
    fn watched(&self) -> bool {
        self.read
    }
    fn bookmarked(&self) -> bool {
        self.bookmark
    }
}

impl DeleteCandidate for EpisodeRef {
    fn watched(&self) -> bool {
        self.seen
    }
    fn bookmarked(&self) -> bool {
        self.bookmark
    }
}

/// The pure delete filter of p1-behavior 1.4, the same for both kinds (T-25a-012). An empty `categories` counts as
/// `[0]`. When one of the categories is in `exclude`, watched items are kept. Bookmarked items are kept unless
/// `remove_bookmarked` is set. Empty input gives empty output (E-01).
pub fn select_for_delete<T: DeleteCandidate>(
    items: Vec<T>,
    exclude: &[i64],
    categories: &[i64],
    remove_bookmarked: bool,
) -> Vec<T> {
    let categories: &[i64] = if categories.is_empty() { &[0] } else { categories };
    let protect_watched = categories.iter().any(|c| exclude.contains(c));
    items
        .into_iter()
        .filter(|item| !(protect_watched && item.watched()))
        .filter(|item| remove_bookmarked || !item.bookmarked())
        .collect()
}

/// The remove-after-marked-as-read hook (T-25a-014, consumer contract of `SetReadStatus` and `SetSeenStatus`). A delete
/// is requested only when the item was marked read or seen and `pref_remove_after_marked_as_read_key` is on. Only the
/// items this change touched are passed in. They are grouped by parent id, one call per series, in first-seen order.
pub fn removal_calls<T>(
    changed: Vec<T>,
    marked_read: bool,
    preference_on: bool,
    parent_id: impl Fn(&T) -> i64,
) -> Vec<(i64, Vec<T>)> {
    if !marked_read || !preference_on {
        return Vec::new();
    }
    let mut calls: Vec<(i64, Vec<T>)> = Vec::new();
    for item in changed {
        let parent = parent_id(&item);
        match calls.iter_mut().find(|(p, _)| *p == parent) {
            Some((_, items)) => items.push(item),
            None => calls.push((parent, vec![item])),
        }
    }
    calls
}

/// `DeleteChapterDownload.awaitAll`.
pub struct DeleteChapterDownload {
    sources: Arc<dyn ChapterSourceResolver>,
    manager: Arc<DownloadManager>,
}

impl DeleteChapterDownload {
    pub fn new(sources: Arc<dyn ChapterSourceResolver>, manager: Arc<DownloadManager>) -> Self {
        Self { sources, manager }
    }

    /// Deletes the chapters the rules allow. Returns how many were selected, and `0` with no file touched when the
    /// source is unknown (E-02). The work runs on a spawned task, so a dropped caller cannot stop it half way (T-25a-016).
    pub async fn await_all(
        &self,
        manga: &MangaRef,
        chapters: Vec<ChapterRef>,
        manga_categories: Vec<i64>,
        settings: &DeleteSettings,
    ) -> Result<usize, DownloadError> {
        let Some(source) = self.sources.chapter_source(manga.source_id) else {
            warn!(target: "kuro_downloads", manga_id = manga.id, source_id = manga.source_id, "delete skipped: source is not installed");
            return Ok(0);
        };
        let rules = DeleteRules {
            exclude_categories: settings.exclude_categories.iter().copied().collect(),
            manga_categories,
            remove_bookmarked: settings.remove_bookmarked,
        };
        let manager = self.manager.clone();
        let manga = manga.clone();
        let task = tokio::spawn(async move { manager.delete_chapters(chapters, &manga, source.as_ref(), &rules).await });
        task.await.map_err(|e| DownloadError::Internal(e.to_string()))?
    }
}

/// `DeleteEpisodeDownload.awaitAll`. The anime actor applies its own delete rules, as it always did.
pub struct DeleteEpisodeDownload {
    sources: Arc<dyn AnimeSourceResolver>,
    queue: AnimeQueue,
}

impl DeleteEpisodeDownload {
    pub fn new(sources: Arc<dyn AnimeSourceResolver>, queue: AnimeQueue) -> Self {
        Self { sources, queue }
    }

    /// The Rust form of `await_items(entry, candidates, names)` (T-25a-010): `anime` is the entry, `episodes` the
    /// candidates, and `categories` the entry's category ids (the rules read the settings in the actor, so no names are
    /// passed). The source is resolved first, and an unknown one gives `Ok(0)` (E-02). The anime actor runs the shared
    /// delete steps of [`EntryKind::Anime`](crate::queue_model::EntryKind) on the blocking pool. Returns how many
    /// episodes the rules selected; the reply arrives after the folders are removed, so the caller can refresh the rows.
    pub async fn await_all(
        &self,
        anime: &AnimeRef,
        episodes: Vec<EpisodeRef>,
        categories: Vec<i64>,
    ) -> Result<usize, DownloadError> {
        let Some(source) = self.sources.anime_source(anime.source_id) else {
            warn!(target: "kuro_downloads", anime_id = anime.id, source_id = anime.source_id, "delete skipped: source is not installed");
            return Ok(0);
        };
        let (reply, answer) = oneshot::channel();
        self.queue.send(AnimeCommand::DeleteEpisodes {
            source,
            anime: anime.clone(),
            episodes,
            categories,
            reply,
        })?;
        answer.await.map_err(|_| DownloadError::Internal("anime queue manager stopped".into()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::layout::AnimeLayout;
    use crate::layout::DownloadLayout;

    struct NoAnimeSource;

    impl AnimeSourceResolver for NoAnimeSource {
        fn anime_source(&self, _source_id: i64) -> Option<AnimeSource> {
            None
        }
    }

    struct NoChapterSource;

    impl ChapterSourceResolver for NoChapterSource {
        fn chapter_source(&self, _source_id: i64) -> Option<Arc<dyn ChapterSource>> {
            None
        }
    }

    fn episode(id: i64) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id: 5,
            url: format!("/e/{id}"),
            name: format!("Ep {id}"),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order: id,
            upload_date: 0,
            episode_number: id as f64,
        }
    }

    #[tokio::test]
    async fn an_unknown_anime_source_deletes_nothing_and_returns_zero() {
        let root = tempfile::tempdir().unwrap();
        let show = AnimeRef { id: 5, source_id: 9, url: "/a/5".into(), title: "Show".into() };
        let owner = AnimeSource { id: 9, name: "Gone".into(), lang: "en".into(), is_http: true, unmetered: false };
        let folder = AnimeLayout::new(Some(root.path().to_path_buf())).anime_dir(&owner, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        let delete = DeleteEpisodeDownload::new(Arc::new(NoAnimeSource), crate::testing::anime_queue(root.path()));
        let deleted = delete.await_all(&show, vec![episode(1)], vec![]).await.unwrap();
        assert_eq!(deleted, 0, "E-02: an unknown source is a logged no-op");
        assert!(folder.join("Ep 1").is_dir(), "no file is touched");
    }

    #[tokio::test]
    async fn an_unknown_manga_source_deletes_nothing_and_returns_zero() {
        let base = tempfile::tempdir().unwrap();
        let manager = crate::testing::manga_manager(base.path());
        let layout = DownloadLayout::new(base.path().join("downloads"));
        let dir = layout.manga_dir("Gone (EN)", "Manga 1").unwrap();
        std::fs::create_dir(dir.join("Chapter 1")).unwrap();
        let manga = crate::testing::manga(1, 9, "Manga 1");
        let delete = DeleteChapterDownload::new(Arc::new(NoChapterSource), manager);
        let chapters = vec![crate::testing::chapter(1, 1, "Chapter 1")];
        let deleted = delete.await_all(&manga, chapters, vec![], &DeleteSettings::default()).await.unwrap();
        assert_eq!(deleted, 0, "E-02: an unknown source is a logged no-op");
        assert!(dir.join("Chapter 1").is_dir(), "no file is touched");
    }
}

#[cfg(test)]
mod select_tests {
    use super::*;
    use crate::testing::chapter;

    fn read(id: i64) -> ChapterRef {
        let mut c = chapter(id, 1, "c");
        c.read = true;
        c
    }

    fn bookmarked(id: i64) -> ChapterRef {
        let mut c = chapter(id, 1, "c");
        c.bookmark = true;
        c
    }

    fn ids(items: Vec<ChapterRef>) -> Vec<i64> {
        items.iter().map(|c| c.id).collect()
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(select_for_delete(Vec::<ChapterRef>::new(), &[1], &[1], true).is_empty(), "E-01");
    }

    #[test]
    fn no_categories_counts_as_category_zero_for_the_exclusion() {
        let out = select_for_delete(vec![read(1), chapter(2, 1, "c")], &[0], &[], true);
        assert_eq!(ids(out), vec![2], "T-E04: the watched item is protected through category 0");
    }

    #[test]
    fn a_watched_item_in_an_excluded_category_is_not_selected() {
        let out = select_for_delete(vec![read(1), chapter(2, 1, "c")], &[5], &[5, 6], true);
        assert_eq!(ids(out), vec![2], "T-E18 and T-E18b");
    }

    #[test]
    fn bookmarks_are_kept_unless_removal_of_bookmarks_is_allowed() {
        assert!(select_for_delete(vec![bookmarked(3)], &[], &[], false).is_empty(), "T-E19: kept");
        assert_eq!(ids(select_for_delete(vec![bookmarked(3)], &[], &[], true)), vec![3], "T-E19: deleted");
    }
}

#[cfg(test)]
mod removal_hook_tests {
    use super::*;

    #[test]
    fn nothing_is_requested_unless_marked_and_the_preference_is_on() {
        assert!(removal_calls(vec![1, 2], false, true, |_| 1).is_empty());
        assert!(removal_calls(vec![1, 2], true, false, |_| 1).is_empty(), "the preference gates the hook");
    }

    #[test]
    fn changed_items_are_grouped_one_call_per_parent_in_first_seen_order() {
        let changed = vec![(10, 1), (20, 2), (11, 1)];
        let calls = removal_calls(changed, true, true, |&(_, parent)| parent);
        assert_eq!(calls, vec![(1, vec![(10, 1), (11, 1)]), (2, vec![(20, 2)])]);
    }
}

