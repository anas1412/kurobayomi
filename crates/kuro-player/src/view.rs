//! Display models for the player window (area 45 p2 T-45b-075, T-45b-078; area 46 T-46a-005,
//! T-46b-015). Pure: the GTK layer calls these on every `Store` change and writes the answers into
//! its widgets, so no widget keeps state of its own (no local duration, speed or aspect value).
//!
//! `PlayerView` replaces the mockup's constants. The quality, audio and subtitle sheets, the
//! auto-play switch geometry and the speed sheet each have a function here that the sheet body is
//! built from.

use crate::chapters::Segment;
use crate::chrome::{PrimarySlot, Transport, chapter_pill_visible, primary_slot, transport};
use crate::enums::VideoAspect;
use crate::hosters::{QualityKeys, group_hosters, is_flat};
use crate::seekbar_model::{duration_label, fraction, position_label, seek_segments};
use crate::speed::{self, SPEED_MAX, SPEED_MIN};
use crate::state::PlayerState;
use crate::time::pretty_time;
use crate::tracks::{TrackLists, VideoTrack, slot_label, track_label};
use crate::types::{HosterState, StateIcon, VideoState};

/// The icon of the play and pause button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayIcon {
    Play,
    Pause,
}

/// Preferences and window facts the view needs besides the state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewOptions {
    /// `invert_duration`.
    pub inverted_duration: bool,
    /// `pref_show_loading`.
    pub show_loading: bool,
    /// `pref_show_current_chapter`.
    pub show_current_chapter: bool,
    /// The stored aspect mode (`pref_player_aspect_state`).
    pub aspect: VideoAspect,
    /// A gesture seek is active, which hides previous and next.
    pub gesture_seek_active: bool,
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            inverted_duration: false,
            show_loading: true,
            show_current_chapter: true,
            aspect: VideoAspect::Fit,
            gesture_seek_active: false,
        }
    }
}

/// What the overlay widgets show for one `PlayerState`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerView {
    pub play_icon: PlayIcon,
    pub transport: Transport,
    /// `controls_locked`: only the unlock affordance shows.
    pub locked: bool,
    pub position: f32,
    pub duration: f32,
    /// The seek bar value in 0..=1.
    pub fraction: f32,
    pub position_label: String,
    pub duration_label: String,
    /// Chapter tick positions (seconds) on the seek bar.
    pub marks: Vec<f32>,
    /// `<mm:ss> • <name>`, or `None` when the button is hidden.
    pub chapter_label: Option<String>,
    /// The skip text wins over the custom primary button.
    pub primary: PrimarySlot,
    pub speed_label: String,
    pub aspect_label: String,
}

/// The chapter button text (U§1.3): the chapter start, a bullet and the name.
pub fn chapter_button_label(chapter: &Segment) -> String {
    format!(
        "{} \u{2022} {}",
        pretty_time(chapter.start as i64),
        chapter.name
    )
}

impl PlayerView {
    pub fn from_state(state: &PlayerState, options: &ViewOptions) -> Self {
        let chapter_starts: Vec<f32> = state.chapters.iter().map(|c| c.start).collect();
        Self {
            // Playing until mpv reports a pause (the Kotlin `_paused` starts false).
            play_icon: if state.paused == Some(true) {
                PlayIcon::Play
            } else {
                PlayIcon::Pause
            },
            transport: transport(
                state.has_prev,
                state.has_next,
                state.is_loading,
                options.show_loading,
                options.gesture_seek_active,
            ),
            locked: state.controls_locked,
            position: state.pos,
            duration: state.duration,
            fraction: fraction(state.pos, state.duration),
            position_label: position_label(state.pos),
            duration_label: duration_label(state.pos, state.duration, options.inverted_duration),
            marks: seek_segments(&chapter_starts, state.duration),
            chapter_label: state
                .current_chapter
                .as_ref()
                .filter(|_| {
                    chapter_pill_visible(
                        state.current_chapter.is_some(),
                        options.show_current_chapter,
                    )
                })
                .map(chapter_button_label),
            primary: primary_slot(
                state.skip_intro_text.as_deref(),
                state
                    .primary_button
                    .as_ref()
                    .map(|_| state.primary_button_title.as_str()),
            ),
            speed_label: speed::speed_label(state.playback_speed),
            aspect_label: crate::aspect::pill_text(options.aspect),
        }
    }
}

// Quality sheet (T-45b-078, U§1.7)

/// One video row of an expanded hoster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoRow {
    pub title: String,
    pub selected: bool,
    pub icon: Option<StateIcon>,
    /// An `Error` video is greyed and a click on it does nothing.
    pub clickable: bool,
}

/// How a hoster row draws, by its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HosterRowKind {
    /// A lazy hoster: tap to load.
    Idle,
    /// A spinner.
    Loading,
    /// The header with the video count, expandable, and the videos when expanded.
    Ready {
        expanded: bool,
        video_count: usize,
        videos: Vec<VideoRow>,
    },
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HosterRow {
    pub name: String,
    pub kind: HosterRowKind,
    /// `Error` and `Loading` rows are not clickable (parity mode, E-08).
    pub clickable: bool,
}

/// The quality sheet. `Flat` is the single `no_hoster_list` hoster: its videos show without a
/// header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QualityView {
    Flat(Vec<VideoRow>),
    Hosters(Vec<HosterRow>),
}

fn video_rows(
    state: &HosterState,
    hoster: usize,
    selected: Option<(usize, usize)>,
) -> Vec<VideoRow> {
    let HosterState::Ready(ready) = state else {
        return Vec::new();
    };
    ready
        .video_list
        .iter()
        .enumerate()
        .map(|(index, video)| {
            let state = state.video_state(index);
            VideoRow {
                title: video.video_title.clone(),
                selected: selected == Some((hoster, index)),
                icon: state.icon(),
                clickable: state != VideoState::Error,
            }
        })
        .collect()
}

/// The quality sheet body. `keys` holds the selection and the expansion by hoster name; the
/// failed and empty groups show only with their preferences (E58).
pub fn quality_view(
    states: &[HosterState],
    keys: &QualityKeys,
    show_failed: bool,
    show_empty: bool,
) -> QualityView {
    let selected = keys.selected_position(states);
    if is_flat(states) {
        return QualityView::Flat(video_rows(&states[0], 0, selected));
    }
    let groups = group_hosters(states);
    let shown = groups
        .valid
        .iter()
        .chain(groups.failed_shown(show_failed))
        .chain(groups.empty_shown(show_empty))
        .copied();
    let mut order: Vec<usize> = shown.collect();
    order.sort_unstable();
    let rows = order
        .into_iter()
        .map(|index| {
            let state = &states[index];
            let name = state.name().to_owned();
            let (kind, clickable) = match state {
                HosterState::Idle(_) => (HosterRowKind::Idle, true),
                HosterState::Loading(_) => (HosterRowKind::Loading, false),
                HosterState::Error(_) => (HosterRowKind::Error, false),
                HosterState::Ready(ready) => (
                    HosterRowKind::Ready {
                        expanded: keys.is_expanded(&name),
                        video_count: ready.video_list.len(),
                        videos: video_rows(state, index, selected),
                    },
                    true,
                ),
            };
            HosterRow {
                name,
                kind,
                clickable,
            }
        })
        .collect();
    QualityView::Hosters(rows)
}

// Audio and subtitle sheets (T-45b-078, U§1.8)

/// One row of the audio or subtitle sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackRow {
    pub id: i64,
    pub label: String,
    pub selected: bool,
    /// `#1` or `#2` for the subtitle slots.
    pub slot: Option<&'static str>,
}

/// The audio rows: "Off" first (it is in the list), the selected id checked.
pub fn audio_rows(tracks: &[VideoTrack], selected_audio: i64) -> Vec<TrackRow> {
    tracks
        .iter()
        .map(|t| TrackRow {
            id: t.id,
            label: track_label(t.id, &t.name, t.language.as_deref()),
            selected: t.id == selected_audio,
            slot: None,
        })
        .collect()
}

/// The subtitle rows: the primary is `#1`, the secondary `#2`.
pub fn subtitle_rows(tracks: &[VideoTrack], selected_sub: (i64, i64)) -> Vec<TrackRow> {
    tracks
        .iter()
        .map(|t| {
            let slot = slot_label(selected_sub, t.id);
            TrackRow {
                id: t.id,
                label: track_label(t.id, &t.name, t.language.as_deref()),
                selected: slot.is_some(),
                slot,
            }
        })
        .collect()
}

/// Both lists from the state, for a sheet that opens from the store.
pub fn track_rows(state: &PlayerState) -> (Vec<TrackRow>, Vec<TrackRow>) {
    (
        subtitle_rows(&state.sub_tracks, state.selected_sub),
        audio_rows(&state.audio_tracks, state.selected_audio),
    )
}

/// The lists as loaded, for tests and the first fill of the store.
pub fn lists_rows(lists: &TrackLists) -> (Vec<TrackRow>, Vec<TrackRow>) {
    (
        subtitle_rows(&lists.subtitles, (-1, -1)),
        audio_rows(&lists.audio, -1),
    )
}

// Auto-play switch (T-46a-005)

/// How long the thumb takes to slide.
pub const SWITCH_SLIDE_MS: u64 = 250;

/// The switch's size in logical pixels (48 x 24).
pub const SWITCH_SIZE: (f32, f32) = (48.0, 24.0);

/// The icon in the thumb and its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbStyle {
    /// `PlayCircle` on the primary colour.
    PlayOnPrimary,
    /// `PauseCircle` in white.
    PauseWhite,
}

impl ThumbStyle {
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::PlayOnPrimary => "play-circle",
            Self::PauseWhite => "pause-circle",
        }
    }
}

/// The geometry of the switch for a widget `width` by `height`: the track is 5/6 of the width and
/// 2/3 of the height, centred, with a fully rounded end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SwitchGeometry {
    pub track_width: f32,
    pub track_height: f32,
    pub track_x: f32,
    pub track_y: f32,
    /// Half the track height, which rounds both ends fully.
    pub track_radius: f32,
    pub thumb_size: f32,
}

pub fn switch_geometry(width: f32, height: f32) -> SwitchGeometry {
    let track_width = width * 5.0 / 6.0;
    let track_height = height * 2.0 / 3.0;
    SwitchGeometry {
        track_width,
        track_height,
        track_x: (width - track_width) / 2.0,
        track_y: (height - track_height) / 2.0,
        track_radius: track_height / 2.0,
        thumb_size: height,
    }
}

pub fn thumb_style(on: bool) -> ThumbStyle {
    if on {
        ThumbStyle::PlayOnPrimary
    } else {
        ThumbStyle::PauseWhite
    }
}

/// The thumb's left edge: `0` at the off end, `width - thumb_size` at the on end. `since_ms` is the
/// time since the toggle; the slide is linear and takes `SWITCH_SLIDE_MS`.
pub fn thumb_x(width: f32, height: f32, on: bool, since_ms: u64) -> f32 {
    let travel = (width - height).max(0.0);
    let t = (since_ms as f32 / SWITCH_SLIDE_MS as f32).clamp(0.0, 1.0);
    let progress = if on { t } else { 1.0 - t };
    travel * progress
}

// Playback speed sheet (T-46b-015)

/// A preset chip.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedChip {
    pub value: f32,
    pub label: String,
    pub selected: bool,
}

/// The speed sheet's slider and chips.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedSheet {
    pub min: f32,
    pub max: f32,
    pub label: &'static str,
    /// `%.2fx`.
    pub value_text: String,
    pub value: f32,
    pub chips: Vec<SpeedChip>,
}

/// The sheet for the current speed and the stored preset strings (`None` when never stored).
pub fn speed_sheet(current: f32, stored_presets: Option<&[String]>) -> SpeedSheet {
    SpeedSheet {
        min: SPEED_MIN,
        max: SPEED_MAX,
        label: "Speed",
        value_text: speed::speed_label(current),
        value: current,
        chips: speed::presets(stored_presets)
            .into_iter()
            .map(|value| SpeedChip {
                value,
                label: speed::speed_label(value),
                selected: value == current,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ChapterType, NO_HOSTER_LIST, ReadyHoster, Video};

    fn video(title: &str) -> Video {
        Video {
            video_title: title.to_owned(),
            ..Video::default()
        }
    }

    fn ready(name: &str, videos: &[(&str, VideoState)]) -> HosterState {
        HosterState::Ready(ReadyHoster {
            name: name.to_owned(),
            video_list: videos.iter().map(|(t, _)| video(t)).collect(),
            video_state: videos.iter().map(|(_, s)| *s).collect(),
        })
    }

    fn segment(start: f32, name: &str) -> Segment {
        Segment {
            index: 0,
            name: name.to_owned(),
            start,
            kind: ChapterType::Other,
        }
    }

    #[test]
    fn the_view_follows_the_state_not_constants() {
        let mut state = PlayerState::default();
        state.pos = 90.0;
        state.duration = 1420.0;
        state.paused = Some(true);
        state.has_prev = false;
        state.has_next = true;
        state.is_loading = false;
        state.controls_locked = true;
        state.playback_speed = 1.5;
        state.skip_intro_text = Some("Skip Opening".to_owned());
        state.chapters = vec![segment(0.0, "Intro"), segment(85.0, "Part A")];
        state.current_chapter = Some(segment(85.0, "Part A"));
        let view = PlayerView::from_state(&state, &ViewOptions::default());
        assert_eq!(view.play_icon, PlayIcon::Play);
        assert!(!view.transport.prev_enabled && view.transport.next_enabled);
        assert!(view.locked);
        assert_eq!(view.position_label, "01:30");
        assert_eq!(view.duration_label, "23:40");
        assert!((view.fraction - 90.0 / 1420.0).abs() < 1e-6);
        assert_eq!(view.marks, vec![0.0, 85.0]);
        assert_eq!(view.chapter_label.as_deref(), Some("01:25 \u{2022} Part A"));
        assert_eq!(view.primary, PrimarySlot::Skip("Skip Opening".to_owned()));
        assert_eq!(view.speed_label, "1.50x");
        assert_eq!(view.aspect_label, "Fit to screen");
    }

    #[test]
    fn playing_shows_pause_and_the_chapter_button_hides_without_a_chapter() {
        let state = PlayerState::default();
        let view = PlayerView::from_state(&state, &ViewOptions::default());
        assert_eq!(view.play_icon, PlayIcon::Pause);
        assert_eq!(view.chapter_label, None);
        assert_eq!(view.duration_label, "--:--");
        let mut chaptered = PlayerState::default();
        chaptered.current_chapter = Some(segment(0.0, "A"));
        let hidden = ViewOptions {
            show_current_chapter: false,
            ..ViewOptions::default()
        };
        assert_eq!(
            PlayerView::from_state(&chaptered, &hidden).chapter_label,
            None
        );
    }

    #[test]
    fn loading_replaces_play_with_the_spinner_when_the_preference_is_on() {
        let state = PlayerState::default();
        assert!(state.is_loading);
        let view = PlayerView::from_state(&state, &ViewOptions::default());
        assert!(view.transport.spinner_visible);
        let off = ViewOptions {
            show_loading: false,
            ..ViewOptions::default()
        };
        assert!(
            PlayerView::from_state(&state, &off)
                .transport
                .play_pause_visible
        );
    }

    #[test]
    fn hoster_rows_render_idle_loading_ready_and_error() {
        let states = vec![
            HosterState::Idle("lazy".into()),
            HosterState::Loading("busy".into()),
            ready(
                "good",
                &[("1080p", VideoState::Ready), ("720p", VideoState::Error)],
            ),
            HosterState::Error("dead".into()),
        ];
        let mut keys = QualityKeys::default();
        keys.select("good", 0);
        keys.toggle_expanded("good");
        let QualityView::Hosters(rows) = quality_view(&states, &keys, true, true) else {
            panic!("expected grouped hosters");
        };
        let by_name = |n: &str| rows.iter().find(|r| r.name == n).unwrap();
        assert_eq!(by_name("lazy").kind, HosterRowKind::Idle);
        assert!(by_name("lazy").clickable);
        assert_eq!(by_name("busy").kind, HosterRowKind::Loading);
        assert!(!by_name("busy").clickable);
        assert_eq!(by_name("dead").kind, HosterRowKind::Error);
        assert!(!by_name("dead").clickable, "an Error row is not clickable");
        let HosterRowKind::Ready {
            expanded,
            video_count,
            videos,
        } = &by_name("good").kind
        else {
            panic!("expected a ready hoster");
        };
        assert!(*expanded);
        assert_eq!(*video_count, 2);
        assert!(videos[0].selected && videos[0].clickable && videos[0].icon.is_none());
        assert!(!videos[1].selected && !videos[1].clickable);
        assert_eq!(videos[1].icon, Some(StateIcon::Error));
    }

    #[test]
    fn failed_and_empty_hosters_show_only_with_their_preferences() {
        let states = vec![
            ready("ok", &[("a", VideoState::Queue)]),
            HosterState::Error("dead".into()),
            ready("empty", &[]),
        ];
        let keys = QualityKeys::default();
        let names =
            |show_failed, show_empty| match quality_view(&states, &keys, show_failed, show_empty) {
                QualityView::Hosters(rows) => rows.into_iter().map(|r| r.name).collect::<Vec<_>>(),
                QualityView::Flat(_) => panic!("grouped"),
            };
        assert_eq!(names(false, false), ["ok"]);
        assert_eq!(names(true, false), ["ok", "dead"]);
        assert_eq!(names(true, true), ["ok", "dead", "empty"]);
    }

    #[test]
    fn a_single_ready_no_hoster_list_is_flat() {
        let states = vec![ready(
            NO_HOSTER_LIST,
            &[("a", VideoState::Ready), ("b", VideoState::Queue)],
        )];
        let mut keys = QualityKeys::default();
        keys.select(NO_HOSTER_LIST, 1);
        let QualityView::Flat(rows) = quality_view(&states, &keys, false, false) else {
            panic!("expected the flat list");
        };
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].selected && rows[1].selected);
    }

    #[test]
    fn track_rows_carry_the_selected_markers() {
        let tracks = vec![
            VideoTrack {
                id: -1,
                name: "Off".into(),
                language: None,
            },
            VideoTrack {
                id: 1,
                name: "Japanese".into(),
                language: Some("jpn".into()),
            },
            VideoTrack {
                id: 2,
                name: "English".into(),
                language: Some("eng".into()),
            },
        ];
        let audio = audio_rows(&tracks, 2);
        assert_eq!(audio[0].label, "Off");
        assert!(!audio[0].selected && !audio[1].selected && audio[2].selected);
        let subs = subtitle_rows(&tracks[1..], (2, 1));
        assert_eq!(subs[0].slot, Some("#2"));
        assert_eq!(subs[1].slot, Some("#1"));
        assert_eq!(subs[1].label, "#2: English (eng)");
        let none = subtitle_rows(&tracks[1..], (-1, -1));
        assert!(none.iter().all(|r| !r.selected && r.slot.is_none()));
    }

    #[test]
    fn the_switch_track_is_five_sixths_by_two_thirds_with_a_round_end() {
        let g = switch_geometry(48.0, 24.0);
        assert_eq!((g.track_width, g.track_height), (40.0, 16.0));
        assert_eq!(g.track_radius, 8.0);
        assert_eq!((g.track_x, g.track_y), (4.0, 4.0));
        assert_eq!(g.thumb_size, 24.0);
    }

    #[test]
    fn the_thumb_style_and_slide() {
        assert_eq!(thumb_style(true), ThumbStyle::PlayOnPrimary);
        assert_eq!(thumb_style(false), ThumbStyle::PauseWhite);
        assert_eq!(ThumbStyle::PlayOnPrimary.icon_name(), "play-circle");
        // Turning on slides from the left end to the right end in 250 ms.
        assert_eq!(thumb_x(48.0, 24.0, true, 0), 0.0);
        assert_eq!(thumb_x(48.0, 24.0, true, 125), 12.0);
        assert_eq!(thumb_x(48.0, 24.0, true, 250), 24.0);
        assert_eq!(thumb_x(48.0, 24.0, true, 900), 24.0);
        // Turning off slides back.
        assert_eq!(thumb_x(48.0, 24.0, false, 0), 24.0);
        assert_eq!(thumb_x(48.0, 24.0, false, 250), 0.0);
    }

    #[test]
    fn the_speed_sheet_binds_range_label_value_and_chips() {
        let sheet = speed_sheet(1.25, None);
        assert_eq!((sheet.min, sheet.max), (0.01, 6.0));
        assert_eq!(sheet.label, "Speed");
        assert_eq!(sheet.value_text, "1.25x");
        assert_eq!(sheet.chips.len(), 12);
        assert_eq!(sheet.chips[4].label, "1.25x");
        assert!(sheet.chips[4].selected);
        assert_eq!(sheet.chips.iter().filter(|c| c.selected).count(), 1);
        let stored = vec!["2.0".to_owned(), "bad".to_owned(), "0.5".to_owned()];
        let custom = speed_sheet(0.5, Some(&stored));
        let values: Vec<f32> = custom.chips.iter().map(|c| c.value).collect();
        assert_eq!(values, vec![0.5, 2.0]);
    }
}
