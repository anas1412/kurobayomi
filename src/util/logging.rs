//! Launch and notice logging targets and the verbose switch (T-53a-014).
//!
//! Two `tracing` targets carry what the shell does with a launch request and with a notice action. The verbose preference
//! (`verbose_logging`) turns them up; with it off, only warnings and the action name are logged, never the payload.

/// Launch requests: argv, deep links, desktop actions.
pub const LAUNCH_TARGET: &str = "kurobayomi::launch";
/// Notice actions sent through `app.notice`.
pub const NOTICE_TARGET: &str = "kurobayomi::notice";

/// The filter directives for the two targets, to add to the subscriber's filter. They follow
/// `kuro_core::logging::level_for`: everything with verbose on, warnings only with it off.
pub fn directives(verbose: bool) -> String {
    let level = if verbose { "trace" } else { "warn" };
    format!("{LAUNCH_TARGET}={level},{NOTICE_TARGET}={level}")
}

/// What to log for an action: the name alone with verbose off; with verbose on the name and the payload, with secrets in
/// URL query and fragment values replaced by `<redacted>` (an OAuth return carries a token).
pub fn describe(verbose: bool, action: &str, payload: &str) -> String {
    if verbose && !payload.is_empty() { format!("{action}: {}", redact(payload)) } else { action.to_owned() }
}

/// Replaces the value of every `key=value` pair after `?`, `&` or `#` whose key looks secret.
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find(['?', '&', '#']) {
        let (head, tail) = rest.split_at(pos + 1);
        out.push_str(head);
        let end = tail.find(['&', '#', ' ', '"']).unwrap_or(tail.len());
        let (pair, after) = tail.split_at(end);
        match pair.split_once('=') {
            Some((key, _)) if is_secret(key) => {
                out.push_str(key);
                out.push_str("=<redacted>");
            }
            _ => out.push_str(pair),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn is_secret(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["token", "code", "secret", "password", "state", "key"].iter().any(|word| key.contains(word))
}

/// Logs a launch request on the launch target.
pub fn log_launch(verbose: bool, action: &str, payload: &str) {
    tracing::debug!(target: "kurobayomi::launch", "{}", describe(verbose, action, payload));
}

/// Logs a notice action on the notice target.
pub fn log_notice(verbose: bool, action: &str, payload: &str) {
    tracing::debug!(target: "kurobayomi::notice", "{}", describe(verbose, action, payload));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbose_turns_both_targets_up_and_off_keeps_warnings() {
        assert_eq!(directives(true), "kurobayomi::launch=trace,kurobayomi::notice=trace");
        assert_eq!(directives(false), "kurobayomi::launch=warn,kurobayomi::notice=warn");
    }

    #[test]
    fn the_directives_are_accepted_by_a_tracing_filter_grammar() {
        // each directive is `target=level` with a known level
        for verbose in [true, false] {
            for part in directives(verbose).split(',') {
                let (target, level) = part.split_once('=').unwrap();
                assert!(target.starts_with("kurobayomi::"));
                assert!(["trace", "warn"].contains(&level));
            }
        }
    }

    #[test]
    fn with_verbose_off_only_the_action_name_is_logged() {
        assert_eq!(describe(false, "open_url", "https://example.test/a?token=abc"), "open_url");
    }

    #[test]
    fn with_verbose_on_the_payload_follows_the_name() {
        assert_eq!(describe(true, "open_manga", "{\"manga_id\":4}"), "open_manga: {\"manga_id\":4}");
        assert_eq!(describe(true, "dismiss", ""), "dismiss");
    }

    #[test]
    fn secret_values_in_urls_are_redacted_even_with_verbose_on() {
        let text = describe(true, "oauth", "kurobayomi://anilist-auth#access_token=SECRET&token_type=Bearer&expires_in=1");
        assert!(!text.contains("SECRET"));
        assert!(text.contains("access_token=<redacted>"));
        assert!(text.contains("expires_in=1"));
        let query = describe(true, "oauth", "https://x.test/cb?code=abc123&other=1");
        assert_eq!(query, "oauth: https://x.test/cb?code=<redacted>&other=1");
    }
}
