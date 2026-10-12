//! Bangumi collection types, status labels and the status picker order (T-31a-021, `BangumiUtils.kt`,
//! `Bangumi.kt:152-186`). The collection `type` (1 to 5) is also the persisted status code. Any other number is an
//! `UnknownStatus` error, never a panic.

use super::error::BangumiError;
use kuro_platform::kind::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum BgmStatus {
    PlanToRead = 1,
    Completed = 2,
    Reading = 3,
    OnHold = 4,
    Dropped = 5,
}

impl BgmStatus {
    /// Every status, in the order of the source's status picker.
    pub const PICKER_ORDER: [BgmStatus; 5] = [
        BgmStatus::Reading,
        BgmStatus::Completed,
        BgmStatus::OnHold,
        BgmStatus::Dropped,
        BgmStatus::PlanToRead,
    ];

    /// The status of a persisted code. `None` for any other number.
    pub fn from_internal(v: i64) -> Option<Self> {
        Self::PICKER_ORDER.into_iter().find(|s| *s as i64 == v)
    }

    /// The value of `type` in collection bodies and responses. The same number as the status.
    pub fn collection_type(self) -> i64 {
        self as i64
    }

    /// The status of a collection `type` read back from the server. A missing or unknown type is `UnknownStatus`
    /// (EC-B-08, EC-B-09).
    pub fn from_collection_type(t: Option<i64>) -> Result<Self, BangumiError> {
        t.and_then(Self::from_internal).ok_or_else(|| {
            BangumiError::UnknownStatus(match t {
                Some(v) => v.to_string(),
                None => "null".to_owned(),
            })
        })
    }
}

/// The English label of a status of `kind` (the source's string resources). `None` for a code without a label, such
/// as a rereading value, which Bangumi never has.
pub fn label(kind: Kind, status: i64) -> Option<&'static str> {
    let status = BgmStatus::from_internal(status)?;
    Some(match (status, kind) {
        (BgmStatus::Reading, Kind::Manga) => "Reading",
        (BgmStatus::Reading, Kind::Anime) => "Watching",
        (BgmStatus::PlanToRead, Kind::Manga) => "Plan to read",
        (BgmStatus::PlanToRead, Kind::Anime) => "Plan to watch",
        (BgmStatus::Completed, _) => "Completed",
        (BgmStatus::OnHold, _) => "On hold",
        (BgmStatus::Dropped, _) => "Dropped",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_31a_021_collection_types_are_one_to_five_and_the_identity() {
        for s in BgmStatus::PICKER_ORDER {
            assert_eq!(s.collection_type(), s as i64);
            assert_eq!(
                BgmStatus::from_collection_type(Some(s.collection_type())),
                Ok(s)
            );
        }
        assert_eq!(BgmStatus::from_internal(0), None);
        assert_eq!(BgmStatus::from_internal(-1), None);
    }

    #[test]
    fn t_31a_021_an_unknown_or_missing_type_is_an_error() {
        assert_eq!(
            BgmStatus::from_collection_type(Some(6)),
            Err(BangumiError::UnknownStatus("6".into()))
        );
        assert_eq!(
            BgmStatus::from_collection_type(None),
            Err(BangumiError::UnknownStatus("null".into()))
        );
    }

    #[test]
    fn t_31a_021_labels_follow_the_kind_and_there_is_no_rereading_label() {
        assert_eq!(label(Kind::Manga, 3), Some("Reading"));
        assert_eq!(label(Kind::Anime, 3), Some("Watching"));
        assert_eq!(label(Kind::Manga, 1), Some("Plan to read"));
        assert_eq!(label(Kind::Anime, 1), Some("Plan to watch"));
        assert_eq!(label(Kind::Anime, 4), Some("On hold"));
        assert_eq!(label(Kind::Manga, -1), None);
    }

    #[test]
    fn t_31a_021_picker_order_is_the_source_order() {
        let codes: Vec<i64> = BgmStatus::PICKER_ORDER.iter().map(|s| *s as i64).collect();
        assert_eq!(codes, vec![3, 2, 4, 5, 1]);
    }
}
