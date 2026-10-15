//! The app's one source registry for the library passes and the backup preflight. A source is found by its kind and its id
//! among the recipe sources, the installed packages and the local source (no built-in sources are installed). The local source (`LOCAL_SOURCE_ID`) answers
//! with no remote calls and no items, as a local source does. An id nothing knows gives `None`, so a pass uses the stub
//! slot and the entry fails with "Source not installed".
//!
//! The lists are read at each call, so a source that starts later is found without a restart. The registry is
//! stateless: it holds nothing that could go stale.

use kuro_i18n::{MsgKey, tr};
use std::sync::Arc;

use kuro_platform::kind::Kind;
use kuro_local::LOCAL_SOURCE_ID;
use kuro_source::anime::{AnimeSource, SAnime, SEpisode};
use kuro_source::model::{SChapter, SManga};
use kuro_source::{MangaSource, SourceError};

use crate::browse::directory;
use crate::library_update::error::UpdateError;
use crate::library_update::source::{Fetched, RemoteFlags, Source, SourceDetails, SourceItem, SourceRegistry};

/// The registry the passes and the backup preflight read.
#[derive(Clone, Copy, Debug, Default)]
pub struct LiveRegistry;

/// A source found by [`LiveRegistry`].
pub enum LiveSource {
    /// The local source: it is unmetered, and its refresh and fetch return nothing without an error.
    Local,
    Manga(Arc<dyn MangaSource>),
    Anime(Arc<dyn AnimeSource>),
}

impl SourceRegistry for LiveRegistry {
    type Source = LiveSource;

    fn get(&self, kind: Kind, source_id: i64) -> Option<LiveSource> {
        if source_id == LOCAL_SOURCE_ID {
            return Some(LiveSource::Local);
        }
        let source = directory::current(kind).catalogue(source_id)?;
        match kind {
            Kind::Manga => source.manga().map(LiveSource::Manga),
            Kind::Anime => source.anime().map(LiveSource::Anime),
        }
    }
}

/// The source as an earlier run recorded it (`browse::directory::install`), for a source that is not loaded now: a backup
/// made without the GUI and the restore screen still name it.
fn recorded(source_id: i64) -> Option<kuro_data::source::StubSource> {
    if !crate::data::is_installed() {
        return None;
    }
    let db = crate::data::app().db.clone();
    [Kind::Manga, Kind::Anime].into_iter().find_map(|kind| kuro_data::source::StubSourceRepo::new(db.clone(), kind).get(source_id).ok().flatten())
}

impl LiveRegistry {
    /// The source with this id under either kind: the backup preflight does not know the kind of a row.
    fn find(&self, source_id: i64) -> Option<LiveSource> {
        self.get(Kind::Manga, source_id).or_else(|| self.get(Kind::Anime, source_id))
    }

}

impl LiveSource {
    fn name(&self) -> String {
        match self {
            Self::Local => tr(MsgKey::action_display_local_badge).to_owned(),
            Self::Manga(source) => source.name().to_owned(),
            Self::Anime(source) => source.name().to_owned(),
        }
    }

    fn lang(&self) -> String {
        match self {
            Self::Local => "other".to_owned(),
            Self::Manga(source) => source.lang().to_owned(),
            Self::Anime(source) => source.lang().to_owned(),
        }
    }

    /// `Name (LANG)`, the label the logs and the notices show for a source the app holds.
    pub fn label(&self) -> String {
        kuro_source::display_name(&self.name(), &self.lang())
    }
}

impl Source for LiveSource {
    fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }

    fn is_unmetered(&self) -> bool {
        match self {
            Self::Local => true,
            Self::Manga(source) => source.is_unmetered(),
            Self::Anime(source) => source.is_unmetered(),
        }
    }

    async fn fetch(&self, _kind: Kind, url: &str, with_details: bool) -> Result<Fetched, UpdateError> {
        match self {
            Self::Local => Ok(Fetched::default()),
            Self::Manga(source) => fetch_manga(&**source, url, with_details).await,
            Self::Anime(source) => fetch_anime(&**source, url, with_details).await,
        }
    }

    /// The details only: a metadata pass never reads the item list, so `fetch_items` is not answered here.
    async fn refresh(&self, _kind: Kind, url: &str, flags: RemoteFlags) -> Result<Option<SourceDetails>, UpdateError> {
        if !flags.fetch_details {
            return Ok(None);
        }
        match self {
            Self::Local => Ok(None),
            Self::Manga(source) => manga_details(&**source, url).await.map(Some),
            Self::Anime(source) => anime_details(&**source, url).await,
        }
    }
}

async fn fetch_manga(source: &dyn MangaSource, url: &str, with_details: bool) -> Result<Fetched, UpdateError> {
    let request = SManga::new(url, "");
    let details = if with_details {
        let manga = kuro_source::isolate(source.get_manga_details(&request)).await?;
        Some(manga_details_of(&manga))
    } else {
        None
    };
    let chapters = kuro_source::isolate(source.get_chapter_list(&request)).await?;
    Ok(Fetched { details, items: chapters.iter().enumerate().map(|(at, chapter)| chapter_item(chapter, at)).collect() })
}

async fn fetch_anime(source: &dyn AnimeSource, url: &str, with_details: bool) -> Result<Fetched, UpdateError> {
    let details = if with_details { anime_details(source, url).await? } else { None };
    let request = SAnime { url: url.to_owned(), ..SAnime::default() };
    let episodes = if source.supports_episode_list() { kuro_source::isolate(source.episode_list(&request)).await? } else { Vec::new() };
    Ok(Fetched { details, items: episodes.iter().enumerate().map(|(at, episode)| episode_item(episode, at)).collect() })
}

async fn manga_details(source: &dyn MangaSource, url: &str) -> Result<SourceDetails, UpdateError> {
    let request = SManga::new(url, "");
    let manga = kuro_source::isolate(source.get_manga_details(&request)).await?;
    Ok(manga_details_of(&manga))
}

/// `None` when the source has no details call, so the pass keeps what the library has.
async fn anime_details(source: &dyn AnimeSource, url: &str) -> Result<Option<SourceDetails>, UpdateError> {
    if !source.supports_details() {
        return Ok(None);
    }
    let request = SAnime { url: url.to_owned(), ..SAnime::default() };
    let anime = kuro_source::isolate(source.anime_details(&request)).await?;
    Ok(Some(anime_details_of(&anime)))
}

fn manga_details_of(manga: &SManga) -> SourceDetails {
    SourceDetails { title: present(&manga.title), status: i64::from(manga.status), thumbnail_url: manga.thumbnail_url.as_deref().and_then(present) }
}

fn anime_details_of(anime: &SAnime) -> SourceDetails {
    SourceDetails { title: present(&anime.title), status: i64::from(anime.status.to_i32()), thumbnail_url: anime.thumbnail_url.as_deref().and_then(present) }
}

/// The text when it has a visible character: a blank title or cover from a source never clears what the library has
/// (behaviour 5.3).
fn present(text: &str) -> Option<String> {
    (!text.trim().is_empty()).then(|| text.to_owned())
}

/// A chapter in the source's own order: `position` is `0` for the newest.
fn chapter_item(chapter: &SChapter, position: usize) -> SourceItem {
    SourceItem {
        url: chapter.url.clone(),
        name: chapter.name.clone(),
        number: f64::from(chapter.chapter_number),
        date_upload_ms: chapter.date_upload,
        source_order: i64::try_from(position).unwrap_or(i64::MAX),
        scanlator: chapter.scanlator.clone(),
    }
}

/// An episode in the source's own order: `position` is `0` for the newest.
fn episode_item(episode: &SEpisode, position: usize) -> SourceItem {
    SourceItem {
        url: episode.url.clone(),
        name: episode.name.clone(),
        number: f64::from(episode.episode_number),
        date_upload_ms: episode.date_upload,
        source_order: i64::try_from(position).unwrap_or(i64::MAX),
        scanlator: episode.scanlator.clone(),
    }
}

impl From<SourceError> for UpdateError {
    fn from(error: SourceError) -> Self {
        match error {
            SourceError::Cancelled => Self::Cancelled,
            SourceError::NotInstalled(_) => Self::SourceNotInstalled,
            other => Self::Other(other.to_string()),
        }
    }
}

/// What the backup preflight and the backup creator ask of the registry. A source the app does not hold is reported
/// by its numeric id, as the backup screens do for a stub.
impl kuro_backup::creator::SourceRegistry for LiveRegistry {
    fn is_installed(&self, source_id: i64) -> bool {
        self.find(source_id).is_some()
    }

    fn name_of(&self, source_id: i64) -> String {
        self.find(source_id).map(|source| source.name()).or_else(|| recorded(source_id).map(|stub| stub.name)).unwrap_or_default()
    }

    fn stub_label(&self, source_id: i64) -> String {
        match self.find(source_id) {
            Some(source) => kuro_source::display_name(&source.name(), &source.lang()),
            None => recorded(source_id).map_or_else(|| source_id.to_string(), |stub| stub.display_name()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_title_or_cover_is_not_a_value() {
        assert_eq!(present("   "), None);
        assert_eq!(present("Title"), Some("Title".to_owned()));
    }

    #[test]
    fn a_chapter_keeps_its_number_date_scanlator_and_position() {
        let mut chapter = SChapter::new("/c/2", "Chapter 2");
        chapter.chapter_number = 2.5;
        chapter.date_upload = 7;
        chapter.scanlator = Some("Group".to_owned());
        let item = chapter_item(&chapter, 3);
        assert_eq!(item.url, "/c/2");
        assert_eq!(item.number, 2.5);
        assert_eq!(item.date_upload_ms, 7);
        assert_eq!(item.source_order, 3);
        assert_eq!(item.scanlator.as_deref(), Some("Group"));
    }

    #[test]
    fn an_anime_details_copy_keeps_the_library_cover_when_the_source_sends_none() {
        let anime = SAnime { title: "Show".to_owned(), thumbnail_url: Some("  ".to_owned()), ..SAnime::default() };
        let details = anime_details_of(&anime);
        assert_eq!(details.title.as_deref(), Some("Show"));
        assert_eq!(details.thumbnail_url, None);
        assert_eq!(details.status, 0);
    }

    #[test]
    fn source_errors_map_to_the_pass_errors() {
        assert_eq!(UpdateError::from(SourceError::Cancelled), UpdateError::Cancelled);
        assert_eq!(UpdateError::from(SourceError::NotInstalled(4)), UpdateError::SourceNotInstalled);
        assert_eq!(UpdateError::from(SourceError::Http(404)), UpdateError::Other("HTTP error 404".to_owned()));
    }

    #[tokio::test]
    async fn the_local_source_lists_nothing_without_an_error() {
        let slot = LiveRegistry.get_or_stub(Kind::Anime, LOCAL_SOURCE_ID);
        assert!(slot.is_local() && slot.is_unmetered());
        assert_eq!(slot.fetch(Kind::Anime, "Show", true).await, Ok(Fetched::default()));
    }

    #[test]
    fn an_id_nothing_knows_is_missing_under_both_kinds() {
        assert!(LiveRegistry.get(Kind::Manga, 987_654).is_none());
        assert!(LiveRegistry.get(Kind::Anime, 987_654).is_none());
    }

    #[test]
    fn the_backup_view_knows_the_local_source_and_labels_an_unknown_id_by_number() {
        use kuro_backup::creator::SourceRegistry as BackupRegistry;
        assert!(<LiveRegistry as BackupRegistry>::is_installed(&LiveRegistry, LOCAL_SOURCE_ID));
        assert!(!<LiveRegistry as BackupRegistry>::is_installed(&LiveRegistry, 987_654));
        assert_eq!(<LiveRegistry as BackupRegistry>::stub_label(&LiveRegistry, 987_654), "987654");
        assert_eq!(<LiveRegistry as BackupRegistry>::name_of(&LiveRegistry, 987_654), "");
    }
}
