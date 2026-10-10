//! The anime page overflow menu (T-39a-075, T-39a-057). Pure: which items show and in what order. Upstream:
//! `EntryToolbar.kt` (the order: change intro length, Refresh, edit categories, Migrate, Share, Settings) and
//! `AnimeScreen.kt` (the conditions: a favorite for the category, Migrate and intro length; episodes for the intro
//! length; an HTTP source for Share; a configurable source for Settings).

/// The overflow entries, in the order they are shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    ChangeIntro,
    Refresh,
    EditCategories,
    Migrate,
    Share,
    Settings,
}

/// What the menu reads from the entry and its source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub favorite: bool,
    /// The entry lists episodes, not seasons.
    pub episodes: bool,
    pub http_source: bool,
    pub configurable_source: bool,
}

/// The items the menu shows for `entry`. Refresh is always there.
pub fn items(entry: Entry) -> Vec<Item> {
    let mut items = Vec::new();
    if entry.favorite && entry.episodes {
        items.push(Item::ChangeIntro);
    }
    items.push(Item::Refresh);
    if entry.favorite {
        items.push(Item::EditCategories);
        items.push(Item::Migrate);
    }
    if entry.http_source {
        items.push(Item::Share);
    }
    if entry.configurable_source {
        items.push(Item::Settings);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(favorite: bool, episodes: bool, http_source: bool, configurable_source: bool) -> Entry {
        Entry { favorite, episodes, http_source, configurable_source }
    }

    #[test]
    fn a_plain_entry_offers_refresh_only() {
        assert_eq!(items(entry(false, true, false, false)), vec![Item::Refresh]);
    }

    #[test]
    fn a_favorite_episode_entry_offers_the_library_items_in_android_order() {
        assert_eq!(
            items(entry(true, true, true, true)),
            vec![Item::ChangeIntro, Item::Refresh, Item::EditCategories, Item::Migrate, Item::Share, Item::Settings],
        );
    }

    #[test]
    fn a_season_entry_never_offers_the_intro_length() {
        assert_eq!(items(entry(true, false, false, false)), vec![Item::Refresh, Item::EditCategories, Item::Migrate]);
    }

    #[test]
    fn settings_shows_only_for_a_configurable_source_and_share_only_for_http() {
        assert_eq!(items(entry(false, true, false, true)), vec![Item::Refresh, Item::Settings]);
        assert_eq!(items(entry(false, true, true, false)), vec![Item::Refresh, Item::Share]);
    }
}
