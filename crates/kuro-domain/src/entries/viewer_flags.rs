//! Reset viewer flags (T-07a-028): `ResetMangaViewerFlags.kt` and `ResetAnimeViewerFlags.kt`, behind the "reset player
//! settings" action. Each sets `viewer` to 0 on every row. For anime that also clears the skip-intro length, the
//! skip-intro disable bit and the airing data, which all live in `viewer_flags`.

use kuro_data::anime::AnimeRepo;
use kuro_data::compat;
use kuro_data::manga::MangaRepo;

/// `resetMangaViewerFlags`: `true` when the write succeeded. A failure is logged and reported as `false`.
pub fn reset_viewer_flags_manga(repo: &MangaRepo) -> bool {
    compat::reset_manga_viewer_flags(repo)
}

/// `resetAnimeViewerFlags`: `true` when the write succeeded. A failure is logged and reported as `false`.
pub fn reset_viewer_flags_anime(repo: &AnimeRepo) -> bool {
    compat::reset_anime_viewer_flags(repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::{anime_db, manga_db};
    use kuro_data::anime::{Anime, AnimeRepo};
    use kuro_data::manga::{Manga, MangaRepo};

    #[test]
    fn manga_reset_zeroes_three_entries_and_reports_success() {
        let (_d, db) = manga_db();
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
        let (_d, db) = anime_db();
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
