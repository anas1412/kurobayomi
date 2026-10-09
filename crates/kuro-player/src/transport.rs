//! Transport commands from the `user-data/aniyomi/*` bridge (area 45, T-45a-019). The bridge
//! forwards them to the view-model handlers; the seek amount itself is applied there.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Transport {
    Pause,
    Unpause,
    PauseUnpause,
    SeekBy(f64),
    SeekTo(f64),
}

/// Parses a bridge property write. `seek_by` and `seek_to` accept `<seconds>` or
/// `<seconds>|<text>`; the text part is ignored here. Unknown names or invalid values give `None`.
pub fn parse_transport(property: &str, value: &str) -> Option<Transport> {
    match property {
        "user-data/aniyomi/pause" => match value {
            "pause" => Some(Transport::Pause),
            "unpause" => Some(Transport::Unpause),
            "pauseunpause" => Some(Transport::PauseUnpause),
            _ => None,
        },
        "user-data/aniyomi/seek_by" | "user-data/aniyomi/seek_by_with_text" => {
            seconds(value).map(Transport::SeekBy)
        }
        "user-data/aniyomi/seek_to" | "user-data/aniyomi/seek_to_with_text" => {
            seconds(value).map(Transport::SeekTo)
        }
        _ => None,
    }
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
    fn seek_with_text_uses_the_value_before_the_pipe() {
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_by_with_text", "90|Skip intro"),
            Some(Transport::SeekBy(90.0))
        );
        assert_eq!(
            parse_transport("user-data/aniyomi/seek_to_with_text", "5|x"),
            Some(Transport::SeekTo(5.0))
        );
    }

    #[test]
    fn other_properties_are_not_transport() {
        assert_eq!(parse_transport("user-data/aniyomi/toggle_ui", "show"), None);
    }
}
