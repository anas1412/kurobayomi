//! The base list row of the manga and anime migration screens (T-38a-044, T-39a-061). Upstream: `BaseMangaListItem.kt` and
//! `BaseAnimeListItem.kt`: a row of 76 px with 16 px side and 3 px vertical padding, and three slots (cover, content,
//! actions). Pure: the row widget reads these values. A click on the cover runs the cover action, which defaults to the
//! row action.

/// The fixed height of a row.
pub const ROW_HEIGHT: i32 = 76;

/// The horizontal padding on each side of a row.
pub const PAD_HORIZONTAL: i32 = 16;

/// The vertical padding above and below the content of a row.
pub const PAD_VERTICAL: i32 = 3;

/// The three slots of a row, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Cover,
    Content,
    Actions,
}

/// The slots in the order they are laid out.
pub const SLOTS: [Slot; 3] = [Slot::Cover, Slot::Content, Slot::Actions];

/// The action a click on the cover runs: the cover's own action when it has one, otherwise the row action.
pub fn cover_action<A: Copy>(row: A, cover: Option<A>) -> A {
    cover.unwrap_or(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_geometry_is_the_upstream_one() {
        assert_eq!((ROW_HEIGHT, PAD_HORIZONTAL, PAD_VERTICAL), (76, 16, 3));
        assert_eq!(SLOTS, [Slot::Cover, Slot::Content, Slot::Actions]);
    }

    #[test]
    fn a_cover_click_defaults_to_the_row_action() {
        assert_eq!(cover_action("open", None), "open");
        assert_eq!(cover_action("open", Some("cover")), "cover");
    }
}
