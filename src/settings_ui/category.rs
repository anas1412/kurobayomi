//! Include and exclude summary of category sets (task T-47a-005).
// Not reachable from `route.rs` yet: the include/exclude summary is for the update-category rows, which are still spec rows.

/// A library category as the settings need it: its stored id, display name and sort order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i32,
    pub name: String,
    pub order: i64,
}
