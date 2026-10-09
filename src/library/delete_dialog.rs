//! The remove dialog of a library selection (`DeleteLibraryEntryDialog.kt`, E-37, E-28): "from library" and
//! "downloaded" are independent boxes, OK needs one checked, and the downloaded box is left out when a local entry is
//! selected. The rules are `actions::DeleteChoice` and `actions::downloads_checkbox_offered`.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};

use crate::library::actions::DeleteChoice;

/// Shows the dialog. `on_confirm` gets the choice when OK is pressed.
pub fn show(kind: EntryKind, downloads_offered: bool, on_confirm: impl Fn(DeleteChoice) + 'static) {
    let (library_label, downloads_label) = match kind {
        EntryKind::Manga => (MsgKey::manga_from_library, MsgKey::downloaded_chapters),
        EntryKind::Anime => (MsgKey::anime_from_library, MsgKey::downloaded_episodes),
    };
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    crate::ui::margins(&column, 18);
    let from_library = gtk::CheckButton::with_label(&tr(library_label));
    column.append(&from_library);
    let downloads = gtk::CheckButton::with_label(&tr(downloads_label));
    if downloads_offered {
        column.append(&downloads);
    }
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(&tr(MsgKey::action_cancel));
    let ok = gtk::Button::builder().label(tr(MsgKey::action_ok)).css_classes(["destructive-action"]).sensitive(false).build();
    buttons.append(&cancel);
    buttons.append(&ok);
    column.append(&buttons);
    let title = gtk::Label::builder().label(tr(MsgKey::action_remove)).css_classes(["title"]).build();
    let window = crate::ui::modal(title.upcast_ref(), &column, 360, -1);

    let choice = {
        let (from_library, downloads) = (from_library.clone(), downloads.clone());
        Rc::new(move || DeleteChoice { from_library: from_library.is_active(), downloads: downloads_offered && downloads.is_active() })
    };
    for check in [&from_library, &downloads] {
        let (ok, choice) = (ok.clone(), choice.clone());
        check.connect_toggled(move |_| ok.set_sensitive(choice().confirmable()));
    }
    let w = window.clone();
    cancel.connect_clicked(move |_| w.close());
    let w = window.clone();
    ok.connect_clicked(move |_| {
        on_confirm(choice());
        w.close();
    });
    window.present();
}
