//! `NetworkToLocalManga` and `NetworkToLocalAnime` (`p1-behavior.md` 4.2): turn an entry from a source listing
//! into the stored row. The lookup and the insert run in one writer transaction (in `kuro-data`), so two
//! callers with the same `(url, source)` leave one row. The title of a non-favourite is replaced in memory
//! only; the database keeps its title until a later update writes it.

use kuro_data::anime::{Anime, AnimeRepo};
use kuro_data::entry::{Entry, EntryRepo};
use kuro_data::legacy::{LegacyEntry, LegacyRepo};
use kuro_data::manga::{Manga, MangaRepo};
use kuro_data::{DataResult, LookupOrInsert};

/// The entry a lookup-or-insert stands for: the new row with its id, the stored row, or, for a stored non-favourite, the
/// stored row with the listing's title (in memory only).
fn resolve(new: &Entry, found: LookupOrInsert<Entry>) -> Entry {
    match found {
        LookupOrInsert::Inserted(id) => Entry { id, ..new.clone() },
        LookupOrInsert::Found(local) if !local.favorite => Entry { title: new.title.clone(), ..local },
        LookupOrInsert::Found(local) => local,
    }
}

/// `NetworkToLocalManga.await` and `NetworkToLocalAnime.await`: the one path.
pub fn network_to_local(repo: &EntryRepo, e: &Entry) -> DataResult<Entry> {
    Ok(resolve(e, repo.lookup_or_insert(e)?))
}

/// [`network_to_local`] over a legacy repository: same rule, through the old shape.
fn network_to_local_legacy<T: LegacyEntry>(repo: &LegacyRepo<T>, shape: &T) -> DataResult<T> {
    let found = match repo.lookup_or_insert(shape)? {
        LookupOrInsert::Inserted(id) => LookupOrInsert::Inserted(id),
        LookupOrInsert::Found(local) => LookupOrInsert::Found(local.to_entry()),
    };
    Ok(resolve(&shape.to_entry(), found).into())
}

/// `NetworkToLocalManga.await(manga)`.
pub fn network_to_local_manga(repo: &MangaRepo, manga: &Manga) -> DataResult<Manga> {
    network_to_local_legacy(repo, manga)
}

/// `NetworkToLocalAnime.await(anime)`.
pub fn network_to_local_anime(repo: &AnimeRepo, anime: &Anime) -> DataResult<Anime> {
    network_to_local_legacy(repo, anime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::{anime_repo, manga_repo};
    use std::sync::Arc;
    use std::thread;

    fn manga(title: &str, favorite: bool) -> Manga {
        Manga {
            url: "/series/9".into(),
            title: title.into(),
            favorite,
            source: 5,
            ..Manga::create()
        }
    }

    fn anime(title: &str, favorite: bool) -> Anime {
        Anime {
            url: "/anime/9".into(),
            title: title.into(),
            favorite,
            source: 5,
            ..Anime::create()
        }
    }

    #[test]
    fn absent_row_is_inserted_and_the_new_id_is_returned() {
        let (_dir, repo) = manga_repo();
        let out = network_to_local_manga(&repo, &manga("Fresh", false)).expect("ok");
        assert!(out.id > 0);
        let stored = repo
            .by_url_and_source("/series/9", 5)
            .expect("ok")
            .expect("row");
        assert_eq!((stored.id, stored.title.as_str()), (out.id, "Fresh"));
    }

    #[test]
    fn existing_non_favourite_returns_source_title_in_memory_only() {
        let (_dir, repo) = manga_repo();
        let id = repo.insert(&manga("Stored", false)).expect("insert");
        let out = network_to_local_manga(&repo, &manga("From source", false)).expect("ok");
        assert_eq!((out.id, out.title.as_str()), (id, "From source"));
        assert_eq!(
            repo.by_id(id).expect("row").title,
            "Stored",
            "database title is unchanged"
        );
    }

    #[test]
    fn existing_favourite_returns_the_stored_row_unchanged() {
        let (_dir, repo) = manga_repo();
        let id = repo.insert(&manga("My edit", true)).expect("insert");
        let out = network_to_local_manga(&repo, &manga("From source", false)).expect("ok");
        assert_eq!(out.title, "My edit");
        assert_eq!(out, repo.by_id(id).expect("row"));
    }

    #[test]
    fn sixteen_concurrent_calls_leave_one_row_and_one_id() {
        let (_dir, repo) = manga_repo();
        let repo = Arc::new(repo);
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let repo = Arc::clone(&repo);
                thread::spawn(move || {
                    network_to_local_manga(&repo, &manga("Race", false))
                        .expect("ok")
                        .id
                })
            })
            .collect();
        let ids: Vec<i64> = handles
            .into_iter()
            .map(|h| h.join().expect("thread"))
            .collect();
        assert!(
            ids.windows(2).all(|w| w[0] == w[1]),
            "all callers see one id: {ids:?}"
        );
        let rows = repo
            .all_source_and_url()
            .expect("ok")
            .into_iter()
            .filter(|(_, url)| url == "/series/9")
            .count();
        assert_eq!(rows, 1);
    }

    #[test]
    fn anime_absent_existing_and_favourite_cases() {
        let (_dir, repo) = anime_repo();
        let fresh = network_to_local_anime(&repo, &anime("Fresh", false)).expect("ok");
        assert!(fresh.id > 0);
        let again = network_to_local_anime(&repo, &anime("Source name", false)).expect("ok");
        assert_eq!((again.id, again.title.as_str()), (fresh.id, "Source name"));
        assert_eq!(repo.by_id(fresh.id).expect("row").title, "Fresh");
        repo.update(&kuro_data::anime::AnimeUpdate {
            favorite: Some(true),
            ..kuro_data::anime::AnimeUpdate::new(fresh.id)
        })
        .expect("update");
        let kept = network_to_local_anime(&repo, &anime("Source again", false)).expect("ok");
        assert_eq!(kept.title, "Fresh", "favourites keep the stored title");
    }
}
