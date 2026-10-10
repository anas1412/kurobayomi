//! Guard for the Android-only surface that the plan drops (T-34a-035, T-34a-064 to T-34a-067). Each drop is "no code, no
//! symbol": this scans the production source of the crates this lane owns for the RGB565 decode option, the secure-screen
//! flag, the install-unknown-apps permission, the Compose fast-scroll workaround and the Compose and EditTextPreference
//! view plumbing. Comments and test modules are not scanned, so a note that names a dropped thing is fine.

use std::fs;
use std::path::{Path, PathBuf};

const OWNED_CRATES: [&str; 8] = [
    "kuro-net",
    "kuro-source",
    "kuro-source-wasm",
    "kuro-extensions",
    "kuro-archive",
    "kuro-local",
    "kuro-images",
    "kuro-backup",
];

const FORBIDDEN: &[&str] = &[
    "RGB565",
    "Rgb565",
    "rgb565",
    "allowRgb565",
    "FLAG_SECURE",
    "setSecureScreen",
    "SecureActivityDelegate",
    "ACTION_MANAGE_UNKNOWN_APP_SOURCES",
    "REQUEST_INSTALL_PACKAGES",
    "launchRequestPackageInstallsPermission",
    "FastScrollAnimateItem",
    "352584409",
    "setComposeContent",
    "ComposeView",
    "EditTextPreference",
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
fn no_dropped_view_or_decode_identifier_appears_in_code() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut hits = Vec::new();
    for krate in OWNED_CRATES {
        let mut files = Vec::new();
        rust_files(&crates.join(krate).join("src"), &mut files);
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
    }
    assert!(
        hits.is_empty(),
        "dropped identifiers in code:\n{}",
        hits.join("\n")
    );
}
