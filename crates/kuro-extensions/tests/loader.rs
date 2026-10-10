//! The package loader over a temporary folder (T-18a-044, edge cases 12.8): a bad package does not fail the batch,
//! folders that are not packages are skipped, and an unsigned package is untrusted.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use kuro_core::hash::sha256_hex;
use kuro_extensions::loader::{COMPONENT_FILE, LoadResult, MANIFEST_FILE, discover};

const COMPONENT: &[u8] = b"\0asm component bytes";

fn manifest(pkg: &str) -> String {
    format!(
        "pkg = \"{pkg}\"\nname = \"Example\"\nversion = \"1.0.0\"\nversion_code = 1\napi = \"1.4\"\n"
    )
}

fn package(root: &Path, folder: &str, pkg: &str, component: Option<&[u8]>) {
    let dir = root.join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(MANIFEST_FILE), manifest(pkg)).unwrap();
    if let Some(bytes) = component {
        fs::write(dir.join(COMPONENT_FILE), bytes).unwrap();
    }
}

#[test]
fn a_bad_package_does_not_fail_the_batch_and_non_packages_are_skipped() {
    let root = tempfile::tempdir().unwrap();
    let exts = root.path();
    package(exts, "good", "eu.example.good", Some(COMPONENT));
    fs::create_dir_all(exts.join("no-manifest")).unwrap();
    fs::write(exts.join("notes.txt"), "a plain file").unwrap();
    package(exts, ".tmp", "eu.example.hidden", Some(COMPONENT));
    package(exts, "broken", "eu.example.broken", None);
    fs::write(exts.join("broken").join(MANIFEST_FILE), "pkg = [").unwrap();

    let signed = BTreeMap::from([("eu.example.good".to_owned(), sha256_hex(COMPONENT))]);
    let results = discover(exts, &signed);
    assert_eq!(
        results.len(),
        2,
        "only the two folders with a manifest are packages: {results:?}"
    );
    assert!(matches!(&results[0], LoadResult::Error { dir, .. } if dir.ends_with("broken")));
    assert!(
        matches!(&results[1], LoadResult::Verified { manifest, .. } if manifest.pkg == "eu.example.good")
    );
}

#[test]
fn an_unsigned_package_is_untrusted_and_a_signed_one_is_verified() {
    let root = tempfile::tempdir().unwrap();
    package(root.path(), "pkg", "eu.example.pkg", Some(COMPONENT));
    let untrusted = discover(root.path(), &BTreeMap::new());
    assert!(matches!(
        untrusted.as_slice(),
        [LoadResult::Untrusted { .. }]
    ));

    let wrong = BTreeMap::from([("eu.example.pkg".to_owned(), sha256_hex(b"other bytes"))]);
    assert!(matches!(
        discover(root.path(), &wrong).as_slice(),
        [LoadResult::Untrusted { .. }]
    ));

    let signed = BTreeMap::from([("eu.example.pkg".to_owned(), sha256_hex(COMPONENT))]);
    assert!(matches!(
        discover(root.path(), &signed).as_slice(),
        [LoadResult::Verified { .. }]
    ));
}

#[test]
fn a_package_without_its_component_is_an_error_naming_the_folder() {
    let root = tempfile::tempdir().unwrap();
    package(root.path(), "missing", "eu.example.missing", None);
    match discover(root.path(), &BTreeMap::new()).as_slice() {
        [LoadResult::Error { dir, .. }] => assert!(dir.ends_with("missing")),
        other => panic!("expected one error, got {other:?}"),
    }
}
