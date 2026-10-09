//! The local source identity (`LocalMangaSource.kt`, `LocalAnimeSource.kt`; T-17a-009). The local source is a live
//! source with id 0 that reads the user's own files. This module holds only its identity: the file reading lives in
//! `kuro-local`. The identity is built once and every caller gets the same `Arc`.

use std::sync::{Arc, OnceLock};

use crate::identity::display_name;

/// Id of both local sources (`LocalMangaSource.ID`, `LocalAnimeSource.ID`).
pub const LOCAL_SOURCE_ID: i64 = 0;
/// Language of both local sources. `other` is not a language the user enables, so the local source is kept by id.
pub const LOCAL_SOURCE_LANG: &str = "other";
/// `AYMR.strings.local_manga_source`.
pub const LOCAL_MANGA_NAME: &str = "Local manga source";
/// `AYMR.strings.local_anime_source`.
pub const LOCAL_ANIME_NAME: &str = "Local anime source";

/// Which local source an identity names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalKind {
    Manga,
    Anime,
}

/// The identity of a local source: its id, language and name.
#[derive(Debug, PartialEq, Eq)]
pub struct LocalSource {
    kind: LocalKind,
}

static MANGA: OnceLock<Arc<LocalSource>> = OnceLock::new();
static ANIME: OnceLock<Arc<LocalSource>> = OnceLock::new();

/// The local source of `kind`. Built on the first call; later calls return the same `Arc`.
pub fn local_source(kind: LocalKind) -> Arc<LocalSource> {
    let cell = match kind {
        LocalKind::Manga => &MANGA,
        LocalKind::Anime => &ANIME,
    };
    Arc::clone(cell.get_or_init(|| Arc::new(LocalSource { kind })))
}

impl LocalSource {
    pub fn kind(&self) -> LocalKind {
        self.kind
    }

    pub fn id(&self) -> i64 {
        LOCAL_SOURCE_ID
    }

    pub fn lang(&self) -> &'static str {
        LOCAL_SOURCE_LANG
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            LocalKind::Manga => LOCAL_MANGA_NAME,
            LocalKind::Anime => LOCAL_ANIME_NAME,
        }
    }

    /// The name the UI shows: `Local manga source (OTHER)`, as Aniyomi shows it.
    pub fn visual_name(&self) -> String {
        display_name(self.name(), self.lang())
    }
}

/// Whether `id` is a local source's id (`isLocal()`).
pub fn is_local(id: i64) -> bool {
    id == LOCAL_SOURCE_ID
}

/// Whether a source is enabled for the list: its language is enabled, or it is the local source, which is always kept
/// whatever the language settings say (`lang in enabledLanguages || id == LocalMangaSource.ID`).
pub fn is_enabled(id: i64, lang: &str, enabled_languages: &[String]) -> bool {
    is_local(id) || enabled_languages.iter().any(|enabled| enabled == lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manga_local_source_has_id_zero_other_language_and_its_name() {
        let local = local_source(LocalKind::Manga);
        assert_eq!(local.id(), 0);
        assert_eq!(local.lang(), "other");
        assert_eq!(local.name(), "Local manga source");
        assert_eq!(local.visual_name(), "Local manga source (OTHER)");
    }

    #[test]
    fn the_anime_local_source_has_the_same_id_and_language_and_its_own_name() {
        let local = local_source(LocalKind::Anime);
        assert_eq!((local.id(), local.lang()), (0, "other"));
        assert_eq!(local.name(), "Local anime source");
    }

    #[test]
    fn the_registry_gets_the_same_arc_on_every_call() {
        let first = local_source(LocalKind::Manga);
        let second = local_source(LocalKind::Manga);
        assert!(Arc::ptr_eq(&first, &second), "the identity is built once");
        assert!(!Arc::ptr_eq(&first, &local_source(LocalKind::Anime)));
    }

    #[test]
    fn id_zero_is_the_only_local_id_and_local_is_enabled_with_no_languages() {
        assert!(is_local(0));
        assert!(!is_local(1));
        assert!(is_enabled(0, "other", &[]), "enabled with no languages at all");
        assert!(is_enabled(0, "other", &["en".to_owned()]), "enabled even when other is not a language");
        assert!(!is_enabled(5, "fr", &["en".to_owned()]));
        assert!(is_enabled(5, "en", &["en".to_owned()]));
    }
}
