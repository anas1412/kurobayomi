//! Chapter and episode facades (`GetChapter.kt`, `GetChapterByUrlAndMangaId.kt`, `GetChaptersByMangaId.kt`,
//! `UpdateChapter.kt` and the episode twins). Error contracts follow `p1-behavior.md` section 4: reads log and
//! return `None` or an empty list, the by-url variant without a log stays silent, and writes log and return `()`.

use kuro_data::chapter::{Chapter, ChapterRepo, ChapterUpdate};
use kuro_data::episode::{Episode, EpisodeRepo, EpisodeUpdate};

/// `GetChapter.await(id)`: `None` for a missing row or on error (the error is logged).
pub fn get_chapter(repo: &ChapterRepo, id: i64) -> Option<Chapter> {
    match repo.by_id(id) {
        Ok(chapter) => chapter,
        Err(e) => {
            tracing::error!(id, error = %e, "cannot read chapter");
            None
        }
    }
}

/// `GetChapter.await(url, mangaId)`: `None` for no match or on error (the error is logged).
pub fn get_chapter_by_url(repo: &ChapterRepo, url: &str, manga_id: i64) -> Option<Chapter> {
    match repo.by_url_and_manga_id(url, manga_id) {
        Ok(chapter) => chapter,
        Err(e) => {
            tracing::error!(url, manga_id, error = %e, "cannot read chapter by url");
            None
        }
    }
}

/// `GetChapterByUrlAndMangaId.await(url, mangaId)`: `None` on error, and the error is not logged (Kotlin swallows it).
pub fn get_chapter_by_url_and_manga_id(repo: &ChapterRepo, url: &str, manga_id: i64) -> Option<Chapter> {
    repo.by_url_and_manga_id(url, manga_id).ok().flatten()
}

/// `GetChaptersByMangaId.await(mangaId, applyScanlatorFilter)`: an empty list on error (the error is logged).
pub fn get_chapters_by_manga_id(repo: &ChapterRepo, manga_id: i64, apply_scanlator_filter: bool) -> Vec<Chapter> {
    match repo.by_manga_id(manga_id, apply_scanlator_filter) {
        Ok(chapters) => chapters,
        Err(e) => {
            tracing::error!(manga_id, error = %e, "cannot read chapters");
            Vec::new()
        }
    }
}

/// `UpdateChapter.await(update)`: the error is logged and swallowed, so the caller cannot tell a failed write.
pub fn update_chapter(repo: &ChapterRepo, update: &ChapterUpdate) {
    if let Err(e) = repo.update(update) {
        tracing::error!(id = update.id, error = %e, "cannot update chapter");
    }
}

/// `UpdateChapter.awaitAll(updates)`: one transaction. An error is logged and swallowed, as for one chapter.
pub fn update_all_chapters(repo: &ChapterRepo, updates: &[ChapterUpdate]) {
    if let Err(e) = repo.update_all(updates) {
        tracing::error!(count = updates.len(), error = %e, "cannot update chapters");
    }
}

/// `GetEpisode.await(id)`: `None` for a missing row or on error (the error is logged).
pub fn get_episode(repo: &EpisodeRepo, id: i64) -> Option<Episode> {
    match repo.by_id(id) {
        Ok(episode) => episode,
        Err(e) => {
            tracing::error!(id, error = %e, "cannot read episode");
            None
        }
    }
}

/// `GetEpisode.await(url, animeId)`: `None` for no match or on error (the error is logged).
pub fn get_episode_by_url(repo: &EpisodeRepo, url: &str, anime_id: i64) -> Option<Episode> {
    match repo.by_url_and_anime_id(url, anime_id) {
        Ok(episode) => episode,
        Err(e) => {
            tracing::error!(url, anime_id, error = %e, "cannot read episode by url");
            None
        }
    }
}

/// `GetEpisodeByUrlAndAnimeId.await(url, animeId)`: `None` on error, and the error is not logged (Kotlin swallows it).
pub fn get_episode_by_url_and_anime_id(repo: &EpisodeRepo, url: &str, anime_id: i64) -> Option<Episode> {
    repo.by_url_and_anime_id(url, anime_id).ok().flatten()
}

/// `GetEpisodesByAnimeId.await(animeId)`: an empty list on error (the error is logged).
pub fn get_episodes_by_anime_id(repo: &EpisodeRepo, anime_id: i64) -> Vec<Episode> {
    match repo.by_anime_id(anime_id) {
        Ok(episodes) => episodes,
        Err(e) => {
            tracing::error!(anime_id, error = %e, "cannot read episodes");
            Vec::new()
        }
    }
}

/// `UpdateEpisode.await(update)`: the error is logged and swallowed, so the caller cannot tell a failed write.
pub fn update_episode(repo: &EpisodeRepo, update: &EpisodeUpdate) {
    if let Err(e) = repo.update(update) {
        tracing::error!(id = update.id, error = %e, "cannot update episode");
    }
}

/// `UpdateEpisode.awaitAll(updates)`: one transaction. An error is logged and swallowed, as for one episode.
pub fn update_all_episodes(repo: &EpisodeRepo, updates: &[EpisodeUpdate]) {
    if let Err(e) = repo.update_all(updates) {
        tracing::error!(count = updates.len(), error = %e, "cannot update episodes");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_data::anime::{Anime, AnimeRepo};
    use kuro_data::chapter::ChapterUpdate;
    use kuro_data::episode::EpisodeUpdate;
    use kuro_data::manga::{Manga, MangaRepo};
    use kuro_db::handle::Db;
    use kuro_db::statements_anime::{EpisodeRow, insert_episode};
    use kuro_db::statements_manga::{ChapterRow, chapters, excluded_scanlators};

    use super::*;
    use crate::testdb::{anime_db, manga_db};

    fn chapter_row(manga_id: i64, url: &str, scanlator: Option<&str>) -> ChapterRow {
        ChapterRow {
            id: -1,
            manga_id,
            url: url.into(),
            name: url.into(),
            scanlator: scanlator.map(str::to_owned),
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

    /// A manga with two chapters, one of them from scanlator `Excluded`. Returns the kept chapter's id.
    fn manga_with_chapters() -> (tempfile::TempDir, Arc<Db>, i64, i64) {
        let (dir, db) = manga_db();
        let manga_id = MangaRepo::new(db.clone())
            .insert(&Manga {
                url: "/m".into(),
                title: "M".into(),
                source: 1,
                ..Manga::create()
            })
            .expect("manga");
        let kept = db
            .write(|tx| {
                let kept = chapters::insert(tx, &chapter_row(manga_id, "kept", Some("Kept")))?;
                chapters::insert(tx, &chapter_row(manga_id, "gone", Some("Excluded")))?;
                excluded_scanlators::insert(tx, manga_id, "Excluded")?;
                Ok(kept)
            })
            .expect("chapters");
        (dir, db, manga_id, kept)
    }

    /// Makes every read and write of the chapters table fail (its name no longer exists).
    fn break_chapters(db: &Db) {
        db.write(|tx| Ok(tx.execute_batch("ALTER TABLE chapters RENAME TO chapters_gone;")?))
            .expect("rename");
    }

    fn anime_with_episodes() -> (tempfile::TempDir, Arc<Db>, i64, i64) {
        let (dir, db) = anime_db();
        let anime_id = AnimeRepo::new(db.clone())
            .insert(&Anime {
                url: "/a".into(),
                title: "A".into(),
                source: 1,
                ..Anime::create()
            })
            .expect("anime");
        let episode_id = db
            .write(|tx| Ok(insert_episode(tx, &episode_row(anime_id, "e1"))?))
            .expect("episode");
        (dir, db, anime_id, episode_id)
    }

    #[test]
    fn chapter_reads_by_id_and_by_url() {
        let (_d, db, manga_id, kept) = manga_with_chapters();
        let repo = ChapterRepo::new(db);
        assert_eq!(get_chapter(&repo, kept).map(|c| c.url), Some("kept".into()));
        assert_eq!(get_chapter(&repo, 9_999), None);
        assert_eq!(get_chapter_by_url(&repo, "kept", manga_id).map(|c| c.id), Some(kept));
        assert_eq!(get_chapter_by_url_and_manga_id(&repo, "kept", manga_id).map(|c| c.id), Some(kept));
        assert_eq!(get_chapter_by_url_and_manga_id(&repo, "kept", manga_id + 1), None);
    }

    #[test]
    fn chapter_reads_fail_quietly_to_none_on_error() {
        let (_d, db, manga_id, kept) = manga_with_chapters();
        break_chapters(&db);
        let repo = ChapterRepo::new(db);
        assert_eq!(get_chapter(&repo, kept), None);
        assert_eq!(get_chapter_by_url(&repo, "kept", manga_id), None);
        assert_eq!(get_chapter_by_url_and_manga_id(&repo, "kept", manga_id), None);
    }

    #[test]
    fn chapters_by_manga_id_apply_the_scanlator_filter_and_empty_on_error() {
        let (_d, db, manga_id, _) = manga_with_chapters();
        let repo = ChapterRepo::new(db.clone());
        assert_eq!(get_chapters_by_manga_id(&repo, manga_id, false).len(), 2);
        let filtered = get_chapters_by_manga_id(&repo, manga_id, true);
        assert_eq!(filtered.iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), ["kept"]);
        break_chapters(&db);
        assert!(get_chapters_by_manga_id(&repo, manga_id, true).is_empty());
    }

    #[test]
    fn update_chapter_writes_and_swallows_errors() {
        let (_d, db, _, kept) = manga_with_chapters();
        let repo = ChapterRepo::new(db.clone());
        update_chapter(&repo, &ChapterUpdate { read: Some(true), ..ChapterUpdate::new(kept) });
        assert!(get_chapter(&repo, kept).expect("row").read);
        break_chapters(&db);
        update_chapter(&repo, &ChapterUpdate { read: Some(false), ..ChapterUpdate::new(kept) });
    }

    #[test]
    fn update_all_chapters_writes_a_batch_and_swallows_errors() {
        let (_d, db, _, kept) = manga_with_chapters();
        let repo = ChapterRepo::new(db.clone());
        update_all_chapters(&repo, &[ChapterUpdate { bookmark: Some(true), ..ChapterUpdate::new(kept) }]);
        assert!(get_chapter(&repo, kept).expect("row").bookmark);
        break_chapters(&db);
        update_all_chapters(&repo, &[ChapterUpdate { bookmark: Some(false), ..ChapterUpdate::new(kept) }]);
    }

    #[test]
    fn episode_reads_by_id_and_by_url() {
        let (_d, db, anime_id, episode_id) = anime_with_episodes();
        let repo = EpisodeRepo::new(db);
        assert_eq!(get_episode(&repo, episode_id).map(|e| e.url), Some("e1".into()));
        assert_eq!(get_episode(&repo, 9_999), None);
        assert_eq!(get_episode_by_url(&repo, "e1", anime_id).map(|e| e.id), Some(episode_id));
        assert_eq!(get_episode_by_url_and_anime_id(&repo, "e1", anime_id).map(|e| e.id), Some(episode_id));
        assert_eq!(get_episode_by_url_and_anime_id(&repo, "e1", anime_id + 1), None);
        assert_eq!(get_episodes_by_anime_id(&repo, anime_id).len(), 1);
    }

    #[test]
    fn episode_reads_fail_quietly_on_error() {
        let (_d, db, anime_id, episode_id) = anime_with_episodes();
        db.write(|tx| Ok(tx.execute_batch("ALTER TABLE episodes RENAME TO episodes_gone;")?))
            .expect("rename");
        let repo = EpisodeRepo::new(db);
        assert_eq!(get_episode(&repo, episode_id), None);
        assert_eq!(get_episode_by_url(&repo, "e1", anime_id), None);
        assert_eq!(get_episode_by_url_and_anime_id(&repo, "e1", anime_id), None);
        assert!(get_episodes_by_anime_id(&repo, anime_id).is_empty());
    }

    #[test]
    fn update_episode_writes_and_update_all_episodes_swallows_errors() {
        let (_d, db, _, episode_id) = anime_with_episodes();
        let repo = EpisodeRepo::new(db.clone());
        update_episode(&repo, &EpisodeUpdate { seen: Some(true), ..EpisodeUpdate::new(episode_id) });
        assert!(get_episode(&repo, episode_id).expect("row").seen);
        db.write(|tx| Ok(tx.execute_batch("ALTER TABLE episodes RENAME TO episodes_gone;")?))
            .expect("rename");
        update_all_episodes(&repo, &[EpisodeUpdate { seen: Some(false), ..EpisodeUpdate::new(episode_id) }]);
        update_episode(&repo, &EpisodeUpdate { seen: Some(false), ..EpisodeUpdate::new(episode_id) });
    }
}
