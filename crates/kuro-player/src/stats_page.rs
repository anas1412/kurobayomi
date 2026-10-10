//! Statistics page chips (area 46 `MoreSheet.kt`, T-46b-009, T-46b-019). Pure: the sheet sends the returned
//! toggle and page commands to mpv, then stores the page.
//!
//! Source of truth (T-46b-042, E72): the stored `pref_player_statistics_page` decides which chip is selected. Toggling
//! the overlay from the keyboard does not update the stored page, so the chip can show the stored page while the
//! overlay shows another one. This is documented, and mpv is not queried when the sheet opens.

/// The six chips of the statistics page, in order: off, then pages 1 to 5 (T-46b-019).
pub const CHIP_PAGES: [i64; 6] = [0, 1, 2, 3, 4, 5];

/// The label of the chip for `page`: `Off` for 0 and `Page N` for 1 to 5. `None` for any other page.
pub fn chip_label(page: i64) -> Option<String> {
    match page {
        0 => Some("Off".to_owned()),
        1..=5 => Some(format!("Page {page}")),
        _ => None,
    }
}

/// What clicking the chip for `page` does, given the page that is currently shown. Both are 0 for
/// off and 1 to 5 for the pages. Returns `(toggle, page_command)`:
/// - `toggle` is the `stats/display-stats-toggle` binding. It runs when turning stats on or off,
///   that is, when exactly one side is 0 (`(page == 0) xor (current == 0)`).
/// - `page_command` is `Some(page)` for `stats/display-page-N`, and `None` for "Off".
pub fn stats_actions(current: i64, page: i64) -> (bool, Option<i64>) {
    ((page == 0) ^ (current == 0), (page != 0).then_some(page))
}

/// The commands run after init when `pref_player_statistics_page` is not 0 (T-45a-053): the
/// `stats/display-stats-toggle` binding, then `stats/display-page-<n>`. Empty for 0 (off).
pub fn startup_commands(page_pref: i64) -> Vec<Vec<String>> {
    if page_pref == 0 {
        return Vec::new();
    }
    vec![
        vec![
            "script-binding".to_owned(),
            "stats/display-stats-toggle".to_owned(),
        ],
        vec![
            "script-binding".to_owned(),
            format!("stats/display-page-{page_pref}"),
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_runs_the_toggle_and_the_page_unless_off() {
        assert!(startup_commands(0).is_empty());
        assert_eq!(
            startup_commands(2),
            vec![
                vec![
                    "script-binding".to_owned(),
                    "stats/display-stats-toggle".to_owned()
                ],
                vec![
                    "script-binding".to_owned(),
                    "stats/display-page-2".to_owned()
                ],
            ]
        );
    }

    #[test]
    fn t_46b_019_the_six_chips_are_off_then_pages_one_to_five() {
        let labels: Vec<String> = CHIP_PAGES.iter().map(|p| chip_label(*p).unwrap()).collect();
        assert_eq!(
            labels,
            ["Off", "Page 1", "Page 2", "Page 3", "Page 4", "Page 5"]
        );
        assert_eq!(chip_label(6), None);
        assert_eq!(chip_label(-1), None);
    }

    #[test]
    fn off_to_off_does_nothing() {
        assert_eq!(stats_actions(0, 0), (false, None));
    }

    #[test]
    fn off_to_a_page_turns_stats_on_and_shows_the_page() {
        assert_eq!(stats_actions(0, 3), (true, Some(3)));
    }

    #[test]
    fn a_page_to_off_turns_stats_off() {
        assert_eq!(stats_actions(3, 0), (true, None));
    }

    #[test]
    fn a_page_to_another_page_only_switches_the_page() {
        assert_eq!(stats_actions(2, 3), (false, Some(3)));
    }

    #[test]
    fn the_same_page_again_only_resends_it() {
        assert_eq!(stats_actions(3, 3), (false, Some(3)));
    }
}
