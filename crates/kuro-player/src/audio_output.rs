//! Pause when the audio output goes away (T-45a-082). It replaces `ACTION_AUDIO_BECOMING_NOISY`, a broadcast that only
//! phones send. The app lists the sinks of PipeWire or PulseAudio, and passes the sink that is playing. When that sink
//! is no longer in the list (headphones unplugged), the app pauses mpv and releases keep-awake. The listing itself is
//! the app's job; this module only decides.

/// Whether playback must pause. `playing` is the sink in use. An empty name means the sink is unknown, and nothing is
/// paused for it.
pub fn output_lost(playing: &str, sinks_now: &[String]) -> bool {
    !playing.is_empty() && !sinks_now.iter().any(|sink| sink == playing)
}

/// The sink names of `pactl list short sinks` (one per line: index, name, driver, format, state).
pub fn parse_sink_list(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.split('\t').nth(1))
        .map(str::to_owned)
        .collect()
}

fn pactl(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("pactl").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The sinks of PipeWire or PulseAudio now, or `None` when `pactl` is missing or the sound server does not answer
/// (then nothing is paused).
pub fn list_sinks() -> Option<Vec<String>> {
    pactl(&["list", "short", "sinks"]).map(|text| parse_sink_list(&text))
}

/// The name of the default sink, or `None` when it cannot be read.
pub fn default_sink() -> Option<String> {
    pactl(&["get-default-sink"]).map(|text| text.trim().to_owned()).filter(|name| !name.is_empty())
}

/// Watches the sink playback started on. The app calls [`SinkWatch::poll`] every few seconds with [`list_sinks`].
#[derive(Debug, Default)]
pub struct SinkWatch {
    playing: String,
}

impl SinkWatch {
    /// Remembers the sink in use when playback starts (`default_sink()`).
    pub fn start(&mut self, sink: Option<String>) {
        self.playing = sink.unwrap_or_default();
    }

    /// Whether the sink in use has disappeared from `sinks_now`. A failed listing (`None`) is not a loss.
    pub fn poll(&self, sinks_now: Option<&[String]>) -> bool {
        sinks_now.is_some_and(|sinks| output_lost(&self.playing, sinks))
    }
}

impl<H: crate::mpv::PropertySink + crate::mpv::PropertySource + crate::window::CommandSink>
    crate::session::Session<H>
{
    /// The sink list changed: when the playing sink is gone, pauses mpv and releases keep-awake. Returns whether it
    /// paused.
    pub fn on_audio_sinks(&mut self, watch: &SinkWatch, sinks_now: Option<&[String]>) -> bool {
        if !watch.poll(sinks_now) {
            return false;
        }
        if let Err(error) = self.host.set_str("pause", "yes") {
            tracing::warn!(%error, "could not pause after the audio output went away");
        }
        self.desktop.sync(Some(true), false, self.state.pos);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sink_names_are_the_second_column() {
        let text = "0\talsa_output.speakers\tPipeWire\ts32le 2ch 48000Hz\tRUNNING\n1\tbluez_sink.AA\tPipeWire\ts16le 2ch\tIDLE\n";
        assert_eq!(parse_sink_list(text), ["alsa_output.speakers", "bluez_sink.AA"]);
        assert!(parse_sink_list("").is_empty());
    }

    #[test]
    fn a_failed_listing_never_pauses() {
        let mut watch = SinkWatch::default();
        watch.start(Some("bluez_sink.AA".into()));
        assert!(!watch.poll(None));
        assert!(watch.poll(Some(&sinks(&["alsa_output.speakers"]))));
        assert!(!watch.poll(Some(&sinks(&["bluez_sink.AA"]))));
    }

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

#[cfg(test)]
mod session_tests {
    use std::cell::RefCell;

    use super::*;
    use crate::mpv::{MpvError, PropertySink, PropertySource};
    use crate::session::Session;
    use crate::window::CommandSink;

    #[derive(Default)]
    struct Fake {
        writes: RefCell<Vec<(String, String)>>,
    }
    impl PropertySink for Fake {
        fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError> {
            self.writes.borrow_mut().push((name.to_owned(), value.to_owned()));
            Ok(())
        }
    }
    impl PropertySource for Fake {
        fn get_str(&self, _: &str) -> Option<String> {
            None
        }
    }
    impl CommandSink for Fake {
        fn command(&self, _: &[&str]) -> Result<(), MpvError> {
            Ok(())
        }
    }

    #[test]
    fn t_45a_082_losing_the_sink_pauses_mpv_and_a_present_sink_does_not() {
        let mut session = Session::new(Fake::default());
        let mut watch = SinkWatch::default();
        watch.start(Some("bluez_sink.AA".into()));
        assert!(!session.on_audio_sinks(&watch, Some(&["bluez_sink.AA".to_owned()])));
        assert!(session.host.writes.borrow().is_empty());
        assert!(session.on_audio_sinks(&watch, Some(&["alsa_output.speakers".to_owned()])));
        assert_eq!(session.host.writes.borrow().as_slice(), [("pause".to_owned(), "yes".to_owned())]);
        assert!(!session.desktop.holds_awake(), "keep-awake is released");
    }
}
