//! One extension package, many sources (`SourceFactory.kt` and `AnimeSourceFactory.kt`, T-20a-012). A package declares
//! its sources through a factory. The loader calls `create_sources` once per load and keeps nothing from the result,
//! so every source's id comes from its own name, language and version (see [`crate::id::generate_id`]).

use crate::anime::AnimeSource;
use crate::source::MangaSource;

/// The manga sources one package declares. Each entry becomes a separate source with its own id.
pub trait MangaSourceFactory: Send + Sync {
    fn create_sources(&self) -> Vec<Box<dyn MangaSource>>;
}

/// The anime sources one package declares, the anime twin of [`MangaSourceFactory`].
pub trait AnimeSourceFactory: Send + Sync {
    fn create_sources(&self) -> Vec<Box<dyn AnimeSource>>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::anime::AnimesPage;
    use crate::error::SourceError;
    use crate::filter_list::FilterList;
    use crate::id::generate_id;
    use crate::model::{Page, SChapter, SManga};

    /// A manga source that only has an identity.
    struct NamedManga {
        name: String,
        lang: String,
    }

    #[async_trait]
    impl MangaSource for NamedManga {
        fn id(&self) -> i64 {
            generate_id(&self.name, &self.lang, 1)
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn lang(&self) -> &str {
            &self.lang
        }

        async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
            Ok(manga.clone())
        }

        async fn get_chapter_list(&self, _manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
            Ok(Vec::new())
        }

        async fn get_page_list(&self, _chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
            Ok(Vec::new())
        }
    }

    /// An anime source that only has an identity.
    struct NamedAnime {
        name: String,
        lang: String,
    }

    #[async_trait]
    impl AnimeSource for NamedAnime {
        fn id(&self) -> i64 {
            generate_id(&self.name, &self.lang, 1)
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn lang(&self) -> &str {
            &self.lang
        }

        fn supports_latest(&self) -> bool {
            false
        }

        async fn popular(&self, _page: i32) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("popular"))
        }

        async fn latest(&self, _page: i32) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("latest"))
        }

        async fn search(&self, _page: i32, _query: &str, _filters: &FilterList) -> Result<AnimesPage, SourceError> {
            Err(SourceError::Unsupported("search"))
        }

        fn provides_hosters(&self) -> bool {
            false
        }
    }

    /// A package whose manifest lists two entries of one name, told apart by language.
    struct Package;

    impl MangaSourceFactory for Package {
        fn create_sources(&self) -> Vec<Box<dyn MangaSource>> {
            vec![
                Box::new(NamedManga { name: "Alpha".into(), lang: "en".into() }),
                Box::new(NamedManga { name: "Alpha".into(), lang: "ja".into() }),
            ]
        }
    }

    impl AnimeSourceFactory for Package {
        fn create_sources(&self) -> Vec<Box<dyn AnimeSource>> {
            vec![
                Box::new(NamedAnime { name: "Alpha".into(), lang: "en".into() }),
                Box::new(NamedAnime { name: "Alpha".into(), lang: "ja".into() }),
            ]
        }
    }

    #[test]
    fn one_manga_package_gives_one_source_per_entry_each_with_its_own_id() {
        let sources = MangaSourceFactory::create_sources(&Package);
        assert_eq!(sources.len(), 2);
        assert_ne!(sources[0].id(), sources[1].id(), "the language is part of the id");
        assert_eq!(sources[0].id(), generate_id("Alpha", "en", 1));
    }

    #[test]
    fn one_anime_package_gives_one_source_per_entry_each_with_its_own_id() {
        let sources = AnimeSourceFactory::create_sources(&Package);
        assert_eq!(sources.len(), 2);
        assert_ne!(sources[0].id(), sources[1].id(), "the language is part of the id");
        assert_eq!(sources[1].id(), generate_id("Alpha", "ja", 1));
    }
}
