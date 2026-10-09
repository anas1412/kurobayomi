//! `SeasonDisplayMode` (`aniyomi/domain/anime/SeasonDisplayMode.kt`): the grid layout of a season list,
//! stored in two bits of `season_flags` (bits 16-17).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonDisplayMode {
    CompactGrid = 0,
    ComfortableGrid = 1,
    CoverOnlyGrid = 2,
    List = 3,
}

impl SeasonDisplayMode {
    /// The mode of a stored value. Values outside 0..=3 have no mode.
    pub fn from_i64(v: i64) -> Option<Self> {
        match v {
            0 => Some(Self::CompactGrid),
            1 => Some(Self::ComfortableGrid),
            2 => Some(Self::CoverOnlyGrid),
            3 => Some(Self::List),
            _ => None,
        }
    }

    /// The mode a stored value shows: 0..=3 map to their mode, and any other value is `CompactGrid` (fix Q11).
    pub fn from_stored(v: i64) -> Self {
        Self::from_i64(v).unwrap_or(Self::CompactGrid)
    }

    pub fn to_i64(self) -> i64 {
        self as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_four_values_and_rejects_four() {
        assert_eq!(
            SeasonDisplayMode::from_i64(0),
            Some(SeasonDisplayMode::CompactGrid)
        );
        assert_eq!(
            SeasonDisplayMode::from_i64(1),
            Some(SeasonDisplayMode::ComfortableGrid)
        );
        assert_eq!(
            SeasonDisplayMode::from_i64(2),
            Some(SeasonDisplayMode::CoverOnlyGrid)
        );
        assert_eq!(
            SeasonDisplayMode::from_i64(3),
            Some(SeasonDisplayMode::List)
        );
        assert_eq!(SeasonDisplayMode::from_i64(4), None);
        assert_eq!(SeasonDisplayMode::from_stored(4), SeasonDisplayMode::CompactGrid, "Q11");
        assert_eq!(SeasonDisplayMode::from_stored(-1), SeasonDisplayMode::CompactGrid, "Q11");
        assert_eq!(SeasonDisplayMode::from_stored(3), SeasonDisplayMode::List);
        assert_eq!(SeasonDisplayMode::from_i64(-1), None);
        assert_eq!(SeasonDisplayMode::List.to_i64(), 3);
    }
}
