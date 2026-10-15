//! Guard: the app's own words come from the string table (`kuro-i18n`), never from English text in the code. The test reads
//! the source and fails on
//!
//! 1. a call that puts text on screen (a tooltip, a label, a title, a toast, a button...) with an English string literal, and
//! 2. a `const NAME: &str = "Some English sentence";` (those are shown somewhere, so they belong in the table too).
//!
//! Fix a failure by adding a message to `crates/kuro-i18n/locales/en/kurobayomi-extra.ftl` and to `kurobayomi.ftl` (same
//! text, at the end), then calling `tr(MsgKey::that_key)`. Other languages fall back to English until translated.
//!
//! What is allowed without a key is listed in [`ALLOWED`], each with its reason: text that is not shown to the user as
//! words of the interface (names, identifiers, protocol strings), and words that are the same in every language.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

/// `(file ending, the const name or the literal)`: English text that may stay in the code, and why.
const ALLOWED: &[(&str, &str, &str)] = &[
    ("src/shell/argv.rs", "USAGE", "the command-line help is for a terminal and is not translated"),
    ("src/pref_model/debug_info.rs", "KERNEL", "a technical label of the debug info, kept in English so reports read the same everywhere"),
    ("src/pref_model/debug_info.rs", "DESKTOP", "as above"),
    ("src/pref_model/debug_info.rs", "SESSION", "as above"),
    ("src/pref_model/debug_info.rs", "GTK", "as above"),
    ("src/reporter.rs", "Kurobayomi", "the name of the app"),
    ("src/shell/sidebar.rs", "Kurobayomi", "the name of the app"),
    ("src/player_view/hls_proxy.rs", "BROWSER_UA", "an HTTP user agent, not shown"),
    ("src/channel.rs", "REPOSITORY", "a GitHub owner/name, not shown"),
    ("src/updates/mount.rs", "UPDATE_LIBRARY", "the English label of a spec toolbar item, compared to find it (spec/*.json holds the text shown)"),
    ("src/updates/mount.rs", "VIEW_UPCOMING", "as above"),
    ("src/pref_model/stores_page.rs", "Discord", "the name of a product, the same in every language"),
];

/// Calls that show their argument to the user.
const SINK: &str = r#"(?:icon_button\([^,()]+,\s*|refresh_button\(|set_tooltip_text\(Some\(|tooltip_text\(|Label::new\(Some\(|Label::builder\(\)\.label\(|\.label\(|with_label\(|heading\(|ui::dim\(|\bdim\(|placeholder_text\(|set_label\(|set_title\(|\.title\(|toast::notify\(|notify\(|DialogButton::\w+\(|Toaster::show\(|\.show\(|\btext\(|tap_item\(|switch_item\(|set_text\()\s*&?\s*"((?:[^"\\]|\\.)+)""#;

fn source_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            // files that are only tests, and this guard itself
            if !(name.contains("test") || name == "i18n_guard.rs" || path.components().any(|c| c.as_os_str() == "testing")) {
                out.push(path);
            }
        }
    }
}

/// The code of a file without comments and without the items behind `#[cfg(test)]` (test modules and test-only items).
fn code_of(text: &str) -> String {
    let mut out = String::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            // skip the attribute and the item it applies to: a braced block (a module, a fn) or a one-line item
            i += 1;
            while i < lines.len() && lines[i].trim_start().starts_with("#[") {
                i += 1;
            }
            let mut depth = 0i32;
            let mut seen_brace = false;
            while i < lines.len() {
                for c in lines[i].chars() {
                    match c {
                        '{' => {
                            depth += 1;
                            seen_brace = true;
                        }
                        '}' => depth -= 1,
                        _ => {}
                    }
                }
                let done = if seen_brace { depth <= 0 } else { lines[i].trim_end().ends_with(';') };
                i += 1;
                if done {
                    break;
                }
            }
            continue;
        }
        if !trimmed.starts_with("//") {
            out.push_str(line);
            out.push('\n');
        }
        i += 1;
    }
    out
}

/// Whether a literal is English words for the interface: a capital letter first and some lower-case letters after.
fn is_english_words(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|c| c.is_ascii_uppercase())
        && text.chars().filter(|c| c.is_ascii_lowercase()).count() >= 2
        && !text.contains("::")
        && !text.starts_with("http")
}

fn unescape(text: &str) -> String {
    text.replace("\\\"", "\"").replace("\\'", "'")
}

fn allowed(file: &str, what: &str) -> bool {
    ALLOWED.iter().any(|(f, w, _)| file.ends_with(f) && *w == what)
}

#[test]
fn the_interface_text_comes_from_the_string_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    source_files(&root.join("src"), &mut files);
    assert!(files.len() > 100, "the guard found the sources");
    let sink = Regex::new(SINK).expect("a valid pattern");
    let constant = Regex::new(r#"(?m)\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*&(?:'static\s+)?str\s*=\s*\n?\s*"((?:[^"\\]|\\.)*)"\s*;"#).expect("a valid pattern");
    let mut found = Vec::new();
    for path in files {
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        let code = code_of(&fs::read_to_string(&path).expect("a readable source"));
        for m in sink.captures_iter(&code) {
            let text = unescape(&m[1]);
            if is_english_words(&text) && !allowed(&relative, &text) {
                found.push(format!("{relative}: shown text {text:?}"));
            }
        }
        for m in constant.captures_iter(&code) {
            let (name, text) = (&m[1], unescape(&m[2]));
            let sentence = is_english_words(&text) && (text.contains(' ') || text.ends_with(['.', '…', '?', '!', ':']));
            if sentence && !allowed(&relative, name) {
                found.push(format!("{relative}: const {name} = {text:?}"));
            }
        }
    }
    assert!(
        found.is_empty(),
        "English text in the code: move each into the string table (see the top of src/i18n_guard.rs):\n{}",
        found.join("\n")
    );
}

#[test]
fn the_cfg_test_items_are_left_out_of_the_check() {
    let code = code_of("fn a() { x.label(\"Shown\"); }\n#[cfg(test)]\nmod tests {\n    fn b() { x.label(\"Test only\"); }\n}\n// x.label(\"Comment\")\nfn c() {}\n#[cfg(test)]\nconst T: &str = \"Test const\";\nfn d() {}\n");
    assert!(code.contains("Shown") && code.contains("fn c()") && code.contains("fn d()"));
    assert!(!code.contains("Test only") && !code.contains("Comment") && !code.contains("Test const"), "{code}");
}

#[test]
fn english_words_are_told_from_identifiers_and_urls() {
    assert!(is_english_words("Check for updates") && is_english_words("Delete"));
    assert!(!is_english_words("view-refresh-symbolic") && !is_english_words("https://example.org") && !is_english_words("GTK"));
    assert!(!is_english_words("kb-status") && !is_english_words("io.github.anas1412.Kurobayomi"));
}
