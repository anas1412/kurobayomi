//! The share and save flows of a link or a file (Kotlin `toShareIntent`, T-52a-048). Desktop has no third-party share
//! targets, so a web link is copied to the clipboard and a file is saved through a save dialog that starts in the
//! Downloads folder. A cancelled dialog does nothing.
//!
//! A file that is already on disk (a saved image, a backup) is shown in the file manager instead (T-53a-040): "Show in
//! folder" reveals it and "Copy link" copies a web address. No share chooser is opened anywhere. The backup type
//! `application/x-protobuf+gzip` is only the backup format's own metadata (area 21) and is never used to pick a share target.
//!
//! Loss recorded: no third-party share targets on desktop.

use std::path::{Path, PathBuf};

use gtk::gio;
use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use crate::shell::argv;
use crate::shell::uri::percent_decode;

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

/// The plan for a file that is already on disk: reveal it. A relative path does nothing.
pub fn show_in_folder(path: &Path) -> Plan {
    if path.is_absolute() { Plan::ShowInFolder(path.to_path_buf()) } else { Plan::Nothing }
}

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

/// Runs a plan on the GTK thread. `parent` anchors the save dialog, and `downloads` is the folder it starts in.
pub fn run(plan: Plan, parent: &gtk::Window, downloads: &Path) {
    match plan {
        Plan::CopyLink(link) => {
            gtk::prelude::WidgetExt::display(parent).clipboard().set_text(&link);
            crate::toast::notify(tr(MsgKey::copied_to_clipboard_plain));
        }
        Plan::SaveFile { source, name } => {
            let dialog = gtk::FileDialog::new();
            dialog.set_title(tr(MsgKey::action_save).as_str());
            dialog.set_initial_name(Some(&name));
            dialog.set_initial_folder(Some(&gio::File::for_path(downloads)));
            dialog.save(Some(parent), None::<&gio::Cancellable>, move |result| {
                // a cancelled dialog reports an error too, and it does nothing
                let Ok(file) = result else { return };
                let Some(dest) = file.path() else { return };
                if let Err(e) = std::fs::copy(&source, &dest) {
                    tracing::warn!("the shared file was not saved: {e}");
                    crate::toast::notify(tr(MsgKey::clipboard_copy_error));
                }
            });
        }
        Plan::ShowInFolder(path) => {
            gtk::FileLauncher::new(Some(&gio::File::for_path(&path))).open_containing_folder(
                Some(parent),
                None::<&gio::Cancellable>,
                move |result| {
                    if let Err(e) = result {
                        tracing::warn!("{} was not shown in the file manager: {e}", path.display());
                    }
                },
            );
        }
        Plan::Nothing => {}
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

    #[test]
    fn no_share_chooser_and_no_backup_share_type_in_the_shell() {
        // the backup MIME stays in the backup crate; the shell has no share sheet
        for (name, text) in [("share.rs", include_str!("share.rs")), ("route.rs", include_str!("route.rs"))] {
            let production = &text[..text.find("#[cfg(test)]").unwrap_or(text.len())];
            let code: Vec<_> = production.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
            for forbidden in ["x-protobuf", "ACTION_SEND", "createChooser", "toShareIntent"] {
                assert!(!code.iter().any(|l| l.contains(forbidden)), "{name}: {forbidden}");
            }
        }
    }
}
