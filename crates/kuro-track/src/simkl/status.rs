//! Simkl status ids, wire strings and labels (T-32a-030; `Simkl.kt:22-32`, `SimklUtils.kt:5-21`, B-3.1).

use super::error::SimklError;

pub const WATCHING: i64 = 1;
pub const COMPLETED: i64 = 2;
pub const ON_HOLD: i64 = 3;
pub const NOT_INTERESTING: i64 = 4;
pub const PLAN_TO_WATCH: i64 = 5;
/// Rewatching is not in the list, and it has no wire string (B-3.1).
pub const REWATCHING: i64 = 0;

/// Order of the status picker (B-3.1).
pub const STATUS_ORDER: [i64; 5] = [WATCHING, COMPLETED, ON_HOLD, NOT_INTERESTING, PLAN_TO_WATCH];

/// The Simkl wire string of a status. Any other status is `UnknownStatus` before any request is built (fix D-SK-7).
pub fn to_wire(status: i64) -> Result<&'static str, SimklError> {
    Ok(match status {
        WATCHING => "watching",
        COMPLETED => "completed",
        ON_HOLD => "hold",
        NOT_INTERESTING => "notinteresting",
        PLAN_TO_WATCH => "plantowatch",
        other => return Err(SimklError::UnknownStatus(other)),
    })
}

/// The status of a Simkl wire string. `dropped` and `notinteresting` both mean 4, and `dropped` is never produced.
pub fn from_wire(wire: &str) -> Result<i64, SimklError> {
    Ok(match wire {
        "watching" => WATCHING,
        "completed" => COMPLETED,
        "hold" => ON_HOLD,
        "dropped" | "notinteresting" => NOT_INTERESTING,
        "plantowatch" => PLAN_TO_WATCH,
        other => return Err(SimklError::UnknownWireStatus(other.to_owned())),
    })
}

/// Message key of a status label. `None` for an id that has no list entry.
pub fn label_key(status: i64) -> Option<&'static str> {
    match status {
        WATCHING => Some("watching"),
        COMPLETED => Some("completed"),
        ON_HOLD => Some("on_hold"),
        NOT_INTERESTING => Some("not_interesting"),
        PLAN_TO_WATCH => Some("plan_to_watch"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_32a_030_ids_are_the_source_values() {
        assert_eq!(
            [
                WATCHING,
                COMPLETED,
                ON_HOLD,
                NOT_INTERESTING,
                PLAN_TO_WATCH,
                REWATCHING
            ],
            [1, 2, 3, 4, 5, 0]
        );
        assert_eq!(STATUS_ORDER, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn t_32a_030_to_wire_maps_the_five_statuses_and_rejects_the_rest() {
        assert_eq!(to_wire(1).unwrap(), "watching");
        assert_eq!(to_wire(2).unwrap(), "completed");
        assert_eq!(to_wire(3).unwrap(), "hold");
        assert_eq!(to_wire(4).unwrap(), "notinteresting");
        assert_eq!(to_wire(5).unwrap(), "plantowatch");
        assert_eq!(to_wire(0), Err(SimklError::UnknownStatus(0)));
        assert_eq!(to_wire(9), Err(SimklError::UnknownStatus(9)));
    }

    #[test]
    fn t_32a_030_from_wire_accepts_dropped_and_rejects_unknown_strings() {
        assert_eq!(from_wire("dropped").unwrap(), 4);
        assert_eq!(from_wire("notinteresting").unwrap(), 4);
        assert_eq!(from_wire("hold").unwrap(), 3);
        assert_eq!(from_wire("plantowatch").unwrap(), 5);
        assert_eq!(from_wire("watching").unwrap(), 1);
        assert_eq!(from_wire("completed").unwrap(), 2);
        assert_eq!(
            from_wire("paused"),
            Err(SimklError::UnknownWireStatus("paused".into()))
        );
    }

    #[test]
    fn t_32a_030_labels_resolve_for_the_five_statuses_only() {
        assert_eq!(label_key(1), Some("watching"));
        assert_eq!(label_key(2), Some("completed"));
        assert_eq!(label_key(3), Some("on_hold"));
        assert_eq!(label_key(4), Some("not_interesting"));
        assert_eq!(label_key(5), Some("plan_to_watch"));
        assert_eq!(label_key(0), None);
    }
}
