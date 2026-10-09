//! ComicInfo bridge: manga to ComicInfo, apply ComicInfo, and status helpers (area:14 section 5.6, behavior 19.6-19.7).

use std::collections::HashSet;

use kuro_source::model::SManga;

use crate::comicinfo::{ComicInfo, ComicInfoPublishingStatus};

/// Kotlin `SManga.getComicInfo()`, the local-source legacy builder. Only Series, Summary, Writer, Penciller, Genre and
/// PublishingStatusTachiyomi are set (behavior 19.6).
pub fn manga_to_comic_info(manga: &SManga) -> ComicInfo {
    ComicInfo {
        series: Some(manga.title.clone()),
        summary: manga.description.clone(),
        writer: manga.author.clone(),
        penciller: manga.artist.clone(),
        genre: manga.genre.clone(),
        publishing_status: Some(ComicInfoPublishingStatus::to_comic_info_value(i64::from(manga.status)).to_owned()),
        ..ComicInfo::default()
    }
}

/// Kotlin `SManga.copyFromComicInfo` (behavior 19.7). Status is always assigned; Title, Number, Translator, Web and
/// SourceAniyomi are never read on import.
pub fn apply_comic_info(manga: &mut SManga, info: &ComicInfo) {
    if let Some(series) = &info.series {
        manga.title = series.clone();
    }
    if let Some(writer) = &info.writer {
        manga.author = Some(writer.clone());
    }
    if let Some(summary) = &info.summary {
        manga.description = Some(summary.clone());
    }

    // Genre: whole strings, no comma split. Artist: split on ", " first.
    let genre = join_distinct(
        [&info.genre, &info.tags, &info.categories]
            .into_iter()
            .flatten()
            .map(String::as_str),
    );
    if !genre.is_empty() {
        manga.genre = Some(genre);
    }
    let artist_parts: Vec<&str> = [
        &info.penciller,
        &info.inker,
        &info.colorist,
        &info.letterer,
        &info.cover_artist,
    ]
    .into_iter()
    .flatten()
    .flat_map(|value| value.split(", "))
    .collect();
    let artist = join_distinct(artist_parts.into_iter());
    if !artist.is_empty() {
        manga.artist = Some(artist);
    }

    manga.status = ComicInfoPublishingStatus::to_s_manga_value(info.publishing_status.as_deref());
}

/// Kotlin `distinct().joinToString(", ") { it.trim() }`: dedupe on the exact strings, then trim each one while joining.
fn join_distinct<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let mut seen = HashSet::new();
    items
        .filter(|item| seen.insert(*item))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manga() -> SManga {
        SManga {
            title: "Foo".into(),
            author: Some("W".into()),
            artist: Some("P".into()),
            description: Some("Story".into()),
            genre: Some("Action".into()),
            status: 1,
            ..SManga::new("Foo", "Foo")
        }
    }

    #[test]
    fn t_14_manga_to_comic_info_fields() {
        let info = manga_to_comic_info(&manga());
        assert_eq!(info.series.as_deref(), Some("Foo"));
        assert_eq!(info.summary.as_deref(), Some("Story"));
        assert_eq!(info.writer.as_deref(), Some("W"));
        assert_eq!(info.penciller.as_deref(), Some("P"));
        assert_eq!(info.genre.as_deref(), Some("Action"));
        assert_eq!(info.publishing_status.as_deref(), Some("Ongoing"));
        assert_eq!(info.title, None);
        assert_eq!(info.number, None);
        assert_eq!(info.source, None);
        assert_eq!(info.tags, None);
    }

    #[test]
    fn the_legacy_status_outside_the_table_writes_unknown() {
        let mut m = manga();
        m.status = 7;
        assert_eq!(manga_to_comic_info(&m).publishing_status.as_deref(), Some("Unknown"));
    }

    #[test]
    fn genre_merges_without_splitting_on_commas() {
        let mut m = manga();
        let info = ComicInfo {
            genre: Some("Action, Drama".into()),
            tags: Some("Drama".into()),
            ..ComicInfo::default()
        };
        apply_comic_info(&mut m, &info);
        assert_eq!(m.genre.as_deref(), Some("Action, Drama, Drama"));
    }

    #[test]
    fn artist_merges_after_splitting_on_comma_space() {
        let mut m = manga();
        let info = ComicInfo {
            penciller: Some("A, B".into()),
            inker: Some("A".into()),
            ..ComicInfo::default()
        };
        apply_comic_info(&mut m, &info);
        assert_eq!(m.artist.as_deref(), Some("A, B"));
    }

    #[test]
    fn an_empty_merge_leaves_genre_and_artist_alone() {
        let mut m = manga();
        apply_comic_info(&mut m, &ComicInfo::default());
        assert_eq!(m.genre.as_deref(), Some("Action"));
        assert_eq!(m.artist.as_deref(), Some("P"));
    }

    #[test]
    fn status_is_always_assigned_even_when_absent() {
        let mut m = manga();
        apply_comic_info(&mut m, &ComicInfo::default());
        assert_eq!(m.status, 0);
    }

    #[test]
    fn the_title_comes_from_series_and_an_empty_series_assigns_empty() {
        let mut m = manga();
        apply_comic_info(
            &mut m,
            &ComicInfo {
                series: Some(String::new()),
                title: Some("Not read".into()),
                ..ComicInfo::default()
            },
        );
        assert_eq!(m.title, "");
    }

    #[test]
    fn unread_fields_are_not_copied() {
        let mut m = manga();
        let before = m.clone();
        apply_comic_info(
            &mut m,
            &ComicInfo {
                title: Some("T".into()),
                number: Some("1".into()),
                translator: Some("Team".into()),
                web: Some("https://x".into()),
                source: Some("S".into()),
                ..ComicInfo::default()
            },
        );
        assert_eq!(m.title, before.title);
        assert_eq!(m.status, 0);
        assert_eq!(m.url, before.url);
    }

    #[test]
    fn the_legacy_fields_round_trip_through_the_file_text() -> Result<(), crate::error::LocalSourceError> {
        let original = manga();
        let xml = manga_to_comic_info(&original).to_xml();
        let info = ComicInfo::from_xml(xml.as_bytes())?;
        let mut back = SManga::new("Foo", "Foo");
        apply_comic_info(&mut back, &info);
        assert_eq!(back.title, "Foo");
        assert_eq!(back.author.as_deref(), Some("W"));
        assert_eq!(back.status, 1);
        Ok(())
    }
}
