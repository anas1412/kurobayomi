//! One remote entry as a source listing returns it.

/// What a listing needs of one remote entry.
pub struct NetworkEntry {
    pub url: String,
    pub title: String,
    pub cover: Option<String>,
}
