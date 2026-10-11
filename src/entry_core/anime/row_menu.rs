//! The right-click menu of an episode row (T-39a-017): which items it offers and the icon and label of each, from the
//! state of the episode. Upstream: `AnimeEpisodeListItem.kt` and p1-behavior 3.6. Pure; the widget
//! is `row_view`.

use kuro_i18n::MsgKey;

use super::actions::EpisodeMenuAction;

/// The download state of an episode as the menu reads it (`AnimeDownload.State`). The page knows `Downloaded` and
/// `NotDownloaded`; the other three are for a queue that reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadState {
    NotDownloaded,
    Queued,
    Downloading,
    Downloaded,
    Error,
}

/// The state of one episode that the four items read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpisodeState {
    pub seen: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    pub download: DownloadState,
}

/// One item of the row's context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub action: EpisodeMenuAction,
    pub label: MsgKey,
    pub icon: &'static str,
}

/// The four items, with the icon and label of what a press does now (Mark as seen or unseen, and so on). Download
/// shows only when the app can queue downloads.
pub fn menu(state: EpisodeState, has_download_handler: bool) -> Vec<MenuItem> {
    let toggle = |action, on: bool, icons: (&'static str, &'static str), labels: (MsgKey, MsgKey)| {
        MenuItem { action, label: if on { labels.1 } else { labels.0 }, icon: if on { icons.1 } else { icons.0 } }
    };
    let mut items = vec![
        toggle(EpisodeMenuAction::ToggleSeen, state.seen, ("object-select-symbolic", "edit-undo-symbolic"), (MsgKey::action_mark_as_seen, MsgKey::action_mark_as_unseen)),
        toggle(
            EpisodeMenuAction::ToggleBookmark,
            state.bookmark,
            ("bookmark-new-symbolic", "list-remove-symbolic"),
            (MsgKey::action_bookmark_episode, MsgKey::action_remove_bookmark_episode),
        ),
        toggle(
            EpisodeMenuAction::ToggleFillermark,
            state.fillermark,
            ("tag-symbolic", "edit-clear-symbolic"),
            (MsgKey::action_fillermark_episode, MsgKey::action_remove_fillermark_episode),
        ),
    ];
    if has_download_handler {
        let (icon, label) = match state.download {
            DownloadState::Downloaded => ("user-trash-symbolic", MsgKey::action_delete),
            DownloadState::Queued | DownloadState::Downloading => ("process-stop-symbolic", MsgKey::action_cancel),
            DownloadState::NotDownloaded | DownloadState::Error => ("folder-download-symbolic", MsgKey::action_download),
        };
        items.push(MenuItem { action: EpisodeMenuAction::Download, label, icon });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> EpisodeState {
        EpisodeState { seen: false, bookmark: false, fillermark: false, download: DownloadState::NotDownloaded }
    }

    #[test]
    fn each_toggle_changes_its_icon_when_set() {
        let off = menu(state(), false);
        let on = menu(EpisodeState { seen: true, bookmark: true, fillermark: true, ..state() }, false);
        for (off, on) in off.iter().zip(&on) {
            assert_ne!(off.icon, on.icon, "{:?}", off.action);
            assert_ne!(off.label, on.label, "{:?}", off.action);
        }
    }

    #[test]
    fn the_download_icon_follows_the_download_state() {
        let icon = |download| menu(EpisodeState { download, ..state() }, true)[3].icon;
        assert_eq!(icon(DownloadState::NotDownloaded), icon(DownloadState::Error));
        assert_eq!(icon(DownloadState::Queued), icon(DownloadState::Downloading));
        assert_ne!(icon(DownloadState::NotDownloaded), icon(DownloadState::Downloaded));
        assert_ne!(icon(DownloadState::Queued), icon(DownloadState::Downloaded));
    }

    #[test]
    fn the_menu_offers_the_four_actions_with_the_label_of_what_a_press_does() {
        let items = menu(state(), true);
        let actions: Vec<_> = items.iter().map(|i| i.action).collect();
        assert_eq!(actions, [EpisodeMenuAction::ToggleSeen, EpisodeMenuAction::ToggleBookmark, EpisodeMenuAction::ToggleFillermark, EpisodeMenuAction::Download]);
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
}
