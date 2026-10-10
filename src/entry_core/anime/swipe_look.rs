//! What a swipe on an episode row looks like and the menu that offers the same actions (T-39a-017). Upstream:
//! `AnimeEpisodeListItem.kt` (`getSwipeAction`, `swipeActionThreshold`) and p1-behavior 3.6. Pure: the icon and the undo
//! state of each action come from the state of the episode, the reveal is the width of the drag, and the menu lists the
//! actions that do something. The widget is `row_view`.

use kuro_domain::library::swipe::SWIPE_THRESHOLD_PX;
use kuro_i18n::MsgKey;
use kuro_prefs::library::EpisodeSwipeAction;

use crate::entry_core::swipe::Side;

/// The download state of an episode as the swipe icon reads it (`AnimeDownload.State`). The page knows `Downloaded` and
/// `NotDownloaded`; the other three are for a queue that reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState {
    NotDownloaded,
    Queued,
    Downloading,
    Downloaded,
    Error,
}

/// The state of one episode that the five actions read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpisodeState {
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub download: DownloadState,
}

/// The icon of a swipe, and whether it undoes something (`isUndo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwipeLook {
    pub icon: &'static str,
    pub undo: bool,
}

/// `getSwipeAction`: the icon and the undo state of one action. `None` for Disabled (no action is drawn).
pub fn look(action: EpisodeSwipeAction, state: EpisodeState) -> Option<SwipeLook> {
    Some(match action {
        EpisodeSwipeAction::ToggleSeen => SwipeLook { icon: if state.seen { "edit-undo-symbolic" } else { "object-select-symbolic" }, undo: state.seen },
        EpisodeSwipeAction::ToggleBookmark => SwipeLook { icon: if state.bookmark { "list-remove-symbolic" } else { "bookmark-new-symbolic" }, undo: state.bookmark },
        EpisodeSwipeAction::ToggleFillermark => SwipeLook { icon: if state.fillermark { "edit-clear-symbolic" } else { "tag-symbolic" }, undo: state.fillermark },
        EpisodeSwipeAction::Download => SwipeLook {
            icon: match state.download {
                DownloadState::NotDownloaded | DownloadState::Error => "folder-download-symbolic",
                DownloadState::Queued | DownloadState::Downloading => "process-stop-symbolic",
                DownloadState::Downloaded => "user-trash-symbolic",
            },
            undo: false,
        },
        EpisodeSwipeAction::Disabled => return None,
    })
}

/// One item of the row's context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub action: EpisodeSwipeAction,
    pub label: MsgKey,
    pub icon: &'static str,
}

/// The four actions that do something, in the order of the settings list, with the label of what a press does now
/// (Mark as seen or unseen, and so on). Disabled is the fifth type: it has no item. Download shows only when the app
/// can queue downloads.
pub fn menu(state: EpisodeState, has_download_handler: bool) -> Vec<MenuItem> {
    let label = |action, plain: MsgKey, undo: MsgKey| match look(action, state) {
        Some(SwipeLook { icon, undo: true }) => MenuItem { action, label: undo, icon },
        Some(SwipeLook { icon, undo: false }) => MenuItem { action, label: plain, icon },
        None => MenuItem { action, label: plain, icon: "" },
    };
    let mut items = vec![
        label(EpisodeSwipeAction::ToggleSeen, MsgKey::action_mark_as_seen, MsgKey::action_mark_as_unseen),
        label(EpisodeSwipeAction::ToggleBookmark, MsgKey::action_bookmark_episode, MsgKey::action_remove_bookmark_episode),
        label(EpisodeSwipeAction::ToggleFillermark, MsgKey::action_fillermark_episode, MsgKey::action_remove_fillermark_episode),
    ];
    if has_download_handler {
        let icon = look(EpisodeSwipeAction::Download, state).map_or("", |l| l.icon);
        let label = match state.download {
            DownloadState::Downloaded => MsgKey::action_delete,
            DownloadState::Queued | DownloadState::Downloading => MsgKey::action_cancel,
            DownloadState::NotDownloaded | DownloadState::Error => MsgKey::action_download,
        };
        items.push(MenuItem { action: EpisodeSwipeAction::Download, label, icon });
    }
    items
}

/// The strip a drag of `dx` and `dy` reveals on the row while the finger or the pointer moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reveal {
    /// The side whose action the drag is heading for (the same side `side_of_drag` gives at release).
    pub side: Side,
    /// The width of the revealed strip, in pixels.
    pub width: i32,
    /// The drag is past the 56 px threshold: the strip is `primaryContainer`, and a release runs the action. Before
    /// that the strip is the dimmed background (`surfaceContainerLowest`).
    pub armed: bool,
}

/// The reveal of a drag in progress, or `None` when it is not a horizontal drag (a scroll) or has not moved.
pub fn reveal(dx: f64, dy: f64) -> Option<Reveal> {
    (dx != 0.0 && dx.abs() > dy.abs()).then(|| Reveal {
        side: if dx < 0.0 { Side::Left } else { Side::Right },
        width: dx.abs().round() as i32,
        armed: dx.abs() >= SWIPE_THRESHOLD_PX,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> EpisodeState {
        EpisodeState { seen: false, bookmark: false, fillermark: false, download: DownloadState::NotDownloaded }
    }

    #[test]
    fn each_toggle_changes_its_icon_and_turns_into_an_undo_when_set() {
        for action in [EpisodeSwipeAction::ToggleSeen, EpisodeSwipeAction::ToggleBookmark, EpisodeSwipeAction::ToggleFillermark] {
            let off = look(action, state()).expect("a look");
            let on = look(action, EpisodeState { seen: true, bookmark: true, fillermark: true, ..state() }).expect("a look");
            assert!(!off.undo && on.undo, "{action:?}");
            assert_ne!(off.icon, on.icon, "{action:?}");
        }
    }

    #[test]
    fn the_download_icon_follows_the_download_state_and_is_never_an_undo() {
        let icon = |download| look(EpisodeSwipeAction::Download, EpisodeState { download, ..state() }).expect("a look");
        assert_eq!(icon(DownloadState::NotDownloaded).icon, icon(DownloadState::Error).icon);
        assert_eq!(icon(DownloadState::Queued).icon, icon(DownloadState::Downloading).icon);
        assert_ne!(icon(DownloadState::NotDownloaded).icon, icon(DownloadState::Downloaded).icon);
        assert_ne!(icon(DownloadState::Queued).icon, icon(DownloadState::Downloaded).icon);
        assert!(!icon(DownloadState::Downloaded).undo);
    }

    #[test]
    fn disabled_draws_nothing() {
        assert_eq!(look(EpisodeSwipeAction::Disabled, state()), None);
    }

    #[test]
    fn the_menu_offers_the_four_actions_with_the_label_of_what_a_press_does() {
        let items = menu(state(), true);
        let actions: Vec<_> = items.iter().map(|i| i.action).collect();
        assert_eq!(actions, [EpisodeSwipeAction::ToggleSeen, EpisodeSwipeAction::ToggleBookmark, EpisodeSwipeAction::ToggleFillermark, EpisodeSwipeAction::Download]);
        assert_eq!(items[0].label, MsgKey::action_mark_as_seen);
        let seen = menu(EpisodeState { seen: true, bookmark: true, download: DownloadState::Downloaded, ..state() }, true);
        assert_eq!(seen[0].label, MsgKey::action_mark_as_unseen);
        assert_eq!(seen[1].label, MsgKey::action_remove_bookmark_episode);
        assert_eq!(seen[3].label, MsgKey::action_delete);
    }

    #[test]
    fn the_download_item_needs_a_download_handler() {
        assert_eq!(menu(state(), false).len(), 3);
    }

    #[test]
    fn the_reveal_grows_with_the_drag_and_arms_at_56_px() {
        assert_eq!(reveal(0.0, 0.0), None);
        assert_eq!(reveal(20.0, 40.0), None, "a vertical drag is a scroll");
        assert_eq!(reveal(30.0, 2.0), Some(Reveal { side: Side::Right, width: 30, armed: false }));
        assert_eq!(reveal(-56.0, 0.0), Some(Reveal { side: Side::Left, width: 56, armed: true }));
        assert!(!reveal(-55.9, 0.0).expect("a reveal").armed);
    }

    #[test]
    fn the_reveal_arms_exactly_where_the_release_runs_the_action() {
        for dx in [-80.0, -56.0, -55.0, 55.0, 56.0, 80.0] {
            let armed = reveal(dx, 0.0).is_some_and(|r| r.armed);
            assert_eq!(armed, crate::entry_core::swipe::side_of_drag(dx, 0.0).is_some(), "dx {dx}");
        }
    }
}
