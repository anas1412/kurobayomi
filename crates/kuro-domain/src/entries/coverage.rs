//! Repository coverage matrix (T-07a-033). Each Kotlin repository method of the four `domain` repositories
//! (manga 16, anime 21, chapter 11, episode 9 = 57) maps to a Rust method of `kuro-data`. Signature differences:
//! a Kotlin `suspend fun ...AsFlow` returns a `Subscription` (`subscribe_*`); a Kotlin `suspend fun` returns a
//! `DataResult` (the Rust method is synchronous and runs on the reader or writer connection); a Kotlin `Unit`
//! update returns `DataResult<()>`, and a Kotlin `Boolean` update returns `DataResult<bool>` only where the flag
//! setters need it (see `flag_setters.rs`).
//!
//! `None` marks a method that `kuro-data` does not have yet. Those gaps are owned by area 05 and are listed in
//! `KNOWN_GAPS`; the test fails for any other gap, and names the Kotlin method in the message.

/// (Kotlin method, Rust method). `None` is a gap.
pub const MATRIX: &[(&str, Option<&str>)] = &[
    // MangaRepository (16)
    ("getMangaById", Some("MangaRepo::by_id")),
    ("getMangaByIdAsFlow", Some("MangaRepo::subscribe_by_id")),
    ("getMangaByUrlAndSourceId", Some("MangaRepo::by_url_and_source")),
    ("getMangaByUrlAndSourceIdAsFlow", Some("MangaRepo::subscribe_by_url_and_source")),
    ("getMangaFavorites", Some("MangaRepo::favorites")),
    ("getReadMangaNotInLibrary", Some("MangaRepo::read_not_in_library")),
    ("getLibraryManga", Some("MangaRepo::library")),
    ("getLibraryMangaAsFlow", Some("MangaRepo::subscribe_library")),
    ("getMangaFavoritesBySourceId", Some("MangaRepo::favorites_by_source")),
    ("getDuplicateLibraryManga", Some("MangaRepo::duplicates")),
    ("getUpcomingManga", Some("MangaRepo::upcoming")),
    ("resetMangaViewerFlags", Some("MangaRepo::reset_viewer_flags")),
    ("setMangaCategories", Some("MangaRepo::set_categories")),
    ("insertManga", Some("MangaRepo::insert")),
    ("updateManga", Some("MangaRepo::update")),
    ("updateAllManga", Some("MangaRepo::update_all")),
    // AnimeRepository (21)
    ("getAnimeById", Some("AnimeRepo::by_id")),
    ("getAnimeByIdAsFlow", Some("AnimeRepo::subscribe_by_id")),
    ("getAnimeByUrlAndSourceId", Some("AnimeRepo::by_url_and_source")),
    ("getAnimeByUrlAndSourceIdAsFlow", Some("AnimeRepo::subscribe_by_url_and_source")),
    ("getAnimeFavorites", Some("AnimeRepo::favorites")),
    ("getWatchedAnimeNotInLibrary", Some("AnimeRepo::watched_not_in_library")),
    ("getLibraryAnime", Some("AnimeRepo::library")),
    ("getLibraryAnimeAsFlow", Some("AnimeRepo::subscribe_library")),
    ("getAnimeFavoritesBySourceId", Some("AnimeRepo::favorites_by_source")),
    ("getDuplicateLibraryAnime", Some("AnimeRepo::duplicates")),
    ("getUpcomingAnime", Some("AnimeRepo::upcoming")),
    ("resetAnimeViewerFlags", Some("AnimeRepo::reset_viewer_flags")),
    ("setAnimeCategories", Some("AnimeRepo::set_categories")),
    ("insertAnime", Some("AnimeRepo::insert")),
    ("updateAnime", Some("AnimeRepo::update")),
    ("updateAllAnime", Some("AnimeRepo::update_all")),
    ("getAnimeSeasonsById", Some("AnimeRepo::seasons_by_parent")),
    ("getAnimeSeasonsByIdAsFlow", Some("AnimeRepo::subscribe_seasons_by_parent")),
    ("removeParentIdByIds", Some("AnimeRepo::remove_parent_ids")),
    ("getDeletableParentAnime", Some("AnimeRepo::deletable_parents")),
    ("getChildrenByParentId", Some("AnimeRepo::children_by_parent")),
    // ChapterRepository (11)
    ("addAllChapters", None),
    ("updateChapter", Some("ChapterRepo::update")),
    ("updateAllChapters", Some("ChapterRepo::update_all")),
    ("removeChaptersWithIds", None),
    ("getChapterByMangaId", Some("ChapterRepo::by_manga_id")),
    ("getScanlatorsByMangaId", None),
    ("getScanlatorsByMangaIdAsFlow", None),
    ("getBookmarkedChaptersByMangaId", None),
    ("getChapterById", Some("ChapterRepo::by_id")),
    ("getChapterByMangaIdAsFlow", Some("ChapterRepo::subscribe_by_manga_id")),
    ("getChapterByUrlAndMangaId", Some("ChapterRepo::by_url_and_manga_id")),
    // EpisodeRepository (9)
    ("addAllEpisodes", None),
    ("updateEpisode", Some("EpisodeRepo::update")),
    ("updateAllEpisodes", Some("EpisodeRepo::update_all")),
    ("removeEpisodesWithIds", None),
    ("getEpisodeByAnimeId", Some("EpisodeRepo::by_anime_id")),
    ("getBookmarkedEpisodesByAnimeId", None),
    ("getEpisodeById", Some("EpisodeRepo::by_id")),
    ("getEpisodeByAnimeIdAsFlow", Some("EpisodeRepo::subscribe_by_anime_id")),
    ("getEpisodeByUrlAndAnimeId", Some("EpisodeRepo::by_url_and_anime_id")),
];

/// Gaps that are known and owned by area 05 (`kuro-data`). Adding a gap here needs a reason; the test rejects any
/// gap not in this list.
pub const KNOWN_GAPS: &[&str] = &[
    "addAllChapters",
    "removeChaptersWithIds",
    "getScanlatorsByMangaId",
    "getScanlatorsByMangaIdAsFlow",
    "getBookmarkedChaptersByMangaId",
    "addAllEpisodes",
    "removeEpisodesWithIds",
    "getBookmarkedEpisodesByAnimeId",
];

#[cfg(test)]
mod tests {
    use kuro_data::anime::AnimeRepo;
    use kuro_data::chapter::ChapterRepo;
    use kuro_data::episode::EpisodeRepo;
    use kuro_data::manga::MangaRepo;

    use super::*;

    /// Each mapped Rust method is named here, so a rename or removal fails to compile.
    #[allow(clippy::let_unit_value, unused_variables)]
    fn compile_checked_references() {
        let _ = MangaRepo::by_id;
        let _ = MangaRepo::subscribe_by_id;
        let _ = MangaRepo::by_url_and_source;
        let _ = MangaRepo::subscribe_by_url_and_source;
        let _ = MangaRepo::favorites;
        let _ = MangaRepo::read_not_in_library;
        let _ = MangaRepo::library;
        let _ = MangaRepo::subscribe_library;
        let _ = MangaRepo::favorites_by_source;
        let _ = MangaRepo::duplicates;
        let _ = MangaRepo::upcoming;
        let _ = MangaRepo::reset_viewer_flags;
        let _ = MangaRepo::set_categories;
        let _ = MangaRepo::insert;
        let _ = MangaRepo::update;
        let _ = MangaRepo::update_all;
        let _ = AnimeRepo::by_id;
        let _ = AnimeRepo::subscribe_by_id;
        let _ = AnimeRepo::by_url_and_source;
        let _ = AnimeRepo::subscribe_by_url_and_source;
        let _ = AnimeRepo::favorites;
        let _ = AnimeRepo::watched_not_in_library;
        let _ = AnimeRepo::library;
        let _ = AnimeRepo::subscribe_library;
        let _ = AnimeRepo::favorites_by_source;
        let _ = AnimeRepo::duplicates;
        let _ = AnimeRepo::upcoming;
        let _ = AnimeRepo::reset_viewer_flags;
        let _ = AnimeRepo::set_categories;
        let _ = AnimeRepo::insert;
        let _ = AnimeRepo::update;
        let _ = AnimeRepo::update_all;
        let _ = AnimeRepo::seasons_by_parent;
        let _ = AnimeRepo::subscribe_seasons_by_parent;
        let _ = AnimeRepo::remove_parent_ids;
        let _ = AnimeRepo::deletable_parents;
        let _ = AnimeRepo::children_by_parent;
        let _ = ChapterRepo::update;
        let _ = ChapterRepo::update_all;
        let _ = ChapterRepo::by_manga_id;
        let _ = ChapterRepo::by_id;
        let _ = ChapterRepo::subscribe_by_manga_id;
        let _ = ChapterRepo::by_url_and_manga_id;
        let _ = EpisodeRepo::update;
        let _ = EpisodeRepo::update_all;
        let _ = EpisodeRepo::by_anime_id;
        let _ = EpisodeRepo::by_id;
        let _ = EpisodeRepo::subscribe_by_anime_id;
        let _ = EpisodeRepo::by_url_and_anime_id;
    }

    #[test]
    fn the_matrix_covers_all_57_kotlin_methods() {
        const MANGA: usize = 16;
        const ANIME: usize = 21;
        const CHAPTER: usize = 11;
        const EPISODE: usize = 9;
        assert_eq!(MANGA + ANIME + CHAPTER + EPISODE, 57);
        assert_eq!(MATRIX.len(), 57);
        assert_eq!(MATRIX[MANGA].0, "getAnimeById", "anime rows start after the 16 manga rows");
        assert_eq!(MATRIX[MANGA + ANIME].0, "addAllChapters", "chapter rows follow the 21 anime rows");
    }

    #[test]
    fn every_gap_is_a_known_gap_and_every_mapping_is_named() {
        let gaps: Vec<&str> = MATRIX.iter().filter(|(_, r)| r.is_none()).map(|(k, _)| *k).collect();
        for kotlin in &gaps {
            assert!(KNOWN_GAPS.contains(kotlin), "unmapped Kotlin method {kotlin} is not in KNOWN_GAPS");
        }
        assert_eq!(gaps.len(), KNOWN_GAPS.len(), "a known gap was fixed: remove it from KNOWN_GAPS");
        assert!(MATRIX.iter().filter_map(|(_, r)| *r).all(|r| r.contains("Repo::")));
        compile_checked_references();
    }
}
