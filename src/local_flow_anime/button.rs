//! The header controls of the anime library (wire-local-anime): "Add local anime folder" and "Rescan local anime". The
//! folder dialog picks the root that holds one folder per series. A scan runs on a worker thread, and its result is a
//! toast.

use gtk::prelude::*;
use gtk::{gio, glib};

use super::sync::{ScanReport, rescan_library, scan_root};

/// The header button. Choosing a folder scans it, so choosing the same folder again is a rescan that keeps seen state.
pub fn add_button() -> gtk::Button {
    let button = crate::ui::icon_button("folder-new-symbolic", "Add local anime folder");
    button.connect_clicked(|_| choose_folder());
    button
}

/// The header controls of the anime library: add a folder, and rescan the series already in the library.
pub fn header_actions() -> gtk::Box {
    let rescan = crate::ui::icon_button("view-refresh-symbolic", "Rescan local anime folders");
    rescan.connect_clicked(|_| rescan_in_background());
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.append(&add_button());
    row.append(&rescan);
    row
}

fn choose_folder() {
    let dialog = gtk::FileDialog::builder().title("Add local anime folder").modal(true).build();
    dialog.select_folder(Some(&crate::ui::window()), gio::Cancellable::NONE, |picked| {
        let Some(root) = picked.ok().and_then(|file| file.path()) else { return };
        scan_in_background(move |db| scan_root(db, &root));
    });
}

fn rescan_in_background() {
    scan_in_background(|db| rescan_library(db, chrono::Utc::now().timestamp_millis()));
}

/// Runs `scan` on a worker thread with the anime database, then shows its result as a toast on the GTK thread.
fn scan_in_background(scan: impl FnOnce(&std::sync::Arc<kuro_db::handle::Db>) -> Result<ScanReport, String> + Send + 'static) {
    let db = crate::data::app().anime_db.clone();
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(scan(&db));
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
