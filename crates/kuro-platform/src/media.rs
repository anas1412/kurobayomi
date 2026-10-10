//! Media keys, the desktop media widget and now-playing info (T-req-platform-005). The player talks to [`MediaControls`];
//! on Linux the implementation is MPRIS (`linux::media`).

use std::time::Duration;

/// A transport command from a media key or the desktop media widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaCommand {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Stop,
    SeekForward(Duration),
    SeekBackward(Duration),
    /// Absolute position in the current item.
    SeekTo(Duration),
}

/// What the player is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

/// The item the desktop shows: title, artwork as a `file://` URL, and length.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NowPlaying {
    pub title: String,
    pub artwork: Option<String>,
    pub duration: Option<Duration>,
}

/// The media service could not be reached or refused a change.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("media controls unavailable: {0}")]
pub struct MediaError(pub String);

/// Receives media commands. Called on the service's thread, so it must not touch GTK directly.
pub type CommandHandler = Box<dyn Fn(MediaCommand) + Send + 'static>;

/// A handler that queues every command, and the receiver the GTK main context reads. The service thread never touches
/// GTK; the UI side awaits `recv()` in `glib::spawn_future_local` and drives the player. A full queue drops the newest
/// command, which only happens when the UI has stopped reading.
pub fn command_channel() -> (CommandHandler, async_channel::Receiver<MediaCommand>) {
    let (tx, rx) = async_channel::bounded(32);
    (Box::new(move |command| {
        let _ = tx.try_send(command);
    }), rx)
}

/// Publishes now-playing state and receives transport commands.
pub trait MediaControls: Send + Sync {
    /// Sends commands to `handler` until the controls are dropped. A second call replaces the first handler.
    fn on_command(&self, handler: CommandHandler) -> Result<(), MediaError>;
    fn set_now_playing(&self, now: &NowPlaying) -> Result<(), MediaError>;
    fn set_playback(&self, state: PlaybackState, position: Option<Duration>) -> Result<(), MediaError>;
}

/// Controls for a session with no media service. Every call succeeds and does nothing, so the player runs unchanged.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoMediaControls;

impl MediaControls for NoMediaControls {
    fn on_command(&self, _handler: CommandHandler) -> Result<(), MediaError> {
        Ok(())
    }

    fn set_now_playing(&self, _now: &NowPlaying) -> Result<(), MediaError> {
        Ok(())
    }

    fn set_playback(&self, _state: PlaybackState, _position: Option<Duration>) -> Result<(), MediaError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_media_controls_accept_every_call() {
        let controls = NoMediaControls;
        controls.on_command(Box::new(|_| {})).unwrap();
        controls.set_now_playing(&NowPlaying { title: "Ep 1".into(), ..Default::default() }).unwrap();
        controls.set_playback(PlaybackState::Playing, Some(Duration::from_secs(5))).unwrap();
    }

    #[test]
    fn the_media_error_names_the_cause() {
        assert_eq!(MediaError("no bus".into()).to_string(), "media controls unavailable: no bus");
    }

    #[test]
    fn command_channel_carries_commands_off_the_service_thread() {
        let (handler, rx) = command_channel();
        std::thread::spawn(move || handler(MediaCommand::Toggle)).join().unwrap();
        assert_eq!(rx.try_recv(), Ok(MediaCommand::Toggle));
    }
}
