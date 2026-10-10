//! Guard for the Android-only network surface that the plan drops (T-12a-037 and T-12a-040). The drop is "no code,
//! no symbol": this scans the non-comment source of `kuro-net` and of `kuro-platform` (which holds the WebView
//! helpers) for the WebView package spoofing, the inferred user agent and the density conversion.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &[
    "X-Requested-With",
    "spoofedPackageName",
    "spoofed_package",
    "getInferredUserAgent",
    "inferred_user_agent",
    "infer_user_agent",
    "dpToPx",
    "dp_to_px",
    "density",
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
fn no_dropped_android_network_identifier_appears_in_code() {
    let mut files = Vec::new();
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for krate in ["kuro-net", "kuro-platform"] {
        rust_files(&crates.join(krate).join("src"), &mut files);
    }
    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for (number, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for word in FORBIDDEN {
                if code.contains(word) {
                    hits.push(format!("{}:{} {word}", file.display(), number + 1));
                }
            }
        }
    }
    assert!(hits.is_empty(), "dropped identifiers in code: {hits:#?}");
}
