//! Decides whether a stored chapter, episode or season must be rewritten after a source refresh
//! (`ShouldUpdateDbChapter.kt`, `ShouldUpdateDbEpisode.kt`, `ShouldUpdateDbSeason.kt`).
//!
//! Deviation D4 / quirk Q1: the upload date is ignored when the source reports 0, so a source that never
//! dates its items no longer rewrites every row on each refresh. Q15 is preserved: a season memo change alone
//! does not count.

use kuro_data::anime::Anime;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_data::item::Item;

/// One item of either kind: scanlator, name, date upload (skipped when the source date is 0), number and source order,
/// plus the anime fields summary, fillermark, preview url and memo. A manga item has none of those (they are equal
/// `None`/false on both sides), so the one rule serves both kinds.
pub fn should_update_db_item(db: &Item, src: &Item) -> bool {
    db.scanlator != src.scanlator
        || db.name != src.name
        || (src.date_upload != 0 && db.date_upload != src.date_upload)
        || db.number != src.number
        || db.source_order != src.source_order
        || db.summary != src.summary
        || db.fillermark != src.fillermark
        || db.preview_url != src.preview_url
        || db.memo != src.memo
}

/// Chapter: [`should_update_db_item`] over the legacy shape.
pub fn should_update_db_chapter(db: &Chapter, src: &Chapter) -> bool {
    should_update_db_item(&Item::from(db), &Item::from(src))
}

/// Episode: [`should_update_db_item`] over the legacy shape.
pub fn should_update_db_episode(db: &Episode, src: &Episode) -> bool {
    should_update_db_item(&Item::from(db), &Item::from(src))
}

/// Season: title, season number and season source order. The memo is not compared (Q15).
pub fn should_update_db_season(db: &Anime, src: &Anime) -> bool {
    db.title != src.title
        || db.season_number != src.season_number
        || db.season_source_order != src.season_source_order
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter() -> Chapter {
        Chapter {
            scanlator: Some("Team".into()),
            name: "Ch 1".into(),
            date_upload: 1_700_000_000_000,
            chapter_number: 1.0,
            source_order: 2,
            ..Chapter::create()
        }
    }

    fn episode() -> Episode {
        Episode {
            scanlator: Some("Sub".into()),
            name: "Ep 1".into(),
            date_upload: 1_700_000_000_000,
            episode_number: 1.0,
            source_order: 2,
            summary: Some("s".into()),
            fillermark: false,
            preview_url: Some("p".into()),
            ..Episode::create()
        }
    }

    fn season() -> Anime {
        Anime {
            title: "Season 1".into(),
            season_number: 1.0,
            season_source_order: 3,
            ..Anime::create()
        }
    }

    #[test]
    fn identical_chapters_do_not_update() {
        assert!(!should_update_db_chapter(&chapter(), &chapter()));
    }

    #[test]
    fn each_chapter_field_flips_the_result() {
        let base = chapter();
        let mut c = base.clone();
        c.scanlator = None;
        assert!(should_update_db_chapter(&base, &c), "scanlator");
        let mut c = base.clone();
        c.name = "other".into();
        assert!(should_update_db_chapter(&base, &c), "name");
        let mut c = base.clone();
        c.date_upload = 1;
        assert!(should_update_db_chapter(&base, &c), "date upload");
        let mut c = base.clone();
        c.chapter_number = 1.5;
        assert!(should_update_db_chapter(&base, &c), "number");
        let mut c = base.clone();
        c.source_order = 9;
        assert!(should_update_db_chapter(&base, &c), "source order");
    }

    #[test]
    fn quirk_q1_zero_source_date_is_ignored() {
        let db = chapter();
        let mut src = chapter();
        src.date_upload = 0;
        assert!(!should_update_db_chapter(&db, &src));
        assert!(!should_update_db_episode(
            &episode(),
            &Episode {
                date_upload: 0,
                ..episode()
            }
        ));
    }

    #[test]
    fn each_episode_field_flips_the_result() {
        let base = episode();
        let flips: [(&str, Episode); 8] = [
            (
                "scanlator",
                Episode {
                    scanlator: None,
                    ..episode()
                },
            ),
            (
                "name",
                Episode {
                    name: "x".into(),
                    ..episode()
                },
            ),
            (
                "date upload",
                Episode {
                    date_upload: 5,
                    ..episode()
                },
            ),
            (
                "number",
                Episode {
                    episode_number: 2.0,
                    ..episode()
                },
            ),
            (
                "source order",
                Episode {
                    source_order: 0,
                    ..episode()
                },
            ),
            (
                "summary",
                Episode {
                    summary: None,
                    ..episode()
                },
            ),
            (
                "fillermark",
                Episode {
                    fillermark: true,
                    ..episode()
                },
            ),
            ("memo", {
                let mut memo = serde_json::Map::new();
                memo.insert("k".into(), serde_json::Value::from(1));
                Episode { memo, ..episode() }
            }),
        ];
        for (field, changed) in flips {
            assert!(should_update_db_episode(&base, &changed), "{field}");
        }
        assert!(
            should_update_db_episode(
                &base,
                &Episode {
                    preview_url: None,
                    ..episode()
                }
            ),
            "preview url"
        );
    }

    #[test]
    fn memo_key_order_does_not_matter() {
        let mut a = serde_json::Map::new();
        a.insert("x".into(), serde_json::Value::from(1));
        a.insert("y".into(), serde_json::Value::from(2));
        let mut b = serde_json::Map::new();
        b.insert("y".into(), serde_json::Value::from(2));
        b.insert("x".into(), serde_json::Value::from(1));
        let db = Episode {
            memo: a,
            ..episode()
        };
        let src = Episode {
            memo: b,
            ..episode()
        };
        assert!(!should_update_db_episode(&db, &src));
    }

    #[test]
    fn season_compares_three_fields_and_not_memo() {
        assert!(!should_update_db_season(&season(), &season()));
        assert!(should_update_db_season(
            &season(),
            &Anime {
                title: "x".into(),
                ..season()
            }
        ));
        assert!(should_update_db_season(
            &season(),
            &Anime {
                season_number: 2.0,
                ..season()
            }
        ));
        assert!(should_update_db_season(
            &season(),
            &Anime {
                season_source_order: 0,
                ..season()
            }
        ));
        let mut memo = serde_json::Map::new();
        memo.insert("k".into(), serde_json::Value::from(true));
        assert!(
            !should_update_db_season(&season(), &Anime { memo, ..season() }),
            "Q15: memo alone is ignored"
        );
    }
}
