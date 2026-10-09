//! Track selection values for `aid`, `sid` and `secondary-sid` (area 45, T-45a-015). mpv writes
//! `no` for a disabled track and `auto` while it chooses one; the app uses `-1` for disabled.

/// Track id meaning "disabled" (`no` in mpv).
pub const TRACK_NONE: i64 = -1;

/// Reads a track property. `no`, a missing value and a non-integer all give `TRACK_NONE`.
pub fn track_id_from_value(value: Option<&str>) -> i64 {
    value
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(TRACK_NONE)
}

/// Reads a track change event. `auto` is ignored (`None`), so no selection update happens.
pub fn track_change(value: &str) -> Option<i64> {
    match value {
        "auto" => None,
        other => Some(track_id_from_value(Some(other))),
    }
}

/// What to write for a track id: `no` for `TRACK_NONE`, the integer otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackValue {
    No,
    Id(i64),
}

impl TrackValue {
    pub fn for_id(id: i64) -> Self {
        if id == TRACK_NONE {
            Self::No
        } else {
            Self::Id(id)
        }
    }
}

/// `VideoTrack(id, name, language)` in the player's track lists (T-45b-016). The "Off" entry has
/// no language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoTrack {
    pub id: i64,
    pub name: String,
    pub language: Option<String>,
}

/// Read access to mpv's `track-list/*` properties (D§7.1). `count` is `None` when the mpv handle
/// is gone (the Kotlin NPE, E-42); a missing count is `Some(0)`.
pub trait TrackProps {
    fn count(&self) -> Option<i64>;
    fn kind(&self, index: i64) -> Option<String>;
    fn lang(&self, index: i64) -> Option<String>;
    fn title(&self, index: i64) -> Option<String>;
    fn id(&self, index: i64) -> Option<i64>;
}

/// The audio and subtitle lists of the loaded file. The audio list starts with "Off".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackLists {
    pub subtitles: Vec<VideoTrack>,
    pub audio: Vec<VideoTrack>,
}

/// `loadTracks()` (behaviour 5.1). A track with an unknown type, or without an id, is skipped.
/// A gone mpv handle clears both lists.
pub fn load_track_lists(props: &impl TrackProps) -> TrackLists {
    let Some(count) = props.count() else {
        return TrackLists {
            subtitles: Vec::new(),
            audio: Vec::new(),
        };
    };
    let mut lists = TrackLists {
        subtitles: Vec::new(),
        audio: vec![VideoTrack {
            id: TRACK_NONE,
            name: "Off".to_owned(),
            language: None,
        }],
    };
    for index in 0..count {
        let Some(kind) = props.kind(index) else {
            continue;
        };
        let Some(id) = props.id(index) else {
            continue;
        };
        let track = VideoTrack {
            id,
            name: props.title(index).unwrap_or_default(),
            language: Some(props.lang(index).unwrap_or_default()),
        };
        match kind.as_str() {
            "sub" => lists.subtitles.push(track),
            "audio" => lists.audio.push(track),
            _ => {}
        }
    }
    lists
}

/// The default tracks chosen after load (`onFinishLoadingTracks`, behaviour 5.2). `None` means
/// that property is not written. `secondary_sid` is only written together with `sid`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DefaultTracks {
    pub sid: Option<i64>,
    pub secondary_sid: Option<i64>,
    pub aid: Option<i64>,
}

/// Picks the default tracks. `prefer` is the TrackSelect seam: given a list and whether it is the
/// subtitle list, it returns the index of the preferred track. Subtitles fall back to the first
/// track. Audio falls back to index 1, the first real track after "Off".
pub fn default_tracks(
    lists: &TrackLists,
    prefer: impl Fn(&[VideoTrack], bool) -> Option<usize>,
) -> DefaultTracks {
    let mut out = DefaultTracks::default();
    let subtitle_index =
        prefer(&lists.subtitles, true).or((!lists.subtitles.is_empty()).then_some(0));
    if let Some(track) = subtitle_index.and_then(|i| lists.subtitles.get(i)) {
        out.sid = Some(track.id);
        out.secondary_sid = Some(TRACK_NONE);
    }
    let audio_index = prefer(&lists.audio, false).or((lists.audio.len() > 1).then_some(1));
    out.aid = audio_index
        .and_then(|i| lists.audio.get(i))
        .map(|track| track.id);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_maps_no_and_garbage_to_minus_one() {
        assert_eq!(track_id_from_value(Some("no")), -1);
        assert_eq!(track_id_from_value(Some("abc")), -1);
        assert_eq!(track_id_from_value(None), -1);
        assert_eq!(track_id_from_value(Some("3")), 3);
    }

    #[test]
    fn writing_minus_one_is_no_and_other_ids_are_ints() {
        assert_eq!(TrackValue::for_id(-1), TrackValue::No);
        assert_eq!(TrackValue::for_id(2), TrackValue::Id(2));
    }

    #[test]
    fn auto_change_is_ignored() {
        assert_eq!(track_change("auto"), None);
        assert_eq!(track_change("no"), Some(-1));
        assert_eq!(track_change("4"), Some(4));
    }

    /// (type, id, lang, title) of one track. A `None` type means the entry is missing.
    type Row = (
        Option<&'static str>,
        Option<i64>,
        Option<&'static str>,
        Option<&'static str>,
    );

    struct FakeTracks {
        count: Option<i64>,
        rows: Vec<Row>,
    }

    impl TrackProps for FakeTracks {
        fn count(&self) -> Option<i64> {
            self.count
        }
        fn kind(&self, index: i64) -> Option<String> {
            self.rows[index as usize].0.map(str::to_owned)
        }
        fn lang(&self, index: i64) -> Option<String> {
            self.rows[index as usize].2.map(str::to_owned)
        }
        fn title(&self, index: i64) -> Option<String> {
            self.rows[index as usize].3.map(str::to_owned)
        }
        fn id(&self, index: i64) -> Option<i64> {
            self.rows[index as usize].1
        }
    }

    fn sample() -> FakeTracks {
        FakeTracks {
            count: Some(4),
            rows: vec![
                (Some("video"), Some(1), None, None),
                (Some("audio"), Some(2), Some("jpn"), Some("Main")),
                (Some("sub"), Some(3), Some("eng"), None),
                (Some("sub"), Some(4), None, Some("Signs")),
            ],
        }
    }

    #[test]
    fn audio_list_starts_with_off_and_subtitles_do_not() {
        let lists = load_track_lists(&sample());
        assert_eq!(
            lists.audio[0],
            VideoTrack {
                id: -1,
                name: "Off".into(),
                language: None
            }
        );
        assert_eq!(lists.audio.len(), 2);
        assert_eq!(lists.audio[1].id, 2);
        assert_eq!(lists.audio[1].language.as_deref(), Some("jpn"));
        assert_eq!(lists.subtitles.len(), 2);
        assert!(lists.subtitles.iter().all(|t| t.id != -1));
    }

    #[test]
    fn missing_title_and_language_default_to_empty() {
        let lists = load_track_lists(&sample());
        assert_eq!(lists.subtitles[0].name, "");
        assert_eq!(lists.subtitles[0].language.as_deref(), Some("eng"));
        assert_eq!(lists.subtitles[1].language.as_deref(), Some(""));
    }

    #[test]
    fn unknown_type_and_missing_id_are_skipped() {
        let fake = FakeTracks {
            count: Some(3),
            rows: vec![
                (Some("button"), Some(9), None, None),
                (Some("sub"), None, None, None),
                (None, Some(5), None, None),
            ],
        };
        let lists = load_track_lists(&fake);
        assert!(lists.subtitles.is_empty());
        assert_eq!(lists.audio.len(), 1);
    }

    #[test]
    fn gone_handle_clears_both_lists() {
        let fake = FakeTracks {
            count: None,
            rows: vec![],
        };
        let lists = load_track_lists(&fake);
        assert!(lists.subtitles.is_empty());
        assert!(lists.audio.is_empty());
    }

    #[test]
    fn default_subtitle_is_first_and_audio_is_index_one() {
        let lists = load_track_lists(&sample());
        let out = default_tracks(&lists, |_, _| None);
        assert_eq!(out.sid, Some(3));
        assert_eq!(out.secondary_sid, Some(-1));
        assert_eq!(out.aid, Some(2));
    }

    #[test]
    fn preferred_track_wins_over_fallback() {
        let lists = load_track_lists(&sample());
        let out = default_tracks(&lists, |tracks, subtitle| {
            if subtitle {
                tracks.iter().position(|t| t.id == 4)
            } else {
                None
            }
        });
        assert_eq!(out.sid, Some(4));
        assert_eq!(out.aid, Some(2));
    }

    #[test]
    fn no_subtitles_and_no_real_audio_write_nothing() {
        let fake = FakeTracks {
            count: Some(0),
            rows: vec![],
        };
        let out = default_tracks(&load_track_lists(&fake), |_, _| None);
        assert_eq!(out, DefaultTracks::default());
    }
}
