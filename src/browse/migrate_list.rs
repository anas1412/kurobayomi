//! The manga Migrate tab (T-40b-070): the library's sources with their favourite counts, in the stored sort. The list is
//! the counts the library holds, each source taken from the directory or, when it is not installed, a stub (`getOrStub`);
//! the local source is left out by the sort, and an empty library shows `information_empty_library` (E-MS-01, E-MS-02).

use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use kuro_domain::source::migration::sort_migration_sources;
use kuro_domain::source::model::Source;
use kuro_prefs::source::{MigrationDirection, MigrationSortingMode, SourcePrefs};

use super::directory::{SourceDirectory, source_or_stub};
use super::pages::{MigrateSourcesNav, MigrateSourcesSnapshot, migrate_sources};

/// The display order of the migration list: every library source with its count, in `mode` and `direction`. A source the
/// directory does not know is a stub and, with no name, shows its id.
pub fn migrate_list(counts: Vec<(i64, i64)>, directory: Option<&dyn SourceDirectory>, mode: MigrationSortingMode, direction: MigrationDirection) -> Vec<(Source, i64)> {
    let list = counts.into_iter().map(|(id, count)| (source_or_stub(directory, id), count)).collect();
    let mut sorted = sort_migration_sources(list, mode, direction);
    for (source, _) in &mut sorted {
        if source.is_stub && source.name.trim().is_empty() {
            source.name = source.id.to_string();
        }
    }
    sorted
}

pub struct MigrateDeps {
    pub prefs: Rc<SourcePrefs>,
    /// `(source id, favourite count)` for every source with a favourite, handed to the callback on the GTK thread.
    pub counts: Rc<dyn Fn(Box<dyn FnOnce(Vec<(i64, i64)>)>)>,
    pub directory: Rc<dyn Fn() -> Option<Arc<dyn SourceDirectory>>>,
    /// A row: the favourites of that source.
    pub open: Rc<dyn Fn(i64)>,
    /// A long press copies the source id.
    pub copy_id: Rc<dyn Fn(i64)>,
}

/// The tab. It reads the counts each time it is shown and after a change of the sort.
pub fn migrate_tab(deps: MigrateDeps) -> gtk::Box {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let deps = Rc::new(deps);
    let shown = (Rc::clone(&deps), holder.downgrade());
    holder.connect_map(move |_| draw(Rc::clone(&shown.0), shown.1.clone()));
    draw(deps, holder.downgrade());
    holder
}

/// A sort toggle: `flip` changes the stored sort, then the list is drawn again once the click has finished (it replaces the
/// button that was pressed).
fn toggle(deps: &Rc<MigrateDeps>, holder: &gtk::Box, flip: fn(&SourcePrefs)) -> Rc<dyn Fn()> {
    let (deps, holder) = (Rc::clone(deps), holder.downgrade());
    Rc::new(move || {
        flip(&deps.prefs);
        let (deps, holder) = (Rc::clone(&deps), holder.clone());
        gtk::glib::idle_add_local_once(move || draw(deps, holder));
    })
}

fn flip_mode(prefs: &SourcePrefs) {
    let next = match prefs.migration_sorting_mode.get() {
        MigrationSortingMode::ALPHABETICAL => MigrationSortingMode::TOTAL,
        MigrationSortingMode::TOTAL => MigrationSortingMode::ALPHABETICAL,
    };
    if let Err(error) = prefs.migration_sorting_mode.set(next) {
        tracing::warn!(%error, "the migration sort could not be stored");
    }
}

fn flip_direction(prefs: &SourcePrefs) {
    let next = match prefs.migration_sorting_direction.get() {
        MigrationDirection::ASCENDING => MigrationDirection::DESCENDING,
        MigrationDirection::DESCENDING => MigrationDirection::ASCENDING,
    };
    if let Err(error) = prefs.migration_sorting_direction.set(next) {
        tracing::warn!(%error, "the migration direction could not be stored");
    }
}

/// Reads the counts and replaces the tab's content with the list in the stored sort.
fn draw(deps: Rc<MigrateDeps>, holder: gtk::glib::WeakRef<gtk::Box>) {
    let counts = Rc::clone(&deps.counts);
    counts(Box::new(move |list| {
        let Some(holder) = holder.upgrade() else { return };
        let (mode, direction) = (deps.prefs.migration_sorting_mode.get(), deps.prefs.migration_sorting_direction.get());
        let directory = (deps.directory)();
        let snapshot = MigrateSourcesSnapshot { sources: migrate_list(list, directory.as_deref(), mode, direction), mode, direction };
        let nav = MigrateSourcesNav {
            open: Rc::clone(&deps.open),
            copy_id: Rc::clone(&deps.copy_id),
            toggle_mode: toggle(&deps, &holder, flip_mode),
            toggle_direction: toggle(&deps, &holder, flip_direction),
        };
        while let Some(child) = holder.first_child() {
            holder.remove(&child);
        }
        holder.append(&migrate_sources(&snapshot, nav));
    }));
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::directory::tests::{FakeDirectory, entry};

    fn directory() -> Arc<dyn SourceDirectory> {
        Arc::new(FakeDirectory { entries: vec![entry(1, "Zulu", "en", "ext.z"), entry(2, "Alpha", "en", "ext.a")], answers: Arc::new(|_, _| Ok(Vec::new())) })
    }

    #[test]
    fn the_list_keeps_stubs_first_leaves_the_local_source_out_and_names_a_stub_by_its_id() {
        let counts = vec![(1, 3), (0, 9), (9, 1), (2, 5)];
        let list = migrate_list(counts.clone(), Some(&*directory()), MigrationSortingMode::ALPHABETICAL, MigrationDirection::ASCENDING);
        assert_eq!(list.iter().map(|(s, c)| (s.id, *c)).collect::<Vec<_>>(), [(9, 1), (2, 5), (1, 3)], "E-MS-02: no local source; the stub first, then by name");
        assert!(list[0].0.is_stub && list[0].0.name == "9", "a stub with no name shows its id");
        assert!(!list[1].0.is_stub, "an installed source is not a stub");
        let by_count = migrate_list(counts, Some(&*directory()), MigrationSortingMode::TOTAL, MigrationDirection::DESCENDING);
        assert_eq!(by_count.iter().map(|(s, _)| s.id).collect::<Vec<_>>(), [2, 1, 9], "descending by count reverses the whole order, so the stub is last");
    }

    #[test]
    fn with_no_directory_every_source_of_the_library_is_a_stub() {
        let list = migrate_list(vec![(4, 2), (5, 1)], None, MigrationSortingMode::TOTAL, MigrationDirection::ASCENDING);
        assert!(list.iter().all(|(source, _)| source.is_stub), "T-40b-070: the flag comes from the directory");
        assert_eq!(list.iter().map(|(s, _)| s.id).collect::<Vec<_>>(), [5, 4]);
    }

    fn labels(root: &gtk::Widget) -> Vec<String> {
        fn walk(widget: &gtk::Widget, out: &mut Vec<String>) {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                out.push(label.label().to_string());
            }
            let mut child = widget.first_child();
            while let Some(next) = child {
                walk(&next, out);
                child = next.next_sibling();
            }
        }
        let mut out = Vec::new();
        walk(root, &mut out);
        out
    }

    fn pump(mut done: impl FnMut() -> bool) -> bool {
        let context = gtk::glib::MainContext::default();
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            while context.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        done()
    }

    fn deps(counts: Vec<(i64, i64)>, prefs: Rc<SourcePrefs>, opened: Rc<RefCell<Vec<i64>>>) -> MigrateDeps {
        MigrateDeps {
            prefs,
            counts: Rc::new(move |deliver| deliver(counts.clone())),
            directory: Rc::new(|| Some(directory())),
            open: Rc::new(move |id| opened.borrow_mut().push(id)),
            copy_id: Rc::new(|_| {}),
        }
    }

    fn the_tab_lists_the_library_sources_and_an_empty_library_shows_the_empty_state() {
        let prefs = Rc::new(SourcePrefs::new(PrefStore::in_memory()));
        let tab = migrate_tab(deps(vec![(1, 3), (2, 5)], Rc::clone(&prefs), Rc::default()));
        let text = labels(tab.upcast_ref());
        let (alpha, zulu) = (text.iter().position(|t| t == "Alpha"), text.iter().position(|t| t == "Zulu"));
        assert!(alpha.is_some() && zulu.is_some() && alpha < zulu, "listed by name: {text:?}");
        assert!(text.iter().any(|t| t == "5") && text.iter().any(|t| t == "3"), "with their favourite counts: {text:?}");

        let empty = migrate_tab(deps(Vec::new(), prefs, Rc::default()));
        assert!(labels(empty.upcast_ref()).contains(&kuro_i18n::tr(kuro_i18n::MsgKey::information_empty_library)), "E-MS-01");
    }

    fn a_sort_toggle_is_stored_and_redraws_the_list() {
        let prefs = Rc::new(SourcePrefs::new(PrefStore::in_memory()));
        let tab = migrate_tab(deps(vec![(1, 9), (2, 1)], Rc::clone(&prefs), Rc::default()));
        let buttons = || {
            fn walk(widget: &gtk::Widget, out: &mut Vec<gtk::Button>) {
                if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                    out.push(button.clone());
                }
                let mut child = widget.first_child();
                while let Some(next) = child {
                    walk(&next, out);
                    child = next.next_sibling();
                }
            }
            let mut out = Vec::new();
            walk(tab.upcast_ref(), &mut out);
            out
        };
        // the first button is the sort mode: Alphabetical becomes Total
        buttons()[0].emit_clicked();
        assert_eq!(prefs.migration_sorting_mode.get(), MigrationSortingMode::TOTAL, "the sort is stored");
        assert!(pump(|| labels(tab.upcast_ref()).contains(&kuro_i18n::tr(kuro_i18n::MsgKey::action_sort_count))), "the page shows the new sort");
        // the second button flips the direction
        buttons()[1].emit_clicked();
        assert_eq!(prefs.migration_sorting_direction.get(), MigrationDirection::DESCENDING);
    }

    pub(crate) fn smoke() {
        the_tab_lists_the_library_sources_and_an_empty_library_shows_the_empty_state();
        a_sort_toggle_is_stored_and_redraws_the_list();
    }
}
