//! Keys of the download preferences that hold category id sets. The backup importer uses them to
//! remap category ids (T-02a-017). The strings come from `category_keys`.

use crate::category::category_keys;
use kuro_platform::kind::Kind;

/// Categories whose new chapters are downloaded.
pub const DOWNLOAD_NEW_CHAPTER_CATEGORIES: &str = category_keys(Kind::Manga).download_new;
/// Categories whose new episodes are downloaded.
pub const DOWNLOAD_NEW_EPISODE_CATEGORIES: &str = category_keys(Kind::Anime).download_new;
/// Categories whose new chapters are never downloaded.
pub const DOWNLOAD_NEW_CHAPTER_CATEGORIES_EXCLUDE: &str = category_keys(Kind::Manga).download_new_exclude;
/// Categories whose new episodes are never downloaded.
pub const DOWNLOAD_NEW_EPISODE_CATEGORIES_EXCLUDE: &str = category_keys(Kind::Anime).download_new_exclude;
/// Categories whose chapters are kept when remove-after-read runs.
pub const REMOVE_EXCLUDE_CHAPTER_CATEGORIES: &str = category_keys(Kind::Manga).remove_exclude;
/// Categories whose episodes are kept when remove-after-watched runs.
pub const REMOVE_EXCLUDE_EPISODE_CATEGORIES: &str = category_keys(Kind::Anime).remove_exclude;

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
    use crate::registry::{is_dropped, lookup};

    #[test]
    fn each_category_key_is_a_dropped_key_and_not_in_the_registry() {
        for key in CATEGORY_SET_KEYS {
            assert!(is_dropped(key), "{key} is a cut setting");
            assert!(lookup(key).is_none(), "{key} is not registered");
        }
    }
}
