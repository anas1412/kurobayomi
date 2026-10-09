//! `LOCAL_SOURCE_ID` and `is_local_source` (area:14 section 1).

/// Both local sources use id 0 (manga and anime keep separate tables, so the two never collide).
pub use kuro_domain::app::source::LOCAL_SOURCE_ID;

/// Kotlin `isLocal()`: an entry or source whose id is 0 is local. No extension source uses id 0.
pub fn is_local_source(source_id: i64) -> bool {
    source_id == LOCAL_SOURCE_ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_id_is_zero_and_only_zero_is_local() {
        assert_eq!(LOCAL_SOURCE_ID, 0);
        assert!(is_local_source(0));
        assert!(!is_local_source(1));
        assert!(!is_local_source(-1));
        assert!(!is_local_source(i64::MAX));
    }
}
