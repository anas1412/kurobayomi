//! Reset viewer flags (T-07a-028): `ResetMangaViewerFlags.kt` and `ResetAnimeViewerFlags.kt`, behind the "reset player
//! settings" action. Each sets `viewer` to 0 on every row. For anime that also clears the skip-intro length, the
//! skip-intro disable bit and the airing data, which all live in `viewer_flags`.

use kuro_data::DataResult;
use kuro_data::compat;
use kuro_data::entry::EntryRepo;
use kuro_platform::kind::Kind;

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
pub fn set_reading_mode(repo: &EntryRepo, id: i64, flag: i64) -> DataResult<bool> {
    repo.set_viewer_bits(id, flag, READING_MODE_MASK)
}

/// `awaitSetSkipIntroLength`: the length goes in its eight bits (`length and 0xFF`, EC-21: 300 stores 44). A masked zero
/// sets the disable bit and clears the length; any other length clears the disable bit.
pub fn set_skip_intro_length(repo: &EntryRepo, id: i64, length: i64) -> DataResult<bool> {
    let (value, mask) = skip_intro_bits(length);
    repo.set_viewer_bits(id, value, mask)
}

/// `awaitSetNextEpisodeAiring`: the episode (16 bits) and the airing time (32 bits) are written in one statement, so two
/// setters on one entry never lose each other's bits (EC-23). `(0, 0)` clears both (EC-22).
pub fn set_next_episode_airing(repo: &EntryRepo, id: i64, episode: i64, airing_at: i64) -> DataResult<bool> {
    let (value, mask) = airing_bits(episode, airing_at);
    repo.set_viewer_bits(id, value, mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::test_db;
    use kuro_data::entry::Entry;

    #[test]
    fn reset_zeroes_the_viewer_flags_of_the_kind_and_reports_success() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        for kind in Kind::ALL {
            let ids: Vec<i64> = (0..3)
                .map(|i| repo.insert(&Entry { url: format!("m{i}"), title: format!("M{i}"), viewer_flags: 0b1010 + i, ..Entry::create(kind) }).unwrap())
                .collect();
            assert!(reset_viewer_flags(&repo, kind));
            for id in ids {
                assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0, "{kind}");
            }
        }
    }

    #[test]
    fn anime_reset_clears_skip_intro_disable_and_airing_bits() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let intro = 90_i64;
        let airing = 1_700_000_000_i64 << 24;
        let disable = 1_i64 << 56;
        let ids: Vec<i64> = (0..3)
            .map(|i| {
                repo.insert(&Entry { url: format!("a{i}"), title: format!("A{i}"), viewer_flags: intro | airing | disable, ..Entry::create(Kind::Anime) })
                    .unwrap()
            })
            .collect();
        assert!(reset_viewer_flags(&repo, Kind::Anime));
        for id in ids {
            assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0);
        }
    }

    use crate::entries::flags::{AIRING_EPISODE_MASK, AIRING_TIME_MASK, INTRO_DISABLE_MASK, INTRO_MASK};

    #[test]
    fn the_anime_masks_are_the_documented_layout() {
        assert_eq!(INTRO_MASK, 0xFF);
        assert_eq!(AIRING_EPISODE_MASK, 0xFFFF00);
        assert_eq!(AIRING_TIME_MASK, 0xFFFF_FFFF << 24);
        assert_eq!(INTRO_DISABLE_MASK, 1 << 56);
    }

    #[test]
    fn the_manga_getter_returns_the_raw_bit_group() {
        assert_eq!(reading_mode(0x3D), 0x5);
        assert_eq!(reading_mode(0x8), 0);
    }

    #[test]
    fn a_reading_mode_write_changes_only_bits_zero_to_two_and_a_missing_entry_gives_false() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&Entry { viewer_flags: 0x38, ..Entry::create(Kind::Manga) }).expect("insert");
        assert!(set_reading_mode(&repo, id, 0x0D).expect("write"), "masked to 0b101");
        assert_eq!(repo.by_id(id).expect("read").viewer_flags, 0x3D);
        assert!(!set_reading_mode(&repo, 999, 1).expect("no error"), "EC-24");
    }

    #[test]
    fn parallel_setters_on_one_anime_keep_both_bits() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&Entry::create(Kind::Anime)).expect("insert");
        std::thread::scope(|scope| {
            scope.spawn(|| {
                for _ in 0..200 {
                    set_skip_intro_length(&repo, id, 30).expect("skip intro");
                }
            });
            scope.spawn(|| {
                for _ in 0..200 {
                    set_next_episode_airing(&repo, id, 7, 1_700_000_000).expect("airing");
                }
            });
        });
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!(viewer & INTRO_MASK, 30, "EC-23: the intro group is set");
        assert_eq!((viewer & AIRING_EPISODE_MASK) >> 8, 7, "EC-23: the airing group is set");
        assert_eq!((viewer & AIRING_TIME_MASK) >> 24, 1_700_000_000);
    }

    #[test]
    fn skip_intro_writes_the_masked_length_and_the_disable_bit() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&Entry::create(Kind::Anime)).expect("insert");
        assert!(set_skip_intro_length(&repo, id, 0).expect("zero"));
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!(viewer & INTRO_DISABLE_MASK, INTRO_DISABLE_MASK);
        assert_eq!(viewer & INTRO_MASK, 0);

        set_skip_intro_length(&repo, id, 30).expect("thirty");
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!(viewer & INTRO_DISABLE_MASK, 0, "clears the disable bit");
        assert_eq!(viewer & INTRO_MASK, 30);

        set_skip_intro_length(&repo, id, 300).expect("three hundred");
        assert_eq!(repo.by_id(id).expect("read").viewer_flags & INTRO_MASK, 44, "EC-21");
    }

    #[test]
    fn next_airing_round_trips_and_zeros_clear_it() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&Entry::create(Kind::Anime)).expect("insert");
        set_next_episode_airing(&repo, id, 12, 1_700_000_000).expect("write");
        let viewer = repo.by_id(id).expect("read").viewer_flags;
        assert_eq!((viewer & AIRING_EPISODE_MASK) >> 8, 12);
        assert_eq!((viewer & AIRING_TIME_MASK) >> 24, 1_700_000_000);

        set_next_episode_airing(&repo, id, 0, 0).expect("clear");
        assert_eq!(repo.by_id(id).expect("read").viewer_flags & (AIRING_EPISODE_MASK | AIRING_TIME_MASK), 0, "EC-22");

        set_next_episode_airing(&repo, id, 70_000, 1 << 33).expect("truncates");
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
