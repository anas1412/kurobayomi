//! MangaUpdates list ids, status order, labels and logo colour (T-32a-010; `MangaUpdates.kt:22-27, 46-67`).

pub const READING_LIST: i64 = 0;
pub const WISH_LIST: i64 = 1;
pub const COMPLETE_LIST: i64 = 2;
pub const UNFINISHED_LIST: i64 = 3;
pub const ON_HOLD_LIST: i64 = 4;
/// Rereading is never sent to MangaUpdates (B-2.1).
pub const REREADING: i64 = -1;

/// Order of the status picker: Reading, Complete, On hold, Unfinished, Wish (B-2.1).
pub const STATUS_ORDER: [i64; 5] = [
    READING_LIST,
    COMPLETE_LIST,
    ON_HOLD_LIST,
    UNFINISHED_LIST,
    WISH_LIST,
];

/// Brand colour `Color.rgb(146, 160, 173)`, `0xAARRGGBB`.
pub const LOGO_COLOR: u32 = 0xFF92_A0AD;

/// Message key of a status label. `None` for an id that has no list.
pub fn label_key(status: i64) -> Option<&'static str> {
    match status {
        READING_LIST => Some("reading_list"),
        WISH_LIST => Some("wish_list"),
        COMPLETE_LIST => Some("complete_list"),
        ON_HOLD_LIST => Some("on_hold_list"),
        UNFINISHED_LIST => Some("unfinished_list"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_32a_010_list_ids_match_the_source() {
        assert_eq!(
            [
                READING_LIST,
                WISH_LIST,
                COMPLETE_LIST,
                UNFINISHED_LIST,
                ON_HOLD_LIST,
                REREADING
            ],
            [0, 1, 2, 3, 4, -1]
        );
    }

    #[test]
    fn t_32a_010_status_order_is_reading_complete_hold_unfinished_wish() {
        assert_eq!(STATUS_ORDER, [0, 2, 4, 3, 1]);
    }

    #[test]
    fn t_32a_010_labels_resolve_to_message_keys_and_unknown_ids_give_none() {
        assert_eq!(label_key(0), Some("reading_list"));
        assert_eq!(label_key(1), Some("wish_list"));
        assert_eq!(label_key(2), Some("complete_list"));
        assert_eq!(label_key(3), Some("unfinished_list"));
        assert_eq!(label_key(4), Some("on_hold_list"));
        assert_eq!(label_key(-1), None);
        assert_eq!(label_key(9), None);
    }

    #[test]
    fn t_32a_010_logo_colour_is_the_source_grey() {
        assert_eq!(LOGO_COLOR, 0xFF92A0AD);
    }
}
