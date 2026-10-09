// Settings main menu and the settings pages that aren't plain preference lists.
use std::rc::Rc;
use std::sync::Arc;

use crate::screens::{built, Built};
use crate::ui::*;
use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};
use kuro_domain::kind::EntryKind;

pub fn body(id: &str) -> Option<Built> {
    match id {
        "settings" => main_menu(),
        "settings_search" => search(),
        "settings_app_language" => language(),
        "settings_player_custom_buttons" => cards(id, &["Skip opening", "Toggle shader"], true),
        "settings_anime_extension_stores" => cards(id, &["Store A", "Store B"], false),
        "settings_manga_extension_repos" => cards(id, &["Repo A"], false),
        "settings_player_editor" => editor(),
        "settings_player_code_editor" => code_editor(),
        "settings_worker_info" => text_dump(id, "Enqueued\n-\n\nFinished\nId: 1a2b\nTags: LibraryUpdate\nState: SUCCEEDED\n\nRunning\n-"),
        "settings_backup_schema" => text_dump(id, "message Backup {\n  repeated BackupManga backupManga = 1;\n  repeated BackupCategory backupCategories = 2;\n  repeated BackupAnime backupAnime = 3;\n}"),
        "settings_clear_manga_database" | "settings_clear_anime_database" => clear_db(id),
        "settings_open_source_licenses" => licenses(),
        "settings_open_source_library_license" => text_dump(id, "Apache License\nVersion 2.0, January 2004\nhttp://www.apache.org/licenses/"),
        _ => None,
    }
}

fn main_menu() -> Option<Built> {
    let col = column();
    let l = boxed_list();
    for m in with_spec(|s| s.settings_main.clone()) {
        let pre = gtk::Image::from_icon_name(&m.icon);
        let r = row(&m.title, m.subtitle.as_deref(), Some(gtk::Image::from_icon_name("go-next-symbolic").upcast()));
        // put the icon in front of the title column
        let inner = r.child().and_downcast::<GBox>().unwrap();
        inner.prepend(&pre);
        r.set_activatable(true);
        let id = m.id.clone();
        r.connect_activate(move |_| open(&id));
        l.append(&r);
    }
    col.append(&l);
    let s = icon_button("system-search-symbolic", "Search settings");
    s.connect_clicked(|_| open("settings_search"));
    built(scrolled(&col), vec![s.upcast()])
}

/// Search every settings item by title/subtitle; results show "Category > Group" breadcrumbs.
fn search() -> Option<Built> {
    let col = column();
    let entry = gtk::SearchEntry::builder().placeholder_text("Search settings").build();
    col.append(&entry);
    let results = boxed_list();
    col.append(&results);
    let empty = status("system-search-symbolic", "No results found");
    empty.set_visible(false);
    col.append(&empty);
    let (r2, e2) = (results.clone(), empty.clone());
    entry.connect_search_changed(move |e| {
        while let Some(c) = r2.first_child() {
            r2.remove(&c);
        }
        let q = e.text().to_lowercase();
        if q.len() < 2 {
            e2.set_visible(false);
            return;
        }
        let hits = with_spec(|s| {
            let mut hits = vec![];
            for m in &s.settings_main {
                let Some(sc) = s.screen(&m.id) else { continue };
                for g in &sc.groups {
                    for it in &g.items {
                        let t = item_title(it);
                        if t.to_lowercase().contains(&q) {
                            hits.push((t, format!("{} > {}", m.title, g.title.clone().unwrap_or_default()), m.id.clone()));
                        }
                    }
                }
            }
            hits
        });
        e2.set_visible(hits.is_empty());
        for (t, crumb, id) in hits.into_iter().take(50) {
            let r = row(&t, Some(&crumb), None);
            r.set_activatable(true);
            r.connect_activate(move |_| open(&id));
            r2.append(&r);
        }
    });
    built(scrolled(&col), vec![])
}

fn item_title(it: &crate::spec::Item) -> String {
    use crate::spec::Item::*;
    match it {
        Switch { title, .. } | List { title, .. } | Multi { title, .. } | Slider { title, .. } | Text { title, .. } | Tristate { title } | Sort { title, .. } | Chips { title, .. } | Link { title, .. } | Action { title, .. } | Info { title } => title.clone(),
        Unknown => String::new(),
    }
}

fn language() -> Option<Built> {
    let col = column();
    let l = boxed_list();
    for (native, local) in [("Default", ""), ("English", "English"), ("العربية", "Arabic"), ("Français", "French"), ("Deutsch", "German"), ("日本語", "Japanese"), ("Español", "Spanish")] {
        let check = gtk::Image::from_icon_name("object-select-symbolic");
        check.set_visible(native == "Default");
        let r = row(native, Some(local), Some(check.clone().upcast()));
        r.set_activatable(true);
        l.append(&r);
    }
    l.connect_row_activated(|l, r| {
        let mut c = l.first_child();
        while let Some(w) = c {
            if let Some(img) = w.first_child().and_then(|h| h.last_child()).and_downcast::<gtk::Image>() {
                img.set_visible(&w == r.upcast_ref::<gtk::Widget>());
            }
            c = w.next_sibling();
        }
    });
    col.append(&l);
    built(scrolled(&col), vec![])
}

fn dialog_of(id: &str, dialog: &str) {
    if let Some(d) = screen(id).dialogs.iter().find(|d| d.id == dialog) {
        show_dialog(d);
    }
}

fn first_dialog_like(id: &str, prefix: &str) -> Option<String> {
    screen(id).dialogs.iter().find(|d| d.id.starts_with(prefix)).map(|d| d.id.clone())
}

/// Card lists with an Add FAB: custom player buttons, extension stores, extension repos.
fn cards(id: &str, names: &[&str], star: bool) -> Option<Built> {
    let ov = gtk::Overlay::new();
    let col = column();
    let l = boxed_list();
    for n in names {
        let h = GBox::new(O::Horizontal, 6);
        margins(&h, 10);
        if star {
            h.append(&gtk::Image::from_icon_name("list-drag-handle-symbolic"));
        } else {
            h.append(&gtk::Image::from_icon_name("tag-symbolic"));
        }
        let name = Label::builder().label(*n).xalign(0.0).hexpand(true).build();
        h.append(&name);
        if star {
            let s = gtk::ToggleButton::builder().icon_name("non-starred-symbolic").tooltip_text("Primary button").build();
            s.connect_toggled(|b| b.set_icon_name(if b.is_active() { "starred-symbolic" } else { "non-starred-symbolic" }));
            h.append(&s);
            let e = icon_button("document-edit-symbolic", "Edit");
            let i = id.to_string();
            e.connect_clicked(move |_| dialog_of(&i, "edit_button"));
            h.append(&e);
        } else {
            h.append(&icon_button("web-browser-symbolic", "Open website"));
            h.append(&icon_button("edit-copy-symbolic", "Copy to clipboard"));
        }
        let d = icon_button("user-trash-symbolic", "Delete");
        let (i, del) = (id.to_string(), first_dialog_like(id, "delete").unwrap_or_default());
        d.connect_clicked(move |_| dialog_of(&i, &del));
        h.append(&d);
        l.append(&gtk::ListBoxRow::builder().child(&h).activatable(false).build());
    }
    col.append(&l);
    ov.set_child(Some(&scrolled(&col)));
    let add = Button::builder().label("Add").css_classes(["suggested-action", "pill"]).halign(Align::End).valign(Align::End).build();
    margins(&add, 18);
    let (i, a) = (id.to_string(), first_dialog_like(id, "add").unwrap_or_default());
    add.connect_clicked(move |_| dialog_of(&i, &a));
    ov.add_overlay(&add);
    built(ov, toolbar(&screen(id)))
}

fn editor() -> Option<Built> {
    let id = "settings_player_editor";
    let ov = gtk::Overlay::new();
    let col = column();
    let kind = gtk::DropDown::from_strings(&["Scripts", "Script options"]);
    kind.set_halign(Align::Start);
    col.append(&kind);
    let l = boxed_list();
    for (icon, f) in [("text-x-script-symbolic", "autoskip.lua"), ("text-x-script-symbolic", "aniskip.lua"), ("emblem-system-symbolic", "aniskip.conf")] {
        let menu = menu_button("view-more-symbolic", &["Rename file".into(), "Delete file".into()], |l| {
            dialog_of("settings_player_editor", if l.starts_with("Rename") { "rename_file" } else { "delete_file" })
        });
        let r = row(f, None, Some(menu.upcast()));
        r.child().and_downcast::<GBox>().unwrap().prepend(&gtk::Image::from_icon_name(icon));
        r.set_activatable(true);
        r.connect_activate(|_| open("settings_player_code_editor"));
        l.append(&r);
    }
    col.append(&l);
    ov.set_child(Some(&scrolled(&col)));
    let add = Button::builder().icon_name("list-add-symbolic").css_classes(["suggested-action", "circular"]).halign(Align::End).valign(Align::End).build();
    margins(&add, 18);
    add.connect_clicked(move |_| dialog_of(id, "create_file"));
    ov.add_overlay(&add);
    built(ov, vec![])
}

fn code_editor() -> Option<Built> {
    let id = "settings_player_code_editor";
    let buf = gtk::TextBuffer::new(None);
    buf.set_text("-- autoskip.lua\nlocal mp = require 'mp'\n\nmp.register_event('file-loaded', function()\n  mp.osd_message('loaded')\nend)\n");
    let view = gtk::TextView::builder().buffer(&buf).monospace(true).left_margin(12).top_margin(12).build();
    let save = icon_button("document-save-symbolic", "Save");
    save.set_sensitive(false);
    let s2 = save.clone();
    buf.connect_changed(move |_| s2.set_sensitive(true));
    save.connect_clicked(|b| b.set_sensitive(false));
    let _ = id;
    built(gtk::ScrolledWindow::builder().child(&view).vexpand(true).build(), vec![save.upcast()])
}

fn text_dump(id: &str, text: &str) -> Option<Built> {
    let l = Label::builder().label(text).xalign(0.0).yalign(0.0).selectable(true).css_classes(["monospace"]).build();
    margins(&l, 16);
    built(scrolled(&l), toolbar(&screen(id)))
}

/// The clear-database screens: one row per source that has non-library entries in the database, a
/// check per row, and Delete (after the spec's confirm dialog) removes the checked sources' entries.
fn clear_db(id: &str) -> Option<Built> {
    let anime = id == "settings_clear_anime_database";
    let kind = if anime { EntryKind::Anime } else { EntryKind::Manga };
    let db = crate::data::app().db(kind);
    // (source id, row title, non-library count), read from the database; a failed read shows nothing to clear.
    let sources: Vec<(i64, String, i64)> = db
        .read(|c| {
            let mut out = Vec::new();
            if anime {
                for (source, count) in kuro_db::statements_anime::non_library_counts_by_source(c)? {
                    let name = kuro_db::statements_anime::stub_source(c, source)?.map(|s| s.name);
                    out.push((source, name.filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Source {source}")), count));
                }
            } else {
                for (source, count) in kuro_db::statements_manga::mangas::get_source_ids_with_non_library_manga(c)? {
                    let name = kuro_db::statements_manga::sources::find_one(c, source)?.map(|s| s.visual_name());
                    out.push((source, name.filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Source {source}")), count));
                }
            }
            Ok(out)
        })
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "cannot list the non-library entries to clear");
            Vec::new()
        });
    let b = GBox::new(O::Vertical, 0);
    if sources.is_empty() {
        let text = screen(id).empty.unwrap_or_else(|| "Nothing to clear".to_owned());
        let label = Label::builder().label(text).wrap(true).xalign(0.0).build();
        margins(&label, 12);
        b.append(&label);
        return built(b, vec![]);
    }
    let col = column();
    let l = boxed_list();
    let entries: Rc<Vec<(i64, gtk::CheckButton, gtk::ListBoxRow)>> = Rc::new(
        sources
            .into_iter()
            .map(|(source, name, count)| {
                let check = gtk::CheckButton::new();
                let r = row(&name, Some(&format!("{count} non-library entries in database")), Some(check.clone().upcast()));
                r.child().and_downcast::<GBox>().unwrap().prepend(&gtk::Image::from_icon_name("application-x-addon-symbolic"));
                l.append(&r);
                (source, check, r)
            })
            .collect(),
    );
    col.append(&l);
    b.append(&scrolled(&col));
    let del = Button::builder().label("Delete").css_classes(["destructive-action"]).sensitive(false).build();
    margins(&del, 12);
    {
        let (entries, db, list, del_btn, id) = (Rc::clone(&entries), Arc::clone(&db), l.clone(), del.clone(), id.to_string());
        del.connect_clicked(move |_| {
            let Some(dialog) = screen(&id).dialogs.into_iter().find(|d| d.id == "confirm_clear") else { return };
            let (entries, db, list, del_btn) = (Rc::clone(&entries), Arc::clone(&db), list.clone(), del_btn.clone());
            show_dialog_then(&dialog, move || {
                let selected: Vec<i64> = entries.iter().filter(|(_, c, _)| c.is_active()).map(|(s, _, _)| *s).collect();
                if selected.is_empty() {
                    return;
                }
                let written = db.write(|tx| {
                    if anime {
                        Ok(kuro_db::statements_anime::clear_non_library_anime_by_sources(tx, &selected)?)
                    } else {
                        Ok(kuro_db::statements_manga::clear_non_library_by_sources(tx, &selected)?)
                    }
                });
                match written {
                    Ok(()) => {
                        for (source, check, r) in entries.iter() {
                            if selected.contains(source) {
                                list.remove(r);
                                check.set_active(false);
                            }
                        }
                        del_btn.set_sensitive(false);
                    }
                    Err(e) => tracing::warn!(error = %e, "clearing the selected sources failed"),
                }
            });
        });
    }
    b.append(&del);
    for (_, c, _) in entries.iter() {
        let (d, all) = (del.clone(), Rc::clone(&entries));
        c.connect_toggled(move |_| d.set_sensitive(all.iter().any(|(_, c, _)| c.is_active())));
    }
    let all = icon_button("edit-select-all-symbolic", "Select all");
    let inv = icon_button("object-flip-horizontal-symbolic", "Select inverse");
    let (e1, e2) = (Rc::clone(&entries), Rc::clone(&entries));
    all.connect_clicked(move |_| e1.iter().for_each(|(_, c, _)| c.set_active(true)));
    inv.connect_clicked(move |_| e2.iter().for_each(|(_, c, _)| c.set_active(!c.is_active())));
    built(b, vec![inv.upcast(), all.upcast()])
}

fn licenses() -> Option<Built> {
    let col = column();
    let l = boxed_list();
    for (name, author, ver, lic) in [("gtk4-rs", "gtk-rs", "0.11.5", "MIT"), ("serde", "David Tolnay", "1.0", "MIT / Apache-2.0"), ("glib", "gtk-rs", "0.22", "MIT")] {
        let r = row(name, Some(&format!("{author} · {lic}")), Some(Label::new(Some(ver)).upcast()));
        r.set_activatable(true);
        r.connect_activate(|_| open("settings_open_source_library_license"));
        l.append(&r);
    }
    col.append(&l);
    built(scrolled(&col), vec![])
}
