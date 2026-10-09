//! Viewer flags of the app layer (`SetMangaViewerFlags`, `SetAnimeViewerFlags`; T-10a-003, T-10a-004). Each setter writes
//! its bit group with one statement (`Repo::set_viewer_bits`), so two setters on one entry never lose each other's bits
//! (EC-23). A missing entry gives `Ok(false)` (EC-24, a documented deviation from the Kotlin throw).

use kuro_data::DataResult;
use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;

use crate::entries::anime::viewer_flags::{AIRING_EPISODE_MASK, AIRING_TIME_MASK, INTRO_DISABLE_MASK, INTRO_MASK};

/// Reading mode: bits 0-2 of `mangas.viewer` (`p1-data-and-config.md` 3.1).
pub const READING_MODE_MASK: i64 = 0x7;
/// Reader orientation: bits 3-5 of `mangas.viewer`.
pub const ORIENTATION_MASK: i64 = 0x38;

/// `Manga::reading_mode()`: the reading-mode bits of a `viewer` word, raw. Unknown values are the caller's to map.
pub fn reading_mode(viewer: i64) -> i64 {
    viewer & READING_MODE_MASK
}

/// `Manga::reader_orientation()`: the orientation bits of a `viewer` word, raw.
pub fn reader_orientation(viewer: i64) -> i64 {
    viewer & ORIENTATION_MASK
}

/// `SetMangaViewerFlags.awaitSetReadingMode`. Only bits 0-2 change; `flag` is masked, so other bits of `flag` are ignored.
pub fn set_manga_reading_mode(repo: &MangaRepo, manga_id: i64, flag: i64) -> DataResult<bool> {
    repo.set_viewer_bits(manga_id, flag, READING_MODE_MASK)
}

/// `SetMangaViewerFlags.awaitSetOrientation`. Only bits 3-5 change.
pub fn set_manga_orientation(repo: &MangaRepo, manga_id: i64, flag: i64) -> DataResult<bool> {
    repo.set_viewer_bits(manga_id, flag, ORIENTATION_MASK)
}

/// `SetAnimeViewerFlags.awaitSetSkipIntroLength`: the length goes in its eight bits (`length and 0xFF`, EC-21: 300 stores
/// 44). A masked zero sets the disable bit and clears the length; any other length clears the disable bit.
pub fn set_anime_skip_intro_length(repo: &AnimeRepo, anime_id: i64, length: i64) -> DataResult<bool> {
    let masked = length & INTRO_MASK;
    let disable = if masked == 0 { INTRO_DISABLE_MASK } else { 0 };
    repo.set_viewer_bits(anime_id, masked | disable, INTRO_MASK | INTRO_DISABLE_MASK)
}

/// `SetAnimeViewerFlags.awaitSetNextEpisodeAiring`: the episode (16 bits) and the airing time (32 bits) are written in one
/// statement. Values above their width are truncated by the mask, without error (EC-22). `(0, 0)` clears both.
pub fn set_anime_next_episode_airing(
    repo: &AnimeRepo,
    anime_id: i64,
    episode: i64,
    airing_at: i64,
) -> DataResult<bool> {
    let value = ((episode & 0xFFFF) << 8) | ((airing_at & 0xFFFF_FFFF) << 24);
    repo.set_viewer_bits(anime_id, value, AIRING_EPISODE_MASK | AIRING_TIME_MASK)
}

/// The skip-intro length the player uses: `0` when the intro is disabled, the default preference when the stored length
/// is `0`, and the stored length otherwise (T-10a-004).
pub fn effective_intro_length(viewer: i64, default_intro_length: i64) -> i64 {
    if viewer & INTRO_DISABLE_MASK == INTRO_DISABLE_MASK {
        return 0;
    }
    match viewer & INTRO_MASK {
        0 => default_intro_length,
        stored => stored,
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;
    use crate::testdb::{anime_repo, manga_repo};

    #[test]
    fn set_flag_matches_the_formula_on_twelve_vectors() {
        // (value, flag, mask, expected), with expected = (value & !mask) | (flag & mask), computed outside Rust.
        let vectors: [(i64, i64, i64, i64); 12] = [
            (0xFFFF, 0x2, 0x7, 0xFFFA),
            (0x0, 0x0, 0x0, 0x0),
            (0x12345678, 0xFF, 0x38, 0x12345678),
            (0x7, 0x8, 0x7, 0x0),
            (0x38, 0x10, 0x38, 0x10),
            (0x1, 0x3, 0x1, 0x1),
            (0x7F, 0x80, 0xFF, 0x80),
            (0xFFFF_FFFF, 0x0, 0xFF00, 0xFFFF_00FF),
            (0x5A5A, 0xA5A5, 0xFFFF, 0xA5A5),
            (0x0068_E778_0000_0D5A, 0x2, 0xFF, 0x0068_E778_0000_0D02),
            (0x1234, 0x5, 0xF0, 0x1204),
            (0x1_0000_0000, 0xF, 0x3C, 0x1_0000_000C),
        ];
        for (value, flag, mask, expected) in vectors {
            assert_eq!(kuro_db::flags::set_flag(value, flag, mask), expected, "set_flag({value:#x}, {flag:#x}, {mask:#x})");
        }
    }

    #[test]
    fn the_anime_masks_are_the_documented_layout() {
        assert_eq!(INTRO_MASK, 0xFF);
        assert_eq!(AIRING_EPISODE_MASK, 0xFFFF00);
        assert_eq!(AIRING_TIME_MASK, 0xFFFF_FFFF << 24);
        assert_eq!(INTRO_DISABLE_MASK, 1 << 56);
    }

    #[test]
    fn the_manga_getters_return_the_raw_bit_groups() {
        assert_eq!(reading_mode(0x3D), 0x5);
        assert_eq!(reader_orientation(0x3D), 0x38);
        assert_eq!(reading_mode(0x8), 0);
    }

    #[test]
    fn a_reading_mode_write_changes_only_bits_zero_to_two() {
        let (_dir, repo) = manga_repo();
        let id = repo.insert(&kuro_data::manga::Manga { viewer_flags: 0x38, ..kuro_data::manga::Manga::create() }).expect("insert");
        assert!(set_manga_reading_mode(&repo, id, 0x0D).expect("write"), "masked to 0b101");
        assert_eq!(repo.by_id(id).expect("read").viewer_flags, 0x3D);
        assert!(set_manga_orientation(&repo, id, 0x10).expect("write"));
        assert_eq!(repo.by_id(id).expect("read").viewer_flags, 0x15);
    }

    #[test]
    fn parallel_setters_on_one_manga_keep_both_bits() {
        let (_dir, repo) = manga_repo();
        let id = repo.insert(&kuro_data::manga::Manga::create()).expect("insert");
        thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..200 {
                    set_manga_reading_mode(&repo, id, 0x2).expect("reading mode");
                }
            });
            scope.spawn(|| {
                for _ in 0..200 {
                    set_manga_orientation(&repo, id, 0x30).expect("orientation");
                }
            });
        });
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!(viewer, 0x32, "EC-23: both bit groups are set");
    }

    #[test]
    fn a_deleted_manga_gives_false_not_an_error() {
        let (_dir, repo) = manga_repo();
        assert!(!set_manga_reading_mode(&repo, 999, 1).expect("no error"), "EC-24");
    }

    #[test]
    fn skip_intro_writes_the_masked_length_and_the_disable_bit() {
        let (_dir, repo) = anime_repo();
        let id = repo.insert(&kuro_data::anime::Anime::create()).expect("insert");
        assert!(set_anime_skip_intro_length(&repo, id, 0).expect("zero"));
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!(viewer & INTRO_DISABLE_MASK, INTRO_DISABLE_MASK);
        assert_eq!(viewer & INTRO_MASK, 0);

        set_anime_skip_intro_length(&repo, id, 30).expect("thirty");
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!(viewer & INTRO_DISABLE_MASK, 0, "clears the disable bit");
        assert_eq!(viewer & INTRO_MASK, 30);

        set_anime_skip_intro_length(&repo, id, 300).expect("three hundred");
        assert_eq!(repo.by_id(id).expect("read").viewer_flags & INTRO_MASK, 44, "EC-21");
    }

    #[test]
    fn next_airing_round_trips_and_zeros_clear_it() {
        let (_dir, repo) = anime_repo();
        let id = repo.insert(&kuro_data::anime::Anime::create()).expect("insert");
        set_anime_next_episode_airing(&repo, id, 12, 1_700_000_000).expect("write");
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!((viewer & AIRING_EPISODE_MASK) >> 8, 12);
        assert_eq!((viewer & AIRING_TIME_MASK) >> 24, 1_700_000_000);

        set_anime_next_episode_airing(&repo, id, 0, 0).expect("clear");
        assert_eq!(repo.by_id(id).expect("read").viewer_flags & (AIRING_EPISODE_MASK | AIRING_TIME_MASK), 0, "EC-22");

        set_anime_next_episode_airing(&repo, id, 70_000, 1 << 33).expect("truncates");
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!((viewer & AIRING_EPISODE_MASK) >> 8, 70_000 & 0xFFFF, "16-bit episode, EC-22");
        assert_eq!(viewer & AIRING_TIME_MASK, 0, "a time above 32 bits is masked away without error");
    }

    #[test]
    fn the_effective_intro_length_follows_disable_then_default_then_stored() {
        assert_eq!(effective_intro_length(INTRO_DISABLE_MASK, 90), 0, "disabled wins");
        assert_eq!(effective_intro_length(0, 90), 90, "stored zero uses the default preference");
        assert_eq!(effective_intro_length(30, 90), 30, "a stored length wins");
    }
}
