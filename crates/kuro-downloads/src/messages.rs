//! English texts of the messages this crate raises that have no key yet in the i18n tree (T-24a-074). The keys are the
//! ones the translation files need; the i18n lane adds them to `kuro-i18n/locales/en/kurobayomi.ftl`. Until then the
//! English text here is what the GUI shows. The tests keep the texts used by the code and this table in step.

/// `(key, English text)` for each new message. `%d` is the error count in [`FINISHED_WITH_ERRORS`].
pub const KEYS: [(&str, &str); 9] = [
    ("download_episode_already_exists", "Episode already exists"),
    ("download_ffmpeg_not_installed", "ffmpeg is not installed"),
    ("download_torrent_server_unavailable", "Torrent server is not available"),
    ("download_queued_press_continue", "Queued. Press Continue to start."),
    ("download_finished_with_errors", "Finished with %d errors"),
    ("download_close_prompt", "Downloads will pause. Quit?"),
    ("download_looking_for_video", "Looking for video\u{2026}"),
    ("download_location_label", "Download location"),
    ("download_notifier_cache_renewal", "Checking downloads"),
];

/// The text of [`KEYS`] for one key.
pub fn text_of(key: &str) -> Option<&'static str> {
    KEYS.iter().find(|(k, _)| *k == key).map(|(_, text)| *text)
}

/// The summary of a finished run with `errors` failed episodes.
pub fn finished_with_errors(errors: usize) -> String {
    text_of("download_finished_with_errors").unwrap_or("").replace("%d", &errors.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::lifecycle::CLOSE_PROMPT;
    use crate::anime::queue_view::LOOKING_FOR_VIDEO_TEXT;

    #[test]
    fn keys_are_unique_and_the_texts_match_the_constants_in_use() {
        let mut keys: Vec<&str> = KEYS.iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), KEYS.len());
        assert_eq!(text_of("download_close_prompt"), Some(CLOSE_PROMPT));
        assert_eq!(text_of("download_looking_for_video"), Some(LOOKING_FOR_VIDEO_TEXT));
        assert_eq!(text_of("download_episode_already_exists"), Some("Episode already exists"));
        assert_eq!(text_of("download_notifier_cache_renewal"), Some("Checking downloads"));
    }

    #[test]
    fn the_error_count_is_filled_in() {
        assert_eq!(finished_with_errors(3), "Finished with 3 errors");
    }
}
