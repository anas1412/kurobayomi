//! The share and save flows of a link or a file (Kotlin `toShareIntent`, T-52a-048). Desktop has no third-party share
//! targets, so a web link is copied to the clipboard and a file is saved through a save dialog that starts in the
//! Downloads folder. A cancelled dialog does nothing.
//!
//! A file that is already on disk (a saved image, a backup) is shown in the file manager instead (T-53a-040): "Show in
//! folder" reveals it and "Copy link" copies a web address. No share chooser is opened anywhere. The backup type
//! `application/x-protobuf+gzip` is only the backup format's own metadata (area 21) and is never used to pick a share target.
//!
//! Loss recorded: no third-party share targets on desktop.

#[cfg(test)]
use std::path::{Path, PathBuf};


#[cfg(test)]
use crate::shell::argv;
#[cfg(test)]
use crate::shell::uri::percent_decode;

#[cfg(test)]
/// What a share or save does for one item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Copy the link to the clipboard and show the copied toast (T-52a-036).
    CopyLink(String),
    /// Save the file at `source`, with the dialog suggesting `name`.
    SaveFile { source: PathBuf, name: String },
    /// Reveal the file in the file manager ("Show in folder").
    ShowInFolder(PathBuf),
    Nothing,
}

#[cfg(test)]
/// The plan for a file that is already on disk: reveal it. A relative path does nothing.
pub fn show_in_folder(path: &Path) -> Plan {
    if path.is_absolute() { Plan::ShowInFolder(path.to_path_buf()) } else { Plan::Nothing }
}

#[cfg(test)]
/// The plan for an item: a web address is copied; a `file://` link or an absolute path is saved under its own name.
/// Anything else (blank text, a relative path) does nothing.
pub fn plan(item: &str) -> Plan {
    let item = item.trim();
    if item.is_empty() {
        return Plan::Nothing;
    }
    if argv::is_web_address(item) {
        return Plan::CopyLink(item.to_string());
    }
    let path = match item.strip_prefix("file://") {
        Some(rest) => PathBuf::from(percent_decode(rest.split(['?', '#']).next().unwrap_or(""), false)),
        None => PathBuf::from(item),
    };
    if !path.is_absolute() {
        return Plan::Nothing;
    }
    let name = path.file_name().and_then(|n| n.to_str()).map(str::to_string);
    match name {
        Some(name) => Plan::SaveFile { source: path, name },
        None => Plan::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_web_address_is_copied_as_it_was_shared() {
        assert_eq!(plan("  https://example.test/anime/1 "), Plan::CopyLink("https://example.test/anime/1".into()));
        assert_eq!(plan("HTTP://x.test"), Plan::CopyLink("HTTP://x.test".into()));
    }

    #[test]
    fn a_file_link_is_saved_under_its_own_name_with_the_path_decoded() {
        assert_eq!(
            plan("file:///home/u/Pictures/page%201.png?x=1"),
            Plan::SaveFile { source: PathBuf::from("/home/u/Pictures/page 1.png"), name: "page 1.png".into() }
        );
    }

    #[test]
    fn an_absolute_path_is_saved_and_a_relative_one_is_not() {
        assert_eq!(
            plan("/home/u/backup.tachibk"),
            Plan::SaveFile { source: PathBuf::from("/home/u/backup.tachibk"), name: "backup.tachibk".into() }
        );
        assert_eq!(plan("notes/backup.tachibk"), Plan::Nothing);
    }

    #[test]
    fn blank_text_and_a_root_path_do_nothing() {
        assert_eq!(plan("   "), Plan::Nothing);
        assert_eq!(plan("/"), Plan::Nothing);
        assert_eq!(plan("plain words"), Plan::Nothing);
    }

    #[test]
    fn a_file_on_disk_is_revealed_not_shared() {
        assert_eq!(show_in_folder(Path::new("/home/u/Downloads/page.png")), Plan::ShowInFolder("/home/u/Downloads/page.png".into()));
        assert_eq!(show_in_folder(Path::new("page.png")), Plan::Nothing);
    }
}
