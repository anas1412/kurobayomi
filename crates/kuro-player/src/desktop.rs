//! The desktop ports of one playback session (T-req-platform-005, T-req-platform-007): media keys and the media widget
//! through `MediaControls`, and the screen-idle inhibit through `IdleInhibitor`. The session calls [`Desktop::sync`] on
//! every pause change, so the controls and the guard follow playback on every path, and dropping the session releases both.

use std::sync::Arc;
use std::time::Duration;

use kuro_platform::idle::{IdleGuard, IdleInhibitor};
use kuro_platform::media::{MediaCommand, MediaControls, NoMediaControls, NowPlaying, PlaybackState, command_channel};

/// The reason shown by desktops that list inhibits.
const REASON: &str = "Playing video";

pub struct Desktop {
    media: Arc<dyn MediaControls>,
    idle: Option<Arc<dyn IdleInhibitor>>,
    guard: Option<IdleGuard>,
    last: Option<PlaybackState>,
}

impl Default for Desktop {
    /// No media service and no inhibit, so a session in a test or without a bus runs unchanged.
    fn default() -> Self {
        Self::new(Arc::new(NoMediaControls), None)
    }
}

impl Desktop {
    pub fn new(media: Arc<dyn MediaControls>, idle: Option<Arc<dyn IdleInhibitor>>) -> Self {
        Self { media, idle, guard: None, last: None }
    }

    /// MPRIS and the screen-saver inhibit of this desktop. `NoMediaControls` stands in when no session bus answers.
    pub fn connect() -> Self {
        let media: Arc<dyn MediaControls> = match kuro_platform::linux::Mpris::connect("Kurobayomi", "kurobayomi") {
            Ok(mpris) => Arc::new(mpris),
            Err(err) => {
                tracing::warn!(%err, "media keys are off");
                Arc::new(NoMediaControls)
            }
        };
        Self::new(media, Some(Arc::new(kuro_platform::linux::ScreenSaver)))
    }

    /// Routes the media keys and the desktop widget to the returned receiver, which the GTK side drains.
    pub fn commands(&self) -> async_channel::Receiver<MediaCommand> {
        let (handler, rx) = command_channel();
        if let Err(err) = self.media.on_command(handler) {
            tracing::warn!(%err, "media commands are not delivered");
        }
        rx
    }

    /// Publishes the title and artwork (a `file://` or `https://` URL) of the item that plays.
    pub fn now_playing(&self, title: &str, artwork: Option<&str>, duration_s: f32) {
        let now = NowPlaying {
            title: title.to_owned(),
            artwork: artwork.map(str::to_owned),
            duration: (duration_s > 0.0).then(|| Duration::from_secs_f32(duration_s)),
        };
        if let Err(err) = self.media.set_now_playing(&now) {
            tracing::warn!(%err, "now-playing was not published");
        }
    }

    /// Follows the pause state: publishes it when it changed, and holds the inhibit exactly while playing.
    pub fn sync(&mut self, paused: Option<bool>, exiting: bool, position_s: f32) {
        let state = match (paused, exiting) {
            (_, true) | (None, _) => PlaybackState::Stopped,
            (Some(true), false) => PlaybackState::Paused,
            (Some(false), false) => PlaybackState::Playing,
        };
        if self.last != Some(state) {
            self.last = Some(state);
            if let Err(err) = self.media.set_playback(state, Some(Duration::from_secs_f32(position_s.max(0.0)))) {
                tracing::warn!(%err, "the playback state was not published");
            }
        }
        if state != PlaybackState::Playing {
            self.guard = None;
        } else if self.guard.is_none() {
            self.guard = self.idle.as_ref().and_then(|idle| idle.inhibit(REASON).map_err(|err| tracing::warn!(%err, "the screen may blank")).ok());
        }
    }

    /// True while the screen is held awake.
    pub fn holds_awake(&self) -> bool {
        self.guard.is_some()
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = self.media.set_playback(PlaybackState::Stopped, None);
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use kuro_platform::media::{CommandHandler, MediaError};

    use super::*;

    #[derive(Default)]
    struct Media {
        playback: Mutex<Vec<PlaybackState>>,
        now: Mutex<Vec<NowPlaying>>,
        handler: Mutex<Option<CommandHandler>>,
    }

    struct Shared(Arc<Media>);

    impl MediaControls for Shared {
        fn on_command(&self, handler: CommandHandler) -> Result<(), MediaError> {
            *self.0.handler.lock().unwrap() = Some(handler);
            Ok(())
        }
        fn set_now_playing(&self, now: &NowPlaying) -> Result<(), MediaError> {
            self.0.now.lock().unwrap().push(now.clone());
            Ok(())
        }
        fn set_playback(&self, state: PlaybackState, _position: Option<Duration>) -> Result<(), MediaError> {
            self.0.playback.lock().unwrap().push(state);
            Ok(())
        }
    }

    struct Idle(Arc<AtomicUsize>);

    impl IdleInhibitor for Idle {
        fn inhibit(&self, _reason: &str) -> io::Result<IdleGuard> {
            self.0.fetch_add(1, Ordering::SeqCst);
            let live = Arc::clone(&self.0);
            Ok(IdleGuard::new(move || {
                live.fetch_sub(1, Ordering::SeqCst);
            }))
        }
    }

    fn desktop() -> (Desktop, Arc<Media>, Arc<AtomicUsize>) {
        let media = Arc::new(Media::default());
        let live = Arc::new(AtomicUsize::new(0));
        (Desktop::new(Arc::new(Shared(Arc::clone(&media))), Some(Arc::new(Idle(Arc::clone(&live))))), media, live)
    }

    #[test]
    fn the_inhibit_is_held_exactly_while_playing() {
        let (mut d, _media, live) = desktop();
        d.sync(None, false, 0.0);
        assert_eq!(live.load(Ordering::SeqCst), 0);
        d.sync(Some(false), false, 1.0);
        d.sync(Some(false), false, 2.0);
        assert_eq!(live.load(Ordering::SeqCst), 1, "one guard however many syncs");
        d.sync(Some(true), false, 3.0);
        assert_eq!(live.load(Ordering::SeqCst), 0);
        d.sync(Some(false), false, 3.0);
        d.sync(Some(false), true, 4.0);
        assert_eq!(live.load(Ordering::SeqCst), 0, "closing releases it");
    }

    #[test]
    fn the_playback_state_is_published_on_change_only() {
        let (mut d, media, _live) = desktop();
        d.sync(Some(false), false, 0.0);
        d.sync(Some(false), false, 1.0);
        d.sync(Some(true), false, 2.0);
        assert_eq!(*media.playback.lock().unwrap(), vec![PlaybackState::Playing, PlaybackState::Paused]);
    }

    #[test]
    fn the_title_and_artwork_are_published() {
        let (d, media, _live) = desktop();
        d.now_playing("Naruto 12", Some("file:///cover.png"), 90.0);
        let now = media.now.lock().unwrap();
        assert_eq!(now[0].title, "Naruto 12");
        assert_eq!(now[0].artwork.as_deref(), Some("file:///cover.png"));
        assert_eq!(now[0].duration, Some(Duration::from_secs(90)));
    }

    #[test]
    fn media_keys_reach_the_returned_receiver() {
        let (d, media, _live) = desktop();
        let rx = d.commands();
        (media.handler.lock().unwrap().as_ref().unwrap())(MediaCommand::Toggle);
        assert_eq!(rx.try_recv(), Ok(MediaCommand::Toggle));
    }
}
