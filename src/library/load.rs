//! Storage reads and preference reads for the library screen. `snapshot` runs on the DB worker; `request` runs on
//! the GTK thread when the user changes a setting. The pipeline itself is in `query`.

use std::collections::BTreeSet;
use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_data::track::{AnimeTrackRepo, MangaTrackRepo};
use kuro_db::handle::Db;
use kuro_domain::category::repo::CategoryRepo;
use kuro_domain::error::DomainError;
use kuro_domain::kind::EntryKind;
use kuro_domain::library::filter::LibraryFilters;
use kuro_domain::library::model::LibraryRow;
use kuro_domain::library::page::BadgeSettings;
use kuro_prefs::library::LibraryPrefs;

use super::query::{Details, Request, Snapshot};

/// Reads the categories, the library rows, the search fields and the tracks of one kind.
pub fn snapshot(db: &Arc<Db>, kind: EntryKind) -> Result<Snapshot, DomainError> {
    let categories = CategoryRepo::new(db.clone(), kind).visible()?;
    let mut snap = Snapshot { categories, ..Snapshot::default() };
    match kind {
        EntryKind::Manga => {
            for l in MangaRepo::new(db.clone()).library()? {
                snap.rows.push(LibraryRow::from_manga(&l));
                snap.details.insert(
                    l.manga.id,
                    Details {
                        author: l.manga.author.clone(),
                        artist: l.manga.artist.clone(),
                        description: l.manga.description.clone(),
                        genres: l.manga.genre.clone().unwrap_or_default(),
                    },
                );
            }
            for t in MangaTrackRepo::new(db.clone()).all()? {
                snap.tracks.entry(t.manga_id).or_default().push((t.tracker_id, t.score));
            }
        }
        EntryKind::Anime => {
            for l in AnimeRepo::new(db.clone()).library()? {
                snap.rows.push(LibraryRow::from_anime(&l));
                snap.details.insert(
                    l.anime.id,
                    Details {
                        author: l.anime.author.clone(),
                        artist: l.anime.artist.clone(),
                        description: l.anime.description.clone(),
                        genres: l.anime.genre.clone().unwrap_or_default(),
                    },
                );
            }
            for t in AnimeTrackRepo::new(db.clone()).all()? {
                snap.tracks.entry(t.anime_id).or_default().push((t.tracker_id, t.score));
            }
        }
    }
    Ok(snap)
}

/// The filter preferences of one kind, plus the global downloaded-only switch and the logged-in trackers.
pub fn filters(
    prefs: &LibraryPrefs,
    kind: EntryKind,
    downloaded_only: bool,
    logged_in: &BTreeSet<i64>,
) -> LibraryFilters {
    let (downloaded, unread, started, bookmarked, completed) = match kind {
        EntryKind::Anime => (
            prefs.filter_downloaded_anime.get(),
            prefs.filter_unseen.get(),
            prefs.filter_started_anime.get(),
            prefs.filter_bookmarked_anime.get(),
            prefs.filter_completed_anime.get(),
        ),
        EntryKind::Manga => (
            prefs.filter_downloaded_manga.get(),
            prefs.filter_unread.get(),
            prefs.filter_started_manga.get(),
            prefs.filter_bookmarked_manga.get(),
            prefs.filter_completed_manga.get(),
        ),
    };
    let tracking = logged_in
        .iter()
        .map(|id| {
            let state = match kind {
                EntryKind::Anime => prefs.filter_tracked_anime(*id as i32).get(),
                EntryKind::Manga => prefs.filter_tracked_manga(*id as i32).get(),
            };
            (*id, state)
        })
        .collect();
    LibraryFilters {
        downloaded,
        unread,
        started,
        bookmarked,
        completed,
        interval_custom: prefs.filter_interval_custom.get(),
        tracking,
        downloaded_only,
        skip_outside_release_period: prefs.auto_update_item_restrictions.get().contains("manga_outside_release_period"),
    }
}

/// Whether a preference change can alter the library pipeline or its layout for `kind` (D-3, D-6). Other keys,
/// such as the remembered tab, do not reload the library.
pub fn affects_library(kind: EntryKind, key: &str) -> bool {
    const SHARED: &[&str] = &[
        "pref_display_mode_library",
        "display_download_badge",
        "display_unread_badge",
        "display_local_badge",
        "display_language_badge",
        "display_continue_reading_button",
        "display_category_tabs",
        "display_number_of_items",
        "hidden_categories",
        "categorized_display",
        "pref_filter_library_interval_custom",
        "library_update_manga_restriction",
        "__APP_STATE_pref_downloaded_only",
    ];
    const ANIME: &[&str] = &[
        "pref_filter_animelib_downloaded_v2",
        "pref_filter_animelib_unread_v2",
        "pref_filter_animelib_started_v2",
        "pref_filter_animelib_bookmarked_v2",
        "pref_filter_animelib_completed_v2",
        "animelib_sorting_mode",
        "library_random_anime_sort_seed",
        "pref_animelib_columns_portrait_key",
        "pref_animelib_columns_landscape_key",
    ];
    const MANGA: &[&str] = &[
        "pref_filter_library_downloaded_v2",
        "pref_filter_library_unread_v2",
        "pref_filter_library_started_v2",
        "pref_filter_library_bookmarked_v2",
        "pref_filter_library_completed_v2",
        "library_sorting_mode",
        "library_random_manga_sort_seed",
        "pref_library_columns_portrait_key",
        "pref_library_columns_landscape_key",
    ];
    if SHARED.contains(&key) {
        return true;
    }
    match kind {
        EntryKind::Anime => ANIME.contains(&key) || key.starts_with("pref_filter_animelib_tracked_"),
        EntryKind::Manga => MANGA.contains(&key) || key.starts_with("pref_filter_library_tracked_"),
    }
}

/// The badge switches. They are shared by both libraries.
pub fn badges(prefs: &LibraryPrefs) -> BadgeSettings {
    BadgeSettings {
        download: prefs.download_badge.get(),
        unread: prefs.unread_badge.get(),
        local: prefs.local_badge.get(),
        language: prefs.language_badge.get(),
    }
}

/// The complete settings of one pass.
pub fn request(
    prefs: &LibraryPrefs,
    kind: EntryKind,
    downloaded_only: bool,
    logged_in: &BTreeSet<i64>,
    query: Option<String>,
) -> Request {
    let random_seed = match kind {
        EntryKind::Anime => prefs.random_anime_sort_seed.get(),
        EntryKind::Manga => prefs.random_manga_sort_seed.get(),
    };
    Request {
        kind,
        filters: filters(prefs, kind, downloaded_only, logged_in),
        badges: badges(prefs),
        query,
        random_seed,
        logged_in: logged_in.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::manga::Manga;

    fn manga_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
        (dir, Arc::new(Db::new(&path, writer).expect("manga db handle")))
    }

    #[test]
    fn snapshot_lists_visible_categories_and_one_row_per_membership() {
        let (_dir, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo
            .insert(&Manga {
                favorite: true,
                source: 7,
                url: "u".into(),
                title: "Title".into(),
                author: Some("Someone".into()),
                ..Manga::create()
            })
            .expect("insert");
        let a = kuro_data::category::create_manga_category(&db, "a", 0).expect("category a");
        let b = kuro_data::category::create_manga_category(&db, "b", 0).expect("category b");
        repo.set_categories(id, &[a, b]).expect("link");
        let snap = snapshot(&db, EntryKind::Manga).expect("snapshot");
        assert_eq!(snap.rows.len(), 2);
        assert!(snap.rows.iter().all(|r| r.id == id && r.title == "Title"));
        assert_eq!(snap.details.get(&id).and_then(|d| d.author.clone()).as_deref(), Some("Someone"));
        assert!(snap.categories.iter().any(|c| c.id == a) && snap.categories.iter().any(|c| c.id == b));
        assert!(snap.tracks.is_empty());
    }

    #[test]
    fn only_library_keys_of_this_kind_reload_the_library() {
        assert!(affects_library(EntryKind::Manga, "library_sorting_mode"));
        assert!(!affects_library(EntryKind::Anime, "library_sorting_mode"));
        assert!(affects_library(EntryKind::Anime, "pref_filter_animelib_tracked_3_v2"));
        assert!(affects_library(EntryKind::Manga, "display_unread_badge"));
        assert!(!affects_library(EntryKind::Manga, "__APP_STATE_last_used_category"));
    }

    #[test]
    fn request_reads_the_default_preferences_as_disabled_filters_and_no_search() {
        let store = kuro_prefs::store::PrefStore::in_memory();
        let prefs = LibraryPrefs::new(store);
        let req = request(&prefs, EntryKind::Manga, false, &BTreeSet::new(), None);
        assert_eq!(req.filters.unread, kuro_prefs::tristate::TriState::DISABLED);
        assert!(req.badges.unread && !req.badges.download);
        assert!(req.query.is_none());
        assert!(req.filters.skip_outside_release_period, "the default restrictions include the release period");
    }
}
