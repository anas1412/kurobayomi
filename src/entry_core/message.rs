//! The snackbar, toast and message-key layer of the entry pages (T-39b-019). Upstream: `AnimeScreenModel.kt` (the
//! `showSnackbar` and toast calls) and behaviour section 19 of the anime plan. A [`Notice`] is what both entry pages
//! hand to the app's one notification layer (`crate::toast`).
//!
//! Decision, queue versus replace (E-UI-3): the plan says both. `p2-edge-cases` E-UI-3 and `p2-rust-design` say a new
//! message replaces the one on screen and its action is dropped (Compose queues them; a documented difference),
//! while the first draft of this file queued them. The page shows what just happened, so a new message replaces:
//! the toast layer keeps one.
//!
//! The "16 keys" of the task: section 19 of the behaviour plan has 28 rows. Twelve of them are messages with text of
//! their own, and they are the variants of [`UserMessage`]. The other sixteen are labels of buttons, titles and
//! texts of dialogs that other modules own (the tracking sheet, the date remover, the share chooser, Cancel, OK,
//! Delete and Add), so there are twelve message keys, not sixteen.

use std::cell::RefCell;
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
    Retry,
}

impl SnackAction {
    pub fn label(self) -> String {
        match self {
            Self::Delete => tr(MsgKey::action_delete),
            Self::Add => tr(MsgKey::action_add),
            Self::Ok => tr(MsgKey::action_ok),
            Self::Retry => tr(MsgKey::action_retry),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snack {
    pub message: UserMessage,
    pub action: Option<SnackAction>,
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
    let snack = |action| Snack { message: message.clone(), action, duration: SNACK_DURATION };
    match &message {
        UserMessage::TrackersUpdatedAnime(_) | UserMessage::TrackError { .. } | UserMessage::NoMatchFound => {
            Presentation::Toast(message)
        }
        UserMessage::DeleteDownloadsForAnime => Presentation::Snack(snack(Some(SnackAction::Delete))),
        UserMessage::AddAnimeToLibrary => Presentation::Snack(snack(Some(SnackAction::Add))),
        UserMessage::ConfirmTrackerUpdateAnime(_) => Presentation::Snack(snack(Some(SnackAction::Ok))),
        _ => Presentation::Snack(snack(None)),
    }
}

/// A failed refresh's snack: its text with Retry and a dismiss button.
pub fn with_retry(message: UserMessage) -> Presentation {
    Presentation::Snack(Snack { message, action: Some(SnackAction::Retry), duration: SNACK_DURATION })
}

/// What an entry page puts on the app's notification layer: the text and, for a snack with a button, the button's label
/// and the page command it runs. A notice with a button stays until it is acted on or dismissed (T-38a-034); a plain
/// one times out after `duration`. A new notice replaces the one on screen, and its action is gone (E-UI-3).
#[derive(Debug)]
pub struct Notice<A> {
    pub text: String,
    pub action: Option<(String, A)>,
    pub duration: Duration,
}

impl<A: 'static> Notice<A> {
    /// A plain message, shown for `duration`.
    pub fn plain(text: impl Into<String>, duration: Duration) -> Self {
        Self { text: text.into(), action: None, duration }
    }

    /// A message with a button.
    pub fn with_action(text: impl Into<String>, label: impl Into<String>, command: A) -> Self {
        Self { text: text.into(), action: Some((label.into(), command)), duration: SNACK_DURATION }
    }

    /// The notice of a presentation. `action` is what its button runs; a toast has no button and a snack without an
    /// action label ignores it.
    pub fn of(presentation: &Presentation, action: Option<A>) -> Self {
        let (message, label, duration) = match presentation {
            Presentation::Snack(snack) => (&snack.message, snack.action.map(SnackAction::label), snack.duration),
            Presentation::Toast(message) => (message, None, TOAST_DURATION),
        };
        Self { text: text(message), action: label.zip(action), duration }
    }

    /// Shows the notice on the toast layer. `run` takes the command when its button is pressed.
    pub fn show(self, run: impl Fn(A) + 'static) {
        match self.action {
            Some((label, command)) => {
                let command = RefCell::new(Some(command));
                crate::toast::notify_action(&self.text, &label, None, move || {
                    if let Some(command) = command.borrow_mut().take() {
                        run(command);
                    }
                });
            }
            None => crate::toast::notify_for(&self.text, self.duration),
        }
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
        assert_eq!(delete.action, Some(SnackAction::Delete));
        assert_eq!(delete.duration, SNACK_DURATION);
        let Presentation::Snack(add) = present(UserMessage::AddAnimeToLibrary) else { panic!("snack") };
        assert_eq!(add.action, Some(SnackAction::Add));
        let Presentation::Snack(ask) = present(UserMessage::ConfirmTrackerUpdateAnime(2)) else { panic!("snack") };
        assert_eq!(ask.action, Some(SnackAction::Ok));
        let Presentation::Snack(saved) = present(UserMessage::EpisodeSettingsUpdated) else { panic!("snack") };
        assert_eq!(saved.action, None);
        assert_eq!(SnackAction::Ok.label(), "OK");
        let Presentation::Snack(failed) = with_retry(UserMessage::Plain("HTTP error 503".into())) else { panic!("snack") };
        assert_eq!(failed.action.map(SnackAction::label), Some("Retry".to_owned()));
    }

    #[test]
    fn trackers_and_track_errors_are_toasts_without_action() {
        assert_eq!(present(UserMessage::TrackersUpdatedAnime(1)), Presentation::Toast(UserMessage::TrackersUpdatedAnime(1)));
        assert!(matches!(present(UserMessage::NoMatchFound), Presentation::Toast(_)));
        assert_eq!(TOAST_DURATION, Duration::from_secs(2));
    }

    #[test]
    fn a_notice_has_a_button_only_when_the_presentation_has_a_label_and_a_command_comes_with_it() {
        let delete = Notice::of(&present(UserMessage::DeleteDownloadsForAnime), Some("delete"));
        assert_eq!(delete.text, "Delete downloaded episodes?");
        assert_eq!(delete.action, Some(("Delete".to_owned(), "delete")));
        let saved = Notice::of(&present(UserMessage::EpisodeSettingsUpdated), Some("x"));
        assert!(saved.action.is_none(), "no label, so no action even when one is passed");
        assert_eq!(saved.duration, SNACK_DURATION, "a snack is 4 s");
        let toast = Notice::of(&present(UserMessage::NoMatchFound), Some("x"));
        assert!(toast.action.is_none());
        assert_eq!(toast.duration, TOAST_DURATION, "a toast is 2 s");
        let retry = Notice::of(&with_retry(UserMessage::Plain("HTTP error 503".into())), Some("retry"));
        assert_eq!(retry.action, Some(("Retry".to_owned(), "retry")));
    }

    #[test]
    fn section_19_has_twelve_message_keys() {
        // one value per variant: a new variant fails to compile here until it is counted
        let all = [
            UserMessage::NoEpisodes,
            UserMessage::NoSeasons,
            UserMessage::DeleteDownloadsForAnime,
            UserMessage::AddAnimeToLibrary,
            UserMessage::TrackersUpdatedAnime(1),
            UserMessage::ConfirmTrackerUpdateAnime(1),
            UserMessage::EpisodeSettingsUpdated,
            UserMessage::SeasonSettingsUpdated,
            UserMessage::DisplayModeEpisode(String::new()),
            UserMessage::TrackError { tracker: String::new(), detail: String::new() },
            UserMessage::NoMatchFound,
            UserMessage::Plain(String::new()),
        ];
        for message in &all {
            match message {
                UserMessage::NoEpisodes
                | UserMessage::NoSeasons
                | UserMessage::DeleteDownloadsForAnime
                | UserMessage::AddAnimeToLibrary
                | UserMessage::TrackersUpdatedAnime(_)
                | UserMessage::ConfirmTrackerUpdateAnime(_)
                | UserMessage::EpisodeSettingsUpdated
                | UserMessage::SeasonSettingsUpdated
                | UserMessage::DisplayModeEpisode(_)
                | UserMessage::TrackError { .. }
                | UserMessage::NoMatchFound
                | UserMessage::Plain(_) => {}
            }
        }
        assert_eq!(all.len(), 12);
    }
}
