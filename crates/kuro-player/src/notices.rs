//! Desktop notices of the local servers (area 45 `HttpServerService.kt` and `TorrentServerService.kt`, T-45a-071 and
//! T-45a-079). Pure: each function builds the `Notice` that the app sends through `kuro-platform`. The action names are
//! routed back to the server that owns the notice. Android notification ids and channel names are not reused: the
//! channels are the ones in `kuro_platform::notifications::CATEGORIES`.

use kuro_platform::notifier::{APP_NAME, Action, Notice};

use crate::strings::english;

/// Replaces an earlier notice of the same server.
pub const HTTP_SERVER_NOTICE_ID: &str = "http_server_running";
pub const TORRENT_SERVER_NOTICE_ID: &str = "torrent_server_running";
/// Application actions. The app stops the server that owns the notice.
pub const ACTION_HTTP_SERVER_CLOSE: &str = "http_server.close";
pub const ACTION_TORRENT_SERVER_STOP: &str = "torrent_server.stop";

/// What the app does with a server notice: show it, or withdraw the notice with this id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoticeOp {
    Show(Notice),
    Withdraw(&'static str),
}

const HTTP_SERVER_CHANNEL: &str = "http_server_channel";
const TORRENT_SERVER_CHANNEL: &str = "torrent_server_channel";

fn text(key: &str) -> String {
    english(key).unwrap_or_default().to_owned()
}

/// The notice shown while the local HTTP server runs: `Http server is running` with a `Close` button (T-45a-071).
pub fn http_server_running() -> Notice {
    Notice {
        id: HTTP_SERVER_NOTICE_ID.to_owned(),
        channel: HTTP_SERVER_CHANNEL.to_owned(),
        title: text("http_server_is_running"),
        text: String::new(),
        progress: None,
        actions: vec![Action {
            label: text("action_close"),
            action: ACTION_HTTP_SERVER_CLOSE.to_owned(),
        }],
    }
}

/// The notice shown while the torrent server runs: the app name as the title, `Torrent Server is running` as the text,
/// and a `Stop` button (T-45a-079).
pub fn torrent_server_running() -> Notice {
    Notice {
        id: TORRENT_SERVER_NOTICE_ID.to_owned(),
        channel: TORRENT_SERVER_CHANNEL.to_owned(),
        title: APP_NAME.to_owned(),
        text: text("torrentserver_is_running"),
        progress: None,
        actions: vec![Action {
            label: text("action_stop"),
            action: ACTION_TORRENT_SERVER_STOP.to_owned(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::notifications::category;

    use super::*;

    #[test]
    fn the_http_notice_says_the_server_runs_and_closes_it() {
        let notice = http_server_running();
        assert_eq!(notice.title, "Http server is running");
        assert_eq!(notice.channel, "http_server_channel");
        assert!(
            category(&notice.channel).is_some(),
            "the channel exists in the category table"
        );
        assert_eq!(
            notice.actions,
            vec![Action {
                label: "Close".into(),
                action: ACTION_HTTP_SERVER_CLOSE.into()
            }]
        );
    }

    #[test]
    fn the_torrent_notice_is_titled_with_the_app_name_and_stops_the_server() {
        let notice = torrent_server_running();
        assert_eq!(notice.title, APP_NAME);
        assert_eq!(notice.text, "Torrent Server is running");
        assert_eq!(notice.channel, "torrent_server_channel");
        assert!(category(&notice.channel).is_some());
        assert_eq!(
            notice.actions,
            vec![Action {
                label: "Stop".into(),
                action: ACTION_TORRENT_SERVER_STOP.into()
            }]
        );
    }

    #[test]
    fn the_two_notices_have_their_own_ids() {
        assert_ne!(http_server_running().id, torrent_server_running().id);
    }
}
