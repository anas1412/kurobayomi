//! Task T-02a-039: no crate other than `kuro-prefs` names the settings files, so no other code can
//! build a preference path or read `config.toml` or `state.toml` directly.

use std::fs;
use std::path::{Path, PathBuf};

/// Matched as quoted string literals, so doc comments that mention the files do not count.
const SETTINGS_FILES: [&str; 2] = ["\"config.toml\"", "\"state.toml\""];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_other_crate_names_a_settings_file() {
    // crates/kuro-prefs -> crates -> workspace root
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("kuro-prefs sits two levels below the workspace root");
    let mut roots = vec![workspace.join("src")];
    for entry in fs::read_dir(workspace.join("crates")).expect("read crates directory") {
        let dir = entry.expect("read directory entry").path();
        if dir.file_name().is_some_and(|n| n != "kuro-prefs") {
            roots.push(dir.join("src"));
        }
    }
    let mut files = Vec::new();
    for root in &roots {
        rust_files(root, &mut files);
    }
    assert!(!files.is_empty(), "no source files found under {}", workspace.display());

    let mut offenders = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).expect("read source file");
        for name in SETTINGS_FILES {
            if text.contains(name) {
                offenders.push(format!("{} names {name}", file.display()));
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}
