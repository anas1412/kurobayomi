//! The numbered tests T1-T79 of `plans/45-player-core/p2-edge-cases-and-tests.md` section 4 that need
//! the committed fixtures, plus the check that every number has a test (T-45b-062). The other
//! numbered tests sit next to their module as `#[cfg(test)]` tests named `tN_...`.

use std::path::PathBuf;

use kuro_data::episode::Episode;
use kuro_domain::entries::anime::episode_flags as flags;
use kuro_player::aniskip::{TrackerRef, parse_mal_id, parse_skip_times, resolve_mal_id};
use kuro_player::aspect::aspect_property_writes;
use kuro_player::chapters;
use kuro_player::enums::VideoAspect;
use kuro_player::intro_length::{effective_length, is_disabled, stored_length, with_length};
use kuro_player::playlist;
use kuro_player::progress::on_second;
use kuro_player::seek::{absolute_flag, relative_flag};
use kuro_player::track_select::{TrackCandidate, TrackPrefs, prefer_index, preferred_track};
use kuro_player::tracks::{TrackProps, default_tracks, load_track_lists};
use kuro_player::types::ChapterType;
use serde_json::Value;

fn path(sub: &str, name: &str) -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), sub, name].iter().collect()
}

fn fixture(name: &str) -> String {
    let file = path("tests/fixtures/player", name);
    std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
}

fn json(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).expect("fixture is valid JSON")
}

/// The 20 episodes of `episodes_20.json` with their download state.
fn episodes() -> Vec<(Episode, bool)> {
    json("episodes_20.json")
        .as_array()
        .expect("array")
        .iter()
        .map(|e| {
            let episode = Episode {
                id: e["id"].as_i64().unwrap(),
                source_order: e["source_order"].as_i64().unwrap(),
                episode_number: e["episode_number"].as_f64().unwrap(),
                name: e["name"].as_str().unwrap().to_owned(),
                seen: e["seen"].as_bool().unwrap(),
                bookmark: e["bookmark"].as_bool().unwrap(),
                fillermark: e["fillermark"].as_bool().unwrap(),
                ..Episode::create()
            };
            (episode, e["downloaded"].as_bool().unwrap())
        })
        .collect()
}

fn list() -> Vec<Episode> {
    episodes().into_iter().map(|(e, _)| e).collect()
}

fn is_downloaded(episode: &Episode) -> bool {
    episodes()
        .into_iter()
        .any(|(e, downloaded)| e.id == episode.id && downloaded)
}

fn filter_flags(name: &str) -> i64 {
    json("anime_flags.json")["filters"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == name)
        .unwrap_or_else(|| panic!("no filter named {name}"))["episode_flags"]
        .as_i64()
        .unwrap()
}

fn ids(episodes: &[Episode]) -> Vec<i64> {
    episodes.iter().map(|e| e.id).collect()
}

fn none(_: &Episode) -> bool {
    false
}

// 4.2 filter

#[test]
fn t6_filter_unseen() {
    let out = playlist::filter(&list(), filter_flags("unseen"), 1, none).unwrap();
    assert!(!out.is_empty());
    assert!(out.iter().all(|e| !e.seen), "only unseen episodes are kept");
    assert_eq!(out.len(), 20 - 6, "episodes 3, 6, ... 18 are seen");
}

#[test]
fn t7_filter_downloaded_not() {
    let out = playlist::filter(&list(), filter_flags("downloaded_not"), 1, is_downloaded).unwrap();
    assert!(out.iter().all(|e| !is_downloaded(e)));
    assert_eq!(out.len(), 10);
}

#[test]
fn t8_selected_always_kept() {
    // Show-seen hides the unseen episodes, and the selected episode 1 is unseen.
    let out = playlist::filter(&list(), filter_flags("show_seen"), 1, none).unwrap();
    assert_eq!(out.last().map(|e| e.id), Some(1));
    assert!(out[..out.len() - 1].iter().all(|e| e.seen));
}

#[test]
fn t9_missing_selected_is_an_error() {
    assert_eq!(playlist::filter(&list(), 0, 99, none), None);
}

#[test]
fn t10_combined_filters_remove_an_episode_that_violates_any_clause() {
    let flags = filter_flags("unseen_and_bookmarked");
    assert_eq!(flags, flags::SHOW_UNSEEN | flags::SHOW_BOOKMARKED);
    // Episode 4 is unseen and bookmarked, so it can be the selected one.
    let out = playlist::filter(&list(), flags, 4, none).unwrap();
    assert_eq!(ids(&out), vec![4, 8, 16, 20]);
    assert!(out.iter().all(|e| !e.seen && e.bookmark));
}

// 4.3 build on the 20-episode fixture

#[test]
fn t11_fixture_source_order_is_inverted() {
    let out = playlist::build(&list(), flags::SORTING_SOURCE, false, none).unwrap();
    assert_eq!(ids(&out), (1..=20).rev().collect::<Vec<_>>());
}

#[test]
fn t12_fixture_number_order_is_ascending() {
    let out = playlist::build(&list(), flags::SORTING_NUMBER, false, none).unwrap();
    let numbers: Vec<f64> = out.iter().map(|e| e.episode_number).collect();
    assert!(numbers.windows(2).all(|w| w[0] <= w[1]), "{numbers:?}");
}

#[test]
fn t13_fixture_downloaded_only_keeps_the_downloaded() {
    let out = playlist::build(&list(), flags::SORTING_NUMBER, true, is_downloaded).unwrap();
    assert_eq!(out.len(), 10);
    assert!(out.iter().all(is_downloaded));
}

// 4.4 TrackSelect on the mixed track list

struct Fixture(Value);

impl Fixture {
    fn item(&self, index: i64) -> Option<&Value> {
        self.0.as_array()?.get(usize::try_from(index).ok()?)
    }
}

impl TrackProps for Fixture {
    fn count(&self) -> Option<i64> {
        Some(self.0.as_array()?.len() as i64)
    }
    fn kind(&self, index: i64) -> Option<String> {
        Some(self.item(index)?["type"].as_str()?.to_owned())
    }
    fn lang(&self, index: i64) -> Option<String> {
        Some(self.item(index)?["lang"].as_str()?.to_owned())
    }
    fn title(&self, index: i64) -> Option<String> {
        Some(self.item(index)?["title"].as_str()?.to_owned())
    }
    fn id(&self, index: i64) -> Option<i64> {
        self.item(index)?["id"].as_i64()
    }
}

fn candidates(subtitles: bool) -> Vec<TrackCandidate> {
    let lists = load_track_lists(&Fixture(json("tracks_mixed.json")));
    let tracks = if subtitles {
        lists.subtitles
    } else {
        lists.audio
    };
    tracks
        .into_iter()
        .map(|t| TrackCandidate {
            id: t.id,
            name: t.name,
            lang: t.language,
        })
        .collect()
}

#[test]
fn the_mixed_fixture_loads_three_subtitles_and_off_plus_two_audio() {
    let lists = load_track_lists(&Fixture(json("tracks_mixed.json")));
    assert_eq!(lists.subtitles.len(), 3);
    assert_eq!(lists.audio.len(), 3);
    assert_eq!(lists.audio[0].id, -1);
}

#[test]
fn t15_whitelist_beats_order() {
    let tracks = [
        TrackCandidate {
            id: 1,
            name: "English".into(),
            lang: Some("en".into()),
        },
        TrackCandidate {
            id: 2,
            name: "English Signs".into(),
            lang: Some("en".into()),
        },
    ];
    assert_eq!(preferred_track(&tracks, "en", "Signs", "", "en"), Some(2));
    assert_eq!(preferred_track(&tracks, "en", "", "", "en"), Some(1));
}

#[test]
fn t16_blacklist_removes() {
    let subs = candidates(true);
    // Without the blacklist the first English subtitle is "English Signs" (id 1).
    assert_eq!(preferred_track(&subs, "en", "", "", "en"), Some(1));
    // The blacklist removes it, and the commentary is the only English one left.
    assert_eq!(preferred_track(&subs, "en", "", "Signs", "en"), Some(2));
    assert_eq!(
        preferred_track(&subs, "en", "Commentary", "Commentary", "en"),
        Some(1)
    );
}

#[test]
fn t18_a_trailing_comma_in_the_blacklist_does_not_exclude_everything() {
    let subs = candidates(true);
    assert_eq!(
        preferred_track(&subs, "en", "", "Commentary, ", "en"),
        Some(1)
    );
}

#[test]
fn t21_audio_ignores_the_whitelist_and_blacklist() {
    let lists = load_track_lists(&Fixture(json("tracks_mixed.json")));
    let prefs = TrackPrefs {
        subtitle_langs: "en".into(),
        subtitle_whitelist: "Commentary".into(),
        subtitle_blacklist: "Signs".into(),
        audio_langs: "en".into(),
        system_lang: "en".into(),
    };
    let out = default_tracks(&lists, |tracks, subtitle| {
        prefer_index(tracks, subtitle, &prefs)
    });
    // Subtitles follow the whitelist, the audio list is chosen without it.
    assert_eq!(out.sid, Some(2));
    assert_eq!(out.aid, Some(2));
    // A blacklist word in an audio title does not remove it.
    let audio = [kuro_player::tracks::VideoTrack {
        id: 7,
        name: "English Signs".into(),
        language: Some("en".into()),
    }];
    assert_eq!(prefer_index(&audio, false, &prefs), Some(0));
    assert_eq!(prefer_index(&audio, true, &prefs), None);
}

// 4.5 chapters from mpv

#[test]
fn the_mpv_chapter_fixture_loads_in_order() {
    let raw: Vec<(Option<String>, f32)> = json("chapters_mpv.json")
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["title"].as_str().map(str::to_owned),
                c["time"].as_f64().unwrap() as f32,
            )
        })
        .collect();
    let list = chapters::load(&raw);
    let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Intro", "Part 1"]);
    assert_eq!(chapters::current_chapter(&list, 299.0), Some(0));
    assert_eq!(chapters::current_chapter(&list, 300.0), Some(1));
    // Merged with an opening at 0-90 s of a 1420 s video, the mpv chapter at 0 is replaced.
    let stamp = kuro_player::types::TimeStamp {
        start: 0.0,
        end: 90.0,
        name: "Opening".into(),
        kind: ChapterType::Opening,
    };
    let merged = chapters::merge(&list, &[stamp], Some(1420));
    assert_eq!(merged[0].name, "Opening");
    assert!(merged.iter().any(|c| c.name == "Part 1"));
    assert!(!merged.iter().any(|c| c.name == "Intro"));
}

// 4.7 aniskip on the recorded responses

#[test]
fn t36_the_live_fixture_decodes() {
    let stamps = parse_skip_times(&fixture("aniskip_21_1.json")).unwrap();
    assert_eq!(stamps.len(), 2);
    assert_eq!(stamps[0].name, "Ending");
    assert_eq!(stamps[0].kind, ChapterType::Ending);
    assert_eq!(stamps[1].name, "Opening");
    assert_eq!(stamps[1].kind, ChapterType::Opening);
}

#[test]
fn t37_the_not_found_fixture_is_none() {
    assert_eq!(parse_skip_times(&fixture("aniskip_not_found.json")), None);
}

#[test]
fn t38_the_mixed_ed_fixture_keeps_the_item_as_other() {
    let stamps = parse_skip_times(&fixture("aniskip_mixed_ed.json")).unwrap();
    assert_eq!(stamps.len(), 2);
    assert_eq!(stamps[0].kind, ChapterType::Other);
    assert_eq!((stamps[0].start, stamps[0].end), (1300.0, 1390.0));
    assert_eq!(stamps[1].kind, ChapterType::Opening);
}

#[test]
fn t39_an_anilist_entry_without_a_mal_id_makes_no_aniskip_request() {
    assert_eq!(parse_mal_id(&fixture("anilist_null.json")), None);
    let looked_up = std::cell::Cell::new(0);
    let id = resolve_mal_id(&[TrackerRef::AniList(154587)], |_| {
        looked_up.set(looked_up.get() + 1);
        parse_mal_id(&fixture("anilist_null.json"))
    });
    assert_eq!(id, None, "no MAL id means no AniSkip call");
    assert_eq!(looked_up.get(), 1);
}

#[test]
fn t40_only_the_first_track_gives_the_mal_id() {
    // [Kitsu, MAL]: the first track is not MAL, so nothing is returned (T-29a-033, PlayerViewModel.kt:2044).
    let id = resolve_mal_id(&[TrackerRef::Other, TrackerRef::MyAnimeList(21)], |_| None);
    assert_eq!(id, None);
    // [Kitsu, AniList]: the same quirk applies to the AniList lookup.
    let id = resolve_mal_id(&[TrackerRef::Other, TrackerRef::AniList(5)], |_| {
        parse_mal_id(&fixture("anilist_mal.json"))
    });
    assert_eq!(id, None);
    // [MAL, Kitsu]: the MAL track is first and gives its remote id.
    let id = resolve_mal_id(&[TrackerRef::MyAnimeList(21), TrackerRef::Other], |_| None);
    assert_eq!(id, Some(21));
}

// 4.8 seek flags

#[test]
fn t41_seek_by_flags() {
    assert_eq!(relative_flag(false), "relative");
    assert_eq!(relative_flag(true), "relative+exact");
}

#[test]
fn t42_seek_to_flags() {
    assert_eq!(absolute_flag(true), "absolute");
    assert_eq!(absolute_flag(false), "absolute+keyframes");
}

// 4.10 aspect

#[test]
fn t50_fit_sets_panscan_zero_and_override_minus_one() {
    assert_eq!(
        aspect_property_writes(VideoAspect::Fit, 1.5),
        [("panscan", 0.0), ("video-aspect-override", -1.0)]
    );
}

#[test]
fn t51_crop_sets_panscan_one() {
    assert_eq!(
        aspect_property_writes(VideoAspect::Crop, 1.5),
        [("panscan", 1.0), ("video-aspect-override", -1.0)]
    );
}

#[test]
fn t52_stretch_uses_the_monitor_ratio() {
    let [pan, ratio] = aspect_property_writes(VideoAspect::Stretch, 1920.0 / 1080.0);
    assert_eq!(pan, ("panscan", 0.0));
    assert_eq!(ratio.0, "video-aspect-override");
    assert!((ratio.1 - 1.7777).abs() < 0.001);
}

// 4.11 completion threshold

#[test]
fn t53_completion_threshold_is_eighty_five_percent() {
    let at = |seconds| on_second(false, Some(1), seconds, 1000, 0.85, true).unwrap();
    assert!(at(850).completed);
    assert!(!at(849).completed);
}

// 4.15 flags and masks

#[test]
fn t77_intro_length_mask_round_trips() {
    let written = with_length(0, 85);
    assert_eq!(stored_length(written), 85);
    assert_eq!(effective_length(Some(written), 85), 85);
    let disabled = with_length(0, 0);
    assert!(is_disabled(disabled));
    assert_eq!(effective_length(Some(disabled), 85), 0);
    assert_eq!(
        with_length(
            0,
            json("anime_flags.json")["viewer"]["intro_length_85"]
                .as_i64()
                .unwrap()
        ),
        written
    );
}

#[test]
fn t79_episode_flag_masks_match_the_source_values() {
    let doc = json("anime_flags.json");
    let table = [
        ("UNSEEN_MASK", flags::UNSEEN_MASK),
        ("DOWNLOADED_MASK", flags::DOWNLOADED_MASK),
        ("BOOKMARKED_MASK", flags::BOOKMARKED_MASK),
        ("FILLERMARKED_MASK", flags::FILLERMARKED_MASK),
        ("SORTING_MASK", flags::SORTING_MASK),
        ("SHOW_UNSEEN", flags::SHOW_UNSEEN),
        ("SHOW_SEEN", flags::SHOW_SEEN),
        ("SHOW_DOWNLOADED", flags::SHOW_DOWNLOADED),
        ("SHOW_NOT_DOWNLOADED", flags::SHOW_NOT_DOWNLOADED),
        ("SHOW_BOOKMARKED", flags::SHOW_BOOKMARKED),
        ("SHOW_NOT_BOOKMARKED", flags::SHOW_NOT_BOOKMARKED),
        ("SHOW_FILLERMARKED", flags::SHOW_FILLERMARKED),
        ("SHOW_NOT_FILLERMARKED", flags::SHOW_NOT_FILLERMARKED),
        ("SORTING_SOURCE", flags::SORTING_SOURCE),
        ("SORTING_NUMBER", flags::SORTING_NUMBER),
        ("SORTING_UPLOAD_DATE", flags::SORTING_UPLOAD_DATE),
        ("SORTING_ALPHABET", flags::SORTING_ALPHABET),
    ];
    let masks = doc["masks"].as_object().unwrap();
    assert_eq!(masks.len(), table.len(), "every fixture mask is checked");
    for (name, value) in table {
        assert_eq!(masks[name].as_i64(), Some(value), "{name}");
    }
    // The four filter masks of the playlist predicate (D§3.1).
    assert_eq!(
        [
            flags::UNSEEN_MASK,
            flags::DOWNLOADED_MASK,
            flags::BOOKMARKED_MASK,
            flags::FILLERMARKED_MASK
        ],
        [0x6, 0x18, 0x60, 0x180]
    );
}

// The check: every number of section 4 has a test.

fn sources() -> String {
    let mut text = String::new();
    for dir in ["src", "tests"] {
        for entry in std::fs::read_dir(path(dir, "")).unwrap().flatten() {
            let file = entry.path();
            if file.extension().is_some_and(|e| e == "rs") {
                text.push_str(&std::fs::read_to_string(&file).unwrap());
            }
        }
    }
    text
}

#[test]
fn every_numbered_test_t1_to_t79_is_present() {
    let text = sources();
    // T69 to T73 tested the `user-data/aniyomi` Lua bridge, which was removed.
    let missing: Vec<u32> = (1..=79)
        .filter(|n| !(69..=73).contains(n))
        .filter(|n| !text.contains(&format!("fn t{n}_")))
        .collect();
    assert!(missing.is_empty(), "no test named for T{missing:?}");
}
