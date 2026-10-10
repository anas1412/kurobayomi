//! Enhanced tracker auto-match on "new search" (T-38b-034; behaviour 8.1, decision D-6). For an enhanced tracker the
//! "new search" button matches the entry and registers the match without a search page. The banner of a failed match
//! says "No match found" unless the user accepts D-6, which shows the real error text.

/// The text of a match that found nothing, and the default text of a failed one (parity).
pub const NO_MATCH: &str = "No match found";

/// What the auto-match produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Matched<T> {
    /// The match, to register.
    Found(T),
    /// The banner text. `log` is the real error text to log, when there was an error.
    Banner { text: String, log: Option<String> },
}

/// Turns the outcome of `match_entry` into the match or the banner. `show_real_error` is decision D-6 accepted.
pub fn resolve<T>(outcome: Result<Option<T>, String>, show_real_error: bool) -> Matched<T> {
    match outcome {
        Ok(Some(found)) => Matched::Found(found),
        Ok(None) => Matched::Banner { text: NO_MATCH.to_owned(), log: None },
        Err(real) => {
            let text = if show_real_error { real.clone() } else { NO_MATCH.to_owned() };
            Matched::Banner { text, log: Some(real) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_match_shows_the_no_match_text() {
        assert_eq!(resolve::<u8>(Ok(None), false), Matched::Banner { text: "No match found".into(), log: None });
    }

    #[test]
    fn a_thrown_error_is_logged_with_its_real_text_and_the_banner_hides_it_by_default() {
        assert_eq!(
            resolve::<u8>(Err("401 expired".into()), false),
            Matched::Banner { text: "No match found".into(), log: Some("401 expired".into()) },
        );
        assert_eq!(
            resolve::<u8>(Err("401 expired".into()), true),
            Matched::Banner { text: "401 expired".into(), log: Some("401 expired".into()) },
            "D-6 accepted: the real text is shown"
        );
    }

    #[test]
    fn a_found_match_is_registered() {
        assert_eq!(resolve(Ok(Some(7u8)), false), Matched::Found(7));
    }
}
