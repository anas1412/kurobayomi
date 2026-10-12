//! GTK glue for local folders (area 43, wire-local-anime): the library menu's "Add local … folder" and "Rescan local …
//! folders" for both kinds, and the add-manga shortcut. Scanning and storing run on a worker thread; the toast and the
//! library refresh run back on the GTK thread.

use std::sync::Arc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use kuro_db::Db;
use kuro_platform::kind::Kind;

use super::sync::{self, Failure, Outcome};

/// The window action that asks for a manga folder and adds it (`win.add-local-manga`, `Ctrl+Shift+O`).
pub const ADD_ACTION: &str = "add-local-manga";

/// Registers the add-manga window action and its shortcut.
pub fn attach(window: &gtk::ApplicationWindow) {
    let add = gio::SimpleAction::new(ADD_ACTION, None);
    add.connect_activate(|_, _| add_folder(Kind::Manga));
    window.add_action(&add);
    if let Some(app) = window.application() {
        app.set_accels_for_action(&format!("win.{ADD_ACTION}"), &["<Primary><Shift>o"]);
    }
}

/// Asks for a folder and adds it: a manga folder is one entry, an anime folder holds one folder per series.
pub fn add_folder(kind: Kind) {
    let title = match kind {
        Kind::Manga => "Add local manga folder",
        Kind::Anime => "Add local anime folder",
    };
    let dialog = gtk::FileDialog::builder().title(title).modal(true).build();
    dialog.select_folder(Some(&crate::ui::window()), gio::Cancellable::NONE, move |picked| {
        let Some(folder) = picked.ok().and_then(|file| file.path()) else { return };
        run(kind, move |db, now| sync::add(db, kind, &folder, now));
    });
}

/// Rescans the local entries of `kind` already in the library.
pub fn rescan(kind: Kind) {
    run(kind, move |db, now| sync::rescan(db, kind, now));
}

/// Runs `job` on a worker thread with the database of `kind`, then shows its result as toasts and reloads the library.
fn run(kind: Kind, job: impl FnOnce(&Arc<Db>, i64) -> Result<Vec<Outcome>, String> + Send + 'static) {
    let db = crate::data::app().db(kind);
    let (sender, receiver) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = sender.send_blocking(job(&db, chrono::Utc::now().timestamp_millis()));
    });
    glib::spawn_future_local(async move {
        let Ok(result) = receiver.recv().await else { return };
        for text in messages(kind, &result) {
            crate::toast::notify(text);
        }
        crate::library::mount::refresh(kind);
    });
}

/// The toast texts of a result: one per manga folder, one for a whole anime folder (a series that failed is logged).
fn messages(kind: Kind, result: &Result<Vec<Outcome>, String>) -> Vec<String> {
    let outcomes = match result {
        Ok(outcomes) => outcomes,
        Err(e) => return vec![format!("Could not scan the folder: {e}")],
    };
    match kind {
        Kind::Manga => outcomes
            .iter()
            .map(|o| match &o.result {
                Ok(done) => format!("{}: {} new, {} renamed chapters", o.title, done.added, done.updated),
                Err(Failure::Read(e)) => format!("Could not read the folder: {e}"),
                Err(Failure::Save(e)) => format!("Could not save {}: {e}", o.title),
            })
            .collect(),
        Kind::Anime => {
            let done: Vec<&sync::Synced> = outcomes.iter().filter_map(|o| o.result.as_ref().ok()).collect();
            let added: usize = done.iter().map(|d| d.added).sum();
            let removed: usize = done.iter().map(|d| d.removed).sum();
            vec![format!("Local anime: {} series, {added} episodes added, {removed} removed", done.len())]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_flow::sync::Synced;

    fn outcome(title: &str, result: Result<Synced, Failure>) -> Outcome {
        Outcome { title: title.into(), result }
    }

    fn synced(added: usize, updated: usize, removed: usize) -> Synced {
        Synced { entry_id: 1, added, updated, removed }
    }

    #[test]
    fn a_manga_toast_per_folder_and_one_anime_toast_with_the_counts() {
        let manga = Ok(vec![
            outcome("Series", Ok(synced(2, 1, 0))),
            outcome("Gone", Err(Failure::Read("no such folder".into()))),
            outcome("Locked", Err(Failure::Save("disk full".into()))),
        ]);
        assert_eq!(
            messages(Kind::Manga, &manga),
            ["Series: 2 new, 1 renamed chapters", "Could not read the folder: no such folder", "Could not save Locked: disk full"]
        );
        let anime = Ok(vec![outcome("A", Ok(synced(2, 0, 1))), outcome("B", Ok(synced(1, 4, 0))), outcome("C", Err(Failure::Read("x".into())))]);
        assert_eq!(messages(Kind::Anime, &anime), ["Local anime: 2 series, 3 episodes added, 1 removed"]);
        assert_eq!(messages(Kind::Anime, &Err("disk gone".into())), ["Could not scan the folder: disk gone"]);
    }
}
