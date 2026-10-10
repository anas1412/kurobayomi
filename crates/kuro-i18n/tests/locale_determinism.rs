//! T-51a-040: the committed locale files stay under the size budget, and regenerating them from the
//! Aniyomi and core moko sources gives the same bytes. The regeneration check needs the sources on disk
//! (`KURO_ANIYOMI_SRC`, default `/home/blackbox/aniyomi-src`); without them it is skipped with a note.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

#[test]
fn regeneration_from_the_moko_sources_is_byte_identical() {
    let src = std::env::var("KURO_ANIYOMI_SRC").unwrap_or_else(|_| "/home/blackbox/aniyomi-src".into());
    if !Path::new(&src).join("i18n/src/commonMain/moko-resources").is_dir() {
        eprintln!("skipped: no moko sources at {src}");
        return;
    }
    let out = tempfile::tempdir().expect("temp dir");
    let status = Command::new(env!("CARGO_BIN_EXE_kuro-i18n-import"))
        .args(["locales", &src, out.path().to_str().expect("utf-8 path")])
        .status()
        .expect("run the importer");
    assert!(status.success(), "importer failed");

    let mut generated: Vec<String> = fs::read_dir(out.path())
        .expect("output dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    generated.sort();
    let committed: Vec<String> = committed_files().into_iter().map(|(tag, _)| tag).filter(|t| t != "en").collect();
    assert_eq!(generated, committed, "the set of translated locales differs");
    for tag in &generated {
        let fresh = fs::read(out.path().join(tag).join("kurobayomi.ftl")).expect("generated file");
        let stored = fs::read(locales_dir().join(tag).join("kurobayomi.ftl")).expect("committed file");
        assert!(fresh == stored, "{tag}/kurobayomi.ftl differs from its regeneration");
    }
}
