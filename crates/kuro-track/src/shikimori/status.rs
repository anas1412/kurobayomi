//! Shikimori status codes, wire strings and labels (T-31a-002, T-31a-018, `ShikimoriUtils.kt`). The codes are the
//! persisted values of `sync_id` 4. A code or string outside the table is an error, never a panic (D4).

use super::error::ShikimoriError;
use crate::model::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i64)]
pub enum ShikiStatus {
    Reading = 1,
    Completed = 2,
    OnHold = 3,
    Dropped = 4,
    PlanToRead = 5,
    Rereading = 6,
}

impl ShikiStatus {
    /// Every status, in the list order of the source (`[1, 2, 3, 4, 5, 6]`).
    pub const ALL: [ShikiStatus; 6] = [
        ShikiStatus::Reading,
        ShikiStatus::Completed,
        ShikiStatus::OnHold,
        ShikiStatus::Dropped,
        ShikiStatus::PlanToRead,
        ShikiStatus::Rereading,
    ];

    /// The status of a persisted code. `None` for any other number.
    pub fn from_internal(v: i64) -> Option<Self> {
        Self::ALL.into_iter().find(|s| *s as i64 == v)
    }

    /// The string sent as `user_rate.status`.
    pub fn api_str(self) -> &'static str {
        match self {
            ShikiStatus::Reading => "watching",
            ShikiStatus::Completed => "completed",
            ShikiStatus::OnHold => "on_hold",
            ShikiStatus::Dropped => "dropped",
            ShikiStatus::PlanToRead => "planned",
            ShikiStatus::Rereading => "rewatching",
        }
    }

    /// The inverse of [`ShikiStatus::api_str`]. An unknown string is `UnknownStatus`.
    pub fn from_api(s: &str) -> Result<Self, ShikimoriError> {
        Ok(match s {
            "watching" => ShikiStatus::Reading,
            "completed" => ShikiStatus::Completed,
            "on_hold" => ShikiStatus::OnHold,
            "dropped" => ShikiStatus::Dropped,
            "planned" => ShikiStatus::PlanToRead,
            "rewatching" => ShikiStatus::Rereading,
            other => return Err(ShikimoriError::UnknownStatus(other.to_owned())),
        })
    }
}

/// The English label of a status of `kind` (the source's string resources). `None` for an unknown code.
pub fn label(kind: Kind, status: i64) -> Option<&'static str> {
    let status = ShikiStatus::from_internal(status)?;
    Some(match (status, kind) {
        (ShikiStatus::Reading, Kind::Manga) => "Reading",
        (ShikiStatus::Reading, Kind::Anime) => "Watching",
        (ShikiStatus::PlanToRead, Kind::Manga) => "Plan to read",
        (ShikiStatus::PlanToRead, Kind::Anime) => "Plan to watch",
        (ShikiStatus::Completed, _) => "Completed",
        (ShikiStatus::OnHold, _) => "On hold",
        (ShikiStatus::Dropped, _) => "Dropped",
        (ShikiStatus::Rereading, Kind::Manga) => "Rereading",
        (ShikiStatus::Rereading, Kind::Anime) => "Rewatching",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_31a_002_codes_are_one_to_six() {
        let codes: Vec<i64> = ShikiStatus::ALL.iter().map(|s| *s as i64).collect();
        assert_eq!(codes, vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(ShikiStatus::from_internal(0), None);
        assert_eq!(ShikiStatus::from_internal(7), None);
        assert_eq!(ShikiStatus::from_internal(5), Some(ShikiStatus::PlanToRead));
    }

    #[test]
    fn t_31a_002_api_strings_round_trip() {
        for s in ShikiStatus::ALL {
            assert_eq!(ShikiStatus::from_api(s.api_str()).unwrap(), s);
        }
        assert_eq!(ShikiStatus::Rereading.api_str(), "rewatching");
        assert_eq!(ShikiStatus::PlanToRead.api_str(), "planned");
    }

    #[test]
    fn t_31a_002_an_unknown_api_string_is_an_error() {
        assert_eq!(
            ShikiStatus::from_api("nope"),
            Err(ShikimoriError::UnknownStatus("nope".into()))
        );
    }

    #[test]
    fn t_31a_018_labels_follow_the_kind() {
        assert_eq!(label(Kind::Manga, 1), Some("Reading"));
        assert_eq!(label(Kind::Anime, 1), Some("Watching"));
        assert_eq!(label(Kind::Anime, 6), Some("Rewatching"));
        assert_eq!(label(Kind::Manga, 6), Some("Rereading"));
        assert_eq!(label(Kind::Manga, 9), None);
    }
}
