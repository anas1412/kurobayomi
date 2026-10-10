//! Series folders and their episode rows (wire-local-anime). A chosen root holds one folder per series, and each series
//! folder holds its video files as episodes. kuro-local reads the files; this module maps them onto episode rows whose
//! url is the file's absolute path.

use std::io;
use std::path::{Path, PathBuf};

use kuro_data::episode::Episode;
use kuro_local::anime::episodes::episode_list;
use kuro_local::fs::list_dir;
use kuro_local::names::is_hidden;
use kuro_source::anime::{SAnime, SEpisode};

/// One series folder. Its name is the title, and its path is the anime url.
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    pub name: String,
    pub dir: PathBuf,
}

/// The visible folders directly under `root`, in name order. Files and hidden folders are left out.
pub fn series_in(root: &Path) -> io::Result<Vec<Series>> {
    let mut found: Vec<Series> = list_dir(root)?
        .into_iter()
        .filter(|e| e.is_dir && !is_hidden(&e.name))
        .map(|e| Series { name: e.name, dir: e.path })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

/// The episode rows of one series folder, newest number first as kuro-local lists them. Their `anime_id` is -1 until
/// the caller knows the anime row. An empty list means no episode file was found.
pub fn episodes_of(root: &Path, series: &Series, now: i64) -> Result<Vec<Episode>, String> {
    let anime = SAnime { url: series.name.clone(), title: series.name.clone(), ..SAnime::default() };
    let found = episode_list(root, &anime).map_err(|e| e.to_string())?;
    Ok(found.iter().enumerate().map(|(order, e)| episode_row(root, now, order, e)).collect())
}

/// One kuro-local episode as a row. kuro-local's url is `<series>/<file>` relative to the root, so the row's url is that
/// path joined onto the root.
fn episode_row(root: &Path, now: i64, order: usize, e: &SEpisode) -> Episode {
    Episode {
        url: root.join(&e.url).to_string_lossy().into_owned(),
        name: e.name.clone(),
        episode_number: f64::from(e.episode_number),
        date_upload: e.date_upload,
        source_order: i64::try_from(order).unwrap_or(i64::MAX),
        date_fetch: now,
        ..Episode::create()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tree(files: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        for file in files {
            let path = root.path().join(file);
            fs::create_dir_all(path.parent().expect("parent")).expect("series folder");
            fs::write(&path, b"video").expect("video file");
        }
        root
    }

    #[test]
    fn series_are_visible_folders_in_name_order() {
        let root = tree(&["Zeta/ep.mkv", "Alpha/ep.mkv", ".hidden/ep.mkv", "loose.mkv"]);
        fs::create_dir(root.path().join("Empty")).expect("empty folder");
        let names: Vec<String> = series_in(root.path()).expect("list").into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["Alpha", "Empty", "Zeta"]);
    }

    #[test]
    fn a_missing_root_is_an_error_not_an_empty_library() {
        let root = tempfile::tempdir().expect("temp dir");
        assert!(series_in(&root.path().join("gone")).is_err());
    }

    #[test]
    fn episodes_get_absolute_urls_and_numbers() {
        let root = tree(&["Show/Show - 1.mkv", "Show/Show - 2.mp4", "Show/notes.txt"]);
        let series = Series { name: "Show".into(), dir: root.path().join("Show") };
        let rows = episodes_of(root.path(), &series, 7).expect("scan");
        let numbers: Vec<f64> = rows.iter().map(|e| e.episode_number).collect();
        assert_eq!(numbers, [2.0, 1.0]);
        assert_eq!(rows[0].url, root.path().join("Show").join("Show - 2.mp4").to_string_lossy());
        assert!(rows.iter().all(|e| e.date_fetch == 7 && e.anime_id == -1));
        assert_eq!(rows.iter().map(|e| e.source_order).collect::<Vec<_>>(), [0, 1]);
    }

    #[test]
    fn a_folder_without_episode_files_gives_no_rows() {
        let root = tree(&["Show/cover.jpg"]);
        let series = Series { name: "Show".into(), dir: root.path().join("Show") };
        assert!(episodes_of(root.path(), &series, 0).expect("scan").is_empty());
    }
}
