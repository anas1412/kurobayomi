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

/// The primary slot on the bottom-right row: the skip-intro text, or nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrimarySlot {
    Skip(String),
    Empty,
}

pub fn primary_slot(skip_text: Option<&str>) -> PrimarySlot {
    match skip_text {
        Some(text) => PrimarySlot::Skip(text.to_owned()),
        None => PrimarySlot::Empty,
    }
}

/// The picture-in-picture button: shown only when `pref_enable_pip` is on, and disabled while a
/// new episode is loading (PC:508).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PipButton {
    pub visible: bool,
    pub enabled: bool,
}

pub fn pip_button(pref_enable_pip: bool, loading: bool) -> PipButton {
    PipButton {
        visible: pref_enable_pip,
        enabled: !loading,
    }
}

/// The chapter text `"<mm:ss> • <name>"` of the chapter pill and the chapter button (T-46a-052,
/// T-45b-076). The time is the chapter start in whole seconds.
pub fn chapter_text(start_secs: f64, name: &str) -> String {
    format!(
        "{} • {name}",
        crate::numfmt::format_elapsed(start_secs as i64)
    )
}

/// Direction of the pill text animation when the chapter changes: forward slides up, backward
/// slides down, and no change (or the first chapter shown) does not animate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChapterSlide {
    Up,
    Down,
    None,
}

pub fn chapter_slide(previous: Option<usize>, current: usize) -> ChapterSlide {
    match previous {
        Some(prev) if current > prev => ChapterSlide::Up,
        Some(prev) if current < prev => ChapterSlide::Down,
        _ => ChapterSlide::None,
    }
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
    fn primary_slot_shows_the_skip_text_or_nothing() {
        assert_eq!(
            primary_slot(Some("Skip intro")),
            PrimarySlot::Skip("Skip intro".into())
        );
        assert_eq!(primary_slot(None), PrimarySlot::Empty);
    }

    #[test]
    fn aspect_cycles_fit_stretch_crop() {
        assert_eq!(next_aspect(VideoAspect::Fit), VideoAspect::Stretch);
        assert_eq!(next_aspect(VideoAspect::Stretch), VideoAspect::Crop);
        assert_eq!(next_aspect(VideoAspect::Crop), VideoAspect::Fit);
    }

    #[test]
    fn chapter_text_is_time_bullet_name() {
        assert_eq!(chapter_text(85.0, "Intro"), "01:25 • Intro");
        assert_eq!(chapter_text(3725.0, "Credits"), "1:02:05 • Credits");
    }

    #[test]
    fn chapter_change_slides_by_direction() {
        assert_eq!(chapter_slide(Some(1), 2), ChapterSlide::Up);
        assert_eq!(chapter_slide(Some(2), 1), ChapterSlide::Down);
        assert_eq!(chapter_slide(Some(2), 2), ChapterSlide::None);
        assert_eq!(chapter_slide(None, 0), ChapterSlide::None);
    }
}
