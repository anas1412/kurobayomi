//! Fixture-driven checks for the player's pure logic (T-46a-056, p1-edge-cases-and-tests 11.3).
//! The files live under `tests/fixtures/player/`.

use kuro_player::chapters::{self, Segment};
use kuro_player::dialog::picker_values;
use kuro_player::time::format_episode_number;
use kuro_player::types::ChapterType;
use serde_json::Value;
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "tests",
        "fixtures",
        "player",
        name,
    ]
    .iter()
    .collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn json(name: &str) -> Value {
    serde_json::from_str(&fixture(name)).expect("fixture is valid JSON")
}

fn load_chapters(name: &str) -> (Vec<Segment>, f32) {
    let doc = json(name);
    let raw: Vec<(Option<String>, f32)> = doc["chapters"]
        .as_array()
        .expect("chapters array")
        .iter()
        .map(|c| {
            (
                c["name"].as_str().map(str::to_owned),
                c["start"].as_f64().expect("start") as f32,
            )
        })
        .collect();
    (
        chapters::load(&raw),
        doc["duration"].as_f64().expect("duration") as f32,
    )
}

#[test]
fn unsorted_chapters_load_in_start_order_and_the_current_one_is_the_last_started() {
    let (list, duration) = load_chapters("chapters_unsorted.json");
    let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Opening", "Part A", "Ending"]);
    assert_eq!(duration, 1420.0);
    assert_eq!(chapters::current_chapter(&list, 100.0), Some(1));
    assert_eq!(chapters::current_chapter(&list, 1300.0), Some(2));
}

#[test]
fn a_first_chapter_after_zero_has_no_chapter_before_it() {
    let (list, _) = load_chapters("chapters_no_zero.json");
    assert_eq!(chapters::current_chapter(&list, 10.0), None);
    assert_eq!(list[0].kind, ChapterType::Other);
}

#[test]
fn a_chapter_at_the_duration_is_kept() {
    let (list, duration) = load_chapters("chapters_at_end.json");
    assert_eq!(list.last().map(|c| c.start), Some(duration));
}

#[test]
fn episode_numbers_format_as_expected() {
    for case in json("episode_numbers.json").as_array().expect("array") {
        let value = case["value"].as_f64().expect("value");
        assert_eq!(format_episode_number(value), case["text"].as_str().unwrap());
    }
}

#[test]
fn display_mode_flags_match_the_number_mask() {
    use kuro_player::episode_list::EPISODE_DISPLAY_MASK;
    for case in json("display_modes.json").as_array().expect("array") {
        let mode = case["mode"].as_i64().expect("mode");
        let number = case["number"].as_bool().expect("number");
        assert_eq!(mode & EPISODE_DISPLAY_MASK == EPISODE_DISPLAY_MASK, number);
    }
}

#[test]
fn picker_specs_give_their_values() {
    // The four specs of the fixture, read as (min, max, step) from the `start|stop|step` fields.
    let specs: Vec<(i32, i32, i32)> = fixture("picker_specs.txt")
        .lines()
        .map(|line| {
            let f: Vec<&str> = line.split('|').collect();
            (
                f[2].parse().unwrap(),
                f[3].parse().unwrap(),
                f[4].parse().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        picker_values(specs[0].0, specs[0].1, specs[0].2),
        Some(vec![0, 3, 6, 9])
    );
    assert_eq!(picker_values(specs[1].0, specs[1].1, specs[1].2), None);
    assert_eq!(
        picker_values(specs[2].0, specs[2].1, specs[2].2),
        Some(vec![5])
    );
    assert_eq!(
        picker_values(specs[3].0, specs[3].1, specs[3].2).map(|v| v.len()),
        Some(5)
    );
}
