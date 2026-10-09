//! Upstream `InputOutputTest`: each input list sorts to its output list under both comparators.

use std::cmp::Ordering;
use std::fs;

use kuro_local::natsort::{natural_cmp, natural_cmp_ci};

/// Upstream `load`: one item per line, lines starting with `#` skipped.
fn load(name: &str) -> Vec<String> {
    let path = format!("{}/tests/fixtures/natsort/{name}", env!("CARGO_MANIFEST_DIR"));
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    text.lines().filter(|l| !l.starts_with('#')).map(str::to_owned).collect()
}

fn check(n: u8, cmp: fn(&str, &str) -> Ordering) {
    let mut items = load(&format!("alphanum_test_{n}_input.txt"));
    items.sort_by(|a, b| cmp(a, b));
    assert_eq!(items, load(&format!("alphanum_test_{n}_output.txt")), "fixture {n}");
}

#[test]
fn the_fixtures_sort_identically() {
    for n in [1, 2] {
        check(n, natural_cmp);
        check(n, natural_cmp_ci);
    }
}
