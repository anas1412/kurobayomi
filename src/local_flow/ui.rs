//! GTK glue for local manga (area 43): the window actions that pick a folder and rescan the local entries. Scanning
//! runs on a plain thread; the database write and the toast run back on the GTK thread.

use std::path::PathBuf;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_data::manga::MangaRepo;
use kuro_local::LOCAL_SOURCE_ID;

use super::scan::{ScannedManga, scan};
use super::sync::sync;

/// The window action that asks for a folder and adds it (`win.add-local-manga`).
pub const ADD_ACTION: &str = "add-local-manga";
/// The window action that rescans every local entry in the library (`win.rescan-local-manga`).
pub const RESCAN_ACTION: &str = "rescan-local-manga";

/// Registers both window actions, with `Ctrl+Shift+O` for the add action.
pub fn attach(window: &gtk::ApplicationWindow) {
    let add = gio::SimpleAction::new(ADD_ACTION, None);
    add.connect_activate(|_, _| pick_folder());
    window.add_action(&add);
    let rescan = gio::SimpleAction::new(RESCAN_ACTION, None);
    rescan.connect_activate(|_, _| rescan_all());
    window.add_action(&rescan);
    if let Some(app) = window.application() {
        app.set_accels_for_action(&format!("win.{ADD_ACTION}"), &["<Primary><Shift>o"]);
    }
}

fn pick_folder() {
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Add local manga folder");
    dialog.select_folder(Some(&crate::ui::window()), None::<&gio::Cancellable>, |result| {
        if let Some(path) = result.ok().and_then(|folder| folder.path()) {
            import(path);
        }
    });
}

/// Rescans every entry of the library that came from a local folder.
fn rescan_all() {
    let db = crate::data::app().db(EntryKind::Manga);
    let folders: Vec<PathBuf> = MangaRepo::new(db)
        .favorites_by_source(LOCAL_SOURCE_ID)
        .unwrap_or_default()
        .iter()
        .map(|m| PathBuf::from(&m.url))
        .collect();
    for folder in folders {
        import(folder);
    }
}

/// Scans `folder` off the GTK thread, then stores it and tells the user what changed.
fn import(folder: PathBuf) {
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = sender.send_blocking(scan(&folder));
    });
    glib::spawn_future_local(async move {
        let Ok(scanned) = receiver.recv().await else { return };
        match scanned {
            Ok(scanned) => store(&scanned),
            Err(error) => crate::toast::notify(format!("Could not read the folder: {error}")),
        }
    });
}

fn store(scanned: &ScannedManga) {
    let db = crate::data::app().db(EntryKind::Manga);
    let now = chrono::Utc::now().timestamp_millis();
    match sync(&db, scanned, now) {
        Ok(done) => crate::toast::notify(format!(
            "{}: {} new, {} renamed chapters",
            scanned.title, done.new_chapters, done.renamed_chapters
        )),
        Err(error) => crate::toast::notify(format!("Could not save {}: {error}", scanned.title)),
    }
}

/// The library header button that asks for a folder and adds it.
pub fn add_button() -> gtk::Button {
    let button = crate::ui::icon_button("folder-new-symbolic", "Add local manga folder");
    button.connect_clicked(|_| pick_folder());
    button
}

/// The library header controls for local manga: add a folder, and rescan the ones already added.
pub fn header_actions() -> gtk::Box {
    let rescan = crate::ui::icon_button("view-refresh-symbolic", "Rescan local manga folders");
    rescan.set_action_name(Some(&format!("win.{RESCAN_ACTION}")));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.append(&add_button());
    row.append(&rescan);
    row
}
