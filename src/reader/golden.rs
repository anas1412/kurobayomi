//! Golden files of the reader tests (T-44a-078, T-44a-079, T-44a-080). They live in `tests/fixtures/reader/golden/`. A
//! test builds a JSON value from the code under test and [`check`] compares it with the committed file. The files are
//! reviewed data: set `KURO_REGEN_GOLDEN=1` to rewrite them after a deliberate change, then read the diff.

use std::path::PathBuf;

use serde_json::Value;

/// The golden directory.
pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reader/golden")
}

/// Compares `actual` with the golden file `name`, or rewrites the file when `KURO_REGEN_GOLDEN` is set.
pub fn check(name: &str, actual: &Value) {
    let path = dir().join(name);
    if std::env::var_os("KURO_REGEN_GOLDEN").is_some() {
        std::fs::create_dir_all(dir()).expect("the golden directory can be created");
        let mut text = serde_json::to_string_pretty(actual).expect("the golden value serialises");
        text.push('\n');
        std::fs::write(&path, text).expect("the golden file can be written");
        return;
    }
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("golden file {name}: {e} (regenerate with KURO_REGEN_GOLDEN=1)"));
    let want: Value = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("golden file {name} is not JSON: {e}"));
    assert!(same(&want, actual), "golden file {name} differs from the code (regenerate with KURO_REGEN_GOLDEN=1 if intended)\n  file: {want}\n  code: {actual}");
}

/// Equality with numbers compared to a relative 1e-12: reading a float back from JSON can be off by one ulp.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => match (x.as_f64(), y.as_f64()) {
            (Some(x), Some(y)) => (x - y).abs() <= 1e-12 * x.abs().max(y.abs()).max(1.0),
            _ => x == y,
        },
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_golden_directory_is_under_the_fixtures() {
        assert!(dir().ends_with("tests/fixtures/reader/golden"));
    }

    #[test]
    fn numbers_match_within_a_rounding_error_and_everything_else_exactly() {
        use serde_json::json;
        assert!(same(&json!({"a": [1.0, 0.9166666666666667]}), &json!({"a": [1.0, 0.9166666666666669]})));
        assert!(!same(&json!([0.5]), &json!([0.51])));
        assert!(!same(&json!(["a"]), &json!(["b"])));
        assert!(!same(&json!({"a": 1}), &json!({"b": 1})));
        assert!(!same(&json!([1, 2]), &json!([1])));
    }
}
