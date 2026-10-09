//! Anime folder discovery, creation, episode rename and source rename (port of the lookups in
//! `AnimeDownloadProvider.kt` and of `renameSource` / `renameEpisode` in `AnimeDownloadManager.kt`, behaviour 6.4, 6.5
//! and 9.3).
//!
//! Layout: `<root>/<source dir>/<anime dir>/<episode dir>`. Lookups never create folders. Only [`AnimeLayout::anime_dir`]
//! and [`AnimeLayout::rename_episode`] do, as in Kotlin.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::error;

use crate::anime::model::{AnimeRef, AnimeSource, EpisodeRef};
use crate::anime::naming::{anime_dir_name, episode_dir_name, source_dir_name, unique_episode_dir_name, valid_episode_dir_names};
use crate::error::DownloadError;
use crate::layout::TMP_DIR_SUFFIX;
use crate::pipeline::MIN_FREE_BYTES;

/// Shown in the error when no downloads root is set.
const UNSET_ROOT: &str = "(unset)";

/// Every folder name an episode may have on disk, first the current name, then the legacy name, then the unique name
/// with the episode id (T-24a-066). Lookups try them in this order.
pub fn episode_folder_names(episode: &EpisodeRef) -> Vec<String> {
    let mut names: Vec<String> = valid_episode_dir_names(&episode.name, episode.scanlator.as_deref()).into();
    names.push(unique_episode_dir_name(&episode.name, episode.scanlator.as_deref(), episode.id));
    names
}

/// Rejects a downloads root that is not a local folder, such as a `content://` URI from the Android storage framework
/// (T-24a-059). The message is shown to the user; the path is kept as the stored text.
pub fn validate_root(raw: &str) -> Result<PathBuf, DownloadError> {
    let trimmed = raw.trim();
    let is_uri = trimmed.contains("://") && !trimmed.starts_with('/');
    if trimmed.is_empty() || is_uri {
        return Err(DownloadError::InvalidLocation(format!(
            "{trimmed}: choose a normal folder on this computer; content URIs are not supported"
        )));
    }
    Ok(PathBuf::from(trimmed))
}

/// The anime downloads root. `None` when the user has not chosen one.
#[derive(Clone, Debug)]
pub struct AnimeLayout {
    root: Option<PathBuf>,
}

impl AnimeLayout {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self { root }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    fn invalid_location(&self) -> DownloadError {
        let shown = self.root.as_deref().map_or_else(|| UNSET_ROOT.to_owned(), |r| r.display().to_string());
        DownloadError::InvalidLocation(shown)
    }

    fn root_path(&self) -> Result<&Path, DownloadError> {
        self.root.as_deref().ok_or_else(|| self.invalid_location())
    }

    fn source_path(&self, source: &AnimeSource) -> Result<PathBuf, DownloadError> {
        Ok(self.root_path()?.join(source_dir_name(&source.name, &source.lang)))
    }

    fn anime_path(&self, source: &AnimeSource, title: &str) -> Result<PathBuf, DownloadError> {
        Ok(self.source_path(source)?.join(anime_dir_name(title)))
    }

    /// `<root>/<source>/<anime>`, created when missing. An unset root or a failed creation is `InvalidLocation` with the
    /// displayed root path (T-24a-006).
    pub fn anime_dir(&self, source: &AnimeSource, title: &str) -> Result<PathBuf, DownloadError> {
        let dir = self.anime_path(source, title)?;
        fs::create_dir_all(&dir).map_err(|_| self.invalid_location())?;
        Ok(dir)
    }

    /// The existing source folder. Does not create it.
    pub fn find_source_dir(&self, source: &AnimeSource) -> Option<PathBuf> {
        self.source_path(source).ok().filter(|p| p.is_dir())
    }

    /// The existing anime folder. Does not create it.
    pub fn find_anime_dir(&self, source: &AnimeSource, title: &str) -> Option<PathBuf> {
        self.anime_path(source, title).ok().filter(|p| p.is_dir())
    }

    /// The episode folder under either the new or the legacy name. The first that exists wins (behaviour 9.2).
    pub fn find_episode_dir(&self, source: &AnimeSource, anime: &AnimeRef, episode: &EpisodeRef) -> Option<PathBuf> {
        let anime_dir = self.find_anime_dir(source, &anime.title)?;
        episode_folder_names(episode).into_iter().map(|n| anime_dir.join(n)).find(|p| p.exists())
    }

    /// The staging folders an episode may use: `<folder name>_tmp` for each name in [`episode_folder_names`]. Nothing
    /// is created and nothing is checked for existence.
    pub fn staging_dirs(&self, source: &AnimeSource, anime: &AnimeRef, episode: &EpisodeRef) -> Vec<PathBuf> {
        let Ok(anime_dir) = self.anime_path(source, &anime.title) else {
            return Vec::new();
        };
        episode_folder_names(episode)
            .into_iter()
            .map(|n| anime_dir.join(format!("{n}{TMP_DIR_SUFFIX}")))
            .collect()
    }

    /// `(anime folder, episode folders that exist)`. `(None, [])` when the anime folder is missing (T-24a-006).
    pub fn find_episode_dirs(
        &self,
        source: &AnimeSource,
        anime: &AnimeRef,
        episodes: &[EpisodeRef],
    ) -> (Option<PathBuf>, Vec<PathBuf>) {
        let Some(anime_dir) = self.find_anime_dir(source, &anime.title) else {
            return (None, Vec::new());
        };
        let found = episodes
            .iter()
            .filter_map(|e| episode_folder_names(e).into_iter().map(|n| anime_dir.join(n)).find(|p| p.exists()))
            .collect();
        (Some(anime_dir), found)
    }

    /// The unfinished `<name>_tmp` folders of these episodes that exist. Deleting an episode removes its partial
    /// download too (E-D01).
    pub fn find_episode_tmp_dirs(&self, source: &AnimeSource, anime: &AnimeRef, episodes: &[EpisodeRef]) -> Vec<PathBuf> {
        let Some(anime_dir) = self.find_anime_dir(source, &anime.title) else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = Vec::new();
        for episode in episodes {
            for name in episode_folder_names(episode) {
                let path = anime_dir.join(format!("{name}{TMP_DIR_SUFFIX}"));
                if path.is_dir() && !found.contains(&path) {
                    found.push(path);
                }
            }
        }
        found
    }

    /// `renameEpisode` (T-24a-007): renames the first existing of the new and legacy names to the new name. Returns the
    /// new path when a rename happened. `Ok(None)` when the names already match, or when no folder exists (logged as an
    /// error, as Kotlin does). A failed rename is an error.
    pub fn rename_episode(
        &self,
        source: &AnimeSource,
        anime: &AnimeRef,
        old: &EpisodeRef,
        new: &EpisodeRef,
    ) -> Result<Option<PathBuf>, DownloadError> {
        let anime_dir = self.anime_dir(source, &anime.title)?;
        let new_name = episode_dir_name(&new.name, new.scanlator.as_deref());
        let found = episode_folder_names(old).into_iter().map(|n| anime_dir.join(n)).find(|p| p.exists());
        let Some(old_folder) = found else {
            error!(target: "kuro::downloads", episode = old.id, "could not rename downloaded episode: no folder");
            return Ok(None);
        };
        if old_folder.file_name().is_some_and(|n| n == new_name.as_str()) {
            return Ok(None);
        }
        let target = anime_dir.join(&new_name);
        fs::rename(&old_folder, &target)?;
        Ok(Some(target))
    }

    /// `renameSource` (T-24a-008): renames the source folder to its new name. A name that differs only by case goes
    /// through `<new>_tmp` first, which a case-insensitive disk needs. Returns `Ok(true)` when a rename happened.
    pub fn rename_source(&self, old: &AnimeSource, new: &AnimeSource) -> Result<bool, DownloadError> {
        let Some(old_folder) = self.find_source_dir(old) else {
            return Ok(false);
        };
        let new_name = source_dir_name(&new.name, &new.lang);
        if old_folder.file_name().is_some_and(|n| n == new_name.as_str()) {
            return Ok(false);
        }
        let root = self.root_path()?;
        let target = root.join(&new_name);
        let case_only =
            old_folder.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == new_name.to_lowercase());
        if case_only {
            let staging = root.join(format!("{new_name}{TMP_DIR_SUFFIX}"));
            fs::rename(&old_folder, &staging)?;
            fs::rename(&staging, &target)?;
        } else {
            fs::rename(&old_folder, &target)?;
        }
        Ok(true)
    }
}

/// Free-space precheck (T-24a-017). Below 200 MiB the episode fails with `LowSpace`. An unknown value (the `statvfs`
/// call failed) skips the check.
pub fn check_free_space(free: Option<u64>) -> Result<(), DownloadError> {
    match free {
        Some(free) if free < MIN_FREE_BYTES => Err(DownloadError::LowSpace { free, need: MIN_FREE_BYTES }),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(name: &str) -> AnimeSource {
        AnimeSource { id: 1, name: name.into(), lang: "en".into(), is_http: true, unmetered: false }
    }

    fn anime() -> AnimeRef {
        AnimeRef { id: 10, source_id: 1, url: "/a".into(), title: "Show".into() }
    }

    fn episode(id: i64, name: &str, scanlator: Option<&str>) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id: 10,
            url: format!("/e/{id}"),
            name: name.into(),
            scanlator: scanlator.map(str::to_owned),
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order: id,
            upload_date: 0,
            episode_number: 0.0,
        }
    }

    #[test]
    fn anime_dir_creates_and_lookups_do_not() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = AnimeLayout::new(Some(tmp.path().to_path_buf()));
        let src = source("Src");
        assert!(layout.find_anime_dir(&src, "Show").is_none());
        assert!(!tmp.path().join("Src (EN)").exists(), "find never creates");
        let dir = layout.anime_dir(&src, "Show").unwrap();
        assert!(dir.is_dir());
        assert_eq!(layout.find_anime_dir(&src, "Show"), Some(dir));
    }

    #[test]
    fn an_unset_root_is_invalid_location_not_a_panic() {
        let layout = AnimeLayout::new(None);
        match layout.anime_dir(&source("Src"), "Show") {
            Err(DownloadError::InvalidLocation(path)) => assert_eq!(path, "(unset)"),
            other => panic!("expected InvalidLocation, got {other:?}"),
        }
        assert!(layout.find_source_dir(&source("Src")).is_none());
    }

    #[test]
    fn a_root_that_is_a_file_reports_its_path() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("not-a-dir");
        fs::write(&file, b"x").unwrap();
        let layout = AnimeLayout::new(Some(file.clone()));
        match layout.anime_dir(&source("Src"), "Show") {
            Err(DownloadError::InvalidLocation(path)) => assert_eq!(path, file.display().to_string()),
            other => panic!("expected InvalidLocation, got {other:?}"),
        }
    }

    #[test]
    fn episode_lookup_finds_the_legacy_name_and_prefers_the_new_one() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = AnimeLayout::new(Some(tmp.path().to_path_buf()));
        let src = source("Src");
        let dir = layout.anime_dir(&src, "Show").unwrap();
        fs::create_dir(dir.join("_Ep 1")).unwrap();
        let ep = episode(1, "Ep 1", Some(" "));
        assert_eq!(layout.find_episode_dir(&src, &anime(), &ep), Some(dir.join("_Ep 1")), "legacy name found");
        fs::create_dir(dir.join("Ep 1")).unwrap();
        assert_eq!(layout.find_episode_dir(&src, &anime(), &ep), Some(dir.join("Ep 1")), "new name wins");
    }

    #[test]
    fn find_episode_dirs_is_none_without_an_anime_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = AnimeLayout::new(Some(tmp.path().to_path_buf()));
        let eps = vec![episode(1, "Ep 1", None)];
        assert_eq!(layout.find_episode_dirs(&source("Src"), &anime(), &eps), (None, Vec::new()));
        let dir = layout.anime_dir(&source("Src"), "Show").unwrap();
        fs::create_dir(dir.join("Ep 1")).unwrap();
        assert_eq!(
            layout.find_episode_dirs(&source("Src"), &anime(), &eps),
            (Some(dir.clone()), vec![dir.join("Ep 1")])
        );
    }

    #[test]
    fn rename_episode_moves_the_legacy_folder_to_the_new_name() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = AnimeLayout::new(Some(tmp.path().to_path_buf()));
        let src = source("Src");
        let dir = layout.anime_dir(&src, "Show").unwrap();
        fs::create_dir(dir.join("_Ep 1")).unwrap();
        let old = episode(1, "Ep 1", Some(" "));
        let new = episode(1, "Ep 1 renamed", None);
        assert_eq!(layout.rename_episode(&src, &anime(), &old, &new).unwrap(), Some(dir.join("Ep 1 renamed")));
        assert!(!dir.join("_Ep 1").exists());
        assert!(dir.join("Ep 1 renamed").is_dir());
    }

    #[test]
    fn rename_episode_is_a_no_op_when_names_match_or_no_folder_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = AnimeLayout::new(Some(tmp.path().to_path_buf()));
        let src = source("Src");
        let ep = episode(1, "Ep 1", None);
        assert_eq!(layout.rename_episode(&src, &anime(), &ep, &ep).unwrap(), None, "no folder: nothing to do");
        let dir = layout.anime_dir(&src, "Show").unwrap();
        fs::create_dir(dir.join("Ep 1")).unwrap();
        assert_eq!(layout.rename_episode(&src, &anime(), &ep, &episode(1, "Ep 1", Some(" "))).unwrap(), None);
    }

    #[test]
    fn rename_source_renames_and_handles_a_case_only_change() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = AnimeLayout::new(Some(tmp.path().to_path_buf()));
        fs::create_dir_all(tmp.path().join("Old (EN)")).unwrap();
        assert!(layout.rename_source(&source("Old"), &source("New")).unwrap());
        assert!(tmp.path().join("New (EN)").is_dir());
        assert!(!tmp.path().join("Old (EN)").exists());
        assert!(layout.rename_source(&source("New"), &source("new")).unwrap(), "case-only change");
        assert!(tmp.path().join("new (EN)").is_dir());
        assert!(!tmp.path().join("New (EN)_tmp").exists(), "staging name is gone");
        assert!(!layout.rename_source(&source("Missing"), &source("X")).unwrap());
    }

    #[test]
    fn free_space_precheck_uses_a_strict_200_mib_floor() {
        assert!(check_free_space(Some(MIN_FREE_BYTES)).is_ok());
        assert!(matches!(check_free_space(Some(MIN_FREE_BYTES - 1)), Err(DownloadError::LowSpace { .. })));
        assert!(check_free_space(None).is_ok(), "a statvfs error skips the check");
        assert_eq!(MIN_FREE_BYTES, 209_715_200);
    }

    #[test]
    fn a_content_uri_is_not_a_valid_root() {
        assert_eq!(validate_root("/home/me/Anime").unwrap(), PathBuf::from("/home/me/Anime"));
        for bad in ["content://com.android.externalstorage.documents/tree/primary%3AAnime", "  ", ""] {
            match validate_root(bad) {
                Err(DownloadError::InvalidLocation(message)) => assert!(!message.is_empty()),
                other => panic!("expected InvalidLocation for {bad:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn staging_candidates_follow_the_folder_names_and_create_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let layout = AnimeLayout::new(Some(tmp.path().to_path_buf()));
        let src = source("Src");
        let ep = episode(7, "Ep 1", Some("Scan"));
        let staging = layout.staging_dirs(&src, &anime(), &ep);
        let dir = tmp.path().join("Src (EN)").join("Show");
        assert_eq!(staging[0], dir.join("Scan_Ep 1_tmp"));
        assert_eq!(staging[1], staging[0], "with a non-blank scanlator the legacy name is the same");
        assert_eq!(staging[2], dir.join("Scan_Ep 1 (7)_tmp"), "the unique name carries the episode id");
        assert!(!tmp.path().join("Src (EN)").exists(), "nothing is created");
    }

    #[test]
    fn the_new_and_the_legacy_fixture_trees_are_found_by_their_folder_names() {
        let new = crate::testing::dl_fixture("tree_new");
        let layout = AnimeLayout::new(Some(new.path().to_path_buf()));
        let found = layout.find_episode_dir(&source("Example"), &anime(), &episode(1, "Episode 1", Some("Scan")));
        assert_eq!(found, Some(new.path().join("Example (EN)/Show/Scan_Episode 1")));
        assert!(new.path().join("Example (EN)/Show/Scan_Episode 1/.nomedia").is_file());

        let legacy = crate::testing::dl_fixture("tree_legacy");
        let layout = AnimeLayout::new(Some(legacy.path().to_path_buf()));
        let found = layout.find_episode_dir(&source("Example"), &anime(), &episode(2, "Episode 2", Some("Scan")));
        assert_eq!(found, Some(legacy.path().join("Example (EN)/Show/Scan_Episode 2")));
        assert_eq!(layout.find_episode_dir(&source("Example"), &anime(), &episode(9, "Episode 9", None)), None);
    }

    #[test]
    fn a_case_only_source_rename_keeps_the_fixture_tree_content() {
        let tree = crate::testing::dl_fixture("tree_new");
        let layout = AnimeLayout::new(Some(tree.path().to_path_buf()));
        assert!(layout.rename_source(&source("Example"), &source("EXAMPLE")).unwrap());
        let names: Vec<String> =
            fs::read_dir(tree.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["EXAMPLE (EN)".to_owned()], "the directory ends with the new spelling");
        assert!(tree.path().join("EXAMPLE (EN)/Show/Scan_Episode 1/Episode 1.mkv").is_file());
    }
}
