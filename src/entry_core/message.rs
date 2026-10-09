//! The snackbar, toast and message-key layer of the entry pages (T-39b-019). Upstream: `AnimeScreenModel.kt` (the
//! `showSnackbar` and toast calls) and behaviour section 19 of the anime plan. Pure: the GTK side takes a
//! [`Presentation`] and draws it, and [`Notices`] keeps the one-at-a-time order (E-UI-3).

use std::collections::VecDeque;
use std::time::Duration;

use kuro_i18n::{Arg, MsgKey, tr, trf};

/// Every user-visible message of the details pages that carries text of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserMessage {
    NoEpisodes,
    NoSeasons,
    DeleteDownloadsForAnime,
    AddAnimeToLibrary,
    TrackersUpdatedAnime(i64),
    ConfirmTrackerUpdateAnime(i64),
    EpisodeSettingsUpdated,
    SeasonSettingsUpdated,
    DisplayModeEpisode(String),
    TrackError { tracker: String, detail: String },
    NoMatchFound,
    /// A text that is not a key of its own: an exception message from a refresh.
    Plain(String),
}

/// The English text of the message, with its arguments substituted.
pub fn text(message: &UserMessage) -> String {
    match message {
        UserMessage::NoEpisodes => tr(MsgKey::no_episodes_error),
        UserMessage::NoSeasons => tr(MsgKey::no_seasons_error),
        UserMessage::DeleteDownloadsForAnime => tr(MsgKey::delete_downloads_for_anime),
        UserMessage::AddAnimeToLibrary => tr(MsgKey::snack_add_to_anime_library),
        UserMessage::TrackersUpdatedAnime(ep) => trf(MsgKey::trackers_updated_summary_anime, &[("arg1", Arg::Int(*ep))]),
        UserMessage::ConfirmTrackerUpdateAnime(ep) => trf(MsgKey::confirm_tracker_update_anime, &[("arg1", Arg::Int(*ep))]),
        UserMessage::EpisodeSettingsUpdated => tr(MsgKey::episode_settings_updated),
        UserMessage::SeasonSettingsUpdated => tr(MsgKey::season_settings_updated),
        UserMessage::DisplayModeEpisode(number) => trf(MsgKey::display_mode_episode, &[("arg1", Arg::Str(number))]),
        UserMessage::TrackError { tracker, detail } => {
            trf(MsgKey::track_error, &[("arg1", Arg::Str(tracker)), ("arg2", Arg::Str(detail))])
        }
        UserMessage::NoMatchFound => tr(MsgKey::error_no_match),
        UserMessage::Plain(text) => text.clone(),
    }
}

/// `Short` is the snackbar default (4 s). Toasts use [`TOAST_DURATION`].
pub const SNACK_DURATION: Duration = Duration::from_secs(4);
pub const TOAST_DURATION: Duration = Duration::from_secs(2);

/// The button a snackbar offers besides the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnackAction {
    Delete,
    Add,
    Ok,
}

impl SnackAction {
    pub fn label(self) -> String {
        match self {
            Self::Delete => tr(MsgKey::action_delete),
            Self::Add => tr(MsgKey::action_add),
            Self::Ok => tr(MsgKey::action_ok),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snack {
    pub message: UserMessage,
    pub action: Option<SnackAction>,
    /// True when the snackbar has a dismiss button.
    pub dismiss: bool,
    pub duration: Duration,
}

/// How a message is shown: a snackbar with its button rules, or a toast with no action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presentation {
    Snack(Snack),
    Toast(UserMessage),
}

/// The presentation of each message, as in behaviour section 19. Trackers updated and tracker errors are toasts;
/// the auto-track ask is a short snackbar with OK and a dismiss button.
pub fn present(message: UserMessage) -> Presentation {
    let snack = |action, dismiss| Snack { message: message.clone(), action, dismiss, duration: SNACK_DURATION };
    match &message {
        UserMessage::TrackersUpdatedAnime(_) | UserMessage::TrackError { .. } | UserMessage::NoMatchFound => {
            Presentation::Toast(message)
        }
        UserMessage::DeleteDownloadsForAnime => Presentation::Snack(snack(Some(SnackAction::Delete), true)),
        UserMessage::AddAnimeToLibrary => Presentation::Snack(snack(Some(SnackAction::Add), true)),
        UserMessage::ConfirmTrackerUpdateAnime(_) => Presentation::Snack(snack(Some(SnackAction::Ok), true)),
        _ => Presentation::Snack(snack(None, false)),
    }
}

/// Snackbars are shown one at a time (E-UI-3): later ones wait in order.
#[derive(Default)]
pub struct Notices {
    waiting: VecDeque<Snack>,
}

impl Notices {
    /// Queues a presentation. A toast is not queued here; the caller shows it at once.
    pub fn push(&mut self, presentation: Presentation) -> Option<UserMessage> {
        match presentation {
            Presentation::Snack(snack) => {
                self.waiting.push_back(snack);
                None
            }
            Presentation::Toast(message) => Some(message),
        }
    }

    /// The snackbar to show now, once the previous one is dismissed or timed out.
    pub fn next(&mut self) -> Option<Snack> {
        self.waiting.pop_front()
    }

    pub fn pending(&self) -> usize {
        self.waiting.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_renders_with_its_arguments() {
        assert_eq!(text(&UserMessage::NoEpisodes), "No episodes found");
        assert_eq!(text(&UserMessage::NoSeasons), "No seasons found");
        assert_eq!(text(&UserMessage::DeleteDownloadsForAnime), "Delete downloaded episodes?");
        assert_eq!(text(&UserMessage::AddAnimeToLibrary), "Add anime to library?");
        assert_eq!(text(&UserMessage::TrackersUpdatedAnime(5)), "Trackers updated to episode 5");
        assert_eq!(text(&UserMessage::ConfirmTrackerUpdateAnime(12)), "Update trackers to episode 12?");
        assert_eq!(text(&UserMessage::EpisodeSettingsUpdated), "Updated default episode settings");
        assert_eq!(text(&UserMessage::SeasonSettingsUpdated), "Updated default season settings");
        assert_eq!(text(&UserMessage::DisplayModeEpisode("3".into())), "Episode 3");
        assert_eq!(
            text(&UserMessage::TrackError { tracker: "AniList".into(), detail: "timeout".into() }),
            "AniList error: timeout"
        );
        assert_eq!(text(&UserMessage::NoMatchFound), "No match found");
        assert_eq!(text(&UserMessage::Plain("boom".into())), "boom");
    }

    #[test]
    fn snack_buttons_follow_the_table() {
        let Presentation::Snack(delete) = present(UserMessage::DeleteDownloadsForAnime) else { panic!("snack") };
        assert_eq!((delete.action, delete.dismiss), (Some(SnackAction::Delete), true));
        assert_eq!(delete.duration, SNACK_DURATION);
        let Presentation::Snack(add) = present(UserMessage::AddAnimeToLibrary) else { panic!("snack") };
        assert_eq!((add.action, add.dismiss), (Some(SnackAction::Add), true));
        let Presentation::Snack(ask) = present(UserMessage::ConfirmTrackerUpdateAnime(2)) else { panic!("snack") };
        assert_eq!((ask.action, ask.dismiss), (Some(SnackAction::Ok), true));
        let Presentation::Snack(saved) = present(UserMessage::EpisodeSettingsUpdated) else { panic!("snack") };
        assert_eq!((saved.action, saved.dismiss), (None, false));
        assert_eq!(SnackAction::Ok.label(), "OK");
    }

    #[test]
    fn trackers_and_track_errors_are_toasts_without_action() {
        assert_eq!(present(UserMessage::TrackersUpdatedAnime(1)), Presentation::Toast(UserMessage::TrackersUpdatedAnime(1)));
        assert!(matches!(present(UserMessage::NoMatchFound), Presentation::Toast(_)));
        assert_eq!(TOAST_DURATION, Duration::from_secs(2));
    }

    #[test]
    fn snackbars_queue_one_after_another_and_toasts_skip_the_queue() {
        let mut notices = Notices::default();
        assert_eq!(notices.push(present(UserMessage::NoEpisodes)), None);
        assert_eq!(notices.push(present(UserMessage::NoSeasons)), None);
        assert_eq!(notices.pending(), 2);
        assert_eq!(notices.push(present(UserMessage::NoMatchFound)), Some(UserMessage::NoMatchFound));
        assert_eq!(notices.next().map(|s| s.message), Some(UserMessage::NoEpisodes));
        assert_eq!(notices.next().map(|s| s.message), Some(UserMessage::NoSeasons));
        assert_eq!(notices.next(), None);
    }
}
