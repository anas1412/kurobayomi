//! The files the script editor lists (tasks T-48a-037 and T-48a-038): names are checked here, the folder is read and
//! written with plain file calls. The widgets live in `scripts.rs`.
use std::path::Path;

pub const INVALID: &str = "Invalid filename";
pub const EXISTS: &str = "File already exists";

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
    names.sort();
    names
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

#[cfg(test)]
mod tests {
    use super::*;

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
