//! Pause when the audio output goes away (T-45a-082). It replaces `ACTION_AUDIO_BECOMING_NOISY`, a broadcast that only
//! phones send. The app lists the sinks of PipeWire or PulseAudio, and passes the sink that is playing. When that sink
//! is no longer in the list (headphones unplugged), the app pauses mpv and releases keep-awake. The listing itself is
//! the app's job; this module only decides.

/// Whether playback must pause. `playing` is the sink in use. An empty name means the sink is unknown, and nothing is
/// paused for it.
pub fn output_lost(playing: &str, sinks_now: &[String]) -> bool {
    !playing.is_empty() && !sinks_now.iter().any(|sink| sink == playing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sinks(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn unplugging_the_playing_sink_pauses() {
        let now = sinks(&["alsa_output.speakers"]);
        assert!(output_lost("alsa_output.headphones", &now));
    }

    #[test]
    fn the_playing_sink_still_present_does_not_pause() {
        let now = sinks(&["alsa_output.headphones", "alsa_output.speakers"]);
        assert!(!output_lost("alsa_output.headphones", &now));
    }

    #[test]
    fn an_unknown_sink_never_pauses() {
        assert!(!output_lost("", &sinks(&[])));
    }
}
