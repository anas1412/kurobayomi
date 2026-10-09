//! Cover descriptors (`MangaCover.kt`, `AnimeCover.kt`). Each one is the cache key of its cover loader:
//! a change of `last_modified` or of `favorite` yields a different key.
//!
//! There is no descriptor for the anime background image: the background cache is keyed by
//! `background_url` and `background_last_modified` directly (area:13 and area:10).

/// `MangaCover(mangaId, sourceId, isMangaFavorite, url, lastModified)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MangaCover {
    pub manga_id: i64,
    pub source_id: i64,
    pub is_manga_favorite: bool,
    pub url: Option<String>,
    pub last_modified: i64,
}

/// `AnimeCover(animeId, sourceId, isAnimeFavorite, url, lastModified)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnimeCover {
    pub anime_id: i64,
    pub source_id: i64,
    pub is_anime_favorite: bool,
    pub url: Option<String>,
    pub last_modified: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entries::anime::AnimeExt;
    use crate::entries::manga::MangaExt;
    use kuro_data::anime::Anime;
    use kuro_data::manga::Manga;
    use std::collections::HashSet;

    fn manga() -> Manga {
        Manga {
            id: 7,
            source: 3,
            favorite: true,
            thumbnail_url: Some("https://img.test/a.jpg".into()),
            cover_last_modified: 1000,
            ..Manga::create()
        }
    }

    #[test]
    fn manga_cover_maps_the_five_fields() {
        let c = manga().as_cover();
        assert_eq!(
            c,
            MangaCover {
                manga_id: 7,
                source_id: 3,
                is_manga_favorite: true,
                url: Some("https://img.test/a.jpg".into()),
                last_modified: 1000,
            }
        );
    }

    #[test]
    fn anime_cover_maps_the_five_fields() {
        let a = Anime {
            id: 9,
            source: 4,
            favorite: false,
            thumbnail_url: None,
            cover_last_modified: 55,
            ..Anime::create()
        };
        let c = a.as_cover();
        assert_eq!(
            (
                c.anime_id,
                c.source_id,
                c.is_anime_favorite,
                c.url,
                c.last_modified
            ),
            (9, 4, false, None, 55)
        );
    }

    #[test]
    fn cover_key_changes_with_last_modified_or_favorite() {
        let base = manga();
        let mut keys = HashSet::new();
        keys.insert(base.as_cover());
        let mut touched = base.clone();
        touched.cover_last_modified = 1001;
        assert!(
            keys.insert(touched.as_cover()),
            "last_modified must change the key"
        );
        let mut unfav = base.clone();
        unfav.favorite = false;
        assert!(
            keys.insert(unfav.as_cover()),
            "favorite must change the key"
        );
        assert!(
            !keys.insert(base.as_cover()),
            "identical descriptor is the same key"
        );
    }
}
