//! Statistics page chips (area 46 `MoreSheet.kt`, T-46b-009). Pure: the sheet sends the returned
//! toggle and page commands to mpv, then stores the page.

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
