//! The downloaded-episode filter of the player playlist (T-39b-060; `T-EPS-DL-1`, E-DL-10). A local anime keeps every
//! episode. Any other anime keeps only the downloaded ones.

/// The episodes the playlist shows. `is_downloaded` is asked only for a non-local anime.
pub fn playlist<T>(episodes: Vec<T>, local: bool, is_downloaded: impl Fn(&T) -> bool) -> Vec<T> {
    if local {
        episodes
    } else {
        episodes.into_iter().filter(|e| is_downloaded(e)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_anime_keeps_every_episode() {
        assert_eq!(playlist(vec![1, 2, 3], true, |e| *e == 2), vec![1, 2, 3]);
    }

    #[test]
    fn any_other_anime_keeps_only_the_downloaded_episodes() {
        assert_eq!(playlist(vec![1, 2, 3], false, |e| *e != 2), vec![1, 3]);
        assert!(playlist(vec![1, 2], false, |_| false).is_empty());
    }
}
