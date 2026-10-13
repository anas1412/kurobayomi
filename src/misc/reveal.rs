//! Reveal notices (T-35a-023 save-image, T-35a-024 backup complete). "Open folder" shows the file in the file manager
//! instead of Android's share chooser. The reveal itself is checked against the allowed folders when the action runs
//! (`router::reveal_path_ok`), so these builders only name the file.


use std::path::PathBuf;
#[cfg(test)]
use std::path::Path;
use std::sync::Mutex;

#[cfg(test)]
use kuro_backup::notify::{Action, Notice, Urgency};

#[cfg(test)]
use crate::misc::ids;
#[cfg(test)]
use crate::misc::notify::{Post, Target};
#[cfg(test)]
use crate::misc::router::AppAction;

#[cfg(test)]
/// The "Open folder" label of the reveal buttons (`notify.action.open_folder`).
pub const OPEN_FOLDER: &str = "Open folder";

#[cfg(test)]
/// The target that reveals `path` in the file manager.
pub fn reveal_target(path: &Path) -> Target {
    Target::Action(AppAction::RevealFile { path: path.to_path_buf() })
}

#[cfg(test)]
/// The save-image notice (id `save-image`, T-35a-086): the tap and "Open folder" reveal the saved image.
pub fn save_image_post(saved: &Path) -> Post {
    let target = reveal_target(saved);
    Post {
        id: ids::SAVE_IMAGE.to_owned(),
        title: "Image saved".to_owned(),
        body: saved.file_name().map(|name| name.to_string_lossy().into_owned()),
        low_priority: false,
        timeout: None,
        tap: Some(target.clone()),
        buttons: vec![(OPEN_FOLDER.to_owned(), target)],
    }
}

#[cfg(test)]
/// A `kuro-backup` notice as a desktop post. `ShowInFolder` becomes the tap and the "Open folder" button (T-35a-024);
/// the error-log action opens the log; the settings and cancel actions are left to their own screens.
pub fn backup_post(notice: &Notice) -> Post {
    let tap = match &notice.action {
        Some(Action::ShowInFolder(path)) => Some(reveal_target(path)),
        Some(Action::OpenErrorLog(path)) => Some(Target::OpenLog(path.to_string_lossy().into_owned())),
        Some(Action::OpenSettings | Action::CancelRestore) | None => None,
    };
    let buttons = match &notice.action {
        Some(Action::ShowInFolder(path)) => vec![(OPEN_FOLDER.to_owned(), reveal_target(path))],
        _ => Vec::new(),
    };
    Post {
        id: notice.id.to_owned(),
        title: notice.title.clone(),
        body: notice.body.clone(),
        low_priority: notice.urgency == Urgency::Low,
        timeout: None,
        tap,
        buttons,
    }
}

/// Folders of files the app itself just wrote and announced, such as the user's pictures folder or a backup folder the
/// user chose. They join the folders a reveal may name (`notify::GtkHost::reveal_roots`), so the file the notice names can
/// be shown while a path from anywhere else is still refused.
static ANNOUNCED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
#[cfg(test)]
const MAX_ANNOUNCED: usize = 16;

#[cfg(test)]
/// Lets a reveal name files in the folder of `file`, which the app has just written.
pub fn allow_folder_of(file: &Path) {
    let Some(folder) = file.parent() else { return };
    let mut folders = ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner());
    folders.retain(|known| known != folder);
    folders.push(folder.to_path_buf());
    if folders.len() > MAX_ANNOUNCED {
        folders.remove(0);
    }
}

/// The folders [`allow_folder_of`] has recorded.
pub fn announced_folders() -> Vec<PathBuf> {
    ANNOUNCED.lock().unwrap_or_else(|e| e.into_inner()).clone()
}


#[cfg(test)]
/// The post for a "Backup created" call, or `None` for any other notice.
fn created_post(id: &str, title: &str, body: Option<&str>) -> Option<Post> {
    let path = Path::new(body?.lines().next()?);
    let created = kuro_backup::notify::manual_created(path);
    if id != created.id || title != created.title || !path.is_absolute() {
        return None;
    }
    let mut notice = created;
    notice.body = body.map(str::to_owned);
    Some(backup_post(&notice))
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::misc::router::reveal_path_ok;
    use std::path::PathBuf;

    #[test]
    fn the_save_image_notice_reveals_the_file_under_its_own_id() {
        // T-35a-023, T-35a-086
        let post = save_image_post(Path::new("/downloads/manga/page.png"));
        assert_eq!(post.id, "save-image");
        assert_eq!(post.body.as_deref(), Some("page.png"));
        let reveal = Target::Action(AppAction::RevealFile { path: "/downloads/manga/page.png".into() });
        assert_eq!(post.tap, Some(reveal.clone()));
        assert_eq!(post.buttons, vec![("Open folder".to_owned(), reveal)]);
    }

    #[test]
    fn a_save_image_path_outside_the_downloads_root_is_refused_when_revealed() {
        // T-35a-023: the reveal is checked against the allowed roots (design 6.1).
        let roots = vec![PathBuf::from("/home/u/downloads")];
        assert!(reveal_path_ok(Path::new("/home/u/downloads/manga/page.png"), &roots));
        assert!(!reveal_path_ok(Path::new("/home/u/.ssh/page.png"), &roots));
    }

    #[test]
    fn the_backup_complete_notice_offers_open_folder_with_a_reveal_of_the_file() {
        // T-35a-024
        let path = Path::new("/home/u/backups/kurobayomi-2026.tachibk");
        let post = backup_post(&kuro_backup::notify::manual_created(path));
        assert_eq!(post.id, kuro_backup::notify::ID_BACKUP_RESULT);
        assert_eq!(post.title, "Backup created");
        let reveal = Target::Action(AppAction::RevealFile { path: path.to_path_buf() });
        assert_eq!(post.tap, Some(reveal.clone()));
        assert_eq!(post.buttons, vec![("Open folder".to_owned(), reveal)]);
        assert!(!post.low_priority);
    }

    #[test]
    fn a_progress_notice_is_low_priority_and_has_no_button() {
        let post = backup_post(&kuro_backup::notify::create_progress());
        assert!(post.low_priority);
        assert!(post.buttons.is_empty());
        assert!(post.tap.is_none());
    }

    #[test]
    fn a_saved_file_makes_its_folder_revealable_and_nothing_else() {
        // T-35a-023, T-35a-024
        allow_folder_of(Path::new("/home/u/Pictures/Manga/page.png"));
        assert!(announced_folders().contains(&PathBuf::from("/home/u/Pictures/Manga")));
        assert!(!announced_folders().contains(&PathBuf::from("/home/u/.ssh")));
    }

    #[test]
    fn only_a_backup_created_call_becomes_the_reveal_notice() {
        let path = "/home/u/backups/kurobayomi-2026.tachibk";
        let post = created_post(kuro_backup::notify::ID_BACKUP_RESULT, "Backup created", Some(path)).expect("a created notice");
        assert_eq!(post.buttons[0].0, "Open folder");
        let skipped = format!("{path}\n1 extension was skipped");
        let post = created_post(kuro_backup::notify::ID_BACKUP_RESULT, "Backup created", Some(&skipped)).unwrap();
        assert_eq!(post.body.as_deref(), Some(skipped.as_str()));
        assert!(created_post(kuro_backup::notify::ID_BACKUP_RESULT, "Backup failed", Some(path)).is_none());
        assert!(created_post(kuro_backup::notify::ID_BACKUP_RESULT, "Backup created", Some("relative/path")).is_none());
        assert!(created_post(kuro_backup::notify::ID_BACKUP_RESULT, "Backup created", None).is_none());
        assert!(created_post("restore", "Backup created", Some(path)).is_none());
    }
}
