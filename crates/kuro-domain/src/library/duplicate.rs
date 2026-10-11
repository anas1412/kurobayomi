//! Duplicate-marking choices (T-08a-029). The stored values are in `kuro-prefs`; this file adds the settings labels.

pub use kuro_prefs::library::{
    MARK_DUPLICATE_CHAPTER_READ_EXISTING, MARK_DUPLICATE_CHAPTER_READ_NEW, MARK_DUPLICATE_EPISODE_SEEN_EXISTING,
    MARK_DUPLICATE_EPISODE_SEEN_NEW,
};

/// The four duplicate-marking choices, as stored value and settings label. Chapters and episodes each have a
/// `existing` and a `new` choice.
pub const DUPLICATE_CHOICES: [(&str, &str); 4] = [
    (MARK_DUPLICATE_CHAPTER_READ_EXISTING, "After reading a chapter"),
    (MARK_DUPLICATE_CHAPTER_READ_NEW, "After fetching new chapter"),
    (MARK_DUPLICATE_EPISODE_SEEN_EXISTING, "After watching an episode"),
    (MARK_DUPLICATE_EPISODE_SEEN_NEW, "After fetching new episode"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_choices_keep_their_stored_values_and_labels() {
        assert_eq!(
            DUPLICATE_CHOICES.map(|(value, _)| value),
            ["existing", "new", "existing_episode", "new_episode"]
        );
        assert_eq!(DUPLICATE_CHOICES[3].1, "After fetching new episode");
    }
}
