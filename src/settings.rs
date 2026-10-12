// Settings pages that are not plain preference lists (the main list and shell are in pref_model).
use std::rc::Rc;
use std::sync::Arc;

use crate::screens::{built, Built};
use crate::ui::*;
use gtk::prelude::*;
use gtk::{Box as GBox, Button, Label, Orientation as O};
use kuro_platform::kind::Kind;

pub fn body(id: &str) -> Option<Built> {
    // the settings shell, the language picker, the extension lists, the backup pages and Debug info (area 47b)
    if crate::pref_model::pages::IDS.contains(&id) {
        return crate::pref_model::pages::body(id);
    }
    match id {
        "settings_search" => search(),
        "settings_backup_schema" => backup_schema(id),
        "settings_clear_manga_database" | "settings_clear_anime_database" => clear_db(id),
        _ => None,
    }
}

/// One searchable item: its title, the `Category > Group` breadcrumb, the screen to open and the lowercase text the
/// query is matched against (the title plus the hidden terms).
struct Hit {
    title: String,
    crumb: String,
    id: String,
    haystack: String,
}

/// The first three letters of each word, so "invert" and "inverted" meet.
fn stems(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.chars().take(3).collect())
        .collect()
}

/// Hidden search terms of an item (T-02a-037): the key, table name and Rust field name of every registry row whose
/// key words (the table word and `pref`/`key` aside) all appear in the title. The spec items carry no key of their
/// own, so the title is what ties an item to its registry rows.
fn hidden_terms(title: &str) -> String {
    use kuro_prefs::registry::REGISTRY;
    let title_stems = stems(title);
    let mut terms = Vec::new();
    for meta in REGISTRY.iter().filter(|m| !m.is_family()) {
        let words: Vec<String> = meta
            .key
            .split('_')
            .filter(|w| !matches!(*w, "pref" | "key" | "v2") && *w != meta.table)
            .flat_map(stems)
            .collect();
        if !words.is_empty() && words.iter().all(|w| title_stems.contains(w)) {
            terms.extend([meta.key, meta.table, meta.field]);
        }
    }
    terms.join(" ").to_lowercase()
}

/// The settings search index: every item of every settings screen, built once.
fn search_index() -> Vec<Hit> {
    with_spec(|s| {
        let mut hits = vec![];
        for m in &s.settings_main {
            let Some(sc) = s.screen(&m.id) else { continue };
            for g in &sc.groups {
                for it in &g.items {
                    let title = item_title(it);
                    let haystack = format!("{} {}", title.to_lowercase(), hidden_terms(&title));
                    hits.push(Hit { title, crumb: format!("{} > {}", m.title, g.title.clone().unwrap_or_default()), id: m.id.clone(), haystack });
                }
            }
        }
        hits
    })
}

/// Maximum number of results shown.
const MAX_HITS: usize = 50;

/// The hits for a lowercase query of at least two characters, capped at [`MAX_HITS`].
fn find<'a>(index: &'a [Hit], query: &str) -> Vec<&'a Hit> {
    if query.len() < 2 {
        return vec![];
    }
    index.iter().filter(|h| h.haystack.contains(query)).take(MAX_HITS).collect()
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
    let index = search_index();
    entry.connect_search_changed(move |e| {
        while let Some(c) = r2.first_child() {
            r2.remove(&c);
        }
        let q = e.text().to_lowercase();
        if q.len() < 2 {
            e2.set_visible(false);
            return;
        }
        let hits = find(&index, &q);
        e2.set_visible(hits.is_empty());
        for hit in hits {
            let r = row(&hit.title, Some(&hit.crumb), None);
            r.set_activatable(true);
            let id = hit.id.clone();
            crate::widgets::row_activate::on_activate(&r, move |_| open(&id));
            r2.append(&r);
        }
    });
    built(scrolled(&col), vec![])
}

#[cfg(test)]
mod search_tests {
    use super::*;

    fn hit(title: &str) -> Hit {
        Hit { title: title.into(), crumb: "Reader > Navigation".into(), id: "settings_reader".into(), haystack: format!("{} {}", title.to_lowercase(), hidden_terms(title)) }
    }

    #[test]
    fn the_registry_key_table_and_field_are_hidden_terms() {
        let index = [hit("Dual page split"), hit("Show page number")];
        let found = find(&index, "dual_page_split_paged");
        assert_eq!(found.len(), 1, "the Rust field name matches");
        assert_eq!(found[0].title, "Dual page split");
        assert_eq!(found[0].crumb, "Reader > Navigation", "the breadcrumb is unchanged");
        assert_eq!(find(&index, "pref_dual_page_split").len(), 1, "the registry key matches");
        assert_eq!(find(&index, "split").len(), 1, "titles still match");
    }

    #[test]
    fn at_most_fifty_hits_and_nothing_below_two_characters() {
        let index: Vec<Hit> = (0..80).map(|i| hit(&format!("Option {i}"))).collect();
        assert_eq!(find(&index, "option").len(), MAX_HITS);
        assert!(find(&index, "o").is_empty());
    }
}

fn item_title(it: &crate::spec::Item) -> String {
    use crate::spec::Item::*;
    match it {
        Switch { title, .. } | List { title, .. } | Multi { title, .. } | Slider { title, .. } | Text { title, .. } | Tristate { title } | Sort { title, .. } | Chips { title, .. } | Link { title, .. } | Action { title, .. } | Info { title } => title.clone(),
        Unknown => String::new(),
    }
}

/// The debug screen of the backup schema: the 13 `Backup` fields, and the Copy button puts the text on the clipboard (T-21a-035).
fn backup_schema(id: &str) -> Option<Built> {
    let text = kuro_backup::schema_text::TEXT;
    let l = Label::builder().label(text).xalign(0.0).yalign(0.0).selectable(true).css_classes(["monospace"]).build();
    margins(&l, 16);
    let tools = toolbar(&screen(id));
    for copy in tools.iter().filter_map(|w| w.downcast_ref::<Button>()) {
        copy.connect_clicked(|_| {
            if let Some(display) = gtk::gdk::Display::default() {
                display.clipboard().set_text(text);
            }
        });
    }
    built(scrolled(&l), tools)
}

/// The clear-database screens: one row per source that has non-library entries in the database, a
/// check per row, and Delete (after the spec's confirm dialog) removes the checked sources' entries.
fn clear_db(id: &str) -> Option<Built> {
    let kind = Kind::from_anime(id == "settings_clear_anime_database");
    let db = crate::data::app().db.clone();
    // (source id, row title, non-library count), read from the database; a failed read shows nothing to clear.
    let sources: Vec<(i64, String, i64)> = db
        .read(|c| {
            let mut out = Vec::new();
            if kind.is_anime() {
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
    let sources = sorted_by_name(sources);
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
    // tapping anywhere on a row flips its check
    let tapped = Rc::clone(&entries);
    l.connect_row_activated(move |_, row| {
        if let Some((_, check, _)) = tapped.iter().find(|(_, _, r)| r == row) {
            check.set_active(!check.is_active());
        }
    });
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
                    let sql = "SELECT count(*) FROM items i JOIN entries e ON e._id = i.entry_id WHERE e.kind = ?1";
                    let before = tx.query_row(sql, [kind.as_str()], |r| r.get::<_, i64>(0))?;
                    if kind.is_anime() {
                        kuro_db::statements_anime::clear_non_library_anime_by_sources(tx, &selected)?;
                    } else {
                        kuro_db::statements_manga::clear_non_library_by_sources(tx, &selected)?;
                    }
                    let after = tx.query_row(sql, [kind.as_str()], |r| r.get::<_, i64>(0))?;
                    Ok((before - after).max(0) as usize)
                });
                match written {
                    Ok(removed) => {
                        after_clear(kind, removed);
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

/// The clear-database rows in source-name order, ignoring case.
fn sorted_by_name(mut sources: Vec<(i64, String, i64)>) -> Vec<(i64, String, i64)> {
    sources.sort_by_cached_key(|(_, name, _)| name.to_lowercase());
    sources
}

/// Housekeeping after "Clear database" committed (T-03a-028): above 1000 removed chapters or episodes the WAL is
/// checkpointed and the user is offered VACUUM. Both run on their own connection to the file, off the open handle.
fn after_clear(kind: Kind, removed: usize) {
    use crate::ui::{DialogButton, Role};
    use kuro_platform::paths::Paths as _;
    let Ok(paths) = kuro_platform::linux::LinuxPaths::system() else { return };
    let path = kuro_db::schema_unified::db_path(&paths.data_dir());
    let after = match kuro_db::schema_unified::open_file(&path).and_then(|c| kuro_db::maintenance::after_chapter_delete(&c, removed)) {
        Ok(after) => after,
        Err(e) => {
            tracing::warn!(error = %e, "database housekeeping after the clear failed");
            return;
        }
    };
    if !after.offer_vacuum {
        return;
    }
    let compact = DialogButton::new("Compact", Role::Suggested, move || {
        let path = path.clone();
        std::thread::spawn(move || {
            if let Err(e) = kuro_db::schema_unified::open_file(&path).and_then(|c| kuro_db::maintenance::vacuum(&c)) {
                tracing::warn!(error = %e, "VACUUM failed");
            }
        });
    });
    let body = crate::ui::message("This can take a while. The app stays usable, but do not close it until it is done.");
    crate::ui::dialog("Compact the database to free the space?", &body, 420, vec![DialogButton::cancel("Not now"), compact]).present();
}

/// Advanced > "Clear WebView data" (T-12a-034): the toast text for the outcome of `clear_data`.
fn clear_webview_message(result: &std::io::Result<()>) -> String {
    use kuro_i18n::{MsgKey, tr};
    tr(if result.is_ok() { MsgKey::webview_data_deleted } else { MsgKey::cache_delete_error })
}

/// Deletes the WebKit data and cache directories and shows the outcome. The failure is logged by `clear_data`.
pub fn clear_webview_data() {
    let result = kuro_platform::linux::LinuxPaths::system()
        .map_err(|e| {
            tracing::error!(error = %e, "Error occurred while clearing WebView data");
            std::io::Error::other(e)
        })
        .and_then(|paths| kuro_platform::webview::data::clear_data(&paths));
    crate::toast::notify(clear_webview_message(&result));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_database_rows_sort_by_source_name_ignoring_case() {
        let rows = vec![(1, "zeta".to_owned(), 3), (2, "Alpha".to_owned(), 1), (3, "beta".to_owned(), 2)];
        let names: Vec<String> = sorted_by_name(rows).into_iter().map(|(_, n, _)| n).collect();
        assert_eq!(names, ["Alpha", "beta", "zeta"]);
    }

    #[test]
    fn webview_clear_outcome_messages() {
        assert_eq!(clear_webview_message(&Ok(())), "WebView data cleared");
        let err = Err(std::io::Error::other("denied"));
        assert_eq!(clear_webview_message(&err), "Error occurred while clearing");
    }
}
