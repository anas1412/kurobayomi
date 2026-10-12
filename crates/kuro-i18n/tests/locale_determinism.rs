//! T-51a-040: the committed locale files stay under the size budget.

use std::fs;
use std::path::{Path, PathBuf};

fn locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

/// `(tag, path)` of each committed locale directory that holds a `kurobayomi.ftl`.
fn committed_files() -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    for entry in fs::read_dir(locales_dir()).expect("locales dir").flatten() {
        let file = entry.path().join("kurobayomi.ftl");
        if file.is_file() {
            out.push((entry.file_name().to_string_lossy().into_owned(), file));
        }
    }
    out.sort();
    out
}

#[test]
fn embedded_locale_files_stay_under_ten_megabytes() {
    let mut total: u64 = 0;
    for (_, file) in committed_files() {
        total += fs::metadata(&file).expect("metadata").len();
    }
    for entry in fs::read_dir(locales_dir().join("en")).expect("en dir").flatten() {
        total += entry.metadata().map(|m| m.len()).unwrap_or(0);
    }
    assert!(total < 10 * 1024 * 1024, "embedded locale files total {total} bytes");
}
