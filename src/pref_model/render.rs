//! Dispatch from one settings item to its row. The match is exhaustive over the 13 variants (no `_` arm).

use gtk::prelude::*;
use gtk::{Image, ListBoxRow};

use super::model::{mpv_subtitle, PreferenceItem};
use super::strings;
use super::widgets::{info, list, multi_select, edit_text, row, slider, switch, text, TRACKER_CHECK_SIZE};

/// The row for one item, or `None` when the item renders nothing (a blank info block).
pub fn item_row(item: &PreferenceItem) -> Option<ListBoxRow> {
    let built = match item {
        PreferenceItem::Text { common, on_click } => text::build(common, on_click.clone(), None).row,
        PreferenceItem::Switch { common, pref, veto } => switch::build(common, pref.clone(), veto.clone()).row,
        PreferenceItem::Slider { common, value, range, on_change, .. } => {
            slider::row(common, *value, range.clone(), on_change.clone())
        }
        PreferenceItem::List { common, pref, entries, veto } => {
            list::row(common, pref.clone(), entries.clone(), veto.clone()).row
        }
        PreferenceItem::BasicList { common, value, entries, on_change } => {
            list::basic_row(common, value.clone(), entries.clone(), on_change.clone()).row
        }
        PreferenceItem::MultiSelect { common, pref, entries, veto } => {
            multi_select::row(common, pref.clone(), entries.clone(), veto.clone()).row
        }
        PreferenceItem::EditText { common, pref, dialog_subtitle, can_be_blank, validate, error_message, veto } => {
            edit_text::row(
                common,
                pref.clone(),
                dialog_subtitle.clone(),
                *can_be_blank,
                validate.clone(),
                error_message.clone(),
                veto.clone(),
            )
            .row
        }
        // Multi-line and MPV editors are not wired yet (task T-47ba-044). Shown read-only meanwhile.
        PreferenceItem::MultiLineEdit { common, pref, .. } => read_only(&common.title, &pref.get()),
        PreferenceItem::MpvConf { common, pref, .. } => read_only(&common.title, &mpv_subtitle(&pref.get())),
        PreferenceItem::EditTextInfo { common, pref, .. } => read_only(&common.title, &pref.get()),
        PreferenceItem::Tracker { name, logged_in, on_login, on_logout } => {
            tracker_row(name, *logged_in, on_login.clone(), on_logout.clone())
        }
        PreferenceItem::Info { text, .. } => return info::build(text),
        PreferenceItem::Custom { build, .. } => custom_row(build()),
    };
    Some(built)
}

fn read_only(title: &str, value: &str) -> ListBoxRow {
    row::build(row::RowSpec { title, subtitle: Some(value), icon: None, trailing: None, on_click: None }).row
}

fn tracker_row(name: &str, logged_in: bool, on_login: std::rc::Rc<dyn Fn()>, on_logout: std::rc::Rc<dyn Fn()>) -> ListBoxRow {
    let check = logged_in.then(|| {
        let label = strings::logged_in();
        let image = Image::from_icon_name("object-select-symbolic");
        image.set_pixel_size(TRACKER_CHECK_SIZE);
        image.set_tooltip_text(Some(&label));
        image.set_accessible_role(gtk::AccessibleRole::Img);
        image.update_property(&[gtk::accessible::Property::Label(&label)]);
        image
    });
    let trailing = check.as_ref().map(|image| image.clone().upcast::<gtk::Widget>());
    let built = row::build(row::RowSpec {
        title: name,
        subtitle: None,
        icon: None,
        trailing: trailing.as_ref(),
        on_click: Some(std::rc::Rc::new(move || if logged_in { on_logout() } else { on_login() })),
    });
    built.row
}

fn custom_row(widget: gtk::Widget) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&widget));
    row
}
