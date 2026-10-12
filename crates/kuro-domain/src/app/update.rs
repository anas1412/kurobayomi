//! Entry metadata writes (`UpdateManga.kt`, `UpdateAnime.kt`; T-10a-007). Every write is a coalesce update, so a `None`
//! field keeps the stored column. The cover and background rules are `p1-behavior.md` section 3 (manga) and the area 17
//! rules of [`crate::source::remote_update`] (anime, the same five rows over the background image).
//!
//! The cache files (cover and background) are ports: [`CoverCache`] for manga, [`CacheFiles`] for anime. The rule
//! decides and the port deletes, so the decision can be tested without files.

use chrono::{DateTime, TimeZone};
use kuro_data::DataResult;
use kuro_data::anime::{Anime, AnimeRepo, AnimeUpdate};
use kuro_data::chapter::ChapterRepo;
use kuro_data::episode::EpisodeRepo;
use kuro_data::manga::{Manga, MangaRepo, MangaUpdate};
use kuro_source::anime::SAnime;
use kuro_source::model::SManga;

use crate::app::source::LOCAL_SOURCE_ID;
use crate::entries::fetch_interval::{get_window, to_anime_update, to_manga_update};
use crate::entries::source_convert::to_db_strategy;
use crate::source::remote_update::{CacheFiles, background_last_modified, cover_last_modified, metadata_update, title_update};

/// The cover files of a manga (`MangaCoverCache`, area 34). `delete_from_cache(manga, false)` keeps the user's custom cover.
pub trait CoverCache: Send + Sync {
    fn has_custom_cover(&self, manga: &Manga) -> bool;
    fn delete_from_cache(&self, manga: &Manga, delete_custom: bool);
}

/// `coverLastModified` of `UpdateManga.awaitUpdateFromSource`, the five rows in order. `None` keeps the stored timestamp.
pub fn manga_cover_last_modified(
    local: &Manga,
    remote: &SManga,
    manual: bool,
    covers: &dyn CoverCache,
    now_ms: i64,
) -> Option<i64> {
    let remote_thumb = remote.thumbnail_url.as_deref().filter(|url| !url.is_empty())?;
    if !manual && local.thumbnail_url.as_deref() == Some(remote_thumb) {
        return None;
    }
    if local.source == LOCAL_SOURCE_ID {
        return Some(now_ms);
    }
    if covers.has_custom_cover(local) {
        covers.delete_from_cache(local, false);
        return None;
    }
    covers.delete_from_cache(local, false);
    Some(now_ms)
}

/// The manga write of a library refresh (`UpdateManga`). Reads and writes go through the repositories; the interval
/// needs the chapters, so the chapter repository is held too.
#[derive(Clone)]
pub struct UpdateManga {
    repo: MangaRepo,
    chapters: ChapterRepo,
}

impl UpdateManga {
    pub fn new(repo: MangaRepo, chapters: ChapterRepo) -> Self {
        Self { repo, chapters }
    }

    /// `await(update)`: one coalesce write.
    pub fn update(&self, update: &MangaUpdate) -> DataResult<()> {
        self.repo.update(update)
    }

    /// `awaitAll(updates)`: every write in one transaction.
    pub fn update_all(&self, updates: &[MangaUpdate]) -> DataResult<()> {
        self.repo.update_all(updates)
    }

    /// `awaitUpdateFromSource(local, remote, manualFetch, coverCache)`. The title is written only for a non-favourite with
    /// a non-empty remote title (EC-18). Genres are written only when the remote genre text holds some (EC-19).
    pub fn update_from_source<Tz: TimeZone>(
        &self,
        local: &Manga,
        remote: &SManga,
        manual: bool,
        covers: &dyn CoverCache,
        now: &DateTime<Tz>,
    ) -> DataResult<()> {
        let title = (!local.favorite && !remote.title.is_empty()).then(|| remote.title.clone());
        let cover_last_modified = manga_cover_last_modified(local, remote, manual, covers, now.timestamp_millis());
        let update = MangaUpdate {
            title,
            cover_last_modified,
            author: remote.author.clone(),
            artist: remote.artist.clone(),
            description: remote.description.clone(),
            genre: kuro_source::model::manga::parse_genres(remote.genre.as_deref()),
            thumbnail_url: remote.thumbnail_url.clone().filter(|url| !url.is_empty()),
            status: Some(i64::from(remote.status)),
            update_strategy: Some(to_db_strategy(remote.update_strategy)),
            initialized: Some(true),
            ..MangaUpdate::new(local.id)
        };
        self.repo.update(&update)
    }

    /// `awaitUpdateFetchInterval(manga, now, window)`: writes `next_update` and `fetch_interval`. `None` is the default
    /// window of `getWindow(now)`.
    pub fn update_fetch_interval<Tz: TimeZone>(
        &self,
        manga: &Manga,
        now: &DateTime<Tz>,
        window: Option<(i64, i64)>,
    ) -> DataResult<()> {
        let window = window.unwrap_or_else(|| get_window(now));
        let update = to_manga_update(&self.chapters, manga, now, window, now.timestamp_millis());
        self.repo.update(&update)
    }

    /// `awaitUpdateLastUpdate(id)`: `last_update = now`.
    pub fn update_last_update<Tz: TimeZone>(&self, id: i64, now: &DateTime<Tz>) -> DataResult<()> {
        self.repo.update(&MangaUpdate { last_update: Some(now.timestamp_millis()), ..MangaUpdate::new(id) })
    }

    /// `awaitUpdateCoverLastModified(id)`: `cover_last_modified = now`.
    pub fn update_cover_last_modified<Tz: TimeZone>(&self, id: i64, now: &DateTime<Tz>) -> DataResult<()> {
        self.repo.update(&MangaUpdate { cover_last_modified: Some(now.timestamp_millis()), ..MangaUpdate::new(id) })
    }

    /// `awaitUpdateFavorite(id, favorite)`: `date_added` is now when favourited and 0 when not (EC-25).
    pub fn update_favorite<Tz: TimeZone>(&self, id: i64, favorite: bool, now: &DateTime<Tz>) -> DataResult<()> {
        let date_added = if favorite { now.timestamp_millis() } else { 0 };
        self.repo.update(&MangaUpdate { favorite: Some(favorite), date_added: Some(date_added), ..MangaUpdate::new(id) })
    }
}

/// The anime write of a remote refresh (`UpdateAnime`, `UpdateAnimeFromRemote.awaitUpdateFromSource`).
#[derive(Clone)]
pub struct UpdateAnime {
    repo: AnimeRepo,
    episodes: EpisodeRepo,
}

impl UpdateAnime {
    pub fn new(repo: AnimeRepo, episodes: EpisodeRepo) -> Self {
        Self { repo, episodes }
    }

    /// `await(update)`: one coalesce write.
    pub fn update(&self, update: &AnimeUpdate) -> DataResult<()> {
        self.repo.update(update)
    }

    /// `awaitAll(updates)`: every write in one transaction.
    pub fn update_all(&self, updates: &[AnimeUpdate]) -> DataResult<()> {
        self.repo.update_all(updates)
    }

    /// `awaitUpdateFromSource`: the cover and the background follow the same five rows, each with its own cache.
    /// The cache files are deleted before the write, as the rule says.
    pub fn update_from_source<Tz: TimeZone>(
        &self,
        local: &Anime,
        remote: &SAnime,
        manual: bool,
        files: &dyn CacheFiles,
        now: &DateTime<Tz>,
    ) -> DataResult<()> {
        let now_ms = now.timestamp_millis();
        let cover = cover_last_modified(local, remote.thumbnail_url.as_deref(), manual, || files.has_custom_cover(local), now_ms);
        let background = background_last_modified(
            local,
            remote.background_url.as_deref(),
            manual,
            || files.has_custom_background(local),
            now_ms,
        );
        if cover.deletes() {
            files.delete_cover(local);
        }
        if background.deletes() {
            files.delete_background(local);
        }
        let title = title_update(local, Some(remote.title.as_str()));
        self.repo.update(&metadata_update(local, remote, title, cover, background))
    }

    /// `awaitUpdateFetchInterval(anime, now, window)`, over every episode of the anime.
    pub fn update_fetch_interval<Tz: TimeZone>(
        &self,
        anime: &Anime,
        now: &DateTime<Tz>,
        window: Option<(i64, i64)>,
    ) -> DataResult<()> {
        let window = window.unwrap_or_else(|| get_window(now));
        let update = to_anime_update(&self.episodes, anime, now, window, now.timestamp_millis());
        self.repo.update(&update)
    }

    /// `awaitUpdateLastUpdate(id)`.
    pub fn update_last_update<Tz: TimeZone>(&self, id: i64, now: &DateTime<Tz>) -> DataResult<()> {
        self.repo.update(&AnimeUpdate { last_update: Some(now.timestamp_millis()), ..AnimeUpdate::new(id) })
    }

    /// `awaitUpdateCoverLastModified(id)`.
    pub fn update_cover_last_modified<Tz: TimeZone>(&self, id: i64, now: &DateTime<Tz>) -> DataResult<()> {
        self.repo.update(&AnimeUpdate { cover_last_modified: Some(now.timestamp_millis()), ..AnimeUpdate::new(id) })
    }

    /// `awaitUpdateBackgroundLastModified(id)`.
    pub fn update_background_last_modified<Tz: TimeZone>(&self, id: i64, now: &DateTime<Tz>) -> DataResult<()> {
        self.repo.update(&AnimeUpdate { background_last_modified: Some(now.timestamp_millis()), ..AnimeUpdate::new(id) })
    }

    /// `awaitUpdateFavorite(id, favorite)`: `date_added` is now when favourited and 0 when not (EC-25).
    pub fn update_favorite<Tz: TimeZone>(&self, id: i64, favorite: bool, now: &DateTime<Tz>) -> DataResult<()> {
        let date_added = if favorite { now.timestamp_millis() } else { 0 };
        self.repo.update(&AnimeUpdate { favorite: Some(favorite), date_added: Some(date_added), ..AnimeUpdate::new(id) })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use chrono::Local;
    use kuro_source::model::UpdateStrategy as SourceUpdateStrategy;

    use super::*;
    use crate::testdb::test_db;
    use kuro_data::anime::AnimeRepo;
    use kuro_data::episode::EpisodeRepo;
    use kuro_data::manga::MangaRepo;

    /// Records every `delete_from_cache` call and answers `has_custom_cover` with a fixed value.
    struct FakeCovers {
        custom: bool,
        deleted: Mutex<Vec<(i64, bool)>>,
    }

    impl FakeCovers {
        fn new(custom: bool) -> Self {
            Self { custom, deleted: Mutex::new(Vec::new()) }
        }
        fn deletions(&self) -> Vec<(i64, bool)> {
            self.deleted.lock().expect("lock").clone()
        }
    }

    impl CoverCache for FakeCovers {
        fn has_custom_cover(&self, _manga: &Manga) -> bool {
            self.custom
        }
        fn delete_from_cache(&self, manga: &Manga, delete_custom: bool) {
            self.deleted.lock().expect("lock").push((manga.id, delete_custom));
        }
    }

    fn at(ms: i64) -> DateTime<Local> {
        Local.timestamp_millis_opt(ms).single().expect("valid instant")
    }

    fn stored(thumb: Option<&str>, source: i64, favorite: bool) -> Manga {
        Manga { id: 7, source, favorite, thumbnail_url: thumb.map(str::to_owned), ..Manga::create() }
    }

    fn remote(thumb: Option<&str>, title: &str) -> SManga {
        SManga {
            thumbnail_url: thumb.map(str::to_owned),
            title: title.into(),
            ..SManga::new("/m", "M")
        }
    }

    #[test]
    fn cover_branches() {
        let now = 5_000;
        // Row 1: no remote thumbnail, nothing deleted.
        let covers = FakeCovers::new(false);
        assert_eq!(manga_cover_last_modified(&stored(Some("t"), 3, false), &remote(None, "M"), false, &covers, now), None);
        assert_eq!(manga_cover_last_modified(&stored(Some("t"), 3, false), &remote(Some(""), "M"), false, &covers, now), None);
        assert!(covers.deletions().is_empty());
        // Row 2: same URL and not a manual fetch, nothing deleted; a manual fetch goes on.
        assert_eq!(manga_cover_last_modified(&stored(Some("t"), 3, false), &remote(Some("t"), "M"), false, &covers, now), None);
        assert_eq!(
            manga_cover_last_modified(&stored(Some("t"), 3, false), &remote(Some("t"), "M"), true, &covers, now),
            Some(now),
            "manual fetch deletes and bumps"
        );
        // Row 3: local entry, timestamp only, no delete (EC-17).
        let local_covers = FakeCovers::new(false);
        assert_eq!(manga_cover_last_modified(&stored(None, LOCAL_SOURCE_ID, false), &remote(Some("n"), "M"), false, &local_covers, now), Some(now));
        assert!(local_covers.deletions().is_empty(), "EC-17: the local branch precedes the delete");
        // Row 4: a custom cover stays, the remote cache goes, no timestamp.
        let custom = FakeCovers::new(true);
        assert_eq!(manga_cover_last_modified(&stored(Some("old"), 3, false), &remote(Some("new"), "M"), false, &custom, now), None);
        assert_eq!(custom.deletions(), [(7, false)]);
        // Row 5: a new remote cover deletes the cache and bumps the timestamp.
        let plain = FakeCovers::new(false);
        assert_eq!(manga_cover_last_modified(&stored(Some("old"), 3, false), &remote(Some("new"), "M"), false, &plain, now), Some(now));
        assert_eq!(plain.deletions(), [(7, false)]);
    }

    #[test]
    fn a_favourite_keeps_its_title_and_a_browse_row_takes_the_remote_one() {
        let (_dir, db) = test_db();
        let repo = MangaRepo::new(db.clone());
        let chapters = ChapterRepo::new(db);
        let covers = FakeCovers::new(false);
        let id = repo.insert(&Manga { title: "Kept".into(), favorite: true, ..Manga::create() }).expect("insert");
        let local = repo.by_id(id).expect("read");
        let interactor = UpdateManga::new(repo.clone(), chapters.clone());
        interactor.update_from_source(&local, &remote(None, "Remote"), false, &covers, &at(9)).expect("update");
        assert_eq!(repo.by_id(id).expect("read").title, "Kept", "EC-18");

        let browse = repo.insert(&Manga { title: "Old".into(), favorite: false, url: "/browse".into(), ..Manga::create() }).expect("insert");
        let local = repo.by_id(browse).expect("read");
        interactor.update_from_source(&local, &remote(None, "Remote"), false, &covers, &at(9)).expect("update");
        assert_eq!(repo.by_id(browse).expect("read").title, "Remote");
    }

    #[test]
    fn a_blank_genre_text_leaves_the_stored_genres_alone() {
        let (_dir, db) = test_db();
        let repo = MangaRepo::new(db.clone());
        let interactor = UpdateManga::new(repo.clone(), ChapterRepo::new(db));
        let id = repo.insert(&Manga { genre: Some(vec!["Action".into()]), ..Manga::create() }).expect("insert");
        let local = repo.by_id(id).expect("read");
        let blank = SManga { genre: Some(String::new()), ..SManga::new("/m", "M") };
        interactor.update_from_source(&local, &blank, false, &FakeCovers::new(false), &at(9)).expect("update");
        assert_eq!(repo.by_id(id).expect("read").genre, Some(vec!["Action".to_owned()]), "EC-19: null genre update keeps it");
        let strategy = SManga { update_strategy: SourceUpdateStrategy::OnlyFetchOnce, ..SManga::new("/m", "M") };
        interactor.update_from_source(&local, &strategy, false, &FakeCovers::new(false), &at(9)).expect("update");
        assert_eq!(repo.by_id(id).expect("read").update_strategy, kuro_db::codecs::UpdateStrategy::OnlyFetchOnce);
    }

    #[test]
    fn favorite_sets_and_clears_date_added() {
        let (_dir, db) = test_db();
        let repo = MangaRepo::new(db.clone());
        let interactor = UpdateManga::new(repo.clone(), ChapterRepo::new(db));
        let id = repo.insert(&Manga::create()).expect("insert");
        interactor.update_favorite(id, true, &at(1_700)).expect("on");
        assert_eq!(repo.by_id(id).expect("read").date_added, 1_700, "EC-25");
        interactor.update_favorite(id, false, &at(1_800)).expect("off");
        assert_eq!(repo.by_id(id).expect("read").date_added, 0);
        interactor.update_last_update(id, &at(2_000)).expect("last update");
        assert_eq!(repo.by_id(id).expect("read").last_update, 2_000);
        interactor.update_cover_last_modified(id, &at(3_000)).expect("cover");
        assert_eq!(repo.by_id(id).expect("read").cover_last_modified, 3_000);
    }

    #[test]
    fn a_fetch_interval_update_writes_next_update_and_interval() {
        let (_dir, db) = test_db();
        let repo = MangaRepo::new(db.clone());
        let interactor = UpdateManga::new(repo.clone(), ChapterRepo::new(db));
        let id = repo.insert(&Manga::create()).expect("insert");
        let manga = repo.by_id(id).expect("read");
        interactor.update_fetch_interval(&manga, &at(1_700_000_000_000), None).expect("interval");
        let after = repo.by_id(id).expect("read");
        assert_eq!(after.fetch_interval, 7, "no chapters: the seven day default");
        assert!(after.next_update > 0);
    }

    #[test]
    fn anime_update_from_source_writes_the_remote_metadata_and_the_background_rule() {
        let (_dir, db) = test_db();
        let repo = AnimeRepo::new(db.clone());
        let episodes = EpisodeRepo::new(db);
        let interactor = UpdateAnime::new(repo.clone(), episodes);
        let id = repo.insert(&Anime { title: "Kept".into(), favorite: true, ..Anime::create() }).expect("insert");
        let local = repo.by_id(id).expect("read");
        let files = AnimeFiles::default();
        let remote = SAnime { title: "Remote".into(), background_url: Some("bg".into()), ..SAnime::default() };
        interactor.update_from_source(&local, &remote, false, &files, &at(9)).expect("update");
        assert_eq!(*files.deleted_covers.lock().expect("lock"), 0, "no remote cover, nothing deleted");
        let after = repo.by_id(id).expect("read");
        assert_eq!(after.title, "Kept", "a favourite keeps its title");
        assert_eq!(after.background_url.as_deref(), Some("bg"));
        assert_eq!(after.background_last_modified, 9, "a new background bumps its timestamp");
    }

    #[derive(Default)]
    struct AnimeFiles {
        deleted_covers: Mutex<usize>,
    }

    impl CacheFiles for AnimeFiles {
        fn has_custom_cover(&self, _anime: &Anime) -> bool {
            false
        }
        fn has_custom_background(&self, _anime: &Anime) -> bool {
            false
        }
        fn delete_cover(&self, _anime: &Anime) {
            *self.deleted_covers.lock().expect("lock") += 1;
        }
        fn delete_background(&self, _anime: &Anime) {}
    }
}
