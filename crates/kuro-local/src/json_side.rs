//! Readers for `chapters.json`, `episodes.json` and `details.json` (area:14 section 20).
//!
//! The shapes are the Kotlin `tachiyomi.core.metadata` DTOs. Unknown keys are ignored. A missing required number
//! (`chapter_number`, `episode_number`) fails the whole file (E-CHAPTERS-1, E-ANIME-4).

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use kuro_source::anime::{AnimeStatus, SAnime};
use kuro_source::model::SManga;

use crate::error::LocalSourceError;
use crate::fs::FsEntry;
use crate::names::{extension, stem};

/// `details.json` for manga (legacy, area:14 behavior 20). Every field is optional: an absent field leaves the entry's
/// value alone.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MangaDetails {
    pub title: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub description: Option<String>,
    pub genre: Option<Vec<String>>,
    pub status: Option<i32>,
}

/// `details.json` for anime. The same six fields as manga, with the anime status values.
pub type AnimeDetails = MangaDetails;

/// One element of `chapters.json` (area:14 data 4.3).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ChapterDetails {
    pub chapter_number: f32,
    pub name: Option<String>,
    pub date_upload: Option<String>,
    pub scanlator: Option<String>,
}

/// One element of `episodes.json` (area:14 data 4.4).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EpisodeDetails {
    pub episode_number: f32,
    pub name: Option<String>,
    pub date_upload: Option<String>,
    #[serde(default)]
    pub fillermark: bool,
    pub scanlator: Option<String>,
    pub summary: Option<String>,
    pub preview_url: Option<String>,
}

/// The side file `<name>.json` of a folder: the first non-folder entry with that stem and a `json` extension, in listing
/// order. `details`, `chapters` and `episodes` are the names used.
pub fn side_file<'a>(entries: &'a [FsEntry], name: &str) -> Option<&'a FsEntry> {
    entries
        .iter()
        .find(|e| !e.is_dir && extension(&e.name) == "json" && stem(&e.name) == name)
}

/// The legacy `details.json` of a folder.
pub fn details_file(entries: &[FsEntry]) -> Option<&FsEntry> {
    side_file(entries, "details")
}

/// Kotlin `equalsTo`: numbers within 0.0001 are the same entry (E-CHAPTERS-3).
pub fn same_number(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.0001
}

/// Kotlin `getMangaDetails`: each non-null `details.json` field replaces the manga's value. Genres are joined with `", "`.
pub fn apply_manga_details(manga: &mut SManga, details: &MangaDetails) {
    if let Some(title) = &details.title {
        manga.title = title.clone();
    }
    if let Some(author) = &details.author {
        manga.author = Some(author.clone());
    }
    if let Some(artist) = &details.artist {
        manga.artist = Some(artist.clone());
    }
    if let Some(description) = &details.description {
        manga.description = Some(description.clone());
    }
    if let Some(genre) = &details.genre {
        manga.genre = Some(genre.join(", "));
    }
    if let Some(status) = details.status {
        manga.status = status;
    }
}

/// Kotlin `getOldAnimeDetails`: the same six fields as manga. The status is the anime status value.
pub fn apply_anime_details(anime: &mut SAnime, details: &AnimeDetails) {
    if let Some(title) = &details.title {
        anime.title = title.clone();
    }
    if let Some(author) = &details.author {
        anime.author = Some(author.clone());
    }
    if let Some(artist) = &details.artist {
        anime.artist = Some(artist.clone());
    }
    if let Some(description) = &details.description {
        anime.description = Some(description.clone());
    }
    if let Some(genre) = &details.genre {
        anime.genre = Some(genre.join(", "));
    }
    if let Some(status) = details.status {
        anime.status = AnimeStatus::from_i32(status);
    }
}

/// `chapters.json`. Any read or decode error gives `None`: the file is ignored silently (E-CHAPTERS-1).
pub fn read_chapters_json(path: &Path) -> Option<Vec<ChapterDetails>> {
    match read_json::<Vec<ChapterDetails>>(path) {
        Ok(list) => Some(list),
        Err(error) => {
            tracing::warn!("ignoring {}: {error}", path.display());
            None
        }
    }
}

/// `episodes.json`. A decode error propagates to the caller (E-ANIME-3).
pub fn read_episodes_json(path: &Path) -> Result<Vec<EpisodeDetails>, LocalSourceError> {
    read_json(path)
}

/// `details.json` as `T`. A decode error propagates to the caller.
pub fn read_details_json<T: DeserializeOwned>(path: &Path) -> Result<T, LocalSourceError> {
    read_json(path)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, LocalSourceError> {
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(dir: &Path, name: &str, text: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, text).expect("write fixture");
        path
    }

    #[test]
    fn a_bad_chapters_file_is_ignored_silently() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing_number = write(dir.path(), "chapters.json", r#"[{"name":"x"}]"#);
        assert!(read_chapters_json(&missing_number).is_none());
        let garbage = write(dir.path(), "garbage.json", "{");
        assert!(read_chapters_json(&garbage).is_none());
        assert!(read_chapters_json(&dir.path().join("absent.json")).is_none());
    }

    #[test]
    fn chapters_keep_file_order_and_optional_fields() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(
            dir.path(),
            "chapters.json",
            r#"[{"chapter_number":1,"name":"Prologue","date_upload":"2024-03-01T12:00:00","scanlator":"A","extra":1},{"chapter_number":1.5}]"#,
        );
        let list = read_chapters_json(&path).expect("decodes");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name.as_deref(), Some("Prologue"));
        assert_eq!(list[0].scanlator.as_deref(), Some("A"));
        assert_eq!(list[1].chapter_number, 1.5);
        assert!(list[1].name.is_none());
    }

    #[test]
    fn a_bad_episodes_file_propagates_and_needs_episode_number() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = write(dir.path(), "episodes.json", r#"[{"name":"x"}]"#);
        assert!(read_episodes_json(&missing).is_err());
        let garbage = write(dir.path(), "bad.json", "[");
        assert!(read_episodes_json(&garbage).is_err());
    }

    #[test]
    fn episode_preview_and_filler_are_read_but_not_required() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(
            dir.path(),
            "episodes.json",
            r#"[{"episode_number":2,"fillermark":true,"preview_url":"file:///x","summary":"S"}]"#,
        );
        let list = read_episodes_json(&path).expect("decodes");
        assert_eq!(list[0].episode_number, 2.0);
        assert_eq!(list[0].summary.as_deref(), Some("S"));
        assert!(list[0].fillermark);
        assert_eq!(list[0].preview_url.as_deref(), Some("file:///x"));
    }

    #[test]
    fn fillermark_defaults_to_false() {
        let list: Vec<EpisodeDetails> =
            serde_json::from_str(r#"[{"episode_number":1}]"#).expect("json");
        assert!(!list[0].fillermark);
        assert!(list[0].preview_url.is_none());
    }

    /// X-216: each fixture decodes, encodes and decodes to the same value, all four DTOs.
    #[test]
    fn x216_round_trip_of_all_four_dtos() {
        fn round<T: DeserializeOwned + Serialize + PartialEq + std::fmt::Debug>(json: &str) {
            let first: T = serde_json::from_str(json).expect("fixture decodes");
            let again: T = serde_json::from_str(&serde_json::to_string(&first).expect("encodes"))
                .expect("decodes again");
            assert_eq!(first, again);
        }
        round::<MangaDetails>(
            r#"{"title":"T","author":"A","artist":"B","description":"D","genre":["x","y"],"status":2}"#,
        );
        round::<AnimeDetails>(r#"{"title":"T","status":1}"#);
        round::<ChapterDetails>(
            r#"{"chapter_number":1.5,"name":"N","date_upload":"2024-03-01T12:00:00","scanlator":"S"}"#,
        );
        round::<EpisodeDetails>(
            r#"{"episode_number":3,"name":"N","date_upload":"d","fillermark":true,"scanlator":"S","summary":"M","preview_url":"u"}"#,
        );
    }

    #[test]
    fn details_fields_are_all_optional_and_genre_is_a_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write(
            dir.path(),
            "details.json",
            r#"{"title":"Foo","author":"A","genre":["Action","Drama"],"status":1}"#,
        );
        let d: MangaDetails = read_details_json(&path).expect("decodes");
        assert_eq!(d.title.as_deref(), Some("Foo"));
        assert_eq!(d.artist, None);
        assert_eq!(d.genre, Some(vec!["Action".to_owned(), "Drama".to_owned()]));
        assert_eq!(d.status, Some(1));
        let bad = write(dir.path(), "bad.json", r#"{"status":"x"}"#);
        assert!(read_details_json::<MangaDetails>(&bad).is_err());
    }
    #[test]
    fn manga_details_fields_join_genres_and_skip_absent_ones() {
        let mut manga = SManga::new("Show", "Old");
        manga.author = Some("Keep".into());
        let details: MangaDetails =
            serde_json::from_str(r#"{"title":"New","genre":["A","B"],"status":2}"#).expect("json");
        apply_manga_details(&mut manga, &details);
        assert_eq!(manga.title, "New");
        assert_eq!(manga.genre.as_deref(), Some("A, B"));
        assert_eq!(manga.status, 2);
        assert_eq!(manga.author.as_deref(), Some("Keep"));
    }

    #[test]
    fn anime_details_use_the_anime_status_and_the_same_fields() {
        let mut anime = SAnime {
            url: "Show".into(),
            title: "Old".into(),
            ..SAnime::default()
        };
        let details: AnimeDetails =
            serde_json::from_str(r#"{"title":"New","artist":"Art","status":7}"#).expect("json");
        apply_anime_details(&mut anime, &details);
        assert_eq!(anime.title, "New");
        assert_eq!(anime.artist.as_deref(), Some("Art"));
        assert_eq!(anime.status, AnimeStatus::from_i32(7));
    }

    #[test]
    fn details_file_needs_the_exact_stem_and_a_json_extension() {
        let entry = |name: &str| FsEntry {
            name: name.into(),
            path: std::path::PathBuf::from(name),
            is_dir: false,
            is_file: true,
            modified_ms: 0,
        };
        let entries = [
            entry("details.txt"),
            entry("Details.json"),
            entry("details.json"),
        ];
        assert_eq!(
            details_file(&entries).map(|e| e.name.as_str()),
            Some("details.json")
        );
    }
}
