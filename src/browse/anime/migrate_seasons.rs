//! The season picker's pure rules (`MigrateSeasonSelectScreenModel.kt`; T-41a-040): the grid columns from the saved
//! preference, and which seasons are listed when favourites are hidden. The page and the season fetch belong to the view
//! and the caller.

/// The adaptive column width in pixels (`GridCells.Adaptive(128.dp)`).
pub const ADAPTIVE_MIN_PX: u32 = 128;

/// How the season grid lays out its columns (edge E38).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Columns {
    Adaptive { min_px: u32 },
    Fixed(u32),
}

/// The column rule of one orientation's `pref_animelib_columns_*` value: 0 is adaptive, any positive count is fixed.
/// Negative values never get stored, so they fall back to adaptive.
pub fn columns(pref: i64) -> Columns {
    match u32::try_from(pref) {
        Ok(count) if count > 0 => Columns::Fixed(count),
        _ => Columns::Adaptive { min_px: ADAPTIVE_MIN_PX },
    }
}

/// The seasons to list, each with whether it is already a favourite. With `hide_in_library` set (the
/// `browse_hide_in_anime_library_items` preference), favourites are left out (edge E14).
pub fn listed<T>(seasons: Vec<(T, bool)>, hide_in_library: bool) -> Vec<T> {
    seasons.into_iter().filter(|(_, favourite)| !hide_in_library || !favourite).map(|(season, _)| season).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_is_adaptive_at_128_and_a_positive_count_is_fixed() {
        assert_eq!(columns(0), Columns::Adaptive { min_px: 128 }, "T47: 0 is adaptive");
        assert_eq!(columns(3), Columns::Fixed(3), "T47: n is fixed n");
        assert_eq!(columns(-2), Columns::Adaptive { min_px: 128 }, "a negative value is never stored; it falls back");
    }

    #[test]
    fn favourites_are_left_out_only_when_the_preference_hides_them() {
        let seasons = vec![("s1", false), ("s2", true), ("s3", false)];
        assert_eq!(listed(seasons.clone(), false), ["s1", "s2", "s3"], "E14: hidden is off by default");
        assert_eq!(listed(seasons, true), ["s1", "s3"], "E14: a favourite season is not listed");
    }
}
