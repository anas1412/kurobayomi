//! Dispatch from one settings item to its row. The match is exhaustive over the 13 variants (no `_` arm).

use gtk::prelude::*;
use gtk::{Image, ListBoxRow};

use super::model::PreferenceItem;
use super::strings;
use super::widgets::dialog;
use crate::settings_ui::player::conf_edit;
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
        // The MPV config row is the conf editor (T-47ba-044): two lines of subtitle, a multi-line dialog, veto-gated.
        PreferenceItem::MpvConf { common, pref, can_be_blank, .. } => {
            let editor = conf_edit::conf_row(&common.title, pref, *can_be_blank);
            return item_row(&editor);
        }
        PreferenceItem::MultiLineEdit { common, pref, can_be_blank, .. } => {
            let editor = conf_edit::multi_line_row(&common.title, pref, *can_be_blank);
            return item_row(&editor);
        }
        PreferenceItem::EditTextInfo { common, pref, dialog_subtitle, can_be_blank, validate, error_message, veto } => {
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
        PreferenceItem::Tracker { name, logged_in, on_login, on_logout } => {
            tracker_row(name, *logged_in, on_login.clone(), on_logout.clone())
        }
        PreferenceItem::Info { text, .. } => return info::build(text),
        PreferenceItem::Custom { build, .. } => custom_row(build()),
    };
    Some(built)
}

fn tracker_row(name: &str, logged_in: bool, on_login: std::rc::Rc<dyn Fn()>, on_logout: std::rc::Rc<dyn Fn()>) -> ListBoxRow {
    let check = logged_in.then(|| {
        let label = strings::logged_in();
        let image = Image::from_icon_name("object-select-symbolic");
        image.set_pixel_size(TRACKER_CHECK_SIZE);
        image.set_tooltip_text(Some(&label));
        image.add_css_class("tracker-check");
        image.set_accessible_role(gtk::AccessibleRole::Img);
        image.update_property(&[gtk::accessible::Property::Label(&label)]);
        image
    });
    let trailing = check.as_ref().map(|image| image.clone().upcast::<gtk::Widget>());
    super::style::install();
    let built = row::build(row::RowSpec {
        title: name,
        subtitle: None,
        icon: None,
        trailing: trailing.as_ref(),
        on_click: Some(std::rc::Rc::new({
            let name = name.to_owned();
            move || if logged_in { confirm_logout(&name, on_logout.clone()) } else { on_login() }
        })),
    });
    if let Some(logo) = tracker_logo(name) {
        if let Some(content) = built.row.child().and_downcast::<gtk::Box>() {
            content.prepend(&logo);
        }
    }
    built.row
}

/// The bundled logo of the login row named `name`, when one is bundled.
fn tracker_logo(name: &str) -> Option<Image> {
    let id = kuro_track::login_rows::LOGIN_ROWS.iter().find(|row| row.name == name)?.tracker;
    match kuro_track::logo::logo_asset(id)? {
        kuro_track::tracker::LogoRef::Embedded(bytes) => crate::settings_ui::rows::logo_image(bytes),
        _ => None,
    }
}

/// Logging out asks first (T-47ba-050, Proposal T-E42). The login stays until the user confirms.
fn confirm_logout(name: &str, on_logout: std::rc::Rc<dyn Fn()>) {
    let body = gtk::Label::builder().label(name).xalign(0.0).build();
    dialog::show(
        &crate::ui::window(),
        strings::LOG_OUT_OF_TRACKER,
        &body,
        Some(&strings::ok()),
        move |confirmed| {
            on_logout();
            confirmed.finish();
        },
        || {},
    );
}

fn custom_row(widget: gtk::Widget) -> ListBoxRow {
    let row = ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&widget));
    row
}
