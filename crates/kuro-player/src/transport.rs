//! Transport commands from the `user-data/aniyomi/*` bridge (area 45, T-45a-019). The bridge
//! forwards them to the view-model handlers; the seek amount itself is applied there.

#[derive(Clone, Debug, PartialEq)]
pub enum Transport {
    Pause,
    Unpause,
    PauseUnpause,
    SeekBy(f64),
    SeekTo(f64),
    /// `seek_by_with_text`: the seconds and the text shown after the seek.
    SeekByWithText(f64, String),
    /// `seek_to_with_text`.
    SeekToWithText(f64, String),
}

/// Parses a bridge property write. `seek_by` and `seek_to` accept `<seconds>`, and the value may
/// carry a `|<text>` tail that is ignored. `seek_by_with_text` and `seek_to_with_text` need
/// `<seconds>|<text>`: the value splits at the first `|` only (`split("|", limit = 2)`), so the text
/// keeps any later `|` (T73). Unknown names, invalid values and a missing text give `None`.
pub fn parse_transport(property: &str, value: &str) -> Option<Transport> {
    match property {
        "user-data/aniyomi/pause" => match value {
            "pause" => Some(Transport::Pause),
            "unpause" => Some(Transport::Unpause),
            "pauseunpause" => Some(Transport::PauseUnpause),
            _ => None,
        },
        "user-data/aniyomi/seek_by" => seconds(value).map(Transport::SeekBy),
        "user-data/aniyomi/seek_to" => seconds(value).map(Transport::SeekTo),
        "user-data/aniyomi/seek_by_with_text" => {
            let (amount, text) = with_text(value)?;
            Some(Transport::SeekByWithText(amount, text))
        }
        "user-data/aniyomi/seek_to_with_text" => {
            let (amount, text) = with_text(value)?;
            Some(Transport::SeekToWithText(amount, text))
        }
        _ => None,
    }
}

/// `data.split("|", limit = 2)`: the seconds, then the rest of the value as the text.
fn with_text(value: &str) -> Option<(f64, String)> {
    let (amount, text) = value.split_once('|')?;
    Some((amount.trim().parse().ok()?, text.to_owned()))
}

fn seconds(value: &str) -> Option<f64> {
    value.split('|').next()?.trim().parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_values_forward() {
        assert_eq!(
            parse_transport("user-data/aniyomi/pause", "pause"),
            Some(Transport::Pause)
        );
        assert_eq!(
            parse_transport("user-data/aniyomi/pause", "unpause"),
            Some(Transport::Unpause)
        );
        assert_eq!(
            parse_transport("user-data/aniyomi/pause", "pauseunpause"),
            Some(Transport::PauseUnpause)
        );
        assert_eq!(parse_transport("user-data/aniyomi/pause", "bogus"), None);
    }

    #[test]
    fn seek_values_are_numeric() {
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_by", "10"),
            Some(Transport::SeekBy(10.0))
        );
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_to", "-2.5"),
            Some(Transport::SeekTo(-2.5))
        );
        assert_eq!(parse_transport("user-data/aniyomi/seek_by", "x"), None);
    }

    #[test]
    fn seek_with_text_keeps_the_text() {
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_by_with_text", "90|Skip intro"),
            Some(Transport::SeekByWithText(90.0, "Skip intro".to_owned()))
        );
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_to_with_text", "5|x"),
            Some(Transport::SeekToWithText(5.0, "x".to_owned()))
        );
    }

    /// T73: the value splits once, so a `|` inside the text stays in the text.
    #[test]
    fn t73_seek_by_text_split_limit_two() {
        assert_eq!(
            parse_transport(
                "user-data/aniyomi/seek_by_with_text",
                "10|Skipped 10 s|really"
            ),
            Some(Transport::SeekByWithText(
                10.0,
                "Skipped 10 s|really".to_owned()
            ))
        );
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_by_with_text", "10|Skipped 10 s"),
            Some(Transport::SeekByWithText(10.0, "Skipped 10 s".to_owned()))
        );
    }

    #[test]
    fn seek_with_text_without_a_pipe_or_a_number_is_dropped() {
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_by_with_text", "10"),
            None
        );
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_to_with_text", "x|y"),
            None
        );
    }

    #[test]
    fn other_properties_are_not_transport() {
        assert_eq!(parse_transport("user-data/aniyomi/toggle_ui", "show"), None);
    }
}
