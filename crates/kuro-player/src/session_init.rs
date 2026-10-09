//! Session init and episode resolution (area 45 p2 T-45b-003; behaviour 2.1 to 2.3, 2.5). Pure: the
//! caller loads the anime, its episodes, the categories and the hosters, and writes the returned mpv
//! user-data. The order of behaviour 2.2 is kept, and each failure carries its message key.

use kuro_data::anime::Anime;
use kuro_data::episode::Episode;

use crate::errors::{InitError, PlayerMessage};
use kuro_domain::entries::anime::AnimeExt;

use crate::playlist;
use crate::types::Hoster;

/// `needsInit(animeId, episodeId)` (behaviour 2.1). A missing current anime or episode always needs
/// init, because `null != id` is true in Kotlin.
pub fn needs_init(
    current_anime: Option<i64>,
    current_episode: Option<i64>,
    anime_id: i64,
    episode_id: i64,
) -> bool {
    current_anime != Some(anime_id) || current_episode != Some(episode_id)
}

/// The hoster list the init starts from: a saved list from the activity's intent, or a list the
/// caller fetched for the episode (behaviour 2.2 step 3e).
#[derive(Clone, Debug, PartialEq)]
pub enum HostersInput {
    Saved(Vec<Hoster>),
    Fetched(Vec<Hoster>),
}

/// The mpv user-data the init writes for Lua scripts (behaviour 2.2, D§7.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserData {
    pub anime_title: String,
    pub intro_length: i64,
    /// Category names joined with `", "`.
    pub category: String,
}

/// What a successful init resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct InitPlan {
    pub playlist: Vec<Episode>,
    pub episode: Episode,
    pub has_previous: bool,
    pub has_next: bool,
    pub hosters: Vec<Hoster>,
    /// The saved `(hoster, video)` index pair, when the hoster list was saved.
    pub quality_index: Option<(usize, usize)>,
    pub user_data: UserData,
}

/// The result of `init`: the fast path, a missing anime (E-02), or a full plan / an error.
#[derive(Clone, Debug, PartialEq)]
pub enum InitOutcome {
    /// The anime and episode are already loaded (E-01): nothing to do.
    Unchanged,
    /// The anime was not found. The caller keeps the default state.
    AnimeMissing,
    Loaded(Box<InitPlan>),
    Failed(InitError),
}

/// The inputs of one init.
pub struct InitInput<'a> {
    pub anime: Option<&'a Anime>,
    pub episodes: &'a [Episode],
    pub episode_id: i64,
    pub downloaded_only: bool,
    pub is_downloaded: &'a dyn Fn(&Episode) -> bool,
    pub categories: &'a [String],
    pub intro_length: i64,
    pub hosters: HostersInput,
    pub hoster_index: Option<usize>,
    pub video_index: Option<usize>,
}

fn keyed(message: PlayerMessage) -> InitError {
    InitError::Localised(message)
}

/// The `init` of behaviour 2.2 for a fresh (anime, episode) pair. `current` is the pair already
/// loaded, and an equal pair returns `Unchanged` before anything else runs.
pub fn init(
    current: (Option<i64>, Option<i64>),
    input: &InitInput<'_>,
    anime_id: i64,
) -> InitOutcome {
    if !needs_init(current.0, current.1, anime_id, input.episode_id) {
        return InitOutcome::Unchanged;
    }
    let Some(anime) = input.anime else {
        return InitOutcome::AnimeMissing;
    };
    let built = match playlist::build(
        input.episodes,
        anime.sorting(),
        input.downloaded_only,
        input.is_downloaded,
    ) {
        Ok(built) => built,
        Err(_) => {
            return InitOutcome::Failed(InitError::Other(Some(
                "Invalid episode sorting method".to_owned(),
            )));
        }
    };
    let Some(playlist) = playlist::filter(
        &built,
        anime.episode_flags,
        input.episode_id,
        input.is_downloaded,
    ) else {
        return InitOutcome::Failed(InitError::Other(Some(format!(
            "Requested episode of id {} not found in episode list",
            input.episode_id
        ))));
    };
    let Some(episode) = playlist.iter().find(|e| e.id == input.episode_id).cloned() else {
        return InitOutcome::Failed(InitError::Other(None));
    };
    let (has_previous, has_next) = playlist::neighbours(&playlist, input.episode_id);
    let (hosters, quality_index) = match input.hosters.clone() {
        HostersInput::Saved(list) => {
            if list.is_empty() {
                return InitOutcome::Failed(keyed(PlayerMessage::SelectHosterFromEmptyList));
            }
            let index = input.hoster_index.zip(input.video_index);
            (list, index)
        }
        HostersInput::Fetched(list) => {
            if list.is_empty() {
                return InitOutcome::Failed(keyed(PlayerMessage::NoHosters));
            }
            (list, None)
        }
    };
    InitOutcome::Loaded(Box::new(InitPlan {
        playlist,
        episode,
        has_previous,
        has_next,
        hosters,
        quality_index,
        user_data: UserData {
            anime_title: anime.title.clone(),
            intro_length: input.intro_length,
            category: input.categories.join(", "),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_domain::entries::anime::episode_flags;

    fn episode(id: i64, number: f64) -> Episode {
        Episode {
            id,
            episode_number: number,
            ..Episode::create()
        }
    }

    fn anime(id: i64) -> Anime {
        Anime {
            id,
            title: "Show".to_owned(),
            episode_flags: episode_flags::SORTING_NUMBER,
            ..Anime::create()
        }
    }

    fn none(_: &Episode) -> bool {
        false
    }

    fn input<'a>(
        anime: Option<&'a Anime>,
        episodes: &'a [Episode],
        hosters: HostersInput,
    ) -> InitInput<'a> {
        InitInput {
            anime,
            episodes,
            episode_id: 2,
            downloaded_only: false,
            is_downloaded: &none,
            categories: &[],
            intro_length: 0,
            hosters,
            hoster_index: None,
            video_index: None,
        }
    }

    fn one_hoster() -> Vec<Hoster> {
        vec![Hoster::from_videos(Vec::new())]
    }

    #[test]
    fn needs_init_is_true_for_a_missing_current_pair() {
        assert!(needs_init(None, None, 1, 2));
        assert!(!needs_init(Some(1), Some(2), 1, 2));
        assert!(needs_init(Some(1), Some(3), 1, 2));
    }

    #[test]
    fn e01_the_same_pair_is_the_fast_path() {
        let eps = vec![episode(1, 1.0), episode(2, 2.0)];
        let a = anime(1);
        let out = init(
            (Some(1), Some(2)),
            &input(Some(&a), &eps, HostersInput::Fetched(one_hoster())),
            1,
        );
        assert_eq!(out, InitOutcome::Unchanged);
    }

    #[test]
    fn e02_a_missing_anime_gives_anime_missing() {
        let eps = vec![episode(2, 2.0)];
        let out = init(
            (None, None),
            &input(None, &eps, HostersInput::Fetched(one_hoster())),
            1,
        );
        assert_eq!(out, InitOutcome::AnimeMissing);
    }

    #[test]
    fn a_fetched_empty_hoster_list_is_no_hosters() {
        let eps = vec![episode(2, 2.0)];
        let a = anime(1);
        let out = init(
            (None, None),
            &input(Some(&a), &eps, HostersInput::Fetched(Vec::new())),
            1,
        );
        assert_eq!(
            out,
            InitOutcome::Failed(InitError::Localised(PlayerMessage::NoHosters))
        );
    }

    #[test]
    fn a_saved_empty_hoster_list_is_select_hoster_from_empty_list() {
        let eps = vec![episode(2, 2.0)];
        let a = anime(1);
        let out = init(
            (None, None),
            &input(Some(&a), &eps, HostersInput::Saved(Vec::new())),
            1,
        );
        assert_eq!(
            out,
            InitOutcome::Failed(InitError::Localised(
                PlayerMessage::SelectHosterFromEmptyList
            ))
        );
    }

    #[test]
    fn a_full_init_resolves_the_episode_neighbours_and_user_data() {
        let eps = vec![episode(1, 1.0), episode(2, 2.0), episode(3, 3.0)];
        let a = anime(1);
        let categories = vec!["Action".to_owned(), "Drama".to_owned()];
        let mut i = input(Some(&a), &eps, HostersInput::Fetched(one_hoster()));
        i.categories = &categories;
        i.intro_length = 90;
        let InitOutcome::Loaded(plan) = init((None, None), &i, 1) else {
            panic!("init must load");
        };
        assert_eq!(plan.episode.id, 2);
        assert!(plan.has_previous);
        assert!(plan.has_next);
        assert_eq!(plan.playlist.len(), 3);
        assert_eq!(plan.quality_index, None);
        assert_eq!(
            plan.user_data,
            UserData {
                anime_title: "Show".to_owned(),
                intro_length: 90,
                category: "Action, Drama".to_owned(),
            }
        );
    }

    #[test]
    fn a_saved_hoster_list_keeps_the_saved_quality_index() {
        let eps = vec![episode(2, 2.0)];
        let a = anime(1);
        let mut i = input(Some(&a), &eps, HostersInput::Saved(one_hoster()));
        i.hoster_index = Some(0);
        i.video_index = Some(0);
        let InitOutcome::Loaded(plan) = init((None, None), &i, 1) else {
            panic!("init must load");
        };
        assert_eq!(plan.quality_index, Some((0, 0)));
        assert!(!plan.has_previous);
        assert!(!plan.has_next);
    }
}
