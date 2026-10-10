//! Keys of the download preferences that hold category id sets. The backup importer uses them to
//! remap category ids (T-02a-017). The strings are the Kotlin key names kept for backup parity.

/// Categories whose new chapters are downloaded.
pub const DOWNLOAD_NEW_CHAPTER_CATEGORIES: &str = "download_new_categories";
/// Categories whose new episodes are downloaded.
pub const DOWNLOAD_NEW_EPISODE_CATEGORIES: &str = "download_new_anime_categories";
/// Categories whose new chapters are never downloaded.
pub const DOWNLOAD_NEW_CHAPTER_CATEGORIES_EXCLUDE: &str = "download_new_categories_exclude";
/// Categories whose new episodes are never downloaded.
pub const DOWNLOAD_NEW_EPISODE_CATEGORIES_EXCLUDE: &str = "download_new_anime_categories_exclude";
/// Categories whose chapters are kept when remove-after-read runs.
pub const REMOVE_EXCLUDE_CHAPTER_CATEGORIES: &str = "remove_exclude_categories";
/// Categories whose episodes are kept when remove-after-watched runs.
pub const REMOVE_EXCLUDE_EPISODE_CATEGORIES: &str = "remove_exclude_anime_categories";

/// The six category-set keys, in registry order.
pub const CATEGORY_SET_KEYS: [&str; 6] = [
    DOWNLOAD_NEW_CHAPTER_CATEGORIES,
    DOWNLOAD_NEW_EPISODE_CATEGORIES,
    DOWNLOAD_NEW_CHAPTER_CATEGORIES_EXCLUDE,
    DOWNLOAD_NEW_EPISODE_CATEGORIES_EXCLUDE,
    REMOVE_EXCLUDE_CHAPTER_CATEGORIES,
    REMOVE_EXCLUDE_EPISODE_CATEGORIES,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Kind, lookup};

    #[test]
    fn each_category_key_is_a_registry_set_with_an_empty_default() {
        for key in CATEGORY_SET_KEYS {
            let (meta, _) = lookup(key).unwrap_or_else(|| panic!("{key} missing from the registry"));
            assert_eq!(meta.kind, Kind::Set, "{key}");
            assert!(meta.default.value() == crate::values::PrefValue::Set(Default::default()), "{key}");
        }
    }
}
