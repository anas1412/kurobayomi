//! Kitsu wire status strings and local status codes (T-30a-015, `KitsuUtils.kt`). A code of the other kind is
//! rejected, and so is an unknown wire string.

use super::error::KitsuError;
use super::{COMPLETED, DROPPED, ON_HOLD, PLAN_TO_READ, PLAN_TO_WATCH, READING, WATCHING};
use kuro_platform::kind::Kind;

/// The reading (manga) or watching (anime) code and the planned code of `kind`.
fn current_and_planned(kind: Kind) -> (i64, i64) {
    match kind {
        Kind::Manga => (READING, PLAN_TO_READ),
        Kind::Anime => (WATCHING, PLAN_TO_WATCH),
    }
}

/// The wire string of a local status of `kind`.
pub fn to_wire(kind: Kind, status: i64) -> Result<&'static str, KitsuError> {
    let (current, planned) = current_and_planned(kind);
    Ok(match status {
        s if s == current => "current",
        COMPLETED => "completed",
        ON_HOLD => "on_hold",
        DROPPED => "dropped",
        s if s == planned => "planned",
        _ => return Err(KitsuError::UnknownStatus(status.to_string())),
    })
}

/// The local status code of a wire string of `kind`.
pub fn from_wire(kind: Kind, wire: &str) -> Result<i64, KitsuError> {
    let (current, planned) = current_and_planned(kind);
    Ok(match wire {
        "current" => current,
        "completed" => COMPLETED,
        "on_hold" => ON_HOLD,
        "dropped" => DROPPED,
        "planned" => planned,
        other => return Err(KitsuError::UnknownStatus(other.to_owned())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_30a_015_wire_and_local_tables_match_for_both_kinds() {
        assert_eq!(to_wire(Kind::Manga, READING).unwrap(), "current");
        assert_eq!(to_wire(Kind::Manga, PLAN_TO_READ).unwrap(), "planned");
        assert_eq!(to_wire(Kind::Anime, WATCHING).unwrap(), "current");
        assert_eq!(to_wire(Kind::Anime, PLAN_TO_WATCH).unwrap(), "planned");
        assert_eq!(from_wire(Kind::Manga, "current").unwrap(), READING);
        assert_eq!(from_wire(Kind::Anime, "current").unwrap(), WATCHING);
        assert_eq!(from_wire(Kind::Anime, "planned").unwrap(), PLAN_TO_WATCH);
        assert_eq!(from_wire(Kind::Manga, "on_hold").unwrap(), ON_HOLD);
        assert_eq!(from_wire(Kind::Manga, "dropped").unwrap(), DROPPED);
        assert_eq!(from_wire(Kind::Manga, "completed").unwrap(), COMPLETED);
    }

    #[test]
    fn t_30a_015_cross_kind_codes_are_rejected() {
        assert!(to_wire(Kind::Manga, WATCHING).is_err());
        assert!(to_wire(Kind::Anime, PLAN_TO_READ).is_err());
        assert!(to_wire(Kind::Manga, PLAN_TO_WATCH).is_err());
        assert!(to_wire(Kind::Anime, READING).is_err());
        assert!(to_wire(Kind::Anime, 99).is_err());
    }

    #[test]
    fn t_30a_015_unknown_wire_status_is_an_error() {
        assert_eq!(
            from_wire(Kind::Manga, "reading_later"),
            Err(KitsuError::UnknownStatus("reading_later".into()))
        );
    }
}
