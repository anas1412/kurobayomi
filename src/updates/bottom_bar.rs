//! The Updates bottom action bar (T-42a-036). Pure: which actions show for the selection (behaviour 2.6, the `takeIf`
//! predicates of the Kotlin bar), their labels and icons, and the "N selected" text. The bar widget lives in
//! `updates_screen` and calls these.

use kuro_i18n::MsgKey;

use crate::updates::bulk::BulkAction;
use crate::updates::download_actions::DownloadAction;
use crate::updates::download_state::DownloadState;

/// The actions the bar can show, in the order the Kotlin bars list them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAction {
    Bookmark,
    RemoveBookmark,
    Fillermark,
    RemoveFillermark,
    MarkRead,
    MarkUnread,
    Download,
    Delete,
    PlayExternally,
    PlayInternally,
}

/// The selected unit as the predicates see it. `read` is the read flag (manga) or the seen flag (anime).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selected {
    pub read: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    /// `last_page_read` (manga) or `last_second_seen` (anime), in ms for anime.
    pub progress: i64,
    pub state: DownloadState,
}

/// The actions that show for `selected` (empty selection shows none). `always_external` is the player preference
/// used for the two play actions, which show only for a single unit.
pub fn visible_actions(anime: bool, selected: &[Selected], always_external: bool) -> Vec<BarAction> {
    if selected.is_empty() {
        return Vec::new();
    }
    let any = |f: fn(&Selected) -> bool| selected.iter().any(f);
    let all = |f: fn(&Selected) -> bool| selected.iter().all(f);
    let mut out = Vec::new();
    if any(|s| !s.bookmark) {
        out.push(BarAction::Bookmark);
    }
    if all(|s| s.bookmark) {
        out.push(BarAction::RemoveBookmark);
    }
    if anime {
        if any(|s| !s.fillermark) {
            out.push(BarAction::Fillermark);
        }
        if all(|s| s.fillermark) {
            out.push(BarAction::RemoveFillermark);
        }
    }
    if any(|s| !s.read) {
        out.push(BarAction::MarkRead);
    }
    if any(|s| s.read || s.progress > 0) {
        out.push(BarAction::MarkUnread);
    }
    if any(|s| s.state != DownloadState::Downloaded) {
        out.push(BarAction::Download);
    }
    if any(|s| s.state == DownloadState::Downloaded) {
        out.push(BarAction::Delete);
    }
    if anime && selected.len() == 1 {
        out.push(if always_external { BarAction::PlayInternally } else { BarAction::PlayExternally });
    }
    out
}

/// What a bar action does when pressed: a batch write, a download action over the selection, or opening the single
/// selected unit in a player. Both play actions open with `alt` (the player that is not the preferred one).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarCommand {
    Bulk(BulkAction),
    Download(DownloadAction),
    Play { alt: bool },
}

/// The command of a bar action. Delete maps to the download action; the screen asks to confirm first (T-42a-032).
pub fn command(action: BarAction) -> BarCommand {
    match action {
        BarAction::Bookmark => BarCommand::Bulk(BulkAction::Bookmark(true)),
        BarAction::RemoveBookmark => BarCommand::Bulk(BulkAction::Bookmark(false)),
        BarAction::Fillermark => BarCommand::Bulk(BulkAction::Fillermark(true)),
        BarAction::RemoveFillermark => BarCommand::Bulk(BulkAction::Fillermark(false)),
        BarAction::MarkRead => BarCommand::Bulk(BulkAction::MarkRead(true)),
        BarAction::MarkUnread => BarCommand::Bulk(BulkAction::MarkRead(false)),
        BarAction::Download => BarCommand::Download(DownloadAction::Start),
        BarAction::Delete => BarCommand::Download(DownloadAction::Delete),
        BarAction::PlayExternally | BarAction::PlayInternally => BarCommand::Play { alt: true },
    }
}

/// Whether the play action opens the external player: the player preference inverted by `alt` (E-U35, behaviour 12.5).
pub fn play_opens_external(always_external: bool, alt: bool) -> bool {
    always_external ^ alt
}

/// The label key of an action for the kind (manga "chapter" words, anime "episode" and "watched" words).
pub fn label(action: BarAction, anime: bool) -> MsgKey {
    match (action, anime) {
        (BarAction::Bookmark, false) => MsgKey::action_bookmark,
        (BarAction::Bookmark, true) => MsgKey::action_bookmark_episode,
        (BarAction::RemoveBookmark, false) => MsgKey::action_remove_bookmark,
        (BarAction::RemoveBookmark, true) => MsgKey::action_remove_bookmark_episode,
        (BarAction::Fillermark, _) => MsgKey::action_fillermark_episode,
        (BarAction::RemoveFillermark, _) => MsgKey::action_remove_fillermark_episode,
        (BarAction::MarkRead, false) => MsgKey::action_mark_as_read,
        (BarAction::MarkRead, true) => MsgKey::action_mark_as_seen,
        (BarAction::MarkUnread, false) => MsgKey::action_mark_as_unread,
        (BarAction::MarkUnread, true) => MsgKey::action_mark_as_unseen,
        (BarAction::Download, _) => MsgKey::action_download,
        (BarAction::Delete, _) => MsgKey::action_delete,
        (BarAction::PlayExternally, _) => MsgKey::action_play_externally,
        (BarAction::PlayInternally, _) => MsgKey::action_play_internally,
    }
}

/// The GTK symbolic icon of an action.
pub fn icon(action: BarAction) -> &'static str {
    match action {
        BarAction::Bookmark => "starred-symbolic",
        BarAction::RemoveBookmark => "non-starred-symbolic",
        BarAction::Fillermark => "tag-symbolic",
        BarAction::RemoveFillermark => "tag-outline-symbolic",
        BarAction::MarkRead => "object-select-symbolic",
        BarAction::MarkUnread => "edit-undo-symbolic",
        BarAction::Download => "folder-download-symbolic",
        BarAction::Delete => "user-trash-symbolic",
        BarAction::PlayExternally => "document-open-symbolic",
        BarAction::PlayInternally => "media-playback-start-symbolic",
    }
}

/// "N selected" (the count label next to the close button). English only until the key is added (T-42a-062).
pub fn selected_text(count: usize) -> String {
    format!("{count} selected")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(read: bool, bookmark: bool, progress: i64, state: DownloadState) -> Selected {
        Selected { read, bookmark, fillermark: false, progress, state }
    }

    #[test]
    fn bottom_bar_empty_selection_shows_nothing() {
        assert!(visible_actions(false, &[], false).is_empty());
    }

    #[test]
    fn bottom_bar_manga_predicates_follow_the_selection() {
        let mixed = [unit(true, false, 0, DownloadState::Downloaded), unit(false, true, 0, DownloadState::NotDownloaded)];
        assert_eq!(
            visible_actions(false, &mixed, false),
            vec![BarAction::Bookmark, BarAction::MarkRead, BarAction::MarkUnread, BarAction::Download, BarAction::Delete]
        );
        let all_read_bookmarked = [unit(true, true, 0, DownloadState::Downloaded)];
        assert_eq!(
            visible_actions(false, &all_read_bookmarked, false),
            vec![BarAction::RemoveBookmark, BarAction::MarkUnread, BarAction::Delete]
        );
    }

    #[test]
    fn bottom_bar_unread_shows_for_a_started_unit() {
        let started = [unit(false, true, 4, DownloadState::NotDownloaded)];
        let actions = visible_actions(false, &started, false);
        assert!(actions.contains(&BarAction::MarkUnread), "unread with progress > 0 counts (EC-42-U-13)");
        assert!(actions.contains(&BarAction::MarkRead));
    }

    #[test]
    fn bottom_bar_anime_fillermark_and_single_player_action() {
        let one = [Selected { read: false, bookmark: true, fillermark: true, progress: 0, state: DownloadState::Queue }];
        let actions = visible_actions(true, &one, false);
        assert!(actions.contains(&BarAction::RemoveFillermark));
        assert!(!actions.contains(&BarAction::Fillermark));
        assert!(actions.contains(&BarAction::PlayExternally));
        assert!(!actions.contains(&BarAction::PlayInternally));
        assert!(visible_actions(true, &one, true).contains(&BarAction::PlayInternally));
        let two = [one[0], one[0]];
        assert!(!visible_actions(true, &two, false).contains(&BarAction::PlayExternally), "players need one unit");
    }

    #[test]
    fn bottom_bar_labels_and_count_text() {
        assert_eq!(label(BarAction::Bookmark, true), MsgKey::action_bookmark_episode);
        assert_eq!(label(BarAction::MarkRead, true), MsgKey::action_mark_as_seen);
        assert_eq!(selected_text(3), "3 selected");
        assert!(!icon(BarAction::Delete).is_empty());
    }

    #[test]
    fn bottom_bar_every_action_has_a_command() {
        use BarAction::*;
        for action in [Bookmark, RemoveBookmark, Fillermark, RemoveFillermark, MarkRead, MarkUnread, Download, Delete, PlayExternally, PlayInternally] {
            let _ = command(action);
        }
        assert_eq!(command(Download), BarCommand::Download(DownloadAction::Start));
        assert_eq!(command(Delete), BarCommand::Download(DownloadAction::Delete));
        assert_eq!(command(PlayExternally), BarCommand::Play { alt: true });
        assert_eq!(command(PlayInternally), BarCommand::Play { alt: true });
        assert_eq!(command(MarkUnread), BarCommand::Bulk(BulkAction::MarkRead(false)));
    }

    #[test]
    fn bottom_bar_player_route_truth_table() {
        assert!(!play_opens_external(false, false));
        assert!(play_opens_external(false, true));
        assert!(play_opens_external(true, false));
        assert!(!play_opens_external(true, true));
    }
}
