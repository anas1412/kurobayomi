//! The "Share crash logs" dump: one text file with the debug block, the problematic extension
//! blocks, the exception text and the newest ERROR lines of the log. The window, the clipboard and
//! the reveal belong to the GTK layer; this module builds the text and writes the file.

use std::fmt::Write as _;
use std::io;
use std::path::{Path, PathBuf};

/// Name of the dump file inside the crash directory.
pub const DUMP_FILE_NAME: &str = "kurobayomi_crash_logs.txt";
/// Directory under the cache directory that holds the dump.
pub const DUMP_DIR: &str = "crash";
/// Most ERROR lines copied from the log.
pub const MAX_ERROR_LINES: usize = 500;
/// Most bytes of ERROR lines copied from the log. The newest lines win.
pub const MAX_LOG_BYTES: usize = 256 * 1024;
/// Stands in for the log section when there is no log file.
pub const NO_LOG_FILE: &str = "(no log file)";

/// Where the dump is written: `<cache_dir>/crash/kurobayomi_crash_logs.txt`.
pub fn dump_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join(DUMP_DIR).join(DUMP_FILE_NAME)
}

/// The values of the debug block. Labels and their order are fixed by [`DebugInfo::render`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebugInfo {
    pub app_id: String,
    pub app_version: String,
    pub os: String,
    pub kernel: String,
    pub desktop: String,
    pub gtk: String,
    pub webview: String,
    pub current_time: String,
    pub mpv: String,
    pub ffmpeg: String,
    pub sqlite: String,
    pub locale: String,
}

impl DebugInfo {
    /// The block as `Label: value` lines, without a trailing newline.
    pub fn render(&self) -> String {
        let rows: [(&str, &str); 12] = [
            ("App ID", &self.app_id),
            ("App version", &self.app_version),
            ("OS", &self.os),
            ("Kernel", &self.kernel),
            ("Desktop", &self.desktop),
            ("GTK version", &self.gtk),
            ("WebView", &self.webview),
            ("Current time", &self.current_time),
            ("MPV version", &self.mpv),
            ("FFmpeg version", &self.ffmpeg),
            ("SQLite version", &self.sqlite),
            ("Locale", &self.locale),
        ];
        rows.iter()
            .map(|(label, value)| format!("{label}: {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One installed extension as the dump sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionStatus {
    pub name: String,
    pub installed_name: String,
    pub installed_code: i64,
    /// Version name and code of the repository copy, `None` when the repository does not list it.
    pub available: Option<(String, i64)>,
    pub obsolete: bool,
}

impl ExtensionStatus {
    /// A newer copy is listed, or the extension is marked obsolete.
    fn is_problematic(&self) -> bool {
        self.obsolete || self.available.as_ref().is_some_and(|(_, code)| *code > self.installed_code)
    }
}

/// The extension block under `title`, or `None` when no extension is problematic. Only problematic
/// extensions are listed, sorted by name.
pub fn extension_block(title: &str, extensions: &[ExtensionStatus]) -> Option<String> {
    let mut rows: Vec<&ExtensionStatus> = extensions.iter().filter(|e| e.is_problematic()).collect();
    if rows.is_empty() {
        return None;
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = format!("{title}\n");
    for e in rows {
        let available = e.available.as_ref().map_or("?", |(version, _)| version.as_str());
        let _ = writeln!(out, "- {}", e.name);
        let _ = writeln!(out, "  Installed: {} / Available: {available}", e.installed_name);
        let _ = writeln!(out, "  Obsolete: {}", e.obsolete);
    }
    Some(out.trim_end().to_owned())
}

/// The newest ERROR lines of `log`, at most [`MAX_ERROR_LINES`] lines and [`MAX_LOG_BYTES`] bytes,
/// oldest first. `None` gives [`NO_LOG_FILE`]. A line is an ERROR line when its level token is
/// `ERROR`, the second field of the log format.
pub fn error_tail(log: Option<&str>) -> String {
    let Some(text) = log else {
        return NO_LOG_FILE.to_owned();
    };
    let errors: Vec<&str> = text
        .lines()
        .filter(|line| line.split_whitespace().nth(1) == Some("ERROR"))
        .collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut bytes = 0;
    for line in errors.iter().rev().take(MAX_ERROR_LINES) {
        let cost = line.len() + 1;
        if bytes + cost > MAX_LOG_BYTES {
            break;
        }
        bytes += cost;
        kept.push(*line);
    }
    kept.reverse();
    kept.join("\n")
}

/// The inputs of one dump.
pub struct DumpInputs<'a> {
    pub debug: &'a DebugInfo,
    pub manga_extensions: &'a [ExtensionStatus],
    pub anime_extensions: &'a [ExtensionStatus],
    /// The exception text, when the dump is made for one.
    pub exception: Option<&'a str>,
    /// The whole log file, `None` when it is missing.
    pub log: Option<&'a str>,
}

/// The dump text: debug block, manga block, anime block, exception, ERROR lines. Blocks are
/// separated by a blank line and the text ends with a newline.
pub fn compose(inputs: &DumpInputs<'_>) -> String {
    let mut blocks = vec![inputs.debug.render()];
    blocks.extend(extension_block("Problematic manga extensions:", inputs.manga_extensions));
    blocks.extend(extension_block("Problematic anime extensions:", inputs.anime_extensions));
    blocks.extend(inputs.exception.map(|e| e.trim_end().to_owned()));
    blocks.push(error_tail(inputs.log));
    let mut text = blocks.join("\n\n");
    text.push('\n');
    text
}

/// Writes `text` to [`dump_path`] under `cache_dir`, deleting an earlier dump first, and returns the
/// path.
pub fn write_dump(cache_dir: &Path, text: &str) -> io::Result<PathBuf> {
    let path = dump_path(cache_dir);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    std::fs::write(&path, text)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debug() -> DebugInfo {
        DebugInfo {
            app_id: "io.github.kurobayomi".into(),
            app_version: "0.1.0 (abc, 1, 2026-10-10)".into(),
            os: "CachyOS Linux".into(),
            kernel: "7.2.9-1-cachyos".into(),
            desktop: "KDE".into(),
            gtk: "4.16.12".into(),
            webview: "WebKitGTK 2.46".into(),
            current_time: "2026-10-10T10:00:00Z".into(),
            mpv: "0.39.0".into(),
            ffmpeg: "n7.1".into(),
            sqlite: "3.46.0".into(),
            locale: "en-US".into(),
        }
    }

    fn ext(name: &str, installed: i64, available: Option<(&str, i64)>, obsolete: bool) -> ExtensionStatus {
        ExtensionStatus {
            name: name.into(),
            installed_name: format!("1.{installed}"),
            installed_code: installed,
            available: available.map(|(v, c)| (v.to_owned(), c)),
            obsolete,
        }
    }

    #[test]
    fn debug_block_has_the_twelve_labels_in_order() {
        let labels: Vec<String> = debug()
            .render()
            .lines()
            .map(|l| l.split(": ").next().unwrap().to_owned())
            .collect();
        assert_eq!(
            labels,
            [
                "App ID", "App version", "OS", "Kernel", "Desktop", "GTK version", "WebView",
                "Current time", "MPV version", "FFmpeg version", "SQLite version", "Locale",
            ]
        );
    }

    #[test]
    fn extension_block_lists_only_newer_or_obsolete_sorted_by_name() {
        let exts = [
            ext("Zeta", 3, Some(("1.4", 4)), false),
            ext("Alpha", 5, Some(("1.5", 5)), false),
            ext("Mid", 2, None, true),
            ext("Beta", 1, Some(("1.9", 9)), false),
        ];
        let block = extension_block("Problematic anime extensions:", &exts).unwrap();
        assert_eq!(
            block,
            "Problematic anime extensions:\n\
             - Beta\n  Installed: 1.1 / Available: 1.9\n  Obsolete: false\n\
             - Mid\n  Installed: 1.2 / Available: ?\n  Obsolete: true\n\
             - Zeta\n  Installed: 1.3 / Available: 1.4\n  Obsolete: false"
        );
    }

    #[test]
    fn extension_block_is_none_when_nothing_is_problematic() {
        let exts = [ext("Alpha", 5, Some(("1.5", 5)), false), ext("Gone", 1, None, false)];
        assert_eq!(extension_block("x", &exts), None);
    }

    #[test]
    fn error_tail_keeps_the_newest_five_hundred_error_lines() {
        let mut log = String::new();
        for i in 0..600 {
            log.push_str(&format!("2026-10-10T10:00:00Z ERROR kuro_core: event {i}\n"));
            log.push_str("2026-10-10T10:00:00Z  INFO kuro_core: skipped\n");
        }
        let tail = error_tail(Some(&log));
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), MAX_ERROR_LINES);
        assert!(lines[0].ends_with("event 100"), "{}", lines[0]);
        assert!(lines[499].ends_with("event 599"));
    }

    #[test]
    fn error_tail_is_capped_at_256_kib() {
        let line = format!("2026-10-10T10:00:00Z ERROR kuro_core: {}", "x".repeat(1000));
        let log = vec![line.as_str(); 400].join("\n");
        let tail = error_tail(Some(&log));
        assert!(tail.len() <= MAX_LOG_BYTES, "{}", tail.len());
        assert_eq!(tail.lines().count(), MAX_LOG_BYTES / (line.len() + 1));
    }

    #[test]
    fn missing_log_gives_the_placeholder() {
        assert_eq!(error_tail(None), "(no log file)");
    }

    #[test]
    fn compose_orders_the_sections_and_skips_empty_extension_blocks() {
        let debug = debug();
        let manga = [ext("Solo", 1, None, true)];
        let text = compose(&DumpInputs {
            debug: &debug,
            manga_extensions: &manga,
            anime_extensions: &[],
            exception: Some("panicked at src/x.rs:1: boom\n"),
            log: Some("2026-10-10T10:00:00Z ERROR kuro_core: late\n"),
        });
        let expected = format!(
            "{}\n\nProblematic manga extensions:\n- Solo\n  Installed: 1.1 / Available: ?\n  Obsolete: true\n\n\
             panicked at src/x.rs:1: boom\n\n2026-10-10T10:00:00Z ERROR kuro_core: late\n",
            debug.render()
        );
        assert_eq!(text, expected);
        assert!(!text.contains("anime extensions"));
    }

    #[test]
    fn write_dump_replaces_the_previous_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let first = write_dump(dir.path(), "first\n").unwrap();
        let second = write_dump(dir.path(), "second\n").unwrap();
        assert_eq!(first, second);
        assert_eq!(first, dir.path().join("crash").join(DUMP_FILE_NAME));
        assert_eq!(std::fs::read_to_string(&second).unwrap(), "second\n");
    }
}
