//! Errors of the tracking core.

use kuro_platform::secrets::SecretError;

/// Failure of a tracking operation. `Message` carries the tracker's own text, shown verbatim.
#[derive(Debug, thiserror::Error)]
pub enum TrackError {
    #[error("{0}")]
    Message(String),
    #[error("score not in list: '{0}'")]
    UnknownScore(String),
    #[error("track has no row id")]
    MissingId,
    #[error("unknown tracker {0}")]
    UnknownTracker(i64),
    #[error("no binding for tracker {0}")]
    MissingRow(i64),
    /// No secret service is reachable. Nothing is stored (`p2-behavior`, T-27b-027).
    #[error("Secret storage is unavailable")]
    SecretsUnavailable,
    #[error(transparent)]
    Secret(SecretError),
    #[error(transparent)]
    Prefs(#[from] kuro_prefs::PrefError),
    #[error(transparent)]
    Db(#[from] kuro_db::DbError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// A failure with its own message key (AniList errors, T-28a-045). Shown in the user's language.
    #[error("{}", crate::messages::english(.0))]
    Key(crate::messages::MsgKey),
}

impl From<SecretError> for TrackError {
    fn from(e: SecretError) -> Self {
        match e {
            SecretError::Unavailable(_) => TrackError::SecretsUnavailable,
            failed => TrackError::Secret(failed),
        }
    }
}

impl TrackError {
    /// The toast for this failure: "<tracker> error: <message>" (B§3.3 step 2). An empty message keeps the colon.
    /// True for failures a retry cannot fix: the entry is not on the list, or the login expired (T-28a-042). Such a
    /// progress update is reported, not put in the retry queue.
    pub fn is_retry_futile(&self) -> bool {
        matches!(
            self,
            TrackError::Key(crate::messages::MsgKey::AniListNotInList)
                | TrackError::Key(crate::messages::MsgKey::AniListSessionExpired)
        )
    }

    pub fn toast(&self, tracker: &str) -> crate::messages::MsgKey {
        if let TrackError::Key(key) = self {
            return key.clone();
        }
        crate::messages::MsgKey::TrackerError {
            tracker: tracker.to_owned(),
            detail: self.to_string(),
        }
    }
}

const REDACTED: &str = "[redacted]";

/// Parameter and field names whose values never reach the log (T-27a-067).
const SECRET_KEYS: &[&str] = &[
    "access_token",
    "refresh_token",
    "token",
    "password",
    "secret",
    "code",
    "state",
];

/// Log-safe copy of `text`. Every query string is replaced with `[redacted]`, and the value of each secret
/// key (`token=`, `"password":`, and so on) is replaced too. Used for every error text that reaches a log.
pub fn redact(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '?' && chars.get(i + 1).is_some_and(|c| !c.is_whitespace()) {
            out.push('?');
            out.push_str(REDACTED);
            i = skip_until(&chars, i + 1, |c| {
                c.is_whitespace() || matches!(c, '"' | '\'' | '#' | ')')
            });
            continue;
        }
        if let Some(key_len) = secret_key_at(&lower, i) {
            let mut j = i + key_len;
            if chars.get(j) == Some(&'"') {
                j += 1;
            }
            while chars.get(j) == Some(&' ') {
                j += 1;
            }
            if matches!(chars.get(j), Some('=' | ':')) {
                j += 1;
                while matches!(chars.get(j), Some(' ' | '"')) {
                    j += 1;
                }
                out.extend(&chars[i..j]);
                out.push_str(REDACTED);
                i = skip_until(&chars, j, |c| {
                    matches!(
                        c,
                        '&' | ' ' | '"' | ',' | '}' | '#' | '\'' | ')' | '\n' | '\t'
                    )
                });
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn skip_until(chars: &[char], mut j: usize, stop: impl Fn(char) -> bool) -> usize {
    while j < chars.len() && !stop(chars[j]) {
        j += 1;
    }
    j
}

/// Length of the secret key that starts at `i`, when it is a whole word.
fn secret_key_at(lower: &[char], i: usize) -> Option<usize> {
    if i > 0 && (lower[i - 1].is_alphanumeric() || lower[i - 1] == '_') {
        return None;
    }
    SECRET_KEYS.iter().find_map(|key| {
        let len = key.len();
        let matches =
            lower.len() >= i + len && key.chars().zip(&lower[i..i + len]).all(|(a, b)| a == *b);
        matches.then_some(len)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_query_strings_and_secret_values_are_removed() {
        let text =
            "https://x.test/cb?code=SECRET1&state=STATE2#access_token=TOKEN3&token_type=bearer";
        let out = redact(text);
        for secret in ["SECRET1", "STATE2", "TOKEN3"] {
            assert!(!out.contains(secret), "{out}");
        }
        assert!(out.starts_with("https://x.test/cb?[redacted]"), "{out}");
    }

    #[test]
    fn json_and_plain_secrets_are_masked_and_prose_is_kept() {
        assert_eq!(
            redact(r#"{"password":"hunter2","user":"ann"}"#),
            r#"{"password":"[redacted]","user":"ann"}"#
        );
        assert_eq!(redact("password=pw9 failed"), "password=[redacted] failed");
        assert_eq!(redact("Login failed"), "Login failed");
        assert_eq!(redact("Are you sure? yes"), "Are you sure? yes");
        assert_eq!(
            redact("barcode=12"),
            "barcode=12",
            "keys must be whole words"
        );
    }
}
