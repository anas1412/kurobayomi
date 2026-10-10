//! The pure rule of a local anime rescan: which episode rows to add, update and remove. Rows match on the episode
//! url, which is the video file's absolute path. An update only touches the file's metadata, so seen state, bookmarks
//! and the saved position survive a rescan.

use std::collections::{HashMap, HashSet};

use kuro_data::episode::{Episode, EpisodeUpdate};

#[derive(Debug, Default, PartialEq)]
pub struct EpisodePlan {
    pub add: Vec<Episode>,
    pub update: Vec<EpisodeUpdate>,
    pub remove: Vec<i64>,
}

/// Compares the episode rows of one anime with the episodes found on disk. `wanted` carries the anime id already.
pub fn plan_episodes(existing: &[Episode], wanted: Vec<Episode>) -> EpisodePlan {
    let by_url: HashMap<&str, &Episode> = existing.iter().map(|e| (e.url.as_str(), e)).collect();
    let wanted_urls: HashSet<String> = wanted.iter().map(|w| w.url.clone()).collect();
    let mut plan = EpisodePlan::default();
    for want in wanted {
        match by_url.get(want.url.as_str()) {
            Some(have) => plan.update.push(EpisodeUpdate {
                name: Some(want.name),
                episode_number: Some(want.episode_number),
                date_upload: Some(want.date_upload),
                source_order: Some(want.source_order),
                ..EpisodeUpdate::new(have.id)
            }),
            None => plan.add.push(want),
        }
    }
    plan.remove = existing.iter().filter(|e| !wanted_urls.contains(&e.url)).map(|e| e.id).collect();
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(id: i64, url: &str, seen: bool) -> Episode {
        Episode { id, url: url.into(), seen, last_second_seen: 42_000, ..Episode::create() }
    }

    fn found(url: &str, name: &str, number: f64) -> Episode {
        Episode { url: url.into(), name: name.into(), episode_number: number, anime_id: 1, ..Episode::create() }
    }

    #[test]
    fn a_new_file_is_added_and_an_unchanged_one_is_updated_not_recreated() {
        let existing = vec![stored(5, "/show/01.mkv", true)];
        let plan = plan_episodes(&existing, vec![found("/show/01.mkv", "01", 1.0), found("/show/02.mkv", "02", 2.0)]);
        assert_eq!(plan.add.len(), 1);
        assert_eq!(plan.add[0].url, "/show/02.mkv");
        assert_eq!(plan.update.len(), 1);
        assert_eq!(plan.update[0].id, 5);
        assert!(plan.remove.is_empty());
    }

    #[test]
    fn an_update_never_touches_seen_state_or_the_saved_position() {
        let existing = vec![stored(5, "/show/01.mkv", true)];
        let plan = plan_episodes(&existing, vec![found("/show/01.mkv", "renamed", 1.0)]);
        let update = &plan.update[0];
        assert_eq!(update.name.as_deref(), Some("renamed"));
        assert_eq!(update.seen, None);
        assert_eq!(update.last_second_seen, None);
        assert_eq!(update.bookmark, None);
    }

    #[test]
    fn a_file_that_vanished_is_removed() {
        let existing = vec![stored(5, "/show/01.mkv", false), stored(6, "/show/02.mkv", false)];
        let plan = plan_episodes(&existing, vec![found("/show/02.mkv", "02", 2.0)]);
        assert_eq!(plan.remove, vec![5]);
        assert!(plan.add.is_empty());
    }

    #[test]
    fn an_empty_folder_removes_every_row() {
        let existing = vec![stored(5, "/show/01.mkv", true)];
        let plan = plan_episodes(&existing, Vec::new());
        assert_eq!(plan, EpisodePlan { add: vec![], update: vec![], remove: vec![5] });
    }
}
