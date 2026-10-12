//! Renames the download folder of a chapter or episode whose name changed during a sync (T-26a-086; the
//! `renameChapter` and `renameEpisode` calls of `SyncChaptersWithSource` and `SyncEpisodesWithSource`).
//!
//! The planner of the sync offers every rewritten row for a rename. The rule that decides is here (S-17): the folder is
//! renamed only when the old item is downloaded (its folder or `.cbz` exists) and its folder name differs from the one
//! the new name gives. An item that is not downloaded is never touched, and no folder is created for it. A failed rename
//! is logged and never fails the sync (EC-11).
//!
//! The downloads folder is the one of the app and the source name comes from a lookup the caller passes (`Name (EN)`,
//! the line the source pages show). A source the lookup does not know keeps its folders as they are.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use kuro_data::anime::Anime;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_data::manga::Manga;
use kuro_domain::items::chapter_sync::ChapterRenames;
use kuro_domain::items::episode_sync::EpisodeRenames;
use kuro_downloads::anime::layout::AnimeLayout;
use kuro_downloads::anime::model::{AnimeRef, AnimeSource, EpisodeRef};
use kuro_downloads::index::DownloadIndex;
use kuro_downloads::layout::{CBZ_EXTENSION, DownloadLayout, chapter_dir_name, valid_chapter_names};

use crate::entry_core::anime::download_backend::split_source_name;

/// The name line of an installed source (`Name (EN)`), or `None` when the source is not known.
pub type SourceNames = Arc<dyn Fn(i64) -> Option<String> + Send + Sync>;

/// The folder names a chapter rename moved: bare names, without `.cbz`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Renamed {
    pub old: String,
    pub new: String,
}

/// The chapter rename of `renameChapter`. `Ok(None)` when nothing was renamed: the manga folder or the old item is
/// missing, or the folder name did not change.
pub fn rename_chapter_folder(
    layout: &DownloadLayout,
    source_name: &str,
    manga_title: &str,
    old: (&str, Option<&str>),
    new: (&str, Option<&str>),
) -> io::Result<Option<Renamed>> {
    let Some(manga_dir) = layout.find_manga_dir(source_name, manga_title) else { return Ok(None) };
    let Some(old_path) = valid_chapter_names(old.0, old.1).into_iter().map(|n| manga_dir.join(n)).find(|p| p.exists()) else {
        return Ok(None);
    };
    let mut new_name = chapter_dir_name(new.0, new.1);
    let is_cbz = old_path.is_file() && old_path.extension().is_some_and(|e| e == CBZ_EXTENSION);
    if is_cbz {
        new_name = format!("{new_name}.{CBZ_EXTENSION}");
    }
    let old_name = old_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if old_name == new_name {
        return Ok(None);
    }
    std::fs::rename(&old_path, manga_dir.join(&new_name))?;
    let bare = |name: &str| name.strip_suffix(&format!(".{CBZ_EXTENSION}")).unwrap_or(name).to_owned();
    Ok(Some(Renamed { old: bare(&old_name), new: bare(&new_name) }))
}

/// The anime episode as the downloads crate names it.
fn episode_ref(e: &Episode) -> EpisodeRef {
    EpisodeRef {
        id: e.id,
        anime_id: e.anime_id,
        url: e.url.clone(),
        name: e.name.clone(),
        scanlator: e.scanlator.clone(),
        seen: e.seen,
        bookmark: e.bookmark,
        fillermark: e.fillermark,
        source_order: e.source_order,
        upload_date: e.date_upload,
        episode_number: e.episode_number,
    }
}

/// The folder renames behind the two sync ports.
pub struct FolderRenames {
    manga_root: PathBuf,
    anime_root: PathBuf,
    source_names: SourceNames,
    index: Option<DownloadIndex>,
}

impl FolderRenames {
    /// `manga_root` and `anime_root` are the downloads folders of each kind. `index` is the manga download cache; a
    /// rename moves the chapter in it so the page shows the chapter downloaded without a rescan.
    pub fn new(manga_root: PathBuf, anime_root: PathBuf, source_names: SourceNames, index: Option<DownloadIndex>) -> Self {
        Self { manga_root, anime_root, source_names, index }
    }
}

impl ChapterRenames for FolderRenames {
    fn rename(&self, manga: &Manga, stored: &Chapter, prepared: &Chapter) {
        let Some(source_name) = (self.source_names)(manga.source) else { return };
        let layout = DownloadLayout::new(&self.manga_root);
        let old = (stored.name.as_str(), stored.scanlator.as_deref());
        let new = (prepared.name.as_str(), prepared.scanlator.as_deref());
        match rename_chapter_folder(&layout, &source_name, &manga.title, old, new) {
            Ok(Some(renamed)) => {
                if let Some(index) = &self.index {
                    index.remove_chapter(manga.source, &manga.title, std::slice::from_ref(&renamed.old));
                    index.add_chapter(manga.source, &DownloadLayout::source_dir_name(&source_name), &manga.title, &renamed.new);
                }
            }
            Ok(None) => {}
            Err(error) => tracing::error!(%error, chapter = stored.id, "could not rename the downloaded chapter"),
        }
    }
}

impl EpisodeRenames for FolderRenames {
    fn rename(&self, anime: &Anime, stored: &Episode, prepared: &Episode) {
        let Some(shown) = (self.source_names)(anime.source) else { return };
        let (name, lang) = split_source_name(&shown);
        let source = AnimeSource { id: anime.source, name, lang, is_http: true, unmetered: false };
        let show = AnimeRef { id: anime.id, source_id: anime.source, url: anime.url.clone(), title: anime.title.clone() };
        let (old, new) = (episode_ref(stored), episode_ref(prepared));
        let layout = AnimeLayout::new(Some(self.anime_root.clone()));
        // The layout logs an error for an episode with no folder, and a rename creates the anime folder; only a
        // downloaded episode gets that far (S-17).
        if layout.find_episode_dir(&source, &show, &old).is_none() {
            return;
        }
        if let Err(error) = layout.rename_episode(&source, &show, &old, &new) {
            tracing::error!(%error, episode = stored.id, "could not rename the downloaded episode");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn tree() -> (tempfile::TempDir, DownloadLayout) {
        let dir = tempfile::tempdir().expect("temp dir");
        let layout = DownloadLayout::new(dir.path());
        (dir, layout)
    }

    fn manga_dir(dir: &tempfile::TempDir) -> PathBuf {
        let path = dir.path().join("Src (EN)").join("Title");
        fs::create_dir_all(&path).expect("manga folder");
        path
    }

    #[test]
    fn s_17_a_downloaded_chapter_whose_dir_name_changed_is_renamed() {
        let (dir, layout) = tree();
        let manga = manga_dir(&dir);
        fs::create_dir(manga.join("Ch 1")).expect("chapter folder");
        let done = rename_chapter_folder(&layout, "Src (EN)", "Title", ("Ch 1", None), ("Ch 1 fixed", None)).expect("rename");
        assert_eq!(done, Some(Renamed { old: "Ch 1".into(), new: "Ch 1 fixed".into() }));
        assert!(manga.join("Ch 1 fixed").is_dir() && !manga.join("Ch 1").exists());
    }

    #[test]
    fn s_17_a_chapter_that_is_not_downloaded_is_left_alone_and_no_folder_is_made() {
        let (dir, layout) = tree();
        assert_eq!(rename_chapter_folder(&layout, "Src (EN)", "Title", ("Ch 1", None), ("Ch 2", None)).expect("rename"), None);
        assert!(!dir.path().join("Src (EN)").exists(), "nothing is created for a missing manga folder");
        let manga = manga_dir(&dir);
        assert_eq!(rename_chapter_folder(&layout, "Src (EN)", "Title", ("Ch 1", None), ("Ch 2", None)).expect("rename"), None);
        assert_eq!(fs::read_dir(&manga).expect("list").count(), 0, "no chapter folder is created either");
    }

    #[test]
    fn s_17_an_unchanged_dir_name_is_not_renamed() {
        let (dir, layout) = tree();
        let manga = manga_dir(&dir);
        fs::create_dir(manga.join("Ch 1")).expect("chapter folder");
        // the planner offers every rewritten row, most of them with the same name
        assert_eq!(rename_chapter_folder(&layout, "Src (EN)", "Title", ("Ch 1", None), ("Ch 1", None)).expect("rename"), None);
        assert!(manga.join("Ch 1").is_dir());
    }

    #[test]
    fn a_cbz_keeps_its_extension_and_a_scanlator_is_part_of_the_name() {
        let (dir, layout) = tree();
        let manga = manga_dir(&dir);
        fs::write(manga.join("Team_Ch 1.cbz"), b"zip").expect("archive");
        let done = rename_chapter_folder(&layout, "Src (EN)", "Title", ("Ch 1", Some("Team")), ("Ch 1b", Some("Team"))).expect("rename");
        assert_eq!(done, Some(Renamed { old: "Team_Ch 1".into(), new: "Team_Ch 1b".into() }), "names are bare");
        assert!(manga.join("Team_Ch 1b.cbz").is_file());
    }

    fn names(known: &'static str) -> SourceNames {
        Arc::new(move |id| (id == 7).then(|| known.to_owned()))
    }

    #[test]
    fn the_chapter_port_renames_through_the_layout_and_ignores_unknown_sources() {
        let (dir, _) = tree();
        let manga_folder = manga_dir(&dir);
        fs::create_dir(manga_folder.join("Ch 1")).expect("chapter folder");
        let manga = Manga { source: 7, title: "Title".into(), ..Manga::create() };
        let stored = Chapter { name: "Ch 1".into(), ..Chapter::create() };
        let prepared = Chapter { name: "Ch 1 v2".into(), ..Chapter::create() };
        let unknown = FolderRenames::new(dir.path().to_path_buf(), dir.path().to_path_buf(), names("Other (EN)"), None);
        ChapterRenames::rename(&unknown, &Manga { source: 8, ..manga.clone() }, &stored, &prepared);
        assert!(manga_folder.join("Ch 1").is_dir(), "a source the lookup does not know is not touched");
        let known = FolderRenames::new(dir.path().to_path_buf(), dir.path().to_path_buf(), names("Src (EN)"), None);
        ChapterRenames::rename(&known, &manga, &stored, &prepared);
        assert!(manga_folder.join("Ch 1 v2").is_dir());
    }

    #[test]
    fn s_17_the_episode_port_renames_only_a_downloaded_episode() {
        let (dir, _) = tree();
        let anime_dir = dir.path().join("Src (EN)").join("Show");
        let anime = Anime { source: 7, title: "Show".into(), ..Anime::create() };
        let stored = Episode { name: "Ep 1".into(), ..Episode::create() };
        let prepared = Episode { name: "Ep 1 fixed".into(), ..Episode::create() };
        let renames = FolderRenames::new(dir.path().join("elsewhere"), dir.path().to_path_buf(), names("Src (EN)"), None);
        EpisodeRenames::rename(&renames, &anime, &stored, &prepared);
        assert!(!anime_dir.exists(), "not downloaded: nothing is renamed or created");
        fs::create_dir_all(anime_dir.join("Ep 1")).expect("episode folder");
        EpisodeRenames::rename(&renames, &anime, &stored, &prepared);
        assert!(anime_dir.join("Ep 1 fixed").is_dir() && !anime_dir.join("Ep 1").exists());
    }
}
