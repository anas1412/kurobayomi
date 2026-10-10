//! The Library List row under Data and storage, Export (task T-47a-091). The row opens the "Select data to include"
//! dialog; Export asks for a save file and writes the favourites as CSV (`library_csv`). The favourites are read when the
//! dialog opens, so Export is never pressed while they are still loading.
use gtk::prelude::*;
use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::library_csv::{self, Columns, Entry};
use crate::settings_ui::rows::text;

pub const ROW_TITLE: &str = "Library List";
pub const DIALOG_TITLE: &str = "Select data to include";
pub const GROUP_TITLE: &str = "Export";

/// The favourites of both databases, manga first, each in the repository's order.
fn favourites() -> Vec<Entry> {
    let app = crate::data::app();
    let manga = MangaRepo::new(app.manga_db.clone()).favorites().unwrap_or_default();
    let anime = AnimeRepo::new(app.anime_db.clone()).favorites().unwrap_or_default();
    let manga = manga.into_iter().map(|m| Entry { title: m.title, kind: "manga", author: m.author, artist: m.artist });
    let anime = anime.into_iter().map(|a| Entry { title: a.title, kind: "anime", author: a.author, artist: a.artist });
    manga.chain(anime).collect()
}

/// The row: a plain text row whose tap opens the dialog.
pub fn row() -> PreferenceItem {
    let mut row = text(ROW_TITLE, None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(std::rc::Rc::new(open_dialog));
    }
    row
}

/// The check boxes as the columns they stand for, in Android order: title, type, author, artist.
fn read(boxes: &[gtk::CheckButton; 4]) -> Columns {
    Columns { title: boxes[0].is_active(), kind: boxes[1].is_active(), author: boxes[2].is_active(), artist: boxes[3].is_active() }
}

/// Shows `columns` in the check boxes and greys the three below Title while Title is off.
fn apply(boxes: &[gtk::CheckButton; 4], columns: Columns) {
    for (button, on) in boxes.iter().zip([columns.title, columns.kind, columns.author, columns.artist]) {
        if button.is_active() != on {
            button.set_active(on);
        }
    }
    for button in &boxes[1..] {
        button.set_sensitive(columns.title);
    }
}

fn open_dialog() {
    let window = gtk::Window::builder().title(ROW_TITLE).modal(true).transient_for(&crate::ui::window()).default_width(320).resizable(false).build();
    let entries = std::rc::Rc::new(favourites());
    let boxes: [gtk::CheckButton; 4] = ["Title", "Type", "Author", "Artist"].map(|label| gtk::CheckButton::with_label(label));
    apply(&boxes, Columns::ALL);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.set_margin_top(16);
    column.set_margin_bottom(16);
    column.set_margin_start(16);
    column.set_margin_end(16);
    let heading = gtk::Label::new(Some(DIALOG_TITLE));
    heading.add_css_class("heading");
    heading.set_halign(gtk::Align::Start);
    column.append(&heading);
    for button in &boxes {
        let all = boxes.clone();
        button.connect_toggled(move |_| {
            // Title off clears the rest; any other change is kept as the user set it
            let shown = read(&all);
            apply(&all, if shown.title { shown } else { shown.with_title(false) });
        });
        column.append(button);
    }

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let export = gtk::Button::with_label("Export");
    export.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&export);
    column.append(&buttons);
    window.set_child(Some(&column));
    window.set_default_widget(Some(&export));

    let close = window.clone();
    cancel.connect_clicked(move |_| close.close());
    let (close, boxes_for_export) = (window.clone(), boxes.clone());
    export.connect_clicked(move |_| {
        let columns = read(&boxes_for_export);
        let body = library_csv::export(&entries, columns);
        let dialog = gtk::FileDialog::new();
        dialog.set_initial_name(Some(library_csv::FILE_NAME));
        let done = close.clone();
        dialog.save(Some(&close), None::<&gtk::gio::Cancellable>, move |result| {
            let Some(path) = result.ok().and_then(|file| file.path()) else { return };
            match std::fs::write(&path, body.as_bytes()) {
                Ok(()) => crate::toast::notify(library_csv::EXPORTED),
                Err(err) => {
                    tracing::warn!(%err, "the library was not exported");
                    crate::toast::notify(library_csv::FAILED);
                }
            }
            done.close();
        });
    });
    window.present();
}

/// The Export group with the Library List row.
pub fn group_row() -> crate::pref_model::model::Preference {
    crate::settings_ui::rows::group(GROUP_TITLE, vec![row()])
}
