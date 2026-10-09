//! Read-side query facades (`GetManga.kt`, `GetAnime.kt`, `GetMangaFavorites.kt`, `GetDuplicateLibraryManga.kt`
//! and their anime twins). Error contracts follow `p1-behavior.md` section 4: `get_*_or_none` logs and returns
//! `None` on any error, the others pass the repository result through.

use kuro_data::anime::{Anime, AnimeRepo, LibraryAnime, SeasonAnime};
use kuro_data::chapter::{Chapter, ChapterRepo};
use kuro_data::episode::{Episode, EpisodeRepo};
use kuro_data::manga::{LibraryManga, Manga, MangaRepo};
use kuro_data::DataResult;

use super::combine::{Joined, Pair, Triple, combine2, combine3};

/// `GetManga.await(id)`: `None` on any error, including a missing row. The error is logged.
pub fn get_manga_or_none(repo: &MangaRepo, id: i64) -> Option<Manga> {
    match repo.by_id(id) {
        Ok(manga) => Some(manga),
        Err(e) => {
            tracing::error!(id, error = %e, "cannot read manga");
            None
        }
    }
}

/// `GetAnime.await(id)`: `None` on any error, including a missing row. The error is logged.
pub fn get_anime_or_none(repo: &AnimeRepo, id: i64) -> Option<Anime> {
    match repo.by_id(id) {
        Ok(anime) => Some(anime),
        Err(e) => {
            tracing::error!(id, error = %e, "cannot read anime");
            None
        }
    }
}

/// `GetMangaByUrlAndSourceId.await(url, sourceId)`: propagates the error, `None` when absent.
pub fn get_manga_by_url_and_source(
    repo: &MangaRepo,
    url: &str,
    source: i64,
) -> DataResult<Option<Manga>> {
    repo.by_url_and_source(url, source)
}

/// `GetAnimeByUrlAndSourceId.await(url, sourceId)`: propagates the error, `None` when absent.
pub fn get_anime_by_url_and_source(
    repo: &AnimeRepo,
    url: &str,
    source: i64,
) -> DataResult<Option<Anime>> {
    repo.by_url_and_source(url, source)
}

/// `GetMangaFavorites.await()`: every row with `favorite = 1`.
pub fn get_manga_favorites(repo: &MangaRepo) -> DataResult<Vec<Manga>> {
    repo.favorites()
}

/// `GetAnimeFavorites.await()`: every row with `favorite = 1`.
pub fn get_anime_favorites(repo: &AnimeRepo) -> DataResult<Vec<Anime>> {
    repo.favorites()
}

/// `GetMangaFavorites.subscribe(sourceId)` as a one-shot read.
pub fn get_manga_favorites_by_source(repo: &MangaRepo, source: i64) -> DataResult<Vec<Manga>> {
    repo.favorites_by_source(source)
}

/// `GetAnimeFavorites.subscribe(sourceId)` as a one-shot read.
pub fn get_anime_favorites_by_source(repo: &AnimeRepo, source: i64) -> DataResult<Vec<Anime>> {
    repo.favorites_by_source(source)
}

/// `GetDuplicateLibraryManga.await(manga)`: favourites with the same lower-cased title and a different id.
pub fn get_duplicate_library_manga(repo: &MangaRepo, manga: &Manga) -> DataResult<Vec<Manga>> {
    repo.duplicates(manga.id, &manga.title.to_lowercase())
}

/// `GetDuplicateLibraryAnime.await(anime)`: favourites with the same lower-cased title and a different id.
pub fn get_duplicate_library_anime(repo: &AnimeRepo, anime: &Anime) -> DataResult<Vec<Anime>> {
    repo.duplicates(anime.id, &anime.title.to_lowercase())
}

/// `GetAnimeSeasonsByParentId.await(animeId)`: the seasons of a parent, or an empty list when the read fails
/// (the error is logged).
pub fn get_seasons_by_parent_id(repo: &AnimeRepo, parent_id: i64) -> Vec<SeasonAnime> {
    match repo.seasons_by_parent(parent_id) {
        Ok(seasons) => seasons,
        Err(e) => {
            tracing::error!(parent_id, error = %e, "cannot read seasons");
            Vec::new()
        }
    }
}

/// `GetMangaWithChapters.awaitManga(id)`: the row, or the `NotFound` error when it is missing.
pub fn await_manga(repo: &MangaRepo, id: i64) -> DataResult<Manga> {
    repo.by_id(id)
}

/// `GetMangaWithChapters.awaitChapters(id, applyScanlatorFilter)`: the chapters, errors passed through.
pub fn await_chapters(repo: &ChapterRepo, manga_id: i64, apply_scanlator_filter: bool) -> DataResult<Vec<Chapter>> {
    repo.by_manga_id(manga_id, apply_scanlator_filter)
}

/// `GetAnimeWithEpisodesAndSeasons.awaitAnime(id)`: the row, or the `NotFound` error when it is missing.
pub fn await_anime(repo: &AnimeRepo, id: i64) -> DataResult<Anime> {
    repo.by_id(id)
}

/// `GetAnimeWithEpisodesAndSeasons.awaitEpisodes(id)`: the episodes, errors passed through.
pub fn await_episodes(repo: &EpisodeRepo, anime_id: i64) -> DataResult<Vec<Episode>> {
    repo.by_anime_id(anime_id)
}

/// `GetAnimeWithEpisodesAndSeasons.awaitSeasons(id)`: the seasons of a parent, errors passed through.
pub fn await_seasons(repo: &AnimeRepo, parent_id: i64) -> DataResult<Vec<SeasonAnime>> {
    repo.seasons_by_parent(parent_id)
}

/// `GetMangaWithChapters.subscribe(id, applyScanlatorFilter)`: `(manga, chapters)`, published once both have a value.
pub fn subscribe_manga_with_chapters(
    manga: &MangaRepo,
    chapters: &ChapterRepo,
    id: i64,
    apply_scanlator_filter: bool,
) -> Joined<Result<Pair<Option<Manga>, Vec<Chapter>>, String>> {
    combine2(
        manga.subscribe_by_id(id),
        chapters.subscribe_by_manga_id(id, apply_scanlator_filter),
    )
}

/// `GetAnimeWithEpisodesAndSeasons.subscribe(id)`: `(anime, episodes, seasons)`, published once all three have a value.
pub fn subscribe_anime_with_episodes_and_seasons(
    anime: &AnimeRepo,
    episodes: &EpisodeRepo,
    id: i64,
) -> Joined<Result<Triple<Option<Anime>, Vec<Episode>, Vec<SeasonAnime>>, String>> {
    combine3(
        anime.subscribe_by_id(id),
        episodes.subscribe_by_anime_id(id),
        anime.subscribe_seasons_by_parent(id),
    )
}

/// `GetLibraryManga.await()`: every library row with its aggregates.
pub fn get_library_manga(repo: &MangaRepo) -> DataResult<Vec<LibraryManga>> {
    repo.library()
}

/// `GetLibraryAnime.await()`: every library row with its aggregates.
pub fn get_library_anime(repo: &AnimeRepo) -> DataResult<Vec<LibraryAnime>> {
    repo.library()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::LibraryState;
    use crate::testdb::{anime_repo, manga_repo};
    use kuro_data::LookupOrInsert;
    use kuro_data::anime::{AnimeUpdate, DeletableEntry, SeasonAnime};
    use kuro_data::manga::MangaUpdate;

    fn manga(url: &str, title: &str, favorite: bool) -> Manga {
        Manga {
            url: url.into(),
            title: title.into(),
            favorite,
            source: 1,
            ..Manga::create()
        }
    }

    fn anime(url: &str, title: &str, favorite: bool) -> Anime {
        Anime {
            url: url.into(),
            title: title.into(),
            favorite,
            source: 1,
            ..Anime::create()
        }
    }

    #[test]
    fn get_or_none_returns_the_row_and_swallows_a_missing_one() {
        let (_dir, repo) = manga_repo();
        let id = repo.insert(&manga("/m/1", "One", true)).expect("insert");
        assert_eq!(
            get_manga_or_none(&repo, id).map(|m| m.title),
            Some("One".into())
        );
        assert_eq!(
            get_manga_or_none(&repo, id + 999),
            None,
            "missing row is logged and becomes None"
        );

        let (_adir, arepo) = anime_repo();
        let aid = arepo.insert(&anime("/a/1", "Show", true)).expect("insert");
        assert_eq!(
            get_anime_or_none(&arepo, aid).map(|a| a.title),
            Some("Show".into())
        );
        assert_eq!(get_anime_or_none(&arepo, aid + 999), None);
    }

    #[test]
    fn by_url_favourites_and_duplicates_pass_through() {
        let (_dir, repo) = manga_repo();
        let a = repo
            .insert(&manga("/m/a", "Same Title", true))
            .expect("insert");
        let b = repo
            .insert(&manga("/m/b", "SAME TITLE", true))
            .expect("insert");
        repo.insert(&manga("/m/c", "Other", false)).expect("insert");

        assert_eq!(
            get_manga_by_url_and_source(&repo, "/m/c", 1)
                .expect("ok")
                .map(|m| m.title),
            Some("Other".into())
        );
        assert_eq!(
            get_manga_by_url_and_source(&repo, "/none", 1).expect("ok"),
            None
        );
        assert_eq!(get_manga_favorites(&repo).expect("ok").len(), 2);
        assert_eq!(
            get_manga_favorites_by_source(&repo, 1).expect("ok").len(),
            2
        );
        assert_eq!(
            get_manga_favorites_by_source(&repo, 7).expect("ok").len(),
            0
        );

        let first = repo.by_id(a).expect("row");
        let dupes = get_duplicate_library_manga(&repo, &first).expect("ok");
        assert_eq!(
            dupes.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![b],
            "same title, other id, any case"
        );
    }

    #[test]
    fn anime_side_passes_through_too() {
        let (_dir, repo) = anime_repo();
        let a = repo.insert(&anime("/a/a", "Show", true)).expect("insert");
        repo.insert(&anime("/a/b", "show", true)).expect("insert");
        repo.insert(&anime("/a/c", "Quiet", false)).expect("insert");
        assert_eq!(
            get_anime_by_url_and_source(&repo, "/a/c", 1)
                .expect("ok")
                .map(|x| x.title),
            Some("Quiet".into())
        );
        assert_eq!(get_anime_favorites(&repo).expect("ok").len(), 2);
        assert_eq!(
            get_anime_favorites_by_source(&repo, 1).expect("ok").len(),
            2
        );
        let first = repo.by_id(a).expect("row");
        assert_eq!(
            get_duplicate_library_anime(&repo, &first)
                .expect("ok")
                .len(),
            1
        );
        assert_eq!(get_library_anime(&repo).expect("ok").len(), 2);
    }

    #[test]
    fn seasons_facade_returns_children_and_an_empty_list_for_no_parent() {
        let (_dir, repo) = anime_repo();
        let parent = repo
            .insert(&Anime {
                fetch_type: kuro_db::schema_anime::FetchType::Seasons,
                ..anime("/p", "Parent", true)
            })
            .expect("insert");
        for n in ["/s1", "/s2"] {
            repo.insert(&Anime {
                parent_id: Some(parent),
                ..anime(n, n, false)
            })
            .expect("insert child");
        }
        assert_eq!(get_seasons_by_parent_id(&repo, parent).len(), 2);
        assert!(get_seasons_by_parent_id(&repo, parent + 999).is_empty());
    }

    #[test]
    fn manga_library_facade_lists_favourites() {
        let (_dir, repo) = manga_repo();
        repo.insert(&manga("/m/x", "X", true)).expect("insert");
        assert_eq!(get_library_manga(&repo).expect("ok").len(), 1);
    }

    #[test]
    fn every_public_repository_method_is_reachable() {
        // Compile-time coverage: every public method of MangaRepo (21) and AnimeRepo (24) is named here.
        let _manga = (
            MangaRepo::by_id as fn(&MangaRepo, i64) -> DataResult<Manga>,
            MangaRepo::by_url_and_source as fn(&MangaRepo, &str, i64) -> DataResult<Option<Manga>>,
            MangaRepo::favorites as fn(&MangaRepo) -> DataResult<Vec<Manga>>,
            MangaRepo::favorites_by_source as fn(&MangaRepo, i64) -> DataResult<Vec<Manga>>,
            MangaRepo::subscribe_by_id
                as fn(&MangaRepo, i64) -> kuro_db::subscribe::Subscription<Option<Manga>>,
            MangaRepo::subscribe_by_url_and_source
                as fn(&MangaRepo, &str, i64) -> kuro_db::subscribe::Subscription<Option<Manga>>,
            MangaRepo::subscribe_favorites_by_source
                as fn(&MangaRepo, i64) -> kuro_db::subscribe::Subscription<Vec<Manga>>,
            MangaRepo::subscribe_library
                as fn(&MangaRepo) -> kuro_db::subscribe::Subscription<LibraryState<LibraryManga>>,
            MangaRepo::read_not_in_library as fn(&MangaRepo) -> DataResult<Vec<Manga>>,
            MangaRepo::library as fn(&MangaRepo) -> DataResult<Vec<LibraryManga>>,
            MangaRepo::duplicates as fn(&MangaRepo, i64, &str) -> DataResult<Vec<Manga>>,
            MangaRepo::upcoming as fn(&MangaRepo, &[i64], i64) -> DataResult<Vec<Manga>>,
            MangaRepo::reset_viewer_flags as fn(&MangaRepo) -> DataResult<()>,
            MangaRepo::set_categories as fn(&MangaRepo, i64, &[i64]) -> DataResult<()>,
            MangaRepo::insert as fn(&MangaRepo, &Manga) -> DataResult<i64>,
            MangaRepo::update as fn(&MangaRepo, &MangaUpdate) -> DataResult<()>,
            MangaRepo::update_all as fn(&MangaRepo, &[MangaUpdate]) -> DataResult<()>,
            MangaRepo::all_source_and_url as fn(&MangaRepo) -> DataResult<Vec<(i64, String)>>,
            MangaRepo::delete_non_library_by_sources as fn(&MangaRepo, &[i64]) -> DataResult<()>,
            MangaRepo::favorite_counts_by_source as fn(&MangaRepo) -> DataResult<Vec<(i64, i64)>>,
            MangaRepo::non_library_counts_by_source
                as fn(&MangaRepo) -> DataResult<Vec<(i64, i64)>>,
            MangaRepo::lookup_or_insert
                as fn(&MangaRepo, &Manga) -> DataResult<LookupOrInsert<Manga>>,
        );
        let _anime = (
            AnimeRepo::by_id as fn(&AnimeRepo, i64) -> DataResult<Anime>,
            AnimeRepo::by_url_and_source as fn(&AnimeRepo, &str, i64) -> DataResult<Option<Anime>>,
            AnimeRepo::subscribe_by_id
                as fn(&AnimeRepo, i64) -> kuro_db::subscribe::Subscription<Option<Anime>>,
            AnimeRepo::subscribe_by_url_and_source
                as fn(&AnimeRepo, &str, i64) -> kuro_db::subscribe::Subscription<Option<Anime>>,
            AnimeRepo::subscribe_favorites_by_source
                as fn(&AnimeRepo, i64) -> kuro_db::subscribe::Subscription<Vec<Anime>>,
            AnimeRepo::subscribe_library
                as fn(&AnimeRepo) -> kuro_db::subscribe::Subscription<LibraryState<LibraryAnime>>,
            AnimeRepo::subscribe_seasons_by_parent
                as fn(&AnimeRepo, i64) -> kuro_db::subscribe::Subscription<Vec<SeasonAnime>>,
            AnimeRepo::favorites as fn(&AnimeRepo) -> DataResult<Vec<Anime>>,
            AnimeRepo::favorites_by_source as fn(&AnimeRepo, i64) -> DataResult<Vec<Anime>>,
            AnimeRepo::watched_not_in_library as fn(&AnimeRepo) -> DataResult<Vec<Anime>>,
            AnimeRepo::library as fn(&AnimeRepo) -> DataResult<Vec<LibraryAnime>>,
            AnimeRepo::duplicates as fn(&AnimeRepo, i64, &str) -> DataResult<Vec<Anime>>,
            AnimeRepo::upcoming as fn(&AnimeRepo, &[i64], i64) -> DataResult<Vec<Anime>>,
            AnimeRepo::reset_viewer_flags as fn(&AnimeRepo) -> DataResult<()>,
            AnimeRepo::set_categories as fn(&AnimeRepo, i64, &[i64]) -> DataResult<()>,
            AnimeRepo::insert as fn(&AnimeRepo, &Anime) -> DataResult<i64>,
            AnimeRepo::update as fn(&AnimeRepo, &AnimeUpdate) -> DataResult<()>,
            AnimeRepo::update_all as fn(&AnimeRepo, &[AnimeUpdate]) -> DataResult<()>,
            AnimeRepo::seasons_by_parent as fn(&AnimeRepo, i64) -> DataResult<Vec<SeasonAnime>>,
            AnimeRepo::children_by_parent as fn(&AnimeRepo, i64) -> DataResult<Vec<Anime>>,
            AnimeRepo::remove_parent_ids as fn(&AnimeRepo, &[i64]) -> DataResult<()>,
            AnimeRepo::deletable_parents as fn(&AnimeRepo) -> DataResult<Vec<DeletableEntry>>,
            AnimeRepo::delete_non_library_by_ids as fn(&AnimeRepo, &[i64]) -> DataResult<()>,
            AnimeRepo::all_source_and_url as fn(&AnimeRepo) -> DataResult<Vec<(i64, String)>>,
            AnimeRepo::favorite_counts_by_source as fn(&AnimeRepo) -> DataResult<Vec<(i64, i64)>>,
            AnimeRepo::lookup_or_insert
                as fn(&AnimeRepo, &Anime) -> DataResult<LookupOrInsert<Anime>>,
        );
    }
}

#[cfg(test)]
mod awaits_and_joins {
    use std::sync::Arc;
    use std::time::Duration;

    use kuro_data::DataError;
    use kuro_data::anime::{Anime, AnimeUpdate};
    use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
    use kuro_data::episode::EpisodeRepo;
    use kuro_data::manga::{Manga, MangaRepo};
    use kuro_db::statements_anime::{EpisodeRow, insert_episode};
    use kuro_db::statements_manga::{ChapterRow, chapters};
    use tokio::sync::watch;

    use super::*;
    use crate::testdb::{anime_db, manga_db};

    fn manga_row(url: &str) -> Manga {
        Manga {
            url: url.into(),
            title: "T".into(),
            source: 1,
            ..Manga::create()
        }
    }

    fn chapter_row(manga_id: i64, url: &str) -> ChapterRow {
        ChapterRow {
            id: -1,
            manga_id,
            url: url.into(),
            name: url.into(),
            scanlator: None,
            read: false,
            bookmark: false,
            last_page_read: 0,
            chapter_number: 1.0,
            source_order: 0,
            date_fetch: 0,
            date_upload: 0,
            last_modified_at: 0,
            version: 0,
            is_syncing: false,
        }
    }

    fn episode_row(anime_id: i64, url: &str) -> EpisodeRow {
        EpisodeRow {
            anime_id,
            url: url.into(),
            name: url.into(),
            episode_number: 1.0,
            ..Default::default()
        }
    }

    /// Waits until a published value satisfies `done`, skipping intermediate ones.
    async fn wait_for<T>(rx: &mut watch::Receiver<Option<Result<T, String>>>, done: impl Fn(&T) -> bool) -> Arc<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        loop {
            if let Some(value) = rx.borrow_and_update().clone() {
                let value = value.expect("the published value is not an error");
                if done(&value) {
                    return Arc::new(value);
                }
            }
            tokio::time::timeout(Duration::from_secs(5), rx.changed())
                .await
                .expect("the expected value within 5 s")
                .expect("sender alive");
        }
    }

    #[test]
    fn await_manga_and_anime_pass_the_missing_row_error_through() {
        let (_d, db) = manga_db();
        let mangas = MangaRepo::new(db.clone());
        let id = mangas.insert(&manga_row("/m")).expect("insert");
        assert_eq!(await_manga(&mangas, id).expect("row").url, "/m");
        assert!(matches!(await_manga(&mangas, id + 99), Err(DataError::NotFound)));

        let (_d2, adb) = anime_db();
        let animes = AnimeRepo::new(adb);
        assert!(matches!(await_anime(&animes, 99), Err(DataError::NotFound)));
    }

    #[test]
    fn await_chapters_episodes_and_seasons_pass_results_through() {
        let (_d, db) = manga_db();
        let mangas = MangaRepo::new(db.clone());
        let mid = mangas.insert(&manga_row("/m")).expect("insert");
        db.write(|tx| Ok(chapters::insert(tx, &chapter_row(mid, "c1"))?)).expect("chapter");
        let chapter_repo = ChapterRepo::new(db);
        assert_eq!(await_chapters(&chapter_repo, mid, false).expect("ok").len(), 1);
        assert!(await_chapters(&chapter_repo, mid + 99, false).expect("ok").is_empty());

        let (_d2, adb) = anime_db();
        let animes = AnimeRepo::new(adb.clone());
        let aid = animes
            .insert(&Anime {
                url: "/a".into(),
                title: "A".into(),
                source: 1,
                ..Anime::create()
            })
            .expect("insert");
        adb.write(|tx| Ok(insert_episode(tx, &episode_row(aid, "e1"))?)).expect("episode");
        let episode_repo = EpisodeRepo::new(adb);
        assert_eq!(await_episodes(&episode_repo, aid).expect("ok").len(), 1);
        assert!(await_seasons(&animes, aid).expect("ok").is_empty());
    }

    #[test]
    fn seasons_facade_is_empty_when_the_read_fails() {
        let (_d, db) = anime_db();
        db.write(|tx| Ok(tx.execute_batch("DROP VIEW animeseasonsView;")?))
            .expect("drop view");
        let repo = AnimeRepo::new(db);
        assert!(get_seasons_by_parent_id(&repo, 1).is_empty(), "logged, then an empty list");
    }

    #[test]
    fn await_chapters_passes_a_database_error_through() {
        let (_d, db) = manga_db();
        db.write(|tx| Ok(tx.execute_batch("ALTER TABLE chapters RENAME TO chapters_gone;")?))
            .expect("rename");
        let repo = ChapterRepo::new(db);
        assert!(await_chapters(&repo, 1, false).is_err(), "the error reaches the caller");
    }

    #[tokio::test]
    async fn manga_with_chapters_emits_after_both_then_on_a_read_toggle() {
        let (_d, db) = manga_db();
        let mangas = MangaRepo::new(db.clone());
        let mid = mangas.insert(&manga_row("/m")).expect("insert");
        let cid = db.write(|tx| Ok(chapters::insert(tx, &chapter_row(mid, "c1"))?)).expect("chapter");
        let chapter_repo = ChapterRepo::new(db);
        let (mut rx, _handle) = subscribe_manga_with_chapters(&mangas, &chapter_repo, mid, false);
        let pair = wait_for(&mut rx, |(m, c)| m.is_some() && !c.is_empty()).await;
        assert_eq!(pair.0.as_ref().as_ref().map(|m| m.url.clone()), Some("/m".into()));
        assert!(!pair.1[0].read);

        chapter_repo
            .update(&ChapterUpdate { read: Some(true), ..ChapterUpdate::new(cid) })
            .expect("toggle");
        let toggled = wait_for(&mut rx, |(_, c)| c.first().is_some_and(|c| c.read)).await;
        assert!(toggled.1[0].read, "a change of the chapters publishes a new pair");
    }

    #[tokio::test]
    async fn dropping_the_manga_with_chapters_receiver_stops_its_task() {
        let (_d, db) = manga_db();
        let mangas = MangaRepo::new(db.clone());
        let mid = mangas.insert(&manga_row("/m")).expect("insert");
        let (rx, handle) = subscribe_manga_with_chapters(&mangas, &ChapterRepo::new(db), mid, false);
        drop(rx);
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("task ends")
            .expect("no panic");
    }

    #[tokio::test]
    async fn anime_with_episodes_and_seasons_emits_after_all_three_then_on_an_episode_insert() {
        let (_d, db) = anime_db();
        let animes = AnimeRepo::new(db.clone());
        let aid = animes
            .insert(&Anime {
                url: "/a".into(),
                title: "A".into(),
                source: 1,
                ..Anime::create()
            })
            .expect("insert");
        let episodes = EpisodeRepo::new(db.clone());
        let (mut rx, _handle) = subscribe_anime_with_episodes_and_seasons(&animes, &episodes, aid);
        let first = wait_for(&mut rx, |(a, e, s)| a.is_some() && e.is_empty() && s.is_empty()).await;
        assert_eq!(first.0.as_ref().as_ref().map(|a| a.url.clone()), Some("/a".into()));

        db.write(|tx| Ok(insert_episode(tx, &episode_row(aid, "e1"))?)).expect("episode");
        let next = wait_for(&mut rx, |(_, e, _)| e.len() == 1).await;
        assert_eq!(next.1.len(), 1, "an episode insert publishes a new triple");

        animes
            .update(&AnimeUpdate {
                title: Some("A2".into()),
                ..AnimeUpdate::new(aid)
            })
            .expect("rename");
        let renamed = wait_for(&mut rx, |(a, _, _)| a.as_ref().as_ref().is_some_and(|a| a.title == "A2")).await;
        assert_eq!(renamed.0.as_ref().as_ref().map(|a| a.title.clone()), Some("A2".into()));
    }

    #[tokio::test]
    async fn dropping_the_anime_joined_receiver_stops_its_task() {
        let (_d, db) = anime_db();
        let animes = AnimeRepo::new(db.clone());
        let aid = animes
            .insert(&Anime {
                url: "/a".into(),
                title: "A".into(),
                source: 1,
                ..Anime::create()
            })
            .expect("insert");
        let (rx, handle) = subscribe_anime_with_episodes_and_seasons(&animes, &EpisodeRepo::new(db), aid);
        drop(rx);
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("task ends")
            .expect("no panic");
    }
}
