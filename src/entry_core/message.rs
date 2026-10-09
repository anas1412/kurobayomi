//! The snackbar, toast and message-key layer of the entry pages (T-39b-019). Upstream: `AnimeScreenModel.kt` (the
//! `showSnackbar` and toast calls) and behaviour section 19 of the anime plan. Pure: the GTK side takes a
//! [`Presentation`] and draws it with `banner_view`, and [`Notices`] is the one-at-a-time host.
//!
//! Decision, queue versus replace (E-UI-3): the plan says both. `p2-edge-cases` E-UI-3 and `p2-rust-design` say a new
//! message replaces the one on screen and its action is dropped (Compose queues them; a documented difference),
//! while the first draft of this file queued them. The page shows what just happened, so a new message replaces:
//! [`Notices::show`] keeps one.
//!
//! The "16 keys" of the task: section 19 of the behaviour plan has 28 rows. Twelve of them are messages with text of
//! their own, and they are the variants of [`UserMessage`]. The other sixteen are labels of buttons, titles and
//! texts of dialogs that other modules own (the tracking sheet, the date remover, the share chooser, Cancel, OK,
//! Delete and Add), so there are twelve message keys, not sixteen.

use std::time::Duration;

use kuro_i18n::{Arg, MsgKey, tr, trf};

use crate::entry_core::banner::{Banner, Banners, Length};

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

/// The host of the snackbar layer: one message at a time (E-UI-3). A new message replaces the one on screen and
/// drops its action. `A` is the page command an action button runs.
pub struct Notices<A: Clone> {
    host: Banners<A>,
    /// The current snack has a dismiss button. A toast and the plain snacks have none.
    dismissible: bool,
}

impl<A: Clone> Default for Notices<A> {
    fn default() -> Self {
        Self { host: Banners::default(), dismissible: false }
    }
}

impl<A: Clone> Notices<A> {
    /// Shows `presentation` now. `action` is what the snack's button runs; a toast has no button and a snack without
    /// an action label ignores it. The length is 4 s for a snack and 2 s for a toast.
    pub fn show(&mut self, presentation: Presentation, action: Option<A>, now_ms: u64) {
        let (message, label, dismiss, duration) = match &presentation {
            Presentation::Snack(snack) => (&snack.message, snack.action.map(SnackAction::label), snack.dismiss, snack.duration),
            Presentation::Toast(message) => (message, None, false, TOAST_DURATION),
        };
        self.host = Banners::default();
        self.dismissible = dismiss;
        self.host.push(text(message), label.zip(action), Length::Millis(duration.as_millis() as u64), now_ms);
    }

    /// Expires the message when its time is up.
    pub fn tick(&mut self, now_ms: u64) {
        self.host.tick(now_ms);
    }

    /// The close button.
    pub fn dismiss(&mut self, now_ms: u64) {
        self.host.dismiss(now_ms);
    }

    /// The action button: the page command to run. The message goes away with it.
    pub fn activate(&mut self, now_ms: u64) -> Option<A> {
        self.host.activate_action(true, now_ms)
    }

    /// The message on screen and whether it has a dismiss button.
    pub fn current(&self) -> Option<(&Banner<A>, bool)> {
        self.host.current().map(|banner| (banner, self.dismissible))
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
    fn a_new_message_replaces_the_one_on_screen_and_drops_its_action() {
        let mut notices: Notices<&'static str> = Notices::default();
        notices.show(present(UserMessage::DeleteDownloadsForAnime), Some("delete"), 0);
        let (banner, dismissible) = notices.current().expect("the prompt");
        assert_eq!((banner.text.as_str(), dismissible), ("Delete downloaded episodes?", true));
        assert_eq!(banner.action.as_ref().map(|(label, cmd)| (label.as_str(), *cmd)), Some(("Delete", "delete")));
        notices.show(present(UserMessage::NoEpisodes), None, 10);
        let (banner, dismissible) = notices.current().expect("the second message");
        assert_eq!((banner.text.as_str(), dismissible, banner.action.is_none()), ("No episodes found", false, true));
        assert_eq!(notices.activate(11), None, "the first one's action is gone (E-UI-3)");
    }

    #[test]
    fn a_snack_lasts_four_seconds_and_a_toast_two_and_a_toast_has_no_action() {
        let mut notices: Notices<u8> = Notices::default();
        notices.show(present(UserMessage::EpisodeSettingsUpdated), Some(1), 100);
        assert!(notices.current().is_some_and(|(b, _)| b.action.is_none()), "no label, so no action even when one is passed");
        notices.tick(4099);
        assert!(notices.current().is_some());
        notices.tick(4100);
        assert!(notices.current().is_none(), "short is 4 s (T-UI-1)");
        notices.show(present(UserMessage::NoMatchFound), Some(1), 0);
        let (banner, dismissible) = notices.current().expect("the toast");
        assert!(banner.action.is_none() && !dismissible);
        notices.tick(1999);
        assert!(notices.current().is_some());
        notices.tick(2000);
        assert!(notices.current().is_none(), "a toast is 2 s");
    }

    #[test]
    fn the_action_button_runs_once_and_closes_the_message() {
        let mut notices: Notices<&'static str> = Notices::default();
        notices.show(present(UserMessage::AddAnimeToLibrary), Some("add"), 0);
        assert_eq!(notices.activate(1), Some("add"));
        assert!(notices.current().is_none());
        notices.show(present(UserMessage::ConfirmTrackerUpdateAnime(3)), Some("ok"), 0);
        notices.dismiss(1);
        assert!(notices.current().is_none());
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
