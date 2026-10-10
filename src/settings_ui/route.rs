//! Puts the screens of this module into the app (task T-47a-041). `screens::body` asks [`body`] first; an id this
//! module builds shows rows bound to the typed preferences, so the rule table decides which rows exist. Where the
//! typed rows do not yet cover the whole spec page, the spec page is built and its rows are replaced one by one
//! (`merge.rs`).
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::pref_model::page;
use crate::pref_model::spec_convert::Registry;
use crate::screens::{built, Built};
use crate::settings_ui::category::Category;
use crate::settings_ui::merge::merge;
use crate::settings_ui::reader::rules::ReaderState;
use crate::settings_ui::{about, advanced, appearance, browse, cache, chooser, downloads, library, library_export, player, reader, rows, search, security, storage, tracking, usage, view};
use kuro_data::extension_repo::ExtensionRepoRepo;
use kuro_data::extension_store::ExtensionStoreRepo;

/// The settings ids this module builds.
pub fn handles(id: &str) -> bool {
    matches!(
        id,
        "settings_appearance"
            | "settings_library"
            | "settings_reader"
            | "settings_player"
            | "settings_player_gestures"
            | "settings_player_custom_buttons"
            | "settings_player_editor"
            | "settings_player_code_editor"
            | "settings_browse"
            | "settings_search"
            | "settings_data_storage"
            | "settings_tracking"
            | "settings_security"
            | "settings_about"
            | "settings_advanced"
            | "settings_player_internal"
            | "settings_player_decoder"
            | "settings_player_subtitle"
            | "settings_player_audio"
            | "settings_player_advanced"
            | "settings_downloads"
            | "settings_open_source_licenses"
            | "settings_worker_info"
    )
}

/// Every preference section over the app's store. Tracker secrets are not read by these pages, so the secret store
/// is the in-memory one.
pub fn prefs() -> Prefs {
    Prefs::with_store(crate::data::app().base.store().clone(), Arc::new(kuro_prefs::memory::MemorySecrets::default()))
}

/// The spec tree of a screen, built with the registry (no typed rows yet).
pub fn spec_tree(id: &str, store: &kuro_prefs::store::PrefStore) -> Vec<Preference> {
    let registry = crate::ui::with_spec(Registry::new);
    let navigate: Rc<dyn Fn(&str)> = Rc::new(|target| crate::ui::open(target));
    registry.build(id, store, &navigate).unwrap_or_default()
}

/// Gives every icon-only button of a toolbar its tooltip as accessible name (T-47a-008), so a screen reader says
/// `Tracking guide` or `Search` instead of the icon.
fn named(widgets: Vec<gtk::Widget>) -> Vec<gtk::Widget> {
    for widget in &widgets {
        if let Some(tip) = widget.tooltip_text() {
            widget.update_property(&[gtk::accessible::Property::Label(&tip)]);
        }
    }
    widgets
}

/// The toolbar of a spec screen, every icon-only button named by its tooltip.
pub fn toolbar(id: &str) -> Vec<gtk::Widget> {
    named(crate::ui::toolbar(&crate::ui::screen(id)))
}

/// Where the help action of Data and storage goes (`SettingsDataScreen.HELP_URL`).
const STORAGE_HELP_URL: &str = "https://aniyomi.org/docs/faq/storage";
/// The toolbar of a screen whose help action (the spec's `Tracking guide` button) opens `url` in the browser.
fn help_toolbar(id: &str, url: &'static str) -> Vec<gtk::Widget> {
    let widgets = toolbar(id);
    if let Some(help) = widgets.iter().find(|w| w.tooltip_text().as_deref() == Some("Tracking guide")).and_then(|w| w.downcast_ref::<gtk::Button>()) {
        help.connect_clicked(move |_| {
            gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
                if let Err(err) = result {
                    tracing::warn!(%err, "could not open the help page");
                }
            });
        });
    }
    widgets
}

/// The toolbar of Data and storage; its help action opens the storage FAQ.
fn data_storage_toolbar() -> Vec<gtk::Widget> {
    help_toolbar("settings_data_storage", STORAGE_HELP_URL)
}

/// The user categories of a kind (the Default category is not one of them), as the settings lists need them.
fn categories_of(kind: EntryKind) -> Vec<Category> {
    let all = crate::data::app().categories(kind).repo().all().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "the categories could not be read");
        Vec::new()
    });
    all.into_iter()
        .filter(|c| !c.is_system())
        .filter_map(|c| Some(Category { id: i32::try_from(c.id).ok()?, name: c.name, order: c.order }))
        .collect()
}

/// Pushes the Categories screen on the tab of `kind`.
fn open_categories(kind: EntryKind) {
    let page = crate::screens::build("categories");
    view::show_kind(&page, library::categories_tab(kind));
    crate::ui::push(&page);
}

fn reader_page() -> Option<Built> {
    let prefs = Rc::new(prefs());
    let store = prefs.store().clone();
    let (typed, for_state, spec_store) = (Rc::clone(&prefs), Rc::clone(&prefs), store.clone());
    Some(view::live_page(
        &store,
        move || ReaderState::from_prefs(&for_state.reader),
        move || merge(spec_tree("settings_reader", &spec_store), reader::page::rows_all(&typed)),
        toolbar("settings_reader"),
    ))
}

/// The Library page. The interval, the category sets and the rows they turn on or off are rebuilt when they change.
fn library_page() -> Option<Built> {
    let prefs = Rc::new(prefs());
    let store = prefs.store().clone();
    let (for_state, for_tree) = (Rc::clone(&prefs), Rc::clone(&prefs));
    let open: Rc<dyn Fn(EntryKind)> = Rc::new(open_categories);
    Some(view::live_page(
        &store,
        move || crate::settings_ui::library_update::rule_state(&for_state),
        move || {
            let typed = library::page_rows(&for_tree, &categories_of(EntryKind::Anime), &categories_of(EntryKind::Manga), Rc::clone(&open));
            merge(spec_tree("settings_library", for_tree.store()), typed)
        },
        toolbar("settings_library"),
    ))
}

fn appearance_page() -> Option<Built> {
    let prefs = prefs();
    let tree = merge(spec_tree("settings_appearance", prefs.store()), appearance::page_rows(&prefs));
    built(page::body(&tree).widget, toolbar("settings_appearance"))
}

fn browse_page() -> Option<Built> {
    let prefs = prefs();
    let tree = merge(spec_tree("settings_browse", prefs.store()), browse::page_rows(&prefs, repo_counts()));
    built(page::body(&tree).widget, toolbar("settings_browse"))
}

/// The internal player page. The rows that depend on the intro skip and AniSkip switches are rebuilt when those change.
fn internal_page() -> Option<Built> {
    let prefs = Rc::new(prefs());
    let store = prefs.store().clone();
    let (for_state, typed) = (Rc::clone(&prefs), Rc::clone(&prefs));
    Some(view::live_page(
        &store,
        move || (for_state.player.enable_skip_intro.get(), for_state.player.ani_skip_enabled.get()),
        move || player::controls::internal_rows(&typed),
        toolbar("settings_player_internal"),
    ))
}

/// The Downloads page: the spec rows with the typed rows in their place. The rows follow the rules of their switches.
fn downloads_page() -> Option<Built> {
    let prefs = Rc::new(prefs());
    let store = prefs.store().clone();
    let (for_state, for_tree) = (Rc::clone(&prefs), Rc::clone(&prefs));
    Some(view::live_page(
        &store,
        move || downloads::rule_state(&for_state),
        move || {
            let typed = downloads::page_rows(&for_tree, &categories_of(EntryKind::Anime), &categories_of(EntryKind::Manga));
            merge(spec_tree("settings_downloads", for_tree.store()), typed)
        },
        toolbar("settings_downloads"),
    ))
}

/// A player page of loose typed rows, with the page toolbar.
fn loose_page(id: &str, rows: Vec<PreferenceItem>) -> Option<Built> {
    built(page::body(&loose(rows)).widget, toolbar(id))
}

/// The rows of a settings screen for the search index: the spec rows, with this module's typed rows merged in.
pub fn tree(id: &str) -> Vec<Preference> {
    let prefs = prefs();
    let (anime, manga) = (categories_of(EntryKind::Anime), categories_of(EntryKind::Manga));
    tree_with(id, &prefs, &anime, &manga, repo_counts())
}

/// The stores and repos the Browse rows count (task T-47a-062), read from the app's databases.
fn repo_counts() -> browse::RepoCounts {
    let app = crate::data::app();
    let anime = ExtensionStoreRepo::new(app.anime_db.clone()).count().unwrap_or(0);
    let manga = ExtensionRepoRepo::new(app.manga_db.clone()).all().map_or(0, |repos| repos.len() as i64);
    browse::RepoCounts { anime, manga }
}

/// A page of loose typed rows: the spec page has no group titles on these rows, and the typed rows cover all of them.
fn loose(rows: Vec<PreferenceItem>) -> Vec<Preference> {
    rows.into_iter().map(rows::item).collect()
}

/// [`tree`] over the given preferences and category lists.
pub fn tree_with(id: &str, prefs: &Prefs, anime: &[Category], manga: &[Category], counts: browse::RepoCounts) -> Vec<Preference> {
    let spec = spec_tree(id, prefs.store());
    let open: Rc<dyn Fn(EntryKind)> = Rc::new(|_| {});
    match id {
        "settings_appearance" => merge(spec, appearance::page_rows(prefs)),
        "settings_library" => merge(spec, library::page_rows(prefs, anime, manga, open)),
        "settings_reader" => merge(spec, reader::page::rows_all(prefs)),
        "settings_browse" => merge(spec, browse::page_rows(prefs, counts)),
        "settings_advanced" => merge(spec, advanced::page_rows(prefs, Rc::new(|| {}))),
        "settings_data_storage" => merge(spec, data_storage_rows(prefs, Rc::new(|| {}))),
        "settings_downloads" => merge(spec, downloads::page_rows(prefs, anime, manga)),
        "settings_tracking" => merge(spec, tracking::page_rows(prefs, Rc::new(|| {}))),
        "settings_security" => merge(spec, security::page_rows(prefs)),
        "settings_about" => merge(spec, about::page_rows()),
        "settings_player_internal" => player::controls::internal_rows(prefs),
        "settings_player_decoder" => loose(player::decoder::page_rows(prefs)),
        "settings_player_subtitle" => loose(player::subtitle::page_rows(prefs)),
        "settings_player_audio" => loose(player::audio::page_rows(prefs)),
        "settings_player_advanced" => loose(player::advanced::page_rows(prefs)),
        _ => spec,
    }
}

/// The typed rows of Data and storage: the storage location row and its info text (loose), the Restore backup row and
/// the storage usage group. `changed` runs after a new folder is stored or the cache is cleared.
fn data_storage_rows(prefs: &Prefs, changed: Rc<dyn Fn()>) -> Vec<Preference> {
    let mut tree: Vec<Preference> = storage::rows(prefs, Rc::clone(&changed)).into_iter().map(rows::item).collect();
    let interval = rows::ranged_choice("Automatic backup frequency", &prefs.backup.backup_interval, |h| kuro_backup::settings::interval_label(h as u32).unwrap_or("").to_owned(), true);
    tree.push(rows::group("Backup and restore", vec![interval, restore_backup_row(), rows::info(&backup_info(prefs))]));
    tree.push(rows::group("Storage usage", vec![cache::clear_row(Rc::clone(&changed)), rows::switch("Clear episode/chapter cache on app launch", &prefs.library.auto_clear_item_cache, true)]));
    tree.push(library_export::group_row());
    tree
}

/// The spec group of the Library List dialog's switches (`export_columns`): the dialog is `library_export`, so the spec
/// group is left out of the page.
fn without_export_dialog(mut tree: Vec<Preference>) -> Vec<Preference> {
    tree.retain(|entry| !matches!(entry, Preference::Group(group) if group.title == library_export::DIALOG_TITLE));
    tree
}

/// The static backup info line of the spec page. The typed line below replaces it: it carries the last automatic backup.
pub const KEEP_COPIES: &str = "You should keep copies of backups in other places as well. Backups may contain sensitive data including any stored passwords; be careful if sharing.";

/// The backup info line: the advice to keep copies, then when the last automatic backup ran (`backup_interval` page).
fn backup_info(prefs: &Prefs) -> String {
    let now = i64::try_from(kuro_core::crash::now_ms()).unwrap_or(i64::MAX);
    let last = prefs.backup.last_auto_backup_timestamp.get();
    format!("{KEEP_COPIES}\n\n{}", kuro_backup::settings::last_auto_info(last, now))
}

/// The spec rows without the static backup info line, which the typed one replaces.
fn without_backup_info(mut tree: Vec<Preference>) -> Vec<Preference> {
    for entry in &mut tree {
        if let Preference::Group(group) = entry {
            group.items.retain(|item| !matches!(item, PreferenceItem::Info { text, .. } if text.starts_with(KEEP_COPIES)));
        }
    }
    tree
}

/// The Restore backup row: a file chooser (`Select backup file`), then the Restore backup page on that file. A restore
/// that is already running keeps the chooser shut.
fn restore_backup_row() -> PreferenceItem {
    let mut row = rows::text("Restore backup", None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(choose_backup_file));
    }
    row
}

fn choose_backup_file() {
    if crate::backup_host::restore_running() {
        crate::toast::notify("Restore is already in progress");
        return;
    }
    let dialog = gtk::FileDialog::new();
    dialog.set_title("Select backup file");
    dialog.open(Some(&crate::ui::window()), None::<&gtk::gio::Cancellable>, |result| {
        let path = match result {
            Ok(file) => file.path(),
            Err(err) => return crate::toast::notify(chooser::failure_toast(&err).unwrap_or("No file selected")),
        };
        match path {
            Some(path) => {
                crate::pref_model::backup_pages::set_restore_file(path);
                crate::ui::open("settings_restore_backup");
            }
            None => crate::toast::notify("No file selected"),
        }
    });
}

/// Puts the spec page of Data and storage, the live storage rows, the banner for a gone base folder and the real
/// usage rows into `holder`. Choosing a folder builds the page again so the subtitle shows it.
fn fill_data_storage(holder: &gtk::Box) {
    while let Some(child) = holder.first_child() {
        holder.remove(&child);
    }
    let prefs = prefs();
    let weak = holder.downgrade();
    let changed: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(holder) = weak.upgrade() {
            fill_data_storage(&holder);
        }
    });
    let mut tree = merge(without_export_dialog(without_backup_info(spec_tree("settings_data_storage", prefs.store()))), data_storage_rows(&prefs, changed));
    if let Some(banner) = storage::banner_row(&prefs) {
        tree.insert(0, rows::item(banner));
    }
    let body = page::body(&tree);
    usage::replace_usage_row(&body.widget, &usage::live_rows());
    holder.append(&body.widget);
}

fn data_storage_page() -> Option<Built> {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_vexpand(true);
    fill_data_storage(&holder);
    built(holder, data_storage_toolbar())
}

/// Security and privacy: the spec page with the notification switch in its place.
fn security_page() -> Option<Built> {
    let prefs = prefs();
    let tree = merge(spec_tree("settings_security", prefs.store()), security::page_rows(&prefs));
    built(page::body(&tree).widget, toolbar("settings_security"))
}

/// About: the spec page with the Version and Check for updates rows in their place.
fn about_page() -> Option<Built> {
    let prefs = prefs();
    let tree = merge(spec_tree("settings_about", prefs.store()), about::page_rows());
    built(page::body(&tree).widget, toolbar("settings_about"))
}

/// Tracking settings: the spec page without the help button (T-27a-047, the dropped link is in the release notes).
fn tracking_page() -> Option<Built> {
    let mut bar = toolbar("settings_tracking");
    bar.retain(|w| w.tooltip_text().as_deref() != Some("Tracking guide"));
    tracking::page(bar)
}

fn gestures_page() -> Option<Built> {
    let prefs = Rc::new(prefs());
    let store = prefs.store().clone();
    let (for_state, typed) = (Rc::clone(&prefs), Rc::clone(&prefs));
    Some(view::live_page(
        &store,
        move || for_state.gestures.default_intro_length.get(),
        move || player::seeking::page_rows(&typed),
        toolbar("settings_player_gestures"),
    ))
}

/// The screen for `id`, or `None` when this module does not build it.
pub fn body(id: &str) -> Option<Built> {
    match id {
        "settings_appearance" => appearance_page(),
        "settings_library" => library_page(),
        "settings_reader" => reader_page(),
        "settings_player" => player::root::body(crate::window_size::size_of(&crate::ui::window()).0),
        "settings_player_internal" => internal_page(),
        "settings_player_gestures" => gestures_page(),
        "settings_player_decoder" => loose_page(id, player::decoder::page_rows(&prefs())),
        "settings_player_subtitle" => loose_page(id, player::subtitle::page_rows(&prefs())),
        "settings_player_audio" => loose_page(id, player::audio::page_rows(&prefs())),
        "settings_player_advanced" => loose_page(id, player::advanced::page_rows(&prefs())),
        "settings_downloads" => downloads_page(),
        "settings_player_custom_buttons" => player::scripts::buttons_page(),
        "settings_player_editor" => player::scripts::editor_page(),
        "settings_player_code_editor" => player::scripts::code_editor_page(),
        "settings_browse" => browse_page(),
        "settings_search" => search::page(),
        "settings_data_storage" => data_storage_page(),
        "settings_advanced" => advanced::page(),
        "settings_tracking" => tracking_page(),
        "settings_security" => security_page(),
        "settings_about" => about_page(),
        "settings_open_source_licenses" => super::licenses::list_page(),
        "settings_worker_info" => super::worker_info::page(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_handled_id_is_a_spec_screen_and_the_others_are_left_alone() {
        let ids = [
            "settings_appearance",
            "settings_library",
            "settings_reader",
            "settings_player",
            "settings_player_gestures",
            "settings_player_custom_buttons",
            "settings_player_editor",
            "settings_player_code_editor",
            "settings_player_internal",
            "settings_player_decoder",
            "settings_player_subtitle",
            "settings_player_audio",
            "settings_player_advanced",
            "settings_downloads",
            "settings_browse",
            "settings_search",
            "settings_data_storage",
            "settings_advanced",
            "settings_tracking",
            "settings_security",
            "settings_about",
        ];
        crate::ui::with_spec(|spec| {
            for id in ids {
                assert!(handles(id) && spec.screen(id).is_some(), "{id}");
            }
        });
        for other in ["settings", "library", "settings_player_torrents", "settings_debug_info"] {
            assert!(!handles(other), "{other}");
        }
    }

    #[test]
    fn the_reader_page_replaces_spec_rows_by_typed_ones_and_hides_what_the_rules_turn_off() {
        let prefs = Prefs::in_memory();
        let rows = |prefs: &Prefs| merge(spec_tree("settings_reader", prefs.store()), reader::page::rows_all(prefs));
        let titles = |tree: &[Preference], group: &str| -> Vec<String> {
            tree.iter()
                .find_map(|e| match e {
                    Preference::Group(g) if g.title == group => Some(g.items.iter().map(|i| i.title().to_owned()).collect()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        let before = titles(&rows(&prefs), "Paged");
        for hidden in ["Invert split page placement", "Flip orientation of rotated wide pages"] {
            assert!(!before.iter().any(|t| t == hidden), "{hidden} is hidden while its switch is off");
        }
        assert!(before.iter().any(|t| t == "Pan wide images"), "pan is on while tap zones are enabled");
        prefs.reader.dual_page_split_paged.set(true).unwrap();
        prefs.reader.navigation_mode_pager.set(5).unwrap();
        let after = titles(&rows(&prefs), "Paged");
        assert!(after.iter().any(|t| t == "Invert split page placement"));
        assert!(!after.iter().any(|t| t == "Pan wide images" || t == "Invert tap zones"), "disabled tap zones hide pan and invert");
        assert_eq!(before.len(), 11 - 2);
        assert_eq!(after.len(), 11 - 3, "invert tap zones, pan and flip rotated are hidden; invert split is back");
        let tree = rows(&prefs);
        let eink = titles(&tree, "E-Ink");
        assert_eq!(eink, ["Flash on page change"], "the E-Ink group is typed too: with the flash off only its switch shows");
    }

    fn group_titles(tree: &[Preference], group: &str) -> Vec<String> {
        tree.iter()
            .find_map(|entry| match entry {
                Preference::Group(g) if g.title == group => Some(g.items.iter().map(|i| i.title().to_owned()).collect()),
                _ => None,
            })
            .unwrap_or_default()
    }

    #[test]
    fn the_trees_the_index_reads_carry_the_typed_rows_and_the_spec_links() {
        let prefs = Prefs::in_memory();
        let appearance = tree_with("settings_appearance", &prefs, &[], &[], browse::RepoCounts::default());
        let display = group_titles(&appearance, "Display");
        for title in ["App language", "Start Screen", "Date format", "Relative timestamps"] {
            assert!(display.iter().any(|t| t == title), "{title} is on the Display group");
        }
        let browse = tree_with("settings_browse", &prefs, &[], &[], browse::RepoCounts::default());
        assert_eq!(
            group_titles(&browse, "Sources"),
            ["Hide anime entries already in library", "Hide manga entries already in library", "Anime extension repos", "Manga extension repos"]
        );
        let library = tree_with("settings_library", &prefs, &[], &[], browse::RepoCounts::default());
        assert!(group_titles(&library, "Season").iter().any(|t| t == "Update seasons with episodes on refresh"));
        assert!(group_titles(&library, "Categories").iter().any(|t| t == "Per-category settings for sort"));
    }

    #[test]
    fn the_search_index_finds_the_subtitle_and_mpv_rows_on_their_player_pages() {
        let prefs = Prefs::in_memory();
        let index = search::index_from(|id| tree_with(id, &prefs, &[], &[], browse::RepoCounts::default()));
        let subtitle = search::hits(&index, "whitelist");
        assert!(subtitle.iter().any(|hit| hit.screen == "settings_player_subtitle"), "the Subtitles page is found");
        let mpv = search::hits(&index, "mpv");
        assert!(mpv.iter().any(|hit| hit.screen == "settings_player_advanced" && hit.title.contains("MPV")), "the Advanced page is found");
        let aniskip = search::hits(&index, "aniskip");
        assert!(aniskip.iter().any(|hit| hit.screen == "settings_player_internal"), "the intro skip group is found");
    }

    #[test]
    fn data_and_storage_carries_the_live_storage_row_in_place_of_the_spec_one() {
        let prefs = Prefs::in_memory();
        let tree = tree_with("settings_data_storage", &prefs, &[], &[], browse::RepoCounts::default());
        let Some(Preference::Item(first)) = tree.first() else { panic!("the page starts with the loose storage rows") };
        let PreferenceItem::Text { on_click, .. } = first else { panic!("the storage location row") };
        assert!(on_click.is_some(), "the row opens the folder chooser");
        assert_eq!(first.title(), "Storage location");
        assert!(group_titles(&tree, "Backup and restore").iter().any(|t| t == "Create backup"), "the rest of the spec page stays");
    }
}
