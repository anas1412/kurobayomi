//! Komga response shapes and request bodies (`KomgaModels.kt`; `p1-data-and-config.md` section 2.2). Only the fields the
//! tracker reads are declared, and unknown fields are ignored. Wire names are camelCase, as Komga sends them.

use serde::{Deserialize, Serialize};

/// The series metadata the tracker reads (`SeriesDto.metadata`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesDto {
    pub metadata: SeriesMetadataDto,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesMetadataDto {
    pub status: String,
    pub title: String,
    pub summary: String,
}

/// A readlist. Only its name is read (`ReadListDto.toTrack`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadListDto {
    pub name: String,
}

/// Progress of a readlist (v1 shape). Converted to the v2 shape with [`ReadProgressDto::to_v2`].
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadProgressDto {
    pub books_count: i32,
    pub books_read_count: i32,
    pub books_unread_count: i32,
    pub books_in_progress_count: i32,
    pub last_read_continuous_index: i32,
}

impl ReadProgressDto {
    /// `ReadProgressDto.toV2()`: the book count is also the max number, and the index becomes the continuous number.
    pub fn to_v2(&self) -> ReadProgressV2Dto {
        ReadProgressV2Dto {
            books_count: self.books_count,
            books_read_count: self.books_read_count,
            books_unread_count: self.books_unread_count,
            books_in_progress_count: self.books_in_progress_count,
            last_read_continuous_number_sort: f64::from(self.last_read_continuous_index),
            max_number_sort: self.books_count as f32,
        }
    }
}

/// Progress of a series or a readlist (v2 shape). `maxNumberSort` is required (E-51).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadProgressV2Dto {
    pub books_count: i32,
    pub books_read_count: i32,
    pub books_unread_count: i32,
    pub books_in_progress_count: i32,
    pub last_read_continuous_number_sort: f64,
    pub max_number_sort: f32,
}

/// Body of a readlist progress `PUT`: a count of books (`lastBookRead`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadProgressUpdateDto {
    pub last_book_read: i32,
}

/// Body of a series progress `PUT`: a book number (`lastBookNumberSortRead`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadProgressUpdateV2Dto {
    pub last_book_number_sort_read: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_33a_070_series_metadata_is_read_and_other_fields_are_ignored() {
        let s: SeriesDto = serde_json::from_str(
            r#"{"id":"9","booksCount":3,"metadata":{"status":"ENDED","title":"T","summary":"S","genres":[]}}"#,
        )
        .unwrap();
        assert_eq!(s.metadata.title, "T");
        assert_eq!(s.metadata.status, "ENDED");
    }

    #[test]
    fn t_33a_071_e51_the_v2_progress_needs_max_number_sort() {
        let r = serde_json::from_str::<ReadProgressV2Dto>(
            r#"{"booksCount":1,"booksReadCount":0,"booksUnreadCount":1,"booksInProgressCount":0,"lastReadContinuousNumberSort":0.0}"#,
        );
        assert!(r.is_err());
    }

    #[test]
    fn t_33a_071_the_v1_progress_converts_to_v2() {
        let v1: ReadProgressDto = serde_json::from_str(
            r#"{"booksCount":5,"booksReadCount":2,"booksUnreadCount":3,"booksInProgressCount":0,"lastReadContinuousIndex":2}"#,
        )
        .unwrap();
        let v2 = v1.to_v2();
        assert_eq!(v2.max_number_sort, 5.0);
        assert_eq!(v2.last_read_continuous_number_sort, 2.0);
    }

    #[test]
    fn t_33a_072_update_bodies_use_the_wire_names() {
        let v2 = serde_json::to_string(&ReadProgressUpdateV2Dto {
            last_book_number_sort_read: 12.0,
        })
        .unwrap();
        assert_eq!(v2, r#"{"lastBookNumberSortRead":12.0}"#);
        let v1 = serde_json::to_string(&ReadProgressUpdateDto { last_book_read: 2 }).unwrap();
        assert_eq!(v1, r#"{"lastBookRead":2}"#);
    }
}
