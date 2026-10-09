//! Chapters from a video's skip timestamps (area 45 `PlayerActivity.setupChapters` and
//! `ChapterUtils.mergeChapters`, T-45a-017). The merged list replaces the chapter model; the caller
//! then sets the current chapter to the playback position.

use crate::types::{ChapterType, TimeStamp};

/// `index` of a chapter taken from the video's timestamps (`IndexedSegment` index -2).
pub const EXTERNAL: i32 = -2;
/// `index` of the end marker that closes a timestamp range (`IndexedSegment` index -1).
pub const END_MARK: i32 = -1;

/// One chapter-bar segment (`IndexedSegment`). `index` is `EXTERNAL`, `END_MARK` or 0 for chapters
/// that came from mpv.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub index: i32,
    pub name: String,
    /// Seconds.
    pub start: f32,
    pub kind: ChapterType,
}

/// ARGB colour of an external chapter start with a known type (`Color(0xFFD8BBDF)`).
pub const EXTERNAL_COLOR: i32 = 0xFFD8_BBDFu32 as i32;

/// The colour of a chapter-bar segment, `None` where the Kotlin leaves it unspecified. Only external
/// starts of a known type are coloured.
pub fn color(segment: &Segment) -> Option<i32> {
    (segment.index == EXTERNAL && segment.kind != ChapterType::Other).then_some(EXTERNAL_COLOR)
}

/// The English label of a chapter type (`player_chapter_type_*`). `Other` has none. Localised text
/// is the UI layer's job, keyed by the same type.
pub fn type_label(kind: ChapterType) -> Option<&'static str> {
    match kind {
        ChapterType::Opening => Some("Opening"),
        ChapterType::Ending => Some("Ending"),
        ChapterType::Recap => Some("Recap"),
        ChapterType::MixedOp => Some("Mixed opening"),
        ChapterType::Other => None,
    }
}

/// The skip button text for a chapter type (`player_skip_action` with the type label, English
/// wording). `None` for `Other`, which has no skip button.
pub fn skip_label(kind: ChapterType) -> Option<String> {
    type_label(kind).map(|label| format!("Skip {label}"))
}

/// Gives an unnamed timestamp the label of its type. A named one, or one of type `Other`, is kept.
pub fn named(stamps: &[TimeStamp]) -> Vec<TimeStamp> {
    stamps
        .iter()
        .map(|stamp| match type_label(stamp.kind) {
            Some(label) if stamp.name.is_empty() => TimeStamp {
                name: label.to_owned(),
                ..stamp.clone()
            },
            _ => stamp.clone(),
        })
        .collect()
}

/// The chapter list after a video's timestamps are merged in. `None` for an empty timestamp list:
/// the chapters stay as they are, and nothing is set.
pub fn with_timestamps(
    current: &[Segment],
    stamps: &[TimeStamp],
    duration: Option<i32>,
) -> Option<Vec<Segment>> {
    (!stamps.is_empty()).then(|| merge(current, &named(stamps), duration))
}

/// Port of `ChapterUtils.mergeChapters`. Timestamps become chapters (with an end marker where the
/// range does not reach the end or the next range), mpv chapters that sit on a timestamp are
/// dropped, and adjacent unnamed segments are collapsed.
pub fn merge(current: &[Segment], stamps: &[TimeStamp], duration: Option<i32>) -> Vec<Segment> {
    let mut sorted: Vec<&TimeStamp> = stamps.iter().collect();
    sorted.sort_by(|a, b| a.start.total_cmp(&b.start));
    let duration = duration.map_or(-2.0, f64::from);

    let chapters: Vec<Segment> = sorted
        .iter()
        .enumerate()
        .flat_map(|(i, stamp)| {
            let start = if i == 0 && stamp.start < 1.0 {
                0.0
            } else {
                stamp.start
            };
            let start_chapter = Segment {
                index: EXTERNAL,
                name: stamp.name.clone(),
                start: start as f32,
                kind: stamp.kind,
            };
            let next_start = sorted.get(i + 1).map(|next| next.start);
            let not_last = (stamp.end - duration).abs() > 1.0;
            let not_adjacent = next_start.is_none_or(|next| (stamp.end - next).abs() > 1.0);
            if not_last && not_adjacent {
                vec![
                    start_chapter,
                    Segment {
                        index: END_MARK,
                        name: String::new(),
                        start: stamp.end as f32,
                        kind: ChapterType::Other,
                    },
                ]
            } else {
                vec![start_chapter]
            }
        })
        .collect();

    let near = |a: f32, b: f32| (a - b).abs() < 1.0;

    let mut player: Vec<Segment> = current
        .iter()
        .filter(|p| {
            !chapters
                .iter()
                .any(|c| near(c.start, p.start) && c.index == EXTERNAL)
        })
        .map(|p| Segment {
            index: 0,
            name: p.name.clone(),
            start: p.start,
            kind: p.kind,
        })
        .collect();
    player.sort_by(|a, b| a.start.total_cmp(&b.start));
    for segment in player.iter_mut().take(1) {
        if segment.start < 1.0 {
            segment.start = 0.0;
        }
    }

    let filtered: Vec<Segment> = chapters
        .iter()
        .filter(|c| {
            !player
                .iter()
                .any(|p| near(c.start, p.start) && c.index != EXTERNAL)
        })
        .cloned()
        .collect();

    let has_chapters = !player.is_empty() || !filtered.is_empty();
    let leading = if has_chapters
        && player.iter().all(|p| p.start != 0.0)
        && filtered.iter().all(|f| f.start != 0.0)
    {
        vec![Segment {
            index: END_MARK,
            name: String::new(),
            start: 0.0,
            kind: ChapterType::Other,
        }]
    } else {
        Vec::new()
    };

    let mut combined: Vec<Segment> = leading.into_iter().chain(player).chain(filtered).collect();
    combined.sort_by(|a, b| a.start.total_cmp(&b.start));

    combined
        .iter()
        .enumerate()
        .filter(|(i, segment)| {
            *i == 0 || !(segment.name.is_empty() && combined[i - 1].name.is_empty())
        })
        .map(|(_, segment)| segment.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(start: f64, end: f64, name: &str, kind: ChapterType) -> TimeStamp {
        TimeStamp {
            start,
            end,
            name: name.to_owned(),
            kind,
        }
    }

    fn names(segments: &[Segment]) -> Vec<&str> {
        segments.iter().map(|s| s.name.as_str()).collect()
    }

    #[test]
    fn unnamed_typed_timestamps_take_the_type_name() {
        let out = named(&[
            stamp(0.0, 90.0, "", ChapterType::Opening),
            stamp(90.0, 120.0, "", ChapterType::Other),
            stamp(120.0, 150.0, "Mine", ChapterType::Ending),
        ]);
        assert_eq!(out[0].name, "Opening");
        assert_eq!(out[1].name, "");
        assert_eq!(out[2].name, "Mine");
    }

    #[test]
    fn every_chapter_type_has_a_label_except_other() {
        assert_eq!(type_label(ChapterType::Ending), Some("Ending"));
        assert_eq!(type_label(ChapterType::Recap), Some("Recap"));
        assert_eq!(type_label(ChapterType::MixedOp), Some("Mixed opening"));
        assert_eq!(type_label(ChapterType::Other), None);
    }

    #[test]
    fn empty_timestamp_list_leaves_chapters_alone() {
        let current = vec![Segment {
            index: 0,
            name: "Intro".to_owned(),
            start: 0.0,
            kind: ChapterType::Other,
        }];
        assert_eq!(with_timestamps(&current, &[], Some(1000)), None);
    }

    #[test]
    fn range_inside_the_video_gets_an_end_marker() {
        let out = with_timestamps(
            &[],
            &[stamp(0.0, 90.0, "", ChapterType::Opening)],
            Some(1000),
        )
        .unwrap();
        assert_eq!(names(&out), vec!["Opening", ""]);
        assert_eq!(out[1].start, 90.0);
        assert_eq!(out[1].index, END_MARK);
    }

    #[test]
    fn range_that_reaches_the_end_has_no_end_marker() {
        let out = with_timestamps(
            &[],
            &[stamp(60.0, 1000.0, "Ending", ChapterType::Ending)],
            Some(1000),
        )
        .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].start, 0.0);
        assert_eq!(out[1].name, "Ending");
        assert_eq!(out[1].start, 60.0);
    }

    #[test]
    fn mpv_chapter_on_a_timestamp_is_replaced() {
        let current = vec![Segment {
            index: 0,
            name: "Chapter 2".to_owned(),
            start: 60.0,
            kind: ChapterType::Other,
        }];
        let out = merge(
            &current,
            &[stamp(60.0, 1000.0, "Opening", ChapterType::Opening)],
            Some(1000),
        );
        assert_eq!(names(&out), vec!["", "Opening"]);
        assert_eq!(out[0].start, 0.0);
    }

    #[test]
    fn unchanged_mpv_chapters_are_kept_in_order() {
        let current = vec![
            Segment {
                index: 0,
                name: "Start".to_owned(),
                start: 0.0,
                kind: ChapterType::Other,
            },
            Segment {
                index: 0,
                name: "Middle".to_owned(),
                start: 300.0,
                kind: ChapterType::Other,
            },
        ];
        let out = merge(
            &current,
            &[stamp(10.0, 20.0, "Op", ChapterType::Opening)],
            Some(1000),
        );
        assert_eq!(out.first().map(|s| s.start), Some(0.0));
        assert!(names(&out).contains(&"Middle"));
        assert!(names(&out).contains(&"Op"));
    }

    fn external(start: f32, kind: ChapterType) -> Segment {
        Segment {
            index: EXTERNAL,
            name: String::new(),
            start,
            kind,
        }
    }

    #[test]
    fn external_start_near_zero_snaps_to_zero() {
        let out = merge(
            &[],
            &[stamp(0.4, 90.0, "", ChapterType::Opening)],
            Some(1000),
        );
        assert_eq!(out[0].start, 0.0);
        assert_eq!(out[0].index, EXTERNAL);
    }

    #[test]
    fn end_marker_added_when_next_stamp_is_not_adjacent() {
        let out = merge(
            &[],
            &[
                stamp(0.0, 90.0, "Op", ChapterType::Opening),
                stamp(120.0, 150.0, "Ed", ChapterType::Ending),
            ],
            Some(1000),
        );
        let markers: Vec<(i32, f32)> = out
            .iter()
            .filter(|s| s.index == END_MARK)
            .map(|s| (s.index, s.start))
            .collect();
        assert!(markers.contains(&(END_MARK, 90.0)));
    }

    #[test]
    fn end_marker_skipped_when_end_is_at_duration() {
        let out = with_timestamps(
            &[],
            &[stamp(60.0, 1000.5, "Ending", ChapterType::Ending)],
            Some(1000),
        )
        .unwrap();
        assert!(out.iter().all(|s| s.index != END_MARK || s.start == 0.0));
    }

    #[test]
    fn mpv_chapter_within_a_second_of_external_is_dropped() {
        let current = vec![Segment {
            index: 0,
            name: "Chapter 2".to_owned(),
            start: 60.6,
            kind: ChapterType::Other,
        }];
        let out = merge(
            &current,
            &[stamp(60.0, 1000.0, "Opening", ChapterType::Opening)],
            Some(1000),
        );
        assert!(!names(&out).contains(&"Chapter 2"));
    }

    #[test]
    fn adjacent_empty_names_are_collapsed() {
        let stamps = [
            stamp(0.0, 30.0, "", ChapterType::Other),
            stamp(30.0, 60.0, "", ChapterType::Other),
            stamp(60.0, 90.0, "", ChapterType::Other),
        ];
        let out = merge(&[], &stamps, Some(1000));
        let empty_run = out
            .windows(2)
            .filter(|w| w[0].name.is_empty() && w[1].name.is_empty())
            .count();
        assert_eq!(empty_run, 0);
    }

    #[test]
    fn null_duration_uses_the_sentinel_and_keeps_end_marker() {
        let out = merge(
            &[],
            &[stamp(1300.0, 1400.0, "Ed", ChapterType::Ending)],
            None,
        );
        assert!(out.iter().any(|s| s.index == END_MARK && s.start == 1400.0));
    }

    #[test]
    fn skip_button_text_uses_the_type_label() {
        assert_eq!(
            skip_label(ChapterType::Opening).as_deref(),
            Some("Skip Opening")
        );
        assert_eq!(
            skip_label(ChapterType::MixedOp).as_deref(),
            Some("Skip Mixed opening")
        );
        assert_eq!(skip_label(ChapterType::Other), None);
    }

    #[test]
    fn external_colour_only_for_known_type() {
        assert_eq!(color(&external(10.0, ChapterType::Other)), None);
        assert_eq!(
            color(&external(10.0, ChapterType::Opening)),
            Some(EXTERNAL_COLOR)
        );
        assert_eq!(EXTERNAL_COLOR as u32, 0xFFD8_BBDF);
        let mpv = Segment {
            index: 0,
            name: String::new(),
            start: 10.0,
            kind: ChapterType::Opening,
        };
        assert_eq!(color(&mpv), None);
    }
}
