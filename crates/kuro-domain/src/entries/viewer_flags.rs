//! Reset viewer flags (T-07a-028): `ResetMangaViewerFlags.kt` and `ResetAnimeViewerFlags.kt`, behind the "reset player
//! settings" action. Each sets `viewer` to 0 on every row. For anime that also clears the skip-intro length, the
//! skip-intro disable bit and the airing data, which all live in `viewer_flags`.

use kuro_data::DataResult;
use kuro_data::compat;
use kuro_data::entry::EntryRepo;
use kuro_platform::kind::Kind;

use super::store::EntryFlagStore;

pub use super::flags::{READING_MODE_MASK, airing_bits, effective_intro_length, skip_intro_bits};

/// `Manga::reading_mode()`: the reading-mode bits of a manga `viewer` word, raw. Unknown values are the caller's to map.
pub fn reading_mode(viewer: i64) -> i64 {
    viewer & READING_MODE_MASK
}

/// `resetViewerFlags`: `viewer = 0` on every entry of the kind. `true` when the write succeeded; a failure is logged and
/// reported as `false`. For anime that also clears the skip-intro length, its disable bit and the airing data.
pub fn reset_viewer_flags(repo: &EntryRepo, kind: Kind) -> bool {
    compat::reset_viewer_flags(repo, kind)
}

/// `awaitSetReadingMode`. Only bits 0-2 change; `flag` is masked. A missing entry gives `Ok(false)` (EC-24).
pub fn set_reading_mode(repo: &impl EntryFlagStore, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_viewer_bits(id, flag, READING_MODE_MASK)
}

/// `awaitSetSkipIntroLength`: the length goes in its eight bits (`length and 0xFF`, EC-21: 300 stores 44). A masked zero
/// sets the disable bit and clears the length; any other length clears the disable bit.
pub fn set_skip_intro_length(repo: &impl EntryFlagStore, id: i64, length: i64) -> DataResult<bool> {
    let (value, mask) = skip_intro_bits(length);
    repo.set_viewer_bits(id, value, mask)
}

/// `awaitSetNextEpisodeAiring`: the episode (16 bits) and the airing time (32 bits) are written in one statement, so two
/// setters on one entry never lose each other's bits (EC-23). `(0, 0)` clears both (EC-22).
pub fn set_next_episode_airing(repo: &impl EntryFlagStore, id: i64, episode: i64, airing_at: i64) -> DataResult<bool> {
    let (value, mask) = airing_bits(episode, airing_at);
    repo.set_viewer_bits(id, value, mask)
}

// ----- the old per-kind names (phase E deletes them) -----

/// `resetMangaViewerFlags`.
pub fn reset_viewer_flags_manga(repo: &kuro_data::manga::MangaRepo) -> bool {
    compat::reset_manga_viewer_flags(repo)
}

/// `resetAnimeViewerFlags`.
pub fn reset_viewer_flags_anime(repo: &kuro_data::anime::AnimeRepo) -> bool {
    compat::reset_anime_viewer_flags(repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::test_db;
    use kuro_data::anime::{Anime, AnimeRepo};
    use kuro_data::manga::{Manga, MangaRepo};

    #[test]
    fn manga_reset_zeroes_three_entries_and_reports_success() {
        let (_d, db) = test_db();
        let repo = MangaRepo::new(db);
        let ids: Vec<i64> = (0..3)
            .map(|i| {
                repo.insert(&Manga { url: format!("m{i}"), title: format!("M{i}"), viewer_flags: 0b1010 + i, ..Manga::create() })
                    .unwrap()
            })
            .collect();
        assert!(reset_viewer_flags_manga(&repo));
        for id in ids {
            assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0);
        }
    }

    #[test]
    fn anime_reset_clears_skip_intro_disable_and_airing_bits() {
        let (_d, db) = test_db();
        let repo = AnimeRepo::new(db);
        let intro = 90_i64;
        let airing = 1_700_000_000_i64 << 24;
        let disable = 1_i64 << 56;
        let ids: Vec<i64> = (0..3)
            .map(|i| {
                repo.insert(&Anime {
                    url: format!("a{i}"),
                    title: format!("A{i}"),
                    viewer_flags: intro | airing | disable,
                    ..Anime::create()
                })
                .unwrap()
            })
            .collect();
        assert!(reset_viewer_flags_anime(&repo));
        for id in ids {
            assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0);
        }
    }
}
