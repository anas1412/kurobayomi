//! Repository coverage matrix (T-07a-033). Each Kotlin repository method of the four `domain` repositories
//! (manga 16, anime 21, chapter 11, episode 9 = 57) maps to a Rust method of `kuro-data`. Signature differences:
//! a Kotlin `suspend fun ...AsFlow` returns a `Subscription` (`subscribe_*`); a Kotlin `suspend fun` returns a
//! `DataResult` (the Rust method is synchronous and runs on the reader or writer connection); a Kotlin `Unit`
//! update returns `DataResult<()>`, and a Kotlin `Boolean` update returns `DataResult<bool>` only where the flag
//! setters need it (see `flag_setters.rs`).
//!
//! `None` marks a method that `kuro-data` does not have yet. Those gaps are owned by area 05 and are listed in
//! `KNOWN_GAPS`; the test fails for any other gap, and names the Kotlin method in the message. The signature
//! differences are in `SIGNATURE_DIFFERENCES`.

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
    ("addAllChapters", Some("ChapterRepo::add_all")),
    ("updateChapter", Some("ChapterRepo::update")),
    ("updateAllChapters", Some("ChapterRepo::update_all")),
    ("removeChaptersWithIds", Some("ChapterRepo::remove_with_ids")),
    ("getChapterByMangaId", Some("ChapterRepo::by_manga_id")),
    ("getScanlatorsByMangaId", Some("ChapterRepo::scanlators")),
    ("getScanlatorsByMangaIdAsFlow", None),
    ("getBookmarkedChaptersByMangaId", Some("ChapterRepo::bookmarked")),
    ("getChapterById", Some("ChapterRepo::by_id")),
    ("getChapterByMangaIdAsFlow", Some("ChapterRepo::subscribe_by_manga_id")),
    ("getChapterByUrlAndMangaId", Some("ChapterRepo::by_url_and_manga_id")),
    // EpisodeRepository (9)
    ("addAllEpisodes", Some("EpisodeRepo::add_all")),
    ("updateEpisode", Some("EpisodeRepo::update")),
    ("updateAllEpisodes", Some("EpisodeRepo::update_all")),
    ("removeEpisodesWithIds", Some("EpisodeRepo::remove_with_ids")),
    ("getEpisodeByAnimeId", Some("EpisodeRepo::by_anime_id")),
    ("getBookmarkedEpisodesByAnimeId", Some("EpisodeRepo::bookmarked")),
    ("getEpisodeById", Some("EpisodeRepo::by_id")),
    ("getEpisodeByAnimeIdAsFlow", Some("EpisodeRepo::subscribe_by_anime_id")),
    ("getEpisodeByUrlAndAnimeId", Some("EpisodeRepo::by_url_and_anime_id")),
];

/// Gaps that are known and owned by area 05 (`kuro-data`). Adding a gap here needs a reason; the test rejects any
/// gap not in this list.
///
/// `getScanlatorsByMangaIdAsFlow`: `ChapterRepo` has `scanlators` but no `subscribe_scanlators` yet; it belongs in
/// `kuro-data/src/chapter.rs` next to `subscribe_by_manga_id` (a `db.subscribe` over `CHAPTER_TABLES` with
/// `get_scanlators_by_manga_id`).
pub const KNOWN_GAPS: &[&str] = &["getScanlatorsByMangaIdAsFlow"];

/// The signature differences between a Kotlin repository method and its Rust twin, by kind (T-07a-033).
pub const SIGNATURE_DIFFERENCES: &[(&str, &str)] = &[
    ("suspend fun xxxAsFlow(..): Flow<T>", "subscribe_xxx(..) -> Subscription<T> (re-run on a table change, debounced)"),
    ("suspend fun xxx(..): T", "xxx(..) -> DataResult<T>, synchronous on the reader or writer connection"),
    ("suspend fun update(..)", "update(..) -> DataResult<()>; the Boolean flag setters live in flag_setters.rs"),
    ("addAll(list): List<T>?", "add_all(&[T]) -> DataResult<Vec<T>>, one transaction, ids in order (EC-07)"),
    ("getChapterByMangaId(id, applyFilter = false)", "by_manga_id(id, apply_scanlator_filter) with the flag required"),
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
        let _ = ChapterRepo::add_all;
        let _ = ChapterRepo::remove_with_ids;
        let _ = ChapterRepo::scanlators;
        let _ = ChapterRepo::bookmarked;
        let _ = ChapterRepo::update;
        let _ = ChapterRepo::update_all;
        let _ = ChapterRepo::by_manga_id;
        let _ = ChapterRepo::by_id;
        let _ = ChapterRepo::subscribe_by_manga_id;
        let _ = ChapterRepo::by_url_and_manga_id;
        let _ = EpisodeRepo::add_all;
        let _ = EpisodeRepo::remove_with_ids;
        let _ = EpisodeRepo::bookmarked;
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

    #[test]
    fn the_signature_differences_are_listed() {
        assert!(SIGNATURE_DIFFERENCES.len() >= 4);
    }
}
