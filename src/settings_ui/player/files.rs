//! The files the script editor lists (tasks T-48a-037 and T-48a-038): names are checked here, the folder is read and
//! written with plain file calls. The widgets live in `scripts.rs`.
use std::path::Path;

pub const INVALID: &str = "Invalid filename";
pub const EXISTS: &str = "File already exists";
/// The toasts of a create, rename or delete that fails (`editor_create_error` and its two siblings).
pub const CREATE_FAILED: &str = "Unable to create file";
pub const RENAME_FAILED: &str = "Unable to rename file";
pub const DELETE_FAILED: &str = "Unable to delete file";

/// The two lists of the editor: Lua scripts and script option files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Scripts,
    Options,
}

impl Kind {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Scripts => "lua",
            Self::Options => "conf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Scripts => "Scripts",
            Self::Options => "Script options",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Scripts => "text-x-script-symbolic",
            Self::Options => "emblem-system-symbolic",
        }
    }
}

/// The file names in `dir` with the extension of `kind`, sorted. A folder that cannot be read is an empty list.
pub fn list(dir: &Path, kind: Kind) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| Path::new(name).extension().is_some_and(|ext| ext == kind.extension()))
        .collect();
    names.sort_by_key(|name| (name.to_lowercase(), name.clone()));
    names
}

/// The name the create dialog starts with: `file.lua` or `file.conf`.
pub fn default_name(kind: Kind) -> String {
    format!("file.{}", kind.extension())
}

/// The length in characters of the name without its last extension, so a dialog can select the base name only.
pub fn base_len(name: &str) -> i32 {
    let base = name.rfind('.').filter(|&at| at > 0).map_or(name, |at| &name[..at]);
    base.chars().count() as i32
}

/// A file size in decimal units (E-ED-11): bytes below 1000, then kB, MB and GB with one decimal.
pub fn size_label(bytes: u64) -> String {
    const UNITS: [&str; 3] = ["kB", "MB", "GB"];
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// The second line of a file row: its size and the day it was last modified. Empty when the file cannot be read.
pub fn detail(path: &Path) -> String {
    let Ok(meta) = std::fs::metadata(path) else { return String::new() };
    let modified = meta
        .modified()
        .ok()
        .map(|time| chrono::DateTime::<chrono::Local>::from(time).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    format!("{} · {modified}", size_label(meta.len()))
}

/// The name a typed filename becomes: trimmed, with the extension of `kind` added when it is missing. A blank name,
/// a path, or a name that starts with a dot is [`INVALID`]; a name already in `existing` is [`EXISTS`].
pub fn checked_name(typed: &str, kind: Kind, existing: &[String]) -> Result<String, &'static str> {
    let typed = typed.trim();
    let bare = typed.trim_end_matches(&format!(".{}", kind.extension()));
    if bare.is_empty() || bare.starts_with('.') || typed.contains(['/', '\\', '\0']) {
        return Err(INVALID);
    }
    let name = format!("{bare}.{}", kind.extension());
    if existing.iter().any(|e| *e == name) { Err(EXISTS) } else { Ok(name) }
}

pub fn create(dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(name)).map(drop)
}

pub fn rename(dir: &Path, from: &str, to: &str) -> std::io::Result<()> {
    std::fs::rename(dir.join(from), dir.join(to))
}

pub fn delete(dir: &Path, name: &str) -> std::io::Result<()> {
    std::fs::remove_file(dir.join(name))
}

/// A script file as the code editor opens it: the text with LF line ends, and what a save must keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub text: String,
    /// False when the file is not UTF-8; its bytes are shown with replacement characters (edge E-CE-03).
    pub utf8: bool,
    /// True when the file used CRLF line ends, which a save restores (edge E-CE-04).
    pub crlf: bool,
}

/// Reads `path` as a [`Document`].
pub fn read_document(path: &Path) -> std::io::Result<Document> {
    let bytes = std::fs::read(path)?;
    let (raw, utf8) = match String::from_utf8(bytes) {
        Ok(text) => (text, true),
        Err(err) => (String::from_utf8_lossy(err.as_bytes()).into_owned(), false),
    };
    let crlf = raw.contains("\r\n");
    Ok(Document { text: raw.replace("\r\n", "\n"), utf8, crlf })
}

/// The text a save writes: LF becomes CRLF again when the file had CRLF line ends.
pub fn encode(text: &str, crlf: bool) -> String {
    if crlf { text.replace('\n', "\r\n") } else { text.to_owned() }
}

/// Writes `contents` to `path` through a temporary file in the same folder, renamed over the target, so a failed write
/// leaves the old file in place (edge E-CE-05). The temporary file is removed when the rename fails.
pub fn save_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("script");
    let temp = path.with_file_name(format!(".{name}.tmp"));
    std::fs::write(&temp, contents)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_use_decimal_units_and_names_sort_without_regard_to_case() {
        assert_eq!(size_label(999), "999 B");
        assert_eq!(size_label(1500), "1.5 kB");
        assert_eq!(size_label(2_500_000), "2.5 MB");
        let dir = tempfile::tempdir().expect("a temp dir");
        for name in ["b.lua", "A.lua", "c.lua"] {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }
        assert_eq!(list(dir.path(), Kind::Scripts), ["A.lua", "b.lua", "c.lua"]);
        assert!(detail(&dir.path().join("A.lua")).starts_with("1 B · "));
    }

    #[test]
    fn create_starts_with_file_dot_extension_and_selects_the_base_name() {
        assert_eq!(default_name(Kind::Scripts), "file.lua");
        assert_eq!(default_name(Kind::Options), "file.conf");
        assert_eq!(base_len("file.lua"), 4);
        assert_eq!(base_len("été.conf"), 3);
        assert_eq!(base_len("noext"), 5);
        assert_eq!(base_len(".hidden"), 7, "a leading dot is not an extension");
    }

    #[test]
    fn the_failure_toasts_are_the_spec_texts() {
        assert_eq!((CREATE_FAILED, RENAME_FAILED, DELETE_FAILED), ("Unable to create file", "Unable to rename file", "Unable to delete file"));
    }

    #[test]
    fn crlf_is_read_as_lf_and_restored_on_save() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("a.lua");
        std::fs::write(&path, "one\r\ntwo\r\n").unwrap();
        let doc = read_document(&path).expect("read");
        assert_eq!(doc.text, "one\ntwo\n");
        assert!(doc.crlf && doc.utf8);
        assert_eq!(encode(&doc.text, doc.crlf), "one\r\ntwo\r\n");
        assert_eq!(encode("plain\n", false), "plain\n");
    }

    #[test]
    fn a_file_that_is_not_utf8_is_flagged_for_the_banner() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("b.conf");
        std::fs::write(&path, [0x61u8, 0xff, 0x62]).unwrap();
        let doc = read_document(&path).expect("read");
        assert!(!doc.utf8);
        assert!(!doc.crlf);
    }

    #[test]
    fn a_save_replaces_the_file_and_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let path = dir.path().join("c.lua");
        std::fs::write(&path, "old").unwrap();
        save_atomic(&path, "new text").expect("save");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new text");
        let names: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["c.lua"]);
    }

    #[test]
    fn names_get_the_extension_and_bad_ones_are_refused() {
        let existing = vec!["a.lua".to_owned()];
        assert_eq!(checked_name(" skip ", Kind::Scripts, &existing), Ok("skip.lua".to_owned()));
        assert_eq!(checked_name("skip.lua", Kind::Scripts, &existing), Ok("skip.lua".to_owned()));
        assert_eq!(checked_name("a", Kind::Scripts, &existing), Err(EXISTS));
        assert_eq!(checked_name("a.lua", Kind::Scripts, &existing), Err(EXISTS));
        assert_eq!(checked_name("a", Kind::Options, &existing), Ok("a.conf".to_owned()));
        for bad in ["", "  ", ".lua", ".hidden", "../x", "a/b", "a\\b", ".."] {
            assert_eq!(checked_name(bad, Kind::Scripts, &[]), Err(INVALID), "{bad:?}");
        }
    }

    #[test]
    fn create_rename_list_and_delete_work_on_a_real_folder() {
        let dir = tempfile::tempdir().unwrap();
        create(dir.path(), "b.lua").unwrap();
        create(dir.path(), "a.lua").unwrap();
        create(dir.path(), "a.conf").unwrap();
        std::fs::create_dir(dir.path().join("sub.lua")).unwrap();
        assert!(create(dir.path(), "a.lua").is_err(), "an existing file is not overwritten");
        assert_eq!(list(dir.path(), Kind::Scripts), ["a.lua", "b.lua"], "sorted, files only, this kind only");
        assert_eq!(list(dir.path(), Kind::Options), ["a.conf"]);
        rename(dir.path(), "b.lua", "c.lua").unwrap();
        assert_eq!(list(dir.path(), Kind::Scripts), ["a.lua", "c.lua"]);
        delete(dir.path(), "a.lua").unwrap();
        assert_eq!(list(dir.path(), Kind::Scripts), ["c.lua"]);
        assert!(list(&dir.path().join("missing"), Kind::Scripts).is_empty());
    }
}
