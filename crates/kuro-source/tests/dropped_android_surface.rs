//! Guard for the Android-only surface that the plan drops (T-14a-053, T-15a-045 to T-15a-047, T-16a-048 to T-16a-050,
//! T-16a-053, T-17a-039 to T-17a-044, T-19a-038 to T-19a-040). Each drop is "no code, no symbol": this scans the
//! non-test source of the crates that own those areas for the identifiers of the dropped Android, Rx, Kotlin-coroutine and
//! installer surface. Comments are not scanned, so a note that names a dropped thing is fine.

use std::fs;
use std::path::{Path, PathBuf};

const CRATES: [&str; 4] = ["kuro-source", "kuro-local", "kuro-extensions", "kuro-prefs"];

const FORBIDDEN: &[&str] = &[
    "VideoUrl",
    "awaitSingle",
    "runBlocking",
    "Dispatchers",
    "CoroutineScope",
    "ImageBitmap",
    "Injekt",
    "MediaScannerConnection",
    "ParcelFileDescriptor",
    "FileProvider",
    "FFmpegKit",
    "PackageInstaller",
    "Shizuku",
    "ACTION_INSTALL_PACKAGE",
    "xmlutil",
    "Jsoup",
    "androidx",
    "PreferenceScreen",
    "eh_enable_source_blacklist",
    "sources_tab_categories",
    "sources_tab_source_categories",
    "extension_installer",
    "ProGuard",
    "prepareNewEpisode",
    "prepare_new_episode",
    "episodeVideoParse",
    "episode_video_parse",
    "chapterPageParse",
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
fn no_dropped_android_identifier_appears_in_code() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut hits = Vec::new();
    for krate in CRATES {
        let mut files = Vec::new();
        rust_files(&crates.join(krate).join("src"), &mut files);
        for file in files {
            let text = fs::read_to_string(&file).unwrap();
            // production code only: a test may name a dropped key to assert that it is gone
            let code_part = text.split("#[cfg(test)]").next().unwrap_or_default();
            for (number, line) in code_part.lines().enumerate() {
                let code = line.split("//").next().unwrap_or_default();
                for token in FORBIDDEN.iter() {
                    if code.contains(token) {
                        hits.push(format!("{}:{}: {token}", file.display(), number + 1));
                    }
                }
            }
        }
    }
    assert!(hits.is_empty(), "dropped identifiers in code:\n{}", hits.join("\n"));
}

#[test]
fn no_rxjava_dependency_is_declared() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for krate in CRATES {
        let manifest = fs::read_to_string(crates.join(krate).join("Cargo.toml")).unwrap();
        assert!(!manifest.to_lowercase().contains("rxjava"), "{krate} declares RxJava");
    }
}

#[test]
fn no_android_build_or_manifest_file_is_kept() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    const ANDROID_FILES: [&str; 5] = ["build.gradle.kts", "build.gradle", "AndroidManifest.xml", "proguard-rules.pro", "consumer-rules.pro"];
    for krate in CRATES {
        for name in ANDROID_FILES {
            let path = crates.join(krate).join(name);
            assert!(!path.exists(), "{} is an Android build file and is dropped", path.display());
        }
    }
}
