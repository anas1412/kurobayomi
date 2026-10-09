//! Anime episode list (area:14 behavior section 11.5, T-14a-023).

use std::path::Path;

use kuro_source::anime::{SAnime, SEpisode};

use crate::error::LocalSourceError;
use crate::format::is_anime_episode;
use crate::fs::{FsEntry, find_child, list_dir};
use crate::names::{is_hidden, stem};
use crate::natsort::natural_cmp_ci;
use crate::recognition::episode_number;

/// The episodes of the folder named by `anime.url`: non-hidden files with an episode extension, newest number first,
/// then name descending. A missing folder gives an empty list.
///
/// The `episodes.json` overrides (T-14a-040), the thumbnail generation (T-14a-026, T-14a-027) and the cover and
/// background fallback from video frames are not in this step.
pub fn episode_list(root: &Path, anime: &SAnime) -> Result<Vec<SEpisode>, LocalSourceError> {
    let Some(dir) = find_child(root, &anime.url).filter(|p| p.is_dir()) else {
        return Ok(Vec::new());
    };
    let mut episodes: Vec<SEpisode> = list_dir(&dir)?
        .iter()
        .filter(|e| is_episode_entry(e))
        .map(|e| episode_of(anime, e))
        .collect();
    episodes.sort_by(|a, b| {
        b.episode_number
            .total_cmp(&a.episode_number)
            .then_with(|| natural_cmp_ci(&b.name, &a.name))
    });
    Ok(episodes)
}

/// `ArchiveAnime.isSupported` on a non-hidden entry. A folder named `x.mkv` is not an episode (E-ANIME-9).
fn is_episode_entry(entry: &FsEntry) -> bool {
    !is_hidden(&entry.name) && !entry.is_dir && is_anime_episode(&entry.name)
}

fn episode_of(anime: &SAnime, entry: &FsEntry) -> SEpisode {
    let name = stem(&entry.name).to_owned();
    SEpisode {
        url: format!("{}/{}", anime.url, entry.name),
        episode_number: episode_number(&anime.title, &name),
        date_upload: entry.modified_ms,
        name,
        ..SEpisode::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;

    fn show_with(files: &[&str]) -> io::Result<(tempfile::TempDir, SAnime)> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("Show"))?;
        for file in files {
            fs::write(root.path().join("Show").join(file), b"x")?;
        }
        Ok((
            root,
            SAnime {
                url: "Show".into(),
                title: "Show".into(),
                ..SAnime::default()
            },
        ))
    }

    #[test]
    fn episodes_are_numbered_sorted_and_keep_their_url() -> io::Result<()> {
        let (root, anime) = show_with(&[
            "Show - 2.mkv",
            "Show - 10.mp4",
            "Show - 1.mkv",
            "notes.txt",
            ".Show - 3.mkv",
        ])?;
        let got = episode_list(root.path(), &anime).map_err(io::Error::other)?;
        let numbers: Vec<f32> = got.iter().map(|e| e.episode_number).collect();
        assert_eq!(numbers, [10.0, 2.0, 1.0]);
        assert_eq!(got[0].url, "Show/Show - 10.mp4");
        assert_eq!(got[0].name, "Show - 10");
        assert!(got[0].date_upload > 0);
        Ok(())
    }

    #[test]
    fn the_episode_extension_is_case_sensitive() -> io::Result<()> {
        let (root, anime) = show_with(&["EP01.MKV", "EP02.mkv"])?;
        let got = episode_list(root.path(), &anime).map_err(io::Error::other)?;
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "EP02");
        Ok(())
    }

    #[test]
    fn a_folder_named_like_an_episode_is_not_one() -> io::Result<()> {
        let (root, anime) = show_with(&[])?;
        fs::create_dir(root.path().join("Show").join("Extra.mkv"))?;
        let got = episode_list(root.path(), &anime).map_err(io::Error::other)?;
        assert!(got.is_empty());
        Ok(())
    }

    #[test]
    fn a_missing_folder_is_an_empty_list() -> io::Result<()> {
        let (root, _) = show_with(&[])?;
        let gone = SAnime {
            url: "Gone".into(),
            ..SAnime::default()
        };
        assert!(episode_list(root.path(), &gone).map_err(io::Error::other)?.is_empty());
        Ok(())
    }
}
