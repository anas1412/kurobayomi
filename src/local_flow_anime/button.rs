//! The "Add local anime folder" header action of the anime library (wire-local-anime). The folder dialog picks the root
//! that holds one folder per series. The scan runs on a worker thread, and its result is a toast.

use std::path::PathBuf;

use gtk::prelude::*;
use gtk::{gio, glib};

use super::sync::{ScanReport, scan_root};

/// The header button. Choosing a folder scans it, so choosing the same folder again is a rescan that keeps seen state.
pub fn add_button() -> gtk::Button {
    let button = crate::ui::icon_button("folder-new-symbolic", "Add local anime folder");
    button.connect_clicked(|_| choose_folder());
    button
}

fn choose_folder() {
    let dialog = gtk::FileDialog::builder().title("Add local anime folder").modal(true).build();
    dialog.select_folder(Some(&crate::ui::window()), gio::Cancellable::NONE, |picked| {
        let Some(root) = picked.ok().and_then(|file| file.path()) else { return };
        scan_in_background(root);
    });
}

fn scan_in_background(root: PathBuf) {
    let db = crate::data::app().anime_db.clone();
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(scan_root(&db, &root));
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            crate::toast::notify(message(&result));
        }
    });
}

/// The toast text for a scan result.
fn message(result: &Result<ScanReport, String>) -> String {
    match result {
        Ok(report) => format!("Local anime: {} series, {} episodes added, {} removed", report.series, report.episodes_added, report.episodes_removed),
        Err(e) => format!("Could not scan the folder: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toast_reports_the_counts_and_the_failure() {
        let ok = Ok(ScanReport { series: 2, episodes_added: 3, episodes_removed: 1 });
        assert_eq!(message(&ok), "Local anime: 2 series, 3 episodes added, 1 removed");
        assert_eq!(message(&Err("disk gone".into())), "Could not scan the folder: disk gone");
    }
}
