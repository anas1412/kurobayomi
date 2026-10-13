//! Pieces of the list options the entry page shares: the masked write a pick sends and the scanlator filter rule.

/// A masked write: `new = (old & !mask) | (flag & mask)`.
pub type Write = (i64, i64);

/// `scanlatorFilterActive`: some excluded scanlator is also one the chapters have (E-43). A stale excluded name, one
/// no chapter carries, does not tint the filter row.
pub fn scanlator_filter_active(available: &[String], excluded: &std::collections::BTreeSet<String>) -> bool {
    available.iter().any(|name| excluded.contains(name))
}
