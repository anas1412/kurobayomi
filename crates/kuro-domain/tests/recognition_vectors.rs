//! The shared recognition vector tables (T-07a-034) run through the public parsers.

#[path = "data/recognition_vectors.rs"]
mod vectors;

use kuro_domain::items::recognition::{parse_chapter_number, parse_episode_number, parse_season_number};

fn check(table: &[vectors::Vector], parse: fn(&str, &str, Option<f64>) -> f64) {
    for &(title, name, expected) in table {
        let got = parse(title, name, None);
        assert_eq!(got.to_bits(), expected.to_bits(), "{name:?}: got {got}, expected {expected}");
    }
}

#[test]
fn the_chapter_vectors_hold() {
    assert_eq!(vectors::CHAPTER_VECTORS.len(), 57);
    check(vectors::CHAPTER_VECTORS, parse_chapter_number);
}

#[test]
fn the_episode_vectors_hold() {
    assert_eq!(vectors::EPISODE_VECTORS.len(), 20);
    check(vectors::EPISODE_VECTORS, parse_episode_number);
}

#[test]
fn the_season_vectors_hold() {
    assert_eq!(vectors::SEASON_VECTORS.len(), 18);
    check(vectors::SEASON_VECTORS, parse_season_number);
}
