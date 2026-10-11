//! Player window lifecycle (area 45 `PlayerActivity` `onNewIntent`, `onPause`, `onResume`,
//! `onDestroy`, fullscreen; behaviour 1.2, 1.5, 1.6, 1.8, 14). Pure decisions. The GTK window keeps
//! one `PlayerState`, applies the returned steps, and performs the mpv and GTK calls.

use std::future::Future;
use std::time::Duration;

use crate::hosts::close_top;
use crate::mpv::{MpvError, MpvHandle};
use crate::session_file::{FINAL_SAVE_DEADLINE_MS, SHUTDOWN_TIMEOUT_MS};
use crate::state::PlayerState;
use crate::volume::resume_volume;

/// One step of an open request (`onNewIntent`, behaviour 1.2), listed in the order to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentStep {
    /// `animeId` or `episodeId` is `-1`: the window closes and nothing else runs.
    Close,
    /// `saveCurrentEpisodeWatchingProgress()` for the episode that is playing now
    /// (`PlayerActivity.kt:201`).
    SaveProgress,
    /// Load the new episode. `reuse_window` is true when a window is already open, so the open
    /// window is reused and no second one is created.
    Load { reuse_window: bool },
}

/// Decides what an open request does. Only one player window exists at a time. When a window is
/// already open, the playing episode is the previous one and its progress is saved before the new
/// one loads. A first open has no previous episode, so there is nothing to save.
pub fn new_intent(window_open: bool, anime_id: i64, episode_id: i64) -> Vec<IntentStep> {
    if anime_id == -1 || episode_id == -1 {
        return vec![IntentStep::Close];
    }
    let mut steps = Vec::with_capacity(2);
    if window_open {
        steps.push(IntentStep::SaveProgress);
    }
    steps.push(IntentStep::Load {
        reuse_window: window_open,
    });
    steps
}

/// Why the window is being paused (`onPause`, behaviour 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseKind {
    /// Entering picture-in-picture: playback continues.
    PictureInPicture,
    /// The window is hidden but not closing: a plain pause.
    Hidden,
    /// The window is finishing: close.
    Closing,
}

/// Ordered effects of a pause (behaviour 1.6). `SetExiting` is applied to the state by `on_pause`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseStep {
    SaveProgress,
    SetExiting,
    Pause,
    DeletePending,
    StopPlayback,
}

/// The steps for each pause kind. Progress is always saved first. A close also deletes the
/// pending episodes and stops playback. A plain pause only pauses. PiP changes nothing else.
pub fn pause_steps(kind: PauseKind) -> Vec<PauseStep> {
    match kind {
        PauseKind::PictureInPicture => vec![PauseStep::SaveProgress],
        PauseKind::Hidden => vec![
            PauseStep::SaveProgress,
            PauseStep::SetExiting,
            PauseStep::Pause,
        ],
        PauseKind::Closing => vec![
            PauseStep::SaveProgress,
            PauseStep::SetExiting,
            PauseStep::DeletePending,
            PauseStep::StopPlayback,
        ],
    }
}

/// Applies `onPause` to the state and returns the steps to run. A PiP pause leaves `is_exiting`
/// unset, so the event gate stays open.
pub fn on_pause(state: &mut PlayerState, kind: PauseKind) -> Vec<PauseStep> {
    let steps = pause_steps(kind);
    if steps.contains(&PauseStep::SetExiting) {
        state.is_exiting = true;
    }
    steps
}

/// `onResume` clears `isExiting`, and only `onResume` does. A `PLAYBACK_RESTART` cannot clear it,
/// because the event gate drops events while it is set (behaviour 6.4).
pub fn on_resume(state: &mut PlayerState) {
    // Coming back from a hidden window asks for the resume volume rule to run once (T-45a-046).
    state.restore_volume_on_resume = state.is_exiting;
    state.is_exiting = false;
}

/// The mpv volume to write after a resume from a hidden window (T-45a-046). It is 100 when the
/// device volume is below its maximum (`volume::resume_volume`), and it is taken once.
pub fn take_volume_restore(state: &mut PlayerState, device_steps: i32) -> Option<i64> {
    if !std::mem::take(&mut state.restore_volume_on_resume) {
        return None;
    }
    resume_volume(device_steps)
}

/// The event gate. While the window is exiting, observer events are dropped, and `execute` drops
/// mpv commands.
pub fn accepts_event(state: &PlayerState) -> bool {
    !state.is_exiting
}

/// Where mpv commands go. `MpvHandle` is the real sink; tests use a recording fake.
pub trait CommandSink {
    fn command(&self, args: &[&str]) -> Result<(), MpvError>;
}

impl CommandSink for MpvHandle {
    fn command(&self, args: &[&str]) -> Result<(), MpvError> {
        MpvHandle::command(self, args)
    }
}

/// `executeMPVCommand` (`PlayerActivity.kt:412-416`). While the window is exiting the command is
/// dropped and mpv is not called. Window code sends its commands through here, not straight to the
/// handle.
pub fn execute(
    state: &PlayerState,
    sink: &impl CommandSink,
    args: &[&str],
) -> Result<(), MpvError> {
    if accepts_event(state) {
        sink.command(args)
    } else {
        Ok(())
    }
}

/// Shutdown steps in order (`onDestroy`, behaviour 1.8; T-45a-039). The noisy-audio receiver is
/// not in the list, because it belongs to T-45a-082.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutdownStep {
    SetExiting,
    /// The last progress save, under `final_save`'s 2 s deadline (E-29).
    FinalSave,
    StopHttpServer,
    ReleaseAudioFocus,
    ReleaseMediaSession,
    RemoveObservers,
    DestroyRenderContext,
    DestroyHandle,
}

/// The order of shutdown. The render context is destroyed before the handle.
pub const SHUTDOWN_ORDER: [ShutdownStep; 8] = [
    ShutdownStep::SetExiting,
    ShutdownStep::FinalSave,
    ShutdownStep::StopHttpServer,
    ShutdownStep::ReleaseAudioFocus,
    ShutdownStep::ReleaseMediaSession,
    ShutdownStep::RemoveObservers,
    ShutdownStep::DestroyRenderContext,
    ShutdownStep::DestroyHandle,
];

/// Runs `save` for at most `deadline`. Returns whether it finished in time. A save that runs over
/// is dropped, so a stuck database never holds the window open.
pub async fn save_within(deadline: Duration, save: impl Future<Output = ()>) -> bool {
    tokio::time::timeout(deadline, save).await.is_ok()
}

/// The final progress save of a closing window, with the 2 s deadline (E-29).
pub async fn final_save(save: impl Future<Output = ()>) -> bool {
    save_within(Duration::from_millis(FINAL_SAVE_DEADLINE_MS), save).await
}

/// How long the runtime may take to shut down after the final save (E-29): 3 s. The caller passes
/// this to `Runtime::shutdown_timeout`.
pub const fn runtime_shutdown_timeout() -> Duration {
    Duration::from_millis(SHUTDOWN_TIMEOUT_MS)
}

/// Starts shutdown: sets `is_exiting` and returns the steps to run in order.
pub fn begin_shutdown(state: &mut PlayerState) -> &'static [ShutdownStep] {
    state.is_exiting = true;
    &SHUTDOWN_ORDER
}

/// Whether the window opens fullscreen. The preference defaults to on when unset.
pub fn opens_fullscreen(pref: Option<bool>) -> bool {
    pref.unwrap_or(true)
}

/// What Escape does. Escape leaves fullscreen when fullscreen. Otherwise it closes the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeAction {
    LeaveFullscreen,
    Close,
}

pub fn escape_action(fullscreen: bool) -> EscapeAction {
    if fullscreen {
        EscapeAction::LeaveFullscreen
    } else {
        EscapeAction::Close
    }
}

/// What Escape does given the whole state. An open sheet, panel or dialog closes first and the
/// window stays; with none open it falls back to `escape_action`.
pub fn escape(state: &mut PlayerState, fullscreen: bool) -> Option<EscapeAction> {
    match close_top(state) {
        Some(_) => None,
        None => Some(escape_action(fullscreen)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn missing_ids_close_the_window() {
        assert_eq!(new_intent(false, -1, 5), vec![IntentStep::Close]);
        assert_eq!(new_intent(true, 5, -1), vec![IntentStep::Close]);
    }

    #[test]
    fn open_window_saves_previous_progress_before_loading() {
        assert_eq!(
            new_intent(true, 1, 2),
            vec![
                IntentStep::SaveProgress,
                IntentStep::Load { reuse_window: true }
            ]
        );
    }

    #[test]
    fn first_open_has_no_previous_progress_to_save() {
        assert_eq!(
            new_intent(false, 1, 2),
            vec![IntentStep::Load {
                reuse_window: false
            }]
        );
    }

    #[test]
    fn pause_saves_progress_before_pausing() {
        assert_eq!(
            pause_steps(PauseKind::Hidden),
            vec![
                PauseStep::SaveProgress,
                PauseStep::SetExiting,
                PauseStep::Pause
            ]
        );
    }

    #[test]
    fn close_deletes_pending_and_stops_without_pausing() {
        let steps = pause_steps(PauseKind::Closing);
        assert_eq!(
            steps,
            vec![
                PauseStep::SaveProgress,
                PauseStep::SetExiting,
                PauseStep::DeletePending,
                PauseStep::StopPlayback,
            ]
        );
        assert!(!steps.contains(&PauseStep::Pause));
    }

    #[test]
    fn pip_pause_keeps_playing_and_leaves_gate_open() {
        let mut state = PlayerState::default();
        let steps = on_pause(&mut state, PauseKind::PictureInPicture);
        assert_eq!(steps, vec![PauseStep::SaveProgress]);
        assert!(!state.is_exiting);
        assert!(accepts_event(&state));
    }

    #[test]
    fn exiting_gate_drops_events_until_resume() {
        let mut state = PlayerState::default();
        on_pause(&mut state, PauseKind::Hidden);
        assert!(state.is_exiting);
        assert!(!accepts_event(&state));
        on_resume(&mut state);
        assert!(!state.is_exiting);
        assert!(accepts_event(&state));
    }

    struct Recorder(RefCell<Vec<Vec<String>>>);

    impl CommandSink for Recorder {
        fn command(&self, args: &[&str]) -> Result<(), MpvError> {
            self.0
                .borrow_mut()
                .push(args.iter().map(|a| (*a).to_owned()).collect());
            Ok(())
        }
    }

    #[test]
    fn exiting_gate_drops_mpv_commands_until_resume() {
        let sink = Recorder(RefCell::new(Vec::new()));
        let mut state = PlayerState::default();

        execute(&state, &sink, &["seek", "5"]).unwrap();
        assert_eq!(sink.0.borrow().len(), 1);

        on_pause(&mut state, PauseKind::Hidden);
        execute(&state, &sink, &["seek", "10"]).unwrap();
        assert_eq!(sink.0.borrow().len(), 1);

        on_resume(&mut state);
        execute(&state, &sink, &["seek", "15"]).unwrap();
        assert_eq!(
            *sink.0.borrow(),
            vec![vec!["seek", "5"], vec!["seek", "15"]]
        );
    }

    #[test]
    fn shutdown_order_stops_server_releases_then_destroys_render_before_handle() {
        let mut state = PlayerState::default();
        let order = begin_shutdown(&mut state);
        assert!(state.is_exiting);
        assert_eq!(order, &SHUTDOWN_ORDER[..]);
        let pos = |s: ShutdownStep| order.iter().position(|x| *x == s).unwrap();
        assert!(pos(ShutdownStep::StopHttpServer) < pos(ShutdownStep::ReleaseAudioFocus));
        assert!(pos(ShutdownStep::ReleaseAudioFocus) < pos(ShutdownStep::ReleaseMediaSession));
        assert!(pos(ShutdownStep::ReleaseMediaSession) < pos(ShutdownStep::RemoveObservers));
        assert!(pos(ShutdownStep::RemoveObservers) < pos(ShutdownStep::DestroyRenderContext));
        assert!(pos(ShutdownStep::DestroyRenderContext) < pos(ShutdownStep::DestroyHandle));
    }

    #[test]
    fn final_save_runs_right_after_the_exiting_flag_and_before_teardown() {
        let pos = |s: ShutdownStep| SHUTDOWN_ORDER.iter().position(|x| *x == s).unwrap();
        assert!(pos(ShutdownStep::SetExiting) < pos(ShutdownStep::FinalSave));
        assert!(pos(ShutdownStep::FinalSave) < pos(ShutdownStep::StopHttpServer));
    }

    #[tokio::test]
    async fn a_save_past_its_deadline_is_dropped() {
        let quick = save_within(Duration::from_millis(500), async {}).await;
        assert!(quick);
        let slow = save_within(
            Duration::from_millis(20),
            tokio::time::sleep(Duration::from_secs(30)),
        )
        .await;
        assert!(!slow);
        assert!(final_save(async {}).await);
    }

    #[test]
    fn runtime_shutdown_waits_three_seconds() {
        assert_eq!(runtime_shutdown_timeout(), Duration::from_secs(3));
        assert_eq!(FINAL_SAVE_DEADLINE_MS, 2000);
    }

    #[test]
    fn fullscreen_defaults_on() {
        assert!(opens_fullscreen(None));
        assert!(opens_fullscreen(Some(true)));
        assert!(!opens_fullscreen(Some(false)));
    }

    #[test]
    fn escape_leaves_fullscreen_or_closes() {
        assert_eq!(escape_action(true), EscapeAction::LeaveFullscreen);
        assert_eq!(escape_action(false), EscapeAction::Close);
    }

    #[test]
    fn escape_closes_the_open_panel_and_keeps_the_window() {
        use crate::enums::Panels;
        let mut state = PlayerState {
            panel: Panels::SubtitleSettings,
            ..PlayerState::default()
        };
        assert_eq!(escape(&mut state, false), None);
        assert_eq!(state.panel, Panels::None);
        assert_eq!(escape(&mut state, false), Some(EscapeAction::Close));
        assert_eq!(
            escape(&mut state, true),
            Some(EscapeAction::LeaveFullscreen)
        );
    }

    #[test]
    fn t46_resume_from_hidden_sets_volume_100_once_when_the_device_is_below_max() {
        let mut state = PlayerState {
            is_exiting: true,
            ..PlayerState::default()
        };
        on_resume(&mut state);
        assert_eq!(take_volume_restore(&mut state, 3), Some(100));
        assert_eq!(take_volume_restore(&mut state, 3), None);
    }

    #[test]
    fn t46_resume_from_hidden_at_device_max_writes_nothing() {
        let mut state = PlayerState {
            is_exiting: true,
            ..PlayerState::default()
        };
        on_resume(&mut state);
        assert_eq!(take_volume_restore(&mut state, crate::system_av::VOLUME_STEPS), None);
    }

    #[test]
    fn plain_resume_restores_nothing() {
        let mut state = PlayerState::default();
        on_resume(&mut state);
        assert_eq!(take_volume_restore(&mut state, 3), None);
    }
}
