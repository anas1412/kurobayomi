//! Guard for the Storage Access Framework surface that the plan drops (T-13a-035). Archives open from a local path
//! only: no `content://` URI, no file descriptor and no `UniFile` reaches this crate's production code.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &[
    "content://",
    "ParcelFileDescriptor",
    "UniFile",
    "from_raw_fd",
    "as_raw_fd",
    "ContentResolver",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_storage_access_framework_identifier_appears_in_code() {
    let mut files = Vec::new();
    rust_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        let code_part = text.split("#[cfg(test)]").next().unwrap_or_default();
        for (number, line) in code_part.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            for token in FORBIDDEN {
                if code.contains(token) {
                    hits.push(format!("{}:{}: {token}", file.display(), number + 1));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "storage access framework identifiers in code:\n{}",
        hits.join("\n")
    );
}

#[test]
fn an_archive_opens_from_a_local_path() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("missing.cbz");
    // A plain path is the only entry point: a missing file is an error, not a content URI lookup.
    assert!(kuro_archive::ArchiveReader::open(&missing).is_err());
}
