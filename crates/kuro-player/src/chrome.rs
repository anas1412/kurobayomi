//! Routing and gating of the control rows (area 46 `TopRightPlayerControls.kt`,
//! `MiddlePlayerControls.kt`, `BottomLeftPlayerControls.kt`, `BottomRightPlayerControls.kt`;
//! T-46a-004, T-46a-006, T-46a-008, T-46a-009). Pure: the GTK rows read these answers and draw
//! the widgets. Widths, fonts and icons stay in the GTK layer.

use crate::enums::VideoAspect;

/// Buttons on the top-right row, in draw order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopRightButton {
    AutoPlay,
    Subtitles,
    Audio,
    Quality,
    More,
}

/// What a press on a top-right button opens or toggles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopRightAction {
    ToggleAutoPlay,
    SubtitleTracksSheet,
    SubtitleSettingsPanel,
    AudioSheet,
    AudioDelayPanel,
    QualitySheet,
    MoreSheet,
    VideoFiltersPanel,
}

/// Short press and long press routes of `TopRightPlayerControls.kt`. Quality opens the same sheet
/// on both presses.
pub fn top_right_action(button: TopRightButton, long_press: bool) -> TopRightAction {
    match (button, long_press) {
        (TopRightButton::AutoPlay, _) => TopRightAction::ToggleAutoPlay,
        (TopRightButton::Subtitles, false) => TopRightAction::SubtitleTracksSheet,
        (TopRightButton::Subtitles, true) => TopRightAction::SubtitleSettingsPanel,
        (TopRightButton::Audio, false) => TopRightAction::AudioSheet,
        (TopRightButton::Audio, true) => TopRightAction::AudioDelayPanel,
        (TopRightButton::Quality, _) => TopRightAction::QualitySheet,
        (TopRightButton::More, false) => TopRightAction::MoreSheet,
        (TopRightButton::More, true) => TopRightAction::VideoFiltersPanel,
    }
}

/// The quality button shows only for an online episode. An offline or unknown state hides it.
pub fn quality_button_visible(online: Option<bool>) -> bool {
    online == Some(true)
}

/// The centre transport row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transport {
    pub prev_enabled: bool,
    pub next_enabled: bool,
    pub play_pause_visible: bool,
    pub spinner_visible: bool,
    /// Previous and next are hidden while a gesture seek is active.
    pub nav_visible: bool,
}

/// Computes the centre row. While loading with `pref_show_loading`, the spinner replaces play and
/// pause. Previous and next are disabled, not hidden, at either end of the list.
pub fn transport(
    has_prev: bool,
    has_next: bool,
    loading: bool,
    show_loading_pref: bool,
    gesture_seek_active: bool,
) -> Transport {
    let spinner_visible = loading && show_loading_pref;
    Transport {
        prev_enabled: has_prev,
        next_enabled: has_next,
        play_pause_visible: !spinner_visible,
        spinner_visible,
        nav_visible: !gesture_seek_active,
    }
}

/// The chapter pill on the bottom-left row shows only when there is a current chapter and the
/// preference is on.
pub fn chapter_pill_visible(has_current_chapter: bool, show_current_chapter: bool) -> bool {
    has_current_chapter && show_current_chapter
}

/// The primary slot on the bottom-right row. The skip-intro text wins, then the custom primary
/// button, then nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrimarySlot {
    Skip(String),
    Custom(String),
    Empty,
}

pub fn primary_slot(skip_text: Option<&str>, custom_title: Option<&str>) -> PrimarySlot {
    match (skip_text, custom_title) {
        (Some(text), _) => PrimarySlot::Skip(text.to_owned()),
        (None, Some(title)) => PrimarySlot::Custom(title.to_owned()),
        (None, None) => PrimarySlot::Empty,
    }
}

/// The picture-in-picture button shows only when the mini-player exists on this platform.
pub fn pip_button_visible(mini_player_available: bool) -> bool {
    mini_player_available
}

/// Next aspect for the aspect button: Fit, then Stretch, then Crop, then Fit.
pub fn next_aspect(current: VideoAspect) -> VideoAspect {
    match current {
        VideoAspect::Fit => VideoAspect::Stretch,
        VideoAspect::Stretch => VideoAspect::Crop,
        VideoAspect::Crop => VideoAspect::Fit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitles_and_audio_and_more_split_on_long_press() {
        assert_eq!(
            top_right_action(TopRightButton::Subtitles, false),
            TopRightAction::SubtitleTracksSheet
        );
        assert_eq!(
            top_right_action(TopRightButton::Subtitles, true),
            TopRightAction::SubtitleSettingsPanel
        );
        assert_eq!(
            top_right_action(TopRightButton::Audio, true),
            TopRightAction::AudioDelayPanel
        );
        assert_eq!(
            top_right_action(TopRightButton::More, true),
            TopRightAction::VideoFiltersPanel
        );
    }

    #[test]
    fn quality_opens_the_same_sheet_on_both_presses() {
        assert_eq!(
            top_right_action(TopRightButton::Quality, false),
            TopRightAction::QualitySheet
        );
        assert_eq!(
            top_right_action(TopRightButton::Quality, true),
            TopRightAction::QualitySheet
        );
        assert!(quality_button_visible(Some(true)));
        assert!(!quality_button_visible(Some(false)));
        assert!(!quality_button_visible(None));
    }

    #[test]
    fn spinner_replaces_play_only_with_the_loading_preference() {
        let on = transport(true, true, true, true, false);
        assert!(on.spinner_visible && !on.play_pause_visible);
        let off = transport(true, true, true, false, false);
        assert!(!off.spinner_visible && off.play_pause_visible);
    }

    #[test]
    fn nav_hides_during_gesture_seek_and_follows_neighbours() {
        let t = transport(false, true, false, true, true);
        assert!(!t.nav_visible);
        assert!(!t.prev_enabled);
        assert!(t.next_enabled);
    }

    #[test]
    fn chapter_pill_needs_a_chapter_and_the_preference() {
        assert!(chapter_pill_visible(true, true));
        assert!(!chapter_pill_visible(false, true));
        assert!(!chapter_pill_visible(true, false));
    }

    #[test]
    fn primary_slot_prefers_skip_then_custom() {
        assert_eq!(
            primary_slot(Some("Skip intro"), Some("X")),
            PrimarySlot::Skip("Skip intro".into())
        );
        assert_eq!(
            primary_slot(None, Some("X")),
            PrimarySlot::Custom("X".into())
        );
        assert_eq!(primary_slot(None, None), PrimarySlot::Empty);
    }

    #[test]
    fn aspect_cycles_fit_stretch_crop() {
        assert_eq!(next_aspect(VideoAspect::Fit), VideoAspect::Stretch);
        assert_eq!(next_aspect(VideoAspect::Stretch), VideoAspect::Crop);
        assert_eq!(next_aspect(VideoAspect::Crop), VideoAspect::Fit);
        assert!(pip_button_visible(true) && !pip_button_visible(false));
    }
}
