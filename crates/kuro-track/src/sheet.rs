//! Logic of the per-entry track sheet (`AnimeTrackInfoDialogHome.kt`, behaviour 13). The GUI draws the card; this
//! decides the overflow menu and its order, the copy toast, the private badge and toggle, and which detail cells
//! exist.

use kuro_platform::opener::Opener;

use crate::item::TrackItem;
use crate::messages::MsgKey;
use crate::tracker::Tracker;

/// Items of the overflow menu, in display order (behaviour 13.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackMenuItem {
    OpenInBrowser,
    CopyLink,
    /// `to_private` is the state the item switches to: "Track privately" when public, "Track publicly" when
    /// private.
    TogglePrivate {
        to_private: bool,
    },
    /// Opens the tracker search again for this entry (T-28a-031).
    SearchAgain,
    Remove,
}

impl TrackMenuItem {
    /// Message key of the item label.
    pub fn label_key(self) -> &'static str {
        match self {
            TrackMenuItem::OpenInBrowser => "action_open_in_browser",
            TrackMenuItem::CopyLink => "action_copy_link",
            TrackMenuItem::TogglePrivate { to_private: true } => "action_toggle_private_on",
            TrackMenuItem::TogglePrivate { to_private: false } => "action_toggle_private_off",
            TrackMenuItem::SearchAgain => "action_search_again",
            TrackMenuItem::Remove => "action_remove",
        }
    }

    /// English label, the fallback text.
    pub fn label(self) -> &'static str {
        match self {
            TrackMenuItem::OpenInBrowser => "Open in browser",
            TrackMenuItem::CopyLink => "Copy link",
            TrackMenuItem::TogglePrivate { to_private: true } => "Track privately",
            TrackMenuItem::TogglePrivate { to_private: false } => "Track publicly",
            TrackMenuItem::SearchAgain => "Search again",
            TrackMenuItem::Remove => "Remove",
        }
    }
}

/// The overflow menu of a bound card. The toggle is present only for trackers with private support. The menu
/// closes after each item.
pub fn track_menu(supports_private: bool, private: bool) -> Vec<TrackMenuItem> {
    let mut items = vec![TrackMenuItem::OpenInBrowser, TrackMenuItem::CopyLink];
    if supports_private {
        items.push(TrackMenuItem::TogglePrivate {
            to_private: !private,
        });
    }
    items.push(TrackMenuItem::SearchAgain);
    items.push(TrackMenuItem::Remove);
    items
}

/// The toast after a copy action: "Copied to clipboard:" and the content, truncated (TT-27-218).
pub fn copy_toast(content: &str) -> MsgKey {
    MsgKey::copied(content)
}

/// The badge drawn on the logo of a private binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivateBadge {
    /// Eye-off icon.
    pub icon: &'static str,
    pub size_px: u32,
    /// Horizontal offset from the logo, in pixels.
    pub offset_x: i32,
    pub label_key: &'static str,
}

/// The badge for a binding, shown only when it is private (TT-27-205).
pub fn private_badge(private: bool) -> Option<PrivateBadge> {
    private.then_some(PrivateBadge {
        icon: "eye-off",
        size_px: 14,
        offset_x: -5,
        label_key: "tracked_privately",
    })
}

/// Score cell is shown only when the tracker has a score list (TT-27-203).
pub fn score_cell_shown(score_list: &[String]) -> bool {
    !score_list.is_empty()
}

/// Start and finish cells are shown only for trackers that support reading dates (TT-27-204, behaviour 13.2).
pub fn date_cells_shown(tracker: &dyn Tracker) -> bool {
    tracker.supports_reading_dates()
}

/// Opacity of an unset value's text, the placeholder in the detail cells (TT-27-201).
pub const UNSET_TEXT_ALPHA: f32 = 0.5;

/// Side of the logo tile in pixels (`TrackLogoIcon.kt`, T-27a-050).
pub const LOGO_TILE_PX: u32 = 48;

/// Inner padding of the logo tile in pixels.
pub const LOGO_PADDING_PX: u32 = 4;

/// The letter drawn on the tile when the tracker has no logo asset: the tracker's first character, uppercased.
pub fn logo_fallback_letter(name: &str) -> Option<char> {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().next().unwrap_or(c))
}

/// The logo tile of a tracker (`TrackLogoIcon.kt`): [`LOGO_TILE_PX`] square with [`LOGO_PADDING_PX`] inside, filled
/// with the tracker's colour (`0xRRGGBB`). `letter` is drawn when the logo asset is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogoTile {
    pub color: u32,
    pub size_px: u32,
    pub padding_px: u32,
    pub letter: Option<char>,
}

pub fn logo_tile(tracker: &dyn Tracker, has_logo_asset: bool) -> LogoTile {
    LogoTile {
        color: tracker.logo_color(),
        size_px: LOGO_TILE_PX,
        padding_px: LOGO_PADDING_PX,
        letter: if has_logo_asset {
            None
        } else {
            logo_fallback_letter(tracker.name())
        },
    }
}

/// The two gestures of a logo tile. They never share an action: a click opens the page, a long press copies the link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoGesture {
    Click,
    LongPress,
}

/// What a logo gesture did.
#[derive(Debug, Clone, PartialEq)]
pub enum LogoEffect {
    /// The page was handed to the opener.
    Opened,
    /// The text for the clipboard and the confirmation toast.
    Copied(String, MsgKey),
    /// A long press on a blank link copies nothing.
    Nothing,
}

/// Runs `gesture` on the stored `url` (B§3.6, B§3.7).
pub fn logo_gesture(
    gesture: LogoGesture,
    url: &str,
    opener: &dyn Opener,
) -> Result<LogoEffect, MsgKey> {
    match gesture {
        LogoGesture::Click => open_tracker_page(url, opener).map(|()| LogoEffect::Opened),
        LogoGesture::LongPress => Ok(copy_link(url)
            .map_or(LogoEffect::Nothing, |(text, toast)| {
                LogoEffect::Copied(text, toast)
            })),
    }
}

/// Opacity of a detail cell's text: [`UNSET_TEXT_ALPHA`] while the value is unset, full otherwise (TT-27-201).
pub fn cell_alpha(is_set: bool) -> f32 {
    if is_set { 1.0 } else { UNSET_TEXT_ALPHA }
}

/// Progress text of a detail cell: "12" when the total is unknown, "12 / 24" when it is known (TT-27-202). The stored
/// fraction is shown truncated.
pub fn progress_text(progress: f64, total: i64) -> String {
    let current = progress as i64;
    if total > 0 {
        format!("{current} / {total}")
    } else {
        current.to_string()
    }
}

/// Score text is hidden for a score of exactly 0.0 (B§11, E-21).
pub fn score_text_shown(score: f64) -> bool {
    score != 0.0
}

/// A start or finish date shows only when the tracker reads dates and the stored value is not 0 (B§11).
pub fn date_shown(tracker: &dyn Tracker, stored_ms: i64) -> bool {
    tracker.supports_reading_dates() && stored_ms != 0
}

/// What a Home row shows. An unbound row ("Add tracking") has nothing else (B§11, U§2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowActions {
    pub add_tracking: bool,
    pub status: bool,
    pub progress: bool,
    pub score: bool,
    pub start_date: bool,
    pub finish_date: bool,
    pub private_toggle: bool,
    pub menu: bool,
}

/// The row gating of one Home item.
pub fn row_actions(item: &TrackItem) -> RowActions {
    let Some(record) = &item.record else {
        return RowActions {
            add_tracking: true,
            status: false,
            progress: false,
            score: false,
            start_date: false,
            finish_date: false,
            private_toggle: false,
            menu: false,
        };
    };
    let tracker: &dyn Tracker = item.tracker.as_ref();
    RowActions {
        add_tracking: false,
        status: true,
        progress: true,
        score: score_cell_shown(tracker.score_list()) && score_text_shown(record.score()),
        start_date: date_shown(tracker, record.start_date()),
        finish_date: date_shown(tracker, record.finish_date()),
        private_toggle: tracker.supports_private_tracking(),
        menu: true,
    }
}

/// Opens the stored tracker page (B§3.6). A blank link opens nothing and gives the toast (E-43).
pub fn open_tracker_page(url: &str, opener: &dyn Opener) -> Result<(), MsgKey> {
    if url.trim().is_empty() {
        return Err(MsgKey::NoTrackingLink);
    }
    opener
        .open_target(url)
        .map_err(|e| MsgKey::Raw(e.to_string()))
}

/// The copy action (B§3.7, E-44, E-45). Returns the full URL for the clipboard and the truncated confirmation. A
/// blank link copies nothing and returns `None`.
pub fn copy_link(url: &str) -> Option<(String, MsgKey)> {
    (!url.trim().is_empty()).then(|| (url.to_owned(), copy_toast(url)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeAnime, MemSecrets, new_log};
    use std::sync::{Arc, Mutex};

    use crate::item::TrackItem;
    use crate::model::TrackerId;
    use crate::tracker::Tracker;

    #[test]
    fn tr_27_049_menu_order_with_private_support() {
        let items = track_menu(true, false);
        assert_eq!(
            items,
            vec![
                TrackMenuItem::OpenInBrowser,
                TrackMenuItem::CopyLink,
                TrackMenuItem::TogglePrivate { to_private: true },
                TrackMenuItem::SearchAgain,
                TrackMenuItem::Remove,
            ]
        );
        let labels: Vec<&str> = items.iter().map(|i| i.label()).collect();
        assert_eq!(
            labels,
            [
                "Open in browser",
                "Copy link",
                "Track privately",
                "Search again",
                "Remove"
            ]
        );
    }

    #[test]
    fn tr_27_063_toggle_label_follows_the_private_state_and_is_hidden_without_support() {
        assert_eq!(track_menu(true, true)[2].label(), "Track publicly");
        assert_eq!(track_menu(true, false)[2].label(), "Track privately");
        let without = track_menu(false, false);
        assert_eq!(
            without.len(),
            4,
            "no toggle for trackers without private support"
        );
        assert!(
            !without
                .iter()
                .any(|i| matches!(i, TrackMenuItem::TogglePrivate { .. }))
        );
        assert_eq!(without.last(), Some(&TrackMenuItem::Remove));
    }

    #[test]
    fn tr_27_218_copy_link_gives_the_copied_toast() {
        let text = english_of(copy_toast("https://example.test/1"));
        assert!(text.starts_with("Copied to clipboard:\n"), "{text}");
        assert!(text.ends_with("https://example.test/1"));
    }

    #[test]
    fn tr_27_063_badge_only_when_private() {
        let badge = private_badge(true).unwrap();
        assert_eq!(
            (badge.icon, badge.size_px, badge.offset_x),
            ("eye-off", 14, -5)
        );
        assert_eq!(badge.label_key, "tracked_privately");
        assert!(private_badge(false).is_none());
    }

    #[test]
    fn tr_27_053_and_064_cells_follow_the_tracker_capabilities() {
        assert!(!score_cell_shown(&[]));
        assert!(score_cell_shown(&["1".to_string()]));
        let plain = FakeAnime::new(TrackerId::KITSU, "Kitsu", MemSecrets::new(), new_log());
        assert!(!date_cells_shown(&plain), "reading dates default to false");
        assert_eq!(UNSET_TEXT_ALPHA, 0.5);
    }

    fn bound(tracker: Arc<dyn Tracker>, score: f64, start: i64) -> TrackItem {
        let mut row = crate::fakes::anime_binding(1, tracker.id(), 2.0);
        row.score = score;
        row.start_date = start;
        TrackItem {
            tracker,
            record: Some(crate::record::TrackRecord::from(row)),
        }
    }

    #[test]
    fn tr_27b_006_an_unbound_row_shows_only_add_tracking() {
        let t: Arc<dyn Tracker> = Arc::new(FakeAnime::new(
            TrackerId::ANILIST,
            "AniList",
            MemSecrets::new(),
            new_log(),
        ));
        let row = row_actions(&TrackItem {
            tracker: t,
            record: None,
        });
        assert!(row.add_tracking);
        assert!(!row.status && !row.progress && !row.score && !row.start_date);
        assert!(!row.finish_date && !row.private_toggle && !row.menu);
    }

    #[test]
    fn tr_27b_006_a_bound_row_hides_a_zero_score_and_unsupported_dates_and_private() {
        let t: Arc<dyn Tracker> = Arc::new(FakeAnime::new(
            TrackerId::KITSU,
            "Kitsu",
            MemSecrets::new(),
            new_log(),
        ));
        let zero = row_actions(&bound(Arc::clone(&t), 0.0, 0));
        assert!(zero.status && zero.progress && zero.menu);
        assert!(!zero.score, "a 0.0 score is hidden");
        assert!(
            !zero.start_date && !zero.finish_date,
            "dates need reading-date support"
        );
        assert!(!zero.private_toggle, "private needs private support");
        assert!(row_actions(&bound(t, 8.0, 0)).score);
    }

    #[test]
    fn tr_27b_006_dates_and_private_follow_the_tracker_support_and_stored_value() {
        let mut fake = FakeAnime::new(TrackerId::ANILIST, "AniList", MemSecrets::new(), new_log());
        fake.reading_dates = true;
        fake.private_support = true;
        let t: Arc<dyn Tracker> = Arc::new(fake);
        let unset = row_actions(&bound(Arc::clone(&t), 0.0, 0));
        assert!(!unset.start_date, "a stored 0 is not shown");
        let set = row_actions(&bound(t, 0.0, 1_000));
        assert!(set.start_date);
        assert!(set.private_toggle);
    }

    struct RecordingOpener(Mutex<Vec<String>>);

    impl Opener for RecordingOpener {
        fn open_target(&self, target: &str) -> std::io::Result<()> {
            self.0.lock().unwrap().push(target.to_owned());
            Ok(())
        }
    }

    #[test]
    fn tr_27b_015_a_blank_link_calls_no_opener_and_shows_the_toast() {
        let opener = RecordingOpener(Mutex::new(Vec::new()));
        assert_eq!(
            open_tracker_page("  ", &opener),
            Err(MsgKey::NoTrackingLink)
        );
        assert!(opener.0.lock().unwrap().is_empty());
        assert_eq!(open_tracker_page("https://x.test/1", &opener), Ok(()));
        assert_eq!(
            *opener.0.lock().unwrap(),
            vec!["https://x.test/1".to_owned()]
        );
        assert_eq!(
            english_of(MsgKey::NoTrackingLink),
            "No link stored for this entry"
        );
    }

    #[test]
    fn tr_27a_050_the_tile_uses_the_tracker_colour_and_a_letter_only_without_a_logo() {
        let tracker = FakeAnime::new(TrackerId::ANILIST, "AniList", MemSecrets::new(), new_log());
        let with_logo = logo_tile(&tracker, true);
        assert_eq!(
            (with_logo.size_px, with_logo.padding_px, with_logo.letter),
            (48, 4, None)
        );
        assert_eq!(with_logo.color, tracker.logo_color());
        assert_eq!(logo_tile(&tracker, false).letter, Some('A'));
        assert_eq!(cell_alpha(false), UNSET_TEXT_ALPHA);
        assert_eq!(cell_alpha(true), 1.0);
    }

    #[test]
    fn tr_27a_050_click_opens_and_long_press_copies_and_the_two_never_cross() {
        let opener = RecordingOpener(Mutex::new(Vec::new()));
        assert_eq!(
            logo_gesture(LogoGesture::LongPress, "https://x.test/1", &opener).map(|e| matches!(
                e,
                LogoEffect::Copied(ref text, _) if text == "https://x.test/1"
            )),
            Ok(true)
        );
        assert!(
            opener.0.lock().unwrap().is_empty(),
            "a long press opens nothing"
        );
        assert_eq!(
            logo_gesture(LogoGesture::Click, "https://x.test/1", &opener),
            Ok(LogoEffect::Opened)
        );
        assert_eq!(
            logo_gesture(LogoGesture::LongPress, " ", &opener),
            Ok(LogoEffect::Nothing)
        );
    }

    #[test]
    fn tr_27b_016_copy_copies_the_full_url_and_toasts_the_truncated_one() {
        assert_eq!(copy_link(""), None, "E-44: a blank link copies nothing");
        let url = format!("https://x.test/{}", "a".repeat(120));
        let (text, toast) = copy_link(&url).unwrap();
        assert_eq!(text, url);
        let MsgKey::CopiedToClipboard(shown) = toast else {
            panic!("expected the copy toast");
        };
        assert_eq!(
            shown.chars().count(),
            49,
            "E-45: truncateCenter(50) gives 23 + '...' + 23"
        );
        assert!(shown.contains("..."));
    }

    fn english_of(key: MsgKey) -> String {
        crate::messages::english(&key)
    }

    #[test]
    fn tr_27a_050_the_tile_is_48_pixels_with_4_pixels_of_padding_and_a_letter_fallback() {
        assert_eq!((LOGO_TILE_PX, LOGO_PADDING_PX), (48, 4));
        assert_eq!(logo_fallback_letter("kitsu"), Some('K'));
        assert_eq!(logo_fallback_letter(""), None);
    }

    #[test]
    fn tr_27a_048_progress_text_is_plain_or_a_fraction_of_the_total() {
        assert_eq!(progress_text(12.0, 24), "12 / 24");
        assert_eq!(
            progress_text(12.9, 0),
            "12",
            "an unknown total shows the number alone"
        );
        assert_eq!(progress_text(0.0, 0), "0");
    }
}

#[cfg(test)]
mod mal_tests {
    use std::sync::Arc;

    use super::*;
    use crate::fakes::MemSecrets;
    use crate::mal::store::{SecretTokenStore, TokenStore};
    use crate::mal::tracker::MalTracker;

    #[test]
    fn t_29a_050_the_mal_menu_has_no_private_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let tokens: Arc<dyn TokenStore> = Arc::new(SecretTokenStore::new(MemSecrets::new()));
        let tracker = MalTracker::new(dir.path(), tokens, Some("cid".into())).expect("tracker");
        let supports = crate::tracker::Tracker::supports_private_tracking(&tracker);
        assert!(!supports);
        for private in [false, true] {
            let items = track_menu(supports, private);
            assert!(
                !items
                    .iter()
                    .any(|item| matches!(item, TrackMenuItem::TogglePrivate { .. })),
                "no private toggle for MAL (private={private})"
            );
        }
    }
}
