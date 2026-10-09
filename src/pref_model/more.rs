//! More tab (Kotlin `MoreScreen`): logo, two switches, the moved tab, download queue, library links,
//! settings links and Help. Navigation goes to the caller through `Target`, except Help, which opens a URL.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Image, ListBoxRow, Orientation, Separator};
use kuro_i18n::{MsgKey, tr, tr_plural};
use kuro_prefs::prefs::Prefs;
use kuro_prefs::ui::{NavStyle, Tab};

use super::bind::accept;
use super::model::ItemCommon;
use super::widgets::row::{self, PrefRow};
use super::widgets::{switch, text, HELP_URL, PREF_HORIZONTAL_PADDING};

/// Space above and below the 64 px logo, before the separator.
const LOGO_VERTICAL_MARGIN: i32 = 56;

/// Where a More row leads. Help is not a target: it opens `HELP_URL` directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The tab moved into More by the nav style.
    Moved(Tab),
    DownloadQueue,
    Categories,
    Statistics,
    DataAndStorage,
    Settings,
    PlayerSettings,
    About,
}

/// Title of the moved tab's row (the tab's own name).
pub fn moved_title(tab: Tab) -> String {
    tr(match tab {
        Tab::AnimeLibrary => MsgKey::label_anime_library,
        Tab::MangaLibrary => MsgKey::label_manga_library,
        Tab::Updates => MsgKey::label_recent_updates,
        Tab::History => MsgKey::history,
        Tab::Browse => MsgKey::browse,
        Tab::More => MsgKey::label_more,
    })
}

/// The download queue as the More tab sees it (`DownloadQueueState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueState {
    Stopped,
    Paused { pending: i64 },
    Downloading { pending: i64 },
}

/// The queue row's subtitle: none when stopped, "Paused" (with the count when items wait) when paused, the count when
/// downloading. The count is the string table's plural, one format string with the number in it.
pub fn queue_subtitle(state: QueueState) -> Option<String> {
    let remaining = |pending: i64| tr_plural(MsgKey::download_queue_summary, pending, &[]);
    match state {
        QueueState::Stopped => None,
        QueueState::Paused { pending: 0 } => Some(tr(MsgKey::paused)),
        QueueState::Paused { pending } => Some(format!("{} • {}", tr(MsgKey::paused), remaining(pending))),
        QueueState::Downloading { pending } => Some(remaining(pending)),
    }
}

pub fn moved_icon(style: NavStyle) -> &'static str {
    match style {
        NavStyle::MOVE_MANGA_TO_MORE => "bookmark-new-symbolic",
        NavStyle::MOVE_UPDATES_TO_MORE | NavStyle::MOVE_HISTORY_TO_MORE => "document-open-recent-symbolic",
        NavStyle::MOVE_BROWSE_TO_MORE => "compass-symbolic",
    }
}

/// A built More page. The download queue subtitle changes live through `set_queue_subtitle`.
pub struct MorePage {
    pub widget: gtk::Widget,
    queue: PrefRow,
}

impl MorePage {
    pub fn set_queue_subtitle(&self, subtitle: Option<&str>) {
        row::set_subtitle(&self.queue.subtitle, subtitle);
    }

    /// Shows the queue state (stopped, paused, downloading) in the row's subtitle.
    pub fn set_queue_state(&self, state: QueueState) {
        self.set_queue_subtitle(queue_subtitle(state).as_deref());
    }
}

/// Builds the page. `queue_subtitle` is the initial queue state text (from the queue combiner).
pub fn page(prefs: &Prefs, queue_subtitle: Option<&str>, on_target: Rc<dyn Fn(Target)>) -> MorePage {
    let column = GBox::new(Orientation::Vertical, 0);
    column.append(&logo());

    let group_one = GBox::new(Orientation::Vertical, 0);
    group_one.append(&switch_row(
        ItemCommon::new(tr(MsgKey::label_downloaded_only)).with_subtitle(&tr(MsgKey::downloaded_only_summary)).with_icon("network-offline-symbolic"),
        prefs.base.downloaded_only.clone(),
    ));
    group_one.append(&switch_row(
        ItemCommon::new(tr(MsgKey::pref_incognito_mode)).with_subtitle(&tr(MsgKey::pref_incognito_mode_summary)).with_icon("dialog-password-symbolic"),
        prefs.base.incognito_mode.clone(),
    ));
    column.append(&group_one);
    column.append(&divider());

    let nav = prefs.ui.nav_style.get();
    let tab = nav.more_tab();
    let group_two = GBox::new(Orientation::Vertical, 0);
    group_two.append(&nav_row(&moved_title(tab), moved_icon(nav), Target::Moved(tab), &on_target));
    let queue = row::build(row::RowSpec {
        title: &tr(MsgKey::label_download_queue),
        subtitle: queue_subtitle,
        icon: Some("folder-download-symbolic"),
        trailing: None,
        on_click: Some(target_click(Target::DownloadQueue, &on_target)),
    });
    group_two.append(&queue.row);
    group_two.append(&nav_row(&tr(MsgKey::general_categories), "tag-symbolic", Target::Categories, &on_target));
    group_two.append(&nav_row(&tr(MsgKey::label_stats), "utilities-system-monitor-symbolic", Target::Statistics, &on_target));
    group_two.append(&nav_row(&tr(MsgKey::label_data_storage), "drive-harddisk-symbolic", Target::DataAndStorage, &on_target));
    column.append(&group_two);
    column.append(&divider());

    let group_three = GBox::new(Orientation::Vertical, 0);
    group_three.append(&nav_row(&tr(MsgKey::label_settings), "emblem-system-symbolic", Target::Settings, &on_target));
    group_three.append(&nav_row(&tr(MsgKey::label_player_settings), "video-display-symbolic", Target::PlayerSettings, &on_target));
    group_three.append(&nav_row(&tr(MsgKey::pref_category_about), "dialog-information-symbolic", Target::About, &on_target));
    group_three.append(&help_row());
    column.append(&group_three);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_child(Some(&column));
    MorePage { widget: scroll.upcast(), queue }
}

fn target_click(target: Target, on_target: &Rc<dyn Fn(Target)>) -> Rc<dyn Fn()> {
    let on_target = on_target.clone();
    Rc::new(move || on_target(target))
}

fn nav_row(title: &str, icon: &'static str, target: Target, on_target: &Rc<dyn Fn(Target)>) -> ListBoxRow {
    let common = ItemCommon::new(title).with_icon(icon);
    text::build(&common, Some(target_click(target, on_target)), None).row
}

fn switch_row(common: ItemCommon, pref: kuro_prefs::pref::Pref<bool>) -> ListBoxRow {
    switch::build(&common, pref, accept()).row
}

fn help_row() -> ListBoxRow {
    let common = ItemCommon::new(tr(MsgKey::label_help)).with_icon("help-about-symbolic");
    let on_click: Rc<dyn Fn()> = Rc::new(|| open_help());
    text::build(&common, Some(on_click), None).row
}

/// Opens the help page in the default browser. A failure is logged and nothing else happens.
fn open_help() {
    let launcher = gtk::UriLauncher::new(HELP_URL);
    launcher.launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
        if let Err(err) = result {
            tracing::warn!(%err, "could not open the help page");
        }
    });
}

fn logo() -> GBox {
    let logo = GBox::new(Orientation::Vertical, 0);
    let icon = Image::from_icon_name("applications-multimedia-symbolic");
    icon.set_pixel_size(64);
    icon.set_focusable(false);
    icon.set_accessible_role(gtk::AccessibleRole::Presentation);
    icon.set_halign(Align::Center);
    icon.set_margin_top(LOGO_VERTICAL_MARGIN);
    icon.set_margin_bottom(LOGO_VERTICAL_MARGIN);
    logo.append(&icon);
    logo.append(&Separator::new(Orientation::Horizontal));
    logo
}

fn divider() -> Separator {
    let separator = Separator::new(Orientation::Horizontal);
    separator.set_margin_start(PREF_HORIZONTAL_PADDING);
    separator.set_margin_end(PREF_HORIZONTAL_PADDING);
    separator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_moved_tab_is_never_a_visible_destination() {
        for style in NavStyle::ALL {
            let moved = style.more_tab();
            assert!(!style.tabs().contains(&moved));
        }
    }

    #[test]
    fn default_nav_style_moves_history() {
        assert_eq!(NavStyle::MOVE_HISTORY_TO_MORE.more_tab(), Tab::History);
        assert_eq!(moved_title(Tab::History), "History");
    }

    #[test]
    fn the_queue_subtitle_is_one_plural_format_with_the_number_in_it() {
        assert_eq!(queue_subtitle(QueueState::Stopped), None);
        assert_eq!(queue_subtitle(QueueState::Downloading { pending: 1 }).as_deref(), Some("1 remaining"));
        assert_eq!(queue_subtitle(QueueState::Downloading { pending: 12 }).as_deref(), Some("12 remaining"));
        assert_eq!(queue_subtitle(QueueState::Paused { pending: 0 }).as_deref(), Some("Paused"));
        assert_eq!(queue_subtitle(QueueState::Paused { pending: 3 }).as_deref(), Some("Paused • 3 remaining"));
    }

    #[test]
    fn every_row_title_comes_from_the_string_table() {
        for tab in [Tab::AnimeLibrary, Tab::MangaLibrary, Tab::Updates, Tab::History, Tab::Browse, Tab::More] {
            assert!(!moved_title(tab).is_empty());
        }
        assert_eq!(moved_title(Tab::AnimeLibrary), "Anime");
        assert_eq!(moved_title(Tab::MangaLibrary), "Manga");
    }

    #[test]
    fn help_points_at_the_troubleshooting_page() {
        assert_eq!(HELP_URL, "https://aniyomi.org/docs/guides/troubleshooting/");
    }
}
