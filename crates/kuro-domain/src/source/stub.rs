//! Uninstalled-source placeholders (`StubMangaSource.kt`, `StubAnimeSource.kt`). Every remote call on a stub fails
//! with `DomainError::SourceNotInstalled` carrying the stub's display name. The persisted record is
//! `kuro_data::source::StubSource`; the behaviour is here, because the error type is a domain type.

use crate::error::DomainError;
use crate::source::model::Source;

pub use kuro_data::source::StubSource;

/// The remote operations of a source. A stub fails every one of them (TC-SRC-04).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteOp {
    MangaDetails,
    MangaChapterList,
    MangaPageList,
    AnimePopular,
    AnimeLatest,
    AnimeSearch,
    AnimeEpisodeUpdate,
    AnimeSeasonUpdate,
    AnimeRelated,
    AnimeHosterList,
    AnimeVideoList,
}

/// `supportsLatest` of an anime stub. Stubs never offer a latest list.
pub fn supports_latest(_stub: &StubSource) -> bool {
    false
}

/// `supportsRelatedAnime` of an anime stub.
pub fn supports_related(_stub: &StubSource) -> bool {
    false
}

/// Answers any remote call on an uninstalled source with `SourceNotInstalled`.
pub fn remote<T>(stub: &StubSource, op: RemoteOp) -> Result<T, DomainError> {
    tracing::debug!(id = stub.id, ?op, "remote call on an uninstalled source");
    Err(DomainError::SourceNotInstalled(stub.display_name()))
}

/// `StubSource.from(source)`: copies id, language and name only.
impl From<&Source> for StubSource {
    fn from(source: &Source) -> Self {
        StubSource { id: source.id, lang: source.lang.clone(), name: source.name.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_OPS: [RemoteOp; 11] = [
        RemoteOp::MangaDetails,
        RemoteOp::MangaChapterList,
        RemoteOp::MangaPageList,
        RemoteOp::AnimePopular,
        RemoteOp::AnimeLatest,
        RemoteOp::AnimeSearch,
        RemoteOp::AnimeEpisodeUpdate,
        RemoteOp::AnimeSeasonUpdate,
        RemoteOp::AnimeRelated,
        RemoteOp::AnimeHosterList,
        RemoteOp::AnimeVideoList,
    ];

    #[test]
    fn every_remote_operation_fails_with_source_not_installed_and_the_display_name() {
        let stub = StubSource { id: 9, lang: "en".into(), name: "Gone".into() };
        for op in ALL_OPS {
            match remote::<()>(&stub, op) {
                Err(DomainError::SourceNotInstalled(name)) => assert_eq!(name, "Gone (EN)", "TC-SRC-04 for {op:?}"),
                other => panic!("{op:?} gave {other:?}"),
            }
        }
    }

    #[test]
    fn display_name_falls_back_to_the_id_when_name_or_language_is_blank() {
        let stub = StubSource { id: 4242, lang: String::new(), name: "Gone".into() };
        match remote::<()>(&stub, RemoteOp::MangaDetails) {
            Err(DomainError::SourceNotInstalled(name)) => assert_eq!(name, "4242", "TC-SRC-03"),
            other => panic!("gave {other:?}"),
        }
    }

    #[test]
    fn anime_stub_offers_no_latest_or_related_lists() {
        let stub = StubSource { id: 1, lang: "en".into(), name: "Gone".into() };
        assert!(!supports_latest(&stub));
        assert!(!supports_related(&stub));
    }

    #[test]
    fn from_source_copies_only_id_language_and_name() {
        let mut source = Source::new(7, "Name", "fr");
        source.pin = crate::source::pin::Pins::PINNED;
        source.is_used_last = true;
        assert_eq!(StubSource::from(&source), StubSource { id: 7, lang: "fr".into(), name: "Name".into() });
    }
}
