//! MangaUpdates search results as tracker search rows (T-32a-021, T-32a-023; `MURecord.toTrackSearch`, B-2.14).

use super::TRACKER_ID;
use super::dto::Record;
use crate::model::search::MangaTrackSearch;

/// Text of an HTML fragment: tags removed, entities decoded (`htmlDecode`, `StringExtensions.kt:65-67`).
pub fn html_to_text(input: &str) -> String {
    let stripped = ammonia::Builder::empty().clean(input).to_string();
    html_escape::decode_html_entities(&stripped).into_owned()
}

/// Maps one record. Missing type and year become empty text, not the literal `null` (fix D-MU-5). The search never
/// gives a chapter count or a score.
pub fn to_track_search(record: &Record) -> MangaTrackSearch {
    let title = html_to_text(record.title.as_deref().unwrap_or_default());
    let tracking_url = record.url.clone().unwrap_or_default();
    let mut out = MangaTrackSearch::new(
        TRACKER_ID,
        record.series_id.unwrap_or(0),
        title,
        tracking_url,
    );
    out.cover_url = record
        .image
        .as_ref()
        .and_then(|i| i.url.as_ref())
        .and_then(|u| u.original.clone())
        .unwrap_or_default();
    out.summary = html_to_text(record.description.as_deref().unwrap_or_default());
    out.publishing_type = record.kind.clone().unwrap_or_default();
    out.start_date = record.year.clone().unwrap_or_default();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TrackerId;

    #[test]
    fn t_32a_023_entities_decode_and_tags_are_removed() {
        assert_eq!(html_to_text("Tom &amp; Jerry <i>x</i>"), "Tom & Jerry x");
    }

    #[test]
    fn t_32a_023_an_unclosed_tag_is_closed_and_text_kept() {
        assert_eq!(html_to_text("<b>unclosed"), "unclosed");
    }

    #[test]
    fn t_32a_023_plain_text_empty_and_unicode_pass_through() {
        assert_eq!(html_to_text(""), "");
        assert_eq!(html_to_text("a < b"), "a < b");
        assert_eq!(html_to_text("日本語 &lt;3"), "日本語 <3");
    }

    #[test]
    fn t_32a_023_the_twelve_fixture_cases_match_kotlin_or_are_known_differences() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/mangaupdates/htmldecode_cases.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 12);
        for case in cases {
            let got = html_to_text(case["input"].as_str().unwrap());
            let kotlin = case["kotlin"].as_str().unwrap();
            match case.get("port").and_then(|p| p.as_str()) {
                Some(port) => {
                    assert_eq!(got, port, "{}", case["name"]);
                    assert_ne!(got, kotlin, "{} is no longer a difference", case["name"]);
                }
                None => assert_eq!(got, kotlin, "{}", case["name"]),
            }
        }
    }

    #[test]
    fn t_32a_021_a_record_maps_to_a_search_row() {
        let record: Record = serde_json::from_str(
            r#"{"series_id":77,"title":"<b>Title</b> &amp; more","url":"https://www.mangaupdates.com/series/x","description":"Desc &quot;q&quot;","image":{"url":{"original":"https://img/o.jpg","thumb":"https://img/t.jpg"}},"type":"Manga","year":"2019"}"#,
        )
        .unwrap();
        let row = to_track_search(&record);
        assert_eq!(row.tracker_id, TrackerId::MANGAUPDATES);
        assert_eq!(row.remote_id, 77);
        assert_eq!(row.title, "Title & more");
        assert_eq!(row.summary, "Desc \"q\"");
        assert_eq!(row.cover_url, "https://img/o.jpg");
        assert_eq!(row.tracking_url, "https://www.mangaupdates.com/series/x");
        assert_eq!(row.publishing_type, "Manga");
        assert_eq!(row.start_date, "2019");
        assert_eq!(row.total_chapters, 0);
        assert_eq!(row.score, None);
        assert_eq!(row.publishing_status, "");
    }

    #[test]
    fn t_32a_021_null_type_and_year_are_empty_strings_not_null_text() {
        let record: Record =
            serde_json::from_str(r#"{"series_id":1,"title":"A","type":null,"year":null}"#).unwrap();
        let row = to_track_search(&record);
        assert_eq!(row.publishing_type, "");
        assert_eq!(row.start_date, "");
        assert_eq!(row.cover_url, "");
        assert_eq!(row.tracking_url, "");
    }

    #[test]
    fn t_32a_021_a_missing_series_id_is_zero() {
        let row = to_track_search(&Record::default());
        assert_eq!(row.remote_id, 0);
        assert_eq!(row.title, "");
    }
}
