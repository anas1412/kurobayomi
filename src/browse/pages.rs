//! The six static builders that replace the mockup rows of `entry.rs` (T-40a-034): `sources`, `extensions`,
//! `migrate_sources`, `browse_source`, `global_search` and `migrate_entries`. Each takes a snapshot of its model and the
//! callbacks of its navigation, and draws no row that the snapshot does not hold: an empty snapshot is an empty page.
//! The screen registry calls these in place of the sample-row builders.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::model::Source;
use kuro_i18n::{Arg, MsgKey, tr, trf};
use kuro_prefs::source::{MigrationDirection, MigrationSortingMode};
use tokio::sync::watch;

use super::entries::FavoriteStore;
use super::extensions_page::{ExtNav, ExtensionBackend, ExtensionsPage};
use super::favorite_flow::{FavoriteFlow, FavoriteHooks};
use super::filter_sheet::FilterSheetView;
use super::filters::SourceFilters;
use super::grid::list_height;
use super::header::OverflowItem;
use super::header_bar::{HeaderDeps, SourceHeader, build as build_header, stub_header};
use super::icons::resolve;
use super::listing::ListItem;
use super::nav::{BackAction, action_url, back_action, last_used_write};
use super::listing_view::{ListingDeps, ListingView};
use super::names::localized_display_name;
use super::primitives::{ToastSpec, stub_source_label};
use super::route::search_listing;
use super::sources::LOCAL_SOURCE_ID;
use super::sources_page::{SourcesNav, SourcesPage};
use super::view::{CoverLoader, attach_row_gestures, source_row, spinner_row};

/// `sources`: the installed-sources tab of one kind.
pub fn sources(nav: SourcesNav, ready: watch::Receiver<bool>) -> SourcesPage {
    SourcesPage::new(nav, ready)
}

/// `extensions`: the extensions tab of one kind.
pub fn extensions(backend: Arc<dyn ExtensionBackend>, nav: ExtNav) -> ExtensionsPage {
    ExtensionsPage::new(backend, nav)
}

// ---------- migrate_sources ----------

/// The migration source picker: the sorted sources with their favourite counts, and the sort the list is in.
#[derive(Clone, Debug)]
pub struct MigrateSourcesSnapshot {
    /// Already sorted for display (`migration::sort_migration_sources`).
    pub sources: Vec<(Source, i64)>,
    pub mode: MigrationSortingMode,
    pub direction: MigrationDirection,
}

pub struct MigrateSourcesNav {
    /// A row: the favourites of that source.
    pub open: Rc<dyn Fn(i64)>,
    /// A long press copies the id.
    pub copy_id: Rc<dyn Fn(i64)>,
    pub toggle_mode: Rc<dyn Fn()>,
    pub toggle_direction: Rc<dyn Fn()>,
}

pub fn migrate_sources(snapshot: &MigrateSourcesSnapshot, nav: MigrateSourcesNav) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let sort = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    sort.set_margin_start(16);
    sort.set_margin_end(16);
    sort.set_margin_top(8);
    sort.append(&gtk::Label::builder().label(tr(MsgKey::migration_selection_prompt)).hexpand(true).xalign(0.0).build());
    let mode = gtk::Button::with_label(&tr(match snapshot.mode {
        MigrationSortingMode::ALPHABETICAL => MsgKey::action_sort_alpha,
        MigrationSortingMode::TOTAL => MsgKey::action_sort_count,
    }));
    let direction = gtk::Button::from_icon_name(match snapshot.direction {
        MigrationDirection::ASCENDING => "view-sort-ascending-symbolic",
        MigrationDirection::DESCENDING => "view-sort-descending-symbolic",
    });
    direction.set_tooltip_text(Some(&tr(match snapshot.direction {
        MigrationDirection::ASCENDING => MsgKey::action_asc,
        MigrationDirection::DESCENDING => MsgKey::action_desc,
    })));
    let toggle_mode = Rc::clone(&nav.toggle_mode);
    mode.connect_clicked(move |_| toggle_mode());
    let toggle_direction = Rc::clone(&nav.toggle_direction);
    direction.connect_clicked(move |_| toggle_direction());
    sort.append(&mode);
    sort.append(&direction);
    root.append(&sort);

    if snapshot.sources.is_empty() {
        let empty = gtk::Label::new(Some(&tr(MsgKey::information_empty_library)));
        empty.set_vexpand(true);
        root.append(&empty);
        return root;
    }
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for (source, count) in &snapshot.sources {
        let language = (!source.lang.is_empty()).then(|| localized_display_name(&source.lang));
        let badge = gtk::Label::new(Some(&count.to_string()));
        badge.add_css_class("accent");
        let icon = resolve(source.is_stub, false, source.id == LOCAL_SOURCE_ID);
        let row = source_row(&source.name, language.as_deref(), icon, None, source.is_stub, Some(badge.upcast_ref()));
        let (open, copy, id) = (Rc::clone(&nav.open), Rc::clone(&nav.copy_id), source.id);
        attach_row_gestures(&row, move || open(id), move || copy(id));
        list.append(&row);
    }
    root.append(&gtk::ScrolledWindow::builder().child(&list).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build());
    root
}

// ---------- browse_source ----------

/// The library side of a browse page: the store its covers read and write, and what the flows report back.
pub struct FavoritesDeps {
    pub store: Arc<dyn FavoriteStore>,
    pub toast: Rc<dyn Fn(ToastSpec)>,
    /// The duplicate dialog's Show manga.
    pub open_entry: Rc<dyn Fn(i64)>,
    /// The duplicate dialog's Migrate existing entry.
    pub migrate: Rc<dyn Fn(i64)>,
    pub now_ms: Rc<dyn Fn() -> i64>,
}

pub struct BrowseSourceDeps {
    pub supports_latest: bool,
    pub configurable: bool,
    /// Incognito applies to the source: opening it does not store it as the last used one (B-EC-17).
    pub incognito: bool,
    pub prefs: Rc<super::prefs::BrowsePrefs>,
    /// The source's id, name, language and stub flag are read from here. `on_long_press` and `favorite_urls` are filled
    /// in by `browse_source` from `favorites`.
    pub listing: ListingDeps,
    /// `source_defaults`, and the sheet that edits a working copy of them.
    pub filters: Rc<SourceFilters>,
    pub favorites: Option<FavoritesDeps>,
    pub on_back: Rc<dyn Fn()>,
    pub on_overflow: Rc<dyn Fn(OverflowItem)>,
    /// Opens a url in the browser (the local source's help guide).
    pub open_url: Rc<dyn Fn(&str)>,
}

/// One source's listing with its header, search and Popular, Latest and Filter chips.
pub struct BrowseSourcePage {
    pub root: gtk::Box,
    pub header: SourceHeader,
    pub listing: ListingView,
    pub search: gtk::SearchEntry,
    pub popular: gtk::ToggleButton,
    pub latest: Option<gtk::ToggleButton>,
    pub filter: Option<gtk::Button>,
    /// Keeps the favourite flow alive with the page.
    pub flow: Option<Rc<FavoriteFlow>>,
}

/// What `browse_source` builds: the listing page, or for a source that is not installed the missing-source page.
pub enum SourcePage {
    Browse(BrowseSourcePage),
    /// Title, back and the message; no search, no chips, no retry (T-40b-040).
    Stub(gtk::Box),
}

impl SourcePage {
    pub fn widget(&self) -> &gtk::Box {
        match self {
            SourcePage::Browse(page) => &page.root,
            SourcePage::Stub(root) => root,
        }
    }
}

/// The page of a source that is not installed: its name as the title, a way back and `Source not installed: <name
/// (LANG)>`, or the bare id when the name or the language is blank (T-41a-008).
fn stub_page(listing: &ListingDeps, on_back: Rc<dyn Fn()>) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&stub_header(&listing.source_name, on_back));
    let label = stub_source_label(&listing.source_name, &listing.source_lang, listing.source_id);
    let message = gtk::Label::new(Some(&trf(MsgKey::source_not_installed, &[("arg1", Arg::Str(&label))])));
    message.set_wrap(true);
    message.set_vexpand(true);
    message.set_valign(gtk::Align::Center);
    root.append(&message);
    root
}

pub fn browse_source(deps: BrowseSourceDeps) -> SourcePage {
    let BrowseSourceDeps { supports_latest, configurable, incognito, prefs, mut listing, filters, favorites, on_back, on_overflow, open_url } = deps;
    if listing.stub {
        return SourcePage::Stub(stub_page(&listing, on_back));
    }
    // opening a source stores it as the last used one, once, unless incognito applies
    let (source_name, is_local, source_id) = (listing.source_name.clone(), listing.is_local, listing.source_id);
    if let Some(id) = last_used_write(incognito, source_id) {
        if let Err(error) = prefs.sources.last_used_manga_source.set(id) {
            tracing::warn!(%error, "could not store the last used source");
        }
    }

    // the help actions open the guide; the rest go to the caller
    let on_action = Rc::clone(&listing.on_action);
    let help_url = Rc::clone(&open_url);
    listing.on_action = Rc::new(move |action| match action_url(action) {
        Some(url) => help_url(url),
        None => on_action(action),
    });
    // the long press and the live favourite flags come from the library store; the flow is built once the listing exists
    let flow_slot: Rc<RefCell<Option<Rc<FavoriteFlow>>>> = Rc::default();
    let on_long = Rc::clone(&flow_slot);
    listing.on_long_press = Rc::new(move |item| {
        let flow = on_long.borrow().clone();
        if let Some(flow) = flow {
            flow.long_press(item);
        }
    });
    listing.favorite_urls = favorites.as_ref().map(|f| f.store.favorite_urls(source_id));
    let listing_view = ListingView::new(listing, BrowseQuery::Popular);

    let search = gtk::SearchEntry::new();
    search.set_placeholder_text(Some(&tr(MsgKey::action_search_hint)));
    search.set_margin_start(12);
    search.set_margin_end(12);
    search.set_margin_top(6);
    let search_bar = gtk::Revealer::new();
    search_bar.set_child(Some(&search));
    let search_active = Rc::new(Cell::new(false));

    let header = build_header(HeaderDeps {
        source_name,
        prefs,
        is_local,
        configurable,
        // Back with an active search and text in the box clears the text first and stays (B-EC-10)
        on_back: {
            let (search, active) = (search.clone(), Rc::clone(&search_active));
            Rc::new(move || match back_action(active.get(), search.text().as_str()) {
                BackAction::ClearQuery => search.set_text(""),
                BackAction::Leave => on_back(),
            })
        },
        on_search: {
            let bar = search_bar.clone();
            let (search, active) = (search.clone(), Rc::clone(&search_active));
            Rc::new(move |on| {
                active.set(on);
                bar.set_reveal_child(on);
                if on {
                    search.grab_focus();
                }
            })
        },
        on_display_mode: {
            let listing = listing_view.clone();
            Rc::new(move |mode| listing.set_display_mode(mode))
        },
        on_overflow: {
            let open_url = Rc::clone(&open_url);
            Rc::new(move |item| match (item, is_local) {
                (OverflowItem::Help, true) => {
                    if let Some(url) = super::nav::help_url() {
                        open_url(url);
                    }
                }
                _ => on_overflow(item),
            })
        },
    });

    // a search is always a new listing, and a blank one is sent as the empty string (B-EC-08, B-EC-09). It starts from
    // the source's defaults when Popular or Latest was showing, and keeps the sheet's values when a search was running.
    let view = listing_view.clone();
    let seed = Rc::clone(&filters);
    search.connect_activate(move |entry| {
        let running = matches!(view.query(), BrowseQuery::Search(_));
        view.restart_with(search_listing(entry.text().as_str()), seed.for_search(running));
    });

    let chips = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    chips.set_margin_start(12);
    chips.set_margin_end(12);
    chips.set_margin_top(8);
    chips.set_margin_bottom(8);
    let popular = gtk::ToggleButton::with_label(&tr(MsgKey::popular));
    popular.set_active(true);
    chips.append(&popular);
    let mut latest_chip = None;
    if supports_latest {
        let latest = gtk::ToggleButton::with_label(&tr(MsgKey::latest));
        latest.set_group(Some(&popular));
        chips.append(&latest);
        let (view, seed) = (listing_view.clone(), Rc::clone(&filters));
        latest.connect_toggled(move |button| {
            if button.is_active() {
                view.restart_with(BrowseQuery::Latest, seed.for_search(false));
            }
        });
        latest_chip = Some(latest);
    }
    let (view, seed) = (listing_view.clone(), Rc::clone(&filters));
    popular.connect_toggled(move |button| {
        if button.is_active() {
            view.restart_with(BrowseQuery::Popular, seed.for_search(false));
        }
    });
    let mut filter_chip = None;
    if filters.has_filters() {
        let chip = gtk::Button::with_label(&tr(MsgKey::action_filter));
        let (view, filters, entry) = (listing_view.clone(), Rc::clone(&filters), search.clone());
        chip.connect_clicked(move |chip| {
            let (target, entry) = (view.clone(), entry.clone());
            let sheet_window: Rc<RefCell<Option<gtk::Window>>> = Rc::default();
            let closer = Rc::clone(&sheet_window);
            let sheet = FilterSheetView::new(
                Rc::clone(&filters),
                // Filter searches with the sheet's values and the text in the search box
                move |list| target.restart_with(search_listing(entry.text().as_str()), list),
                move || {
                    if let Some(window) = closer.borrow_mut().take() {
                        window.close();
                    }
                },
            );
            *sheet_window.borrow_mut() = Some(sheet.present_over(chip));
        });
        chips.append(&chip);
        filter_chip = Some(chip);
    }

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.append(&header.bar);
    root.append(&search_bar);
    root.append(&chips);
    root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    listing_view.widget().set_vexpand(true);
    root.append(listing_view.widget());

    // the library flows: a long press asks, a flag flips at once and a failed write puts it back
    let flow = favorites.map(|FavoritesDeps { store, toast, open_entry, migrate, now_ms }| {
        let (flag_view, toast_view) = (listing_view.downgrade(), listing_view.downgrade());
        let hooks = FavoriteHooks {
            set_flag: Rc::new(move |id, flag| {
                if let Some(view) = flag_view.upgrade() {
                    view.set_favorite(id, flag);
                }
            }),
            toast: Rc::new(move |spec| {
                if toast_view.upgrade().is_some() {
                    toast(spec);
                }
            }),
            open_entry,
            migrate,
            now_ms,
        };
        FavoriteFlow::new(store, &root, hooks)
    });
    *flow_slot.borrow_mut() = flow.clone();

    // list mode fills the window below the top bar (B§2.8)
    let (view, bar, last) = (listing_view.downgrade(), header.bar.clone(), Cell::new(-1));
    root.add_tick_callback(move |root, _| {
        let px = list_height(root.height(), bar.height());
        if px != last.replace(px) {
            match view.upgrade() {
                Some(view) => view.set_min_content_height(px),
                None => return gtk::glib::ControlFlow::Break,
            }
        }
        gtk::glib::ControlFlow::Continue
    });

    SourcePage::Browse(BrowseSourcePage { root, header, listing: listing_view, search, popular, latest: latest_chip, filter: filter_chip, flow })
}

// ---------- global_search ----------

/// What one source's search shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchState {
    Loading,
    Error(String),
    Results(Vec<ListItem>),
}

#[derive(Clone, Debug)]
pub struct SearchRow {
    pub source: Source,
    pub state: SearchState,
}

/// Global search: the query, a progress fraction, and one row per source in the order the model chose.
#[derive(Clone, Debug)]
pub struct GlobalSearchSnapshot {
    pub query: String,
    /// 0.0 to 1.0, the share of sources that answered.
    pub progress: f64,
    pub rows: Vec<SearchRow>,
}

pub struct GlobalSearchNav {
    /// Enter in the search entry.
    pub search: Rc<dyn Fn(&str)>,
    /// The arrow of a source's header: its browse listing with the query.
    pub open_source: Rc<dyn Fn(i64)>,
    /// A cover.
    pub open_entry: Rc<dyn Fn(i64, &ListItem)>,
    pub load_cover: CoverLoader,
}

/// The global search page. It shows a spinner until the source registry is initialised, then the rows of the snapshot;
/// `update` replaces the rows and the progress as results arrive, and leaves the entry alone.
pub struct GlobalSearchPage {
    root: gtk::Widget,
    entry: gtk::SearchEntry,
    progress: gtk::ProgressBar,
    column: gtk::Box,
    nav: Rc<GlobalSearchNav>,
}

pub fn global_search(nav: GlobalSearchNav, ready: watch::Receiver<bool>) -> GlobalSearchPage {
    let nav = Rc::new(nav);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some(&tr(MsgKey::action_global_search_hint)));
    entry.set_margin_start(12);
    entry.set_margin_end(12);
    entry.set_margin_top(6);
    let search = Rc::clone(&nav);
    entry.connect_activate(move |entry| (search.search)(entry.text().as_str()));
    body.append(&entry);
    let progress = gtk::ProgressBar::new();
    progress.set_visible(false);
    body.append(&progress);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    body.append(&gtk::ScrolledWindow::builder().child(&column).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build());
    // the registry's readiness gates the whole page: the existing source gate shows the spinner, then crossfades in
    let root = crate::source_gate::when_sources_ready(&kuro_core::sources::SourceGate::new(ready), &body);
    GlobalSearchPage { root, entry, progress, column, nav }
}

impl GlobalSearchPage {
    pub fn widget(&self) -> &gtk::Widget {
        &self.root
    }

    pub fn update(&self, snapshot: &GlobalSearchSnapshot) {
        if self.entry.text().as_str() != snapshot.query && !self.entry.has_focus() {
            self.entry.set_text(&snapshot.query);
        }
        self.progress.set_fraction(snapshot.progress.clamp(0.0, 1.0));
        self.progress.set_visible(snapshot.progress < 1.0 && !snapshot.rows.is_empty());
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        for row in &snapshot.rows {
            self.column.append(&search_row(row, &self.nav));
        }
    }
}

fn search_row(row: &SearchRow, nav: &GlobalSearchNav) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
    titles.set_hexpand(true);
    titles.append(&gtk::Label::builder().label(&row.source.name).xalign(0.0).css_classes(["heading"]).build());
    if !row.source.lang.is_empty() {
        titles.append(&gtk::Label::builder().label(localized_display_name(&row.source.lang)).xalign(0.0).css_classes(["dim-label"]).build());
    }
    header.append(&titles);
    let go = gtk::Button::from_icon_name("go-next-symbolic");
    go.set_tooltip_text(Some(&row.source.name));
    let (open, id) = (Rc::clone(&nav.open_source), row.source.id);
    go.connect_clicked(move |_| open(id));
    header.append(&go);
    section.append(&header);
    match &row.state {
        SearchState::Loading => section.append(&spinner_row()),
        SearchState::Error(message) => {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            line.append(&gtk::Image::from_icon_name("dialog-error-symbolic"));
            line.append(&gtk::Label::builder().label(message).xalign(0.0).wrap(true).css_classes(["error"]).build());
            section.append(&line);
        }
        SearchState::Results(items) if items.is_empty() => {
            section.append(&gtk::Label::builder().label(tr(MsgKey::no_results_found)).xalign(0.0).css_classes(["dim-label"]).build());
        }
        SearchState::Results(items) => {
            let covers = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            for item in items {
                let cell = super::view::grid_cell(
                    Some(item),
                    kuro_prefs::library::LibraryDisplayMode::ComfortableGrid,
                    &nav.load_cover,
                    &super::view::always(),
                    super::grid::COVER_WIDTH_PX,
                );
                let button = gtk::Button::builder().child(&cell).css_classes(["flat"]).build();
                let (open, item, id) = (Rc::clone(&nav.open_entry), item.clone(), row.source.id);
                button.connect_clicked(move |_| open(id, &item));
                covers.append(&button);
            }
            section.append(&gtk::ScrolledWindow::builder().child(&covers).vscrollbar_policy(gtk::PolicyType::Never).build());
        }
    }
    section
}

// ---------- migrate_entries ----------

/// One favourite of the source being migrated from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrateEntry {
    pub id: i64,
    pub title: String,
    pub cover: Option<String>,
}

pub struct MigrateEntriesNav {
    /// The row: the migrate search for this entry.
    pub open_search: Rc<dyn Fn(i64)>,
    /// The cover: the entry itself.
    pub open_entry: Rc<dyn Fn(i64)>,
    pub load_cover: CoverLoader,
}

/// The favourites of one source, sorted by case-insensitive title. A load failure shows the error and an empty list.
pub fn migrate_entries(entries: &[MigrateEntry], error: Option<&str>, nav: MigrateEntriesNav) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    if let Some(message) = error {
        root.append(&gtk::Label::builder().label(message).margin_top(8).css_classes(["error"]).build());
    }
    let mut sorted: Vec<&MigrateEntry> = entries.iter().collect();
    sorted.sort_by_key(|e| e.title.to_lowercase());
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    for entry in sorted {
        let item = ListItem { id: entry.id, title: entry.title.clone(), cover: entry.cover.clone(), ..ListItem::default() };
        let row = super::view::list_cell(Some(&item), &nav.load_cover, &super::view::always());
        let (open_search, id) = (Rc::clone(&nav.open_search), entry.id);
        // a click on the row opens the migrate search; a click on the cover opens the entry and claims the click
        attach_row_gestures(&row, move || open_search(id), || {});
        if let Some(cover) = row.first_child() {
            let open_entry = Rc::clone(&nav.open_entry);
            let click = gtk::GestureClick::new();
            click.connect_released(move |gesture, _, _, _| {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                open_entry(id);
            });
            cover.add_controller(click);
        }
        list.append(&row);
    }
    root.append(&gtk::ScrolledWindow::builder().child(&list).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build());
    root
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use super::*;

    fn labels(root: &gtk::Widget, out: &mut Vec<String>) {
        if let Some(label) = root.downcast_ref::<gtk::Label>() {
            out.push(label.label().to_string());
        }
        let mut child = root.first_child();
        while let Some(widget) = child {
            labels(&widget, out);
            child = widget.next_sibling();
        }
    }

    fn all_labels(root: &impl IsA<gtk::Widget>) -> Vec<String> {
        let mut out = Vec::new();
        labels(root.upcast_ref(), &mut out);
        out
    }

    fn nav_log() -> Rc<RefCell<Vec<String>>> {
        Rc::default()
    }

    fn no_sample_rows_in_an_empty_snapshot() {
        let log = nav_log();
        let (a, b, c, d) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let migrate = migrate_sources(
            &MigrateSourcesSnapshot { sources: vec![], mode: MigrationSortingMode::ALPHABETICAL, direction: MigrationDirection::ASCENDING },
            MigrateSourcesNav {
                open: Rc::new(move |id| a.borrow_mut().push(format!("open {id}"))),
                copy_id: Rc::new(move |id| b.borrow_mut().push(format!("copy {id}"))),
                toggle_mode: Rc::new(move || c.borrow_mut().push("mode".into())),
                toggle_direction: Rc::new(move || d.borrow_mut().push("direction".into())),
            },
        );
        let text = all_labels(&migrate);
        assert!(text.contains(&tr(MsgKey::information_empty_library)), "an empty list says so");
        assert!(!text.iter().any(|t| t.contains("Source A") || t.contains("Extension A")), "{text:?}");

        let (_tx, ready) = watch::channel(true);
        let search = global_search(
            GlobalSearchNav {
                search: Rc::new(|_| {}),
                open_source: Rc::new(|_| {}),
                open_entry: Rc::new(|_, _| {}),
                load_cover: Rc::new(|_, _| {}),
            },
            ready,
        );
        search.update(&GlobalSearchSnapshot { query: String::new(), progress: 1.0, rows: vec![] });
        assert!(!all_labels(search.widget()).iter().any(|t| t.starts_with("Source")), "no invented sources");

        let entries = migrate_entries(
            &[],
            None,
            MigrateEntriesNav { open_search: Rc::new(|_| {}), open_entry: Rc::new(|_| {}), load_cover: Rc::new(|_, _| {}) },
        );
        assert!(!all_labels(&entries).iter().any(|t| t.starts_with("Title")), "no invented entries");
    }

    fn migrate_sources_lists_the_snapshot_with_counts_and_marks_stubs() {
        let log = nav_log();
        let (a, b) = (Rc::clone(&log), Rc::clone(&log));
        let mut stub = Source::new(2, "Gone", "en");
        stub.is_stub = true;
        let snapshot = MigrateSourcesSnapshot {
            sources: vec![(Source::new(1, "Alpha", "en"), 12), (stub, 3), (Source::new(3, "Nolang", ""), 1)],
            mode: MigrationSortingMode::TOTAL,
            direction: MigrationDirection::DESCENDING,
        };
        let page = migrate_sources(
            &snapshot,
            MigrateSourcesNav {
                open: Rc::new(move |id| a.borrow_mut().push(format!("open {id}"))),
                copy_id: Rc::new(move |id| b.borrow_mut().push(format!("copy {id}"))),
                toggle_mode: Rc::new(|| {}),
                toggle_direction: Rc::new(|| {}),
            },
        );
        let text = all_labels(&page);
        for expected in ["Alpha", "12", "Gone", "3", "Nolang", "1", &tr(MsgKey::not_installed), &tr(MsgKey::action_sort_count)] {
            assert!(text.iter().any(|t| t == expected), "{expected} in {text:?}");
        }
        assert_eq!(text.iter().filter(|t| **t == tr(MsgKey::not_installed)).count(), 1, "only the stub is marked");
    }

    fn global_search_shows_each_state_of_a_source() {
        let source = |id, name: &str| Source::new(id, name, "en");
        let item = ListItem { title: "Berserk".into(), ..ListItem::default() };
        let (ready_tx, ready) = watch::channel(false);
        let page = global_search(
            GlobalSearchNav {
                search: Rc::new(|_| {}),
                open_source: Rc::new(|_| {}),
                open_entry: Rc::new(|_, _| {}),
                load_cover: Rc::new(|_, _| {}),
            },
            ready,
        );
        assert_eq!(
            page.widget().downcast_ref::<gtk::Stack>().and_then(|s| s.visible_child_name()).as_deref(),
            Some("spinner"),
            "a spinner until the registry is initialised"
        );
        ready_tx.send_replace(true);
        let context = gtk::glib::MainContext::default();
        let start = std::time::Instant::now();
        while page.widget().downcast_ref::<gtk::Stack>().and_then(|s| s.visible_child_name()).as_deref() != Some("content")
            && start.elapsed().as_secs() < 2
        {
            while context.iteration(false) {}
        }
        page.update(&GlobalSearchSnapshot {
            query: "ber".into(),
            progress: 0.5,
            rows: vec![
                SearchRow { source: source(1, "Alpha"), state: SearchState::Results(vec![item]) },
                SearchRow { source: source(2, "Beta"), state: SearchState::Results(vec![]) },
                SearchRow { source: source(3, "Gamma"), state: SearchState::Error("offline".into()) },
                SearchRow { source: source(4, "Delta"), state: SearchState::Loading },
            ],
        });
        let text = all_labels(page.widget());
        for expected in ["Alpha", "Berserk", "Beta", "Gamma", "offline", "Delta", &tr(MsgKey::no_results_found)] {
            assert!(text.iter().any(|t| t == expected), "{expected} in {text:?}");
        }
    }

    fn migrate_entries_sorts_by_title_without_case_and_shows_a_load_error() {
        let entries = [
            MigrateEntry { id: 1, title: "zebra".into(), cover: None },
            MigrateEntry { id: 2, title: "Apple".into(), cover: None },
            MigrateEntry { id: 3, title: "banana".into(), cover: None },
        ];
        let page = migrate_entries(
            &entries,
            Some("could not load"),
            MigrateEntriesNav { open_search: Rc::new(|_| {}), open_entry: Rc::new(|_| {}), load_cover: Rc::new(|_, _| {}) },
        );
        let text = all_labels(&page);
        assert_eq!(text, ["could not load", "Apple", "banana", "zebra"]);
    }

    // ----- browse_source -----

    use std::sync::Mutex;

    use kuro_prefs::store::PrefStore;
    use kuro_source::filter::Filter;
    use kuro_source::filter_list::FilterList;

    use crate::browse::listing::{ListedPage, LoadFuture, PageLoader};
    use crate::browse::primitives::EmptyAction;

    /// Serves the scripted results and records each request.
    struct Script {
        results: Mutex<Vec<Result<ListedPage, String>>>,
        requests: Mutex<Vec<(BrowseQuery, FilterList, u32)>>,
    }

    impl PageLoader for Script {
        fn load(&self, query: BrowseQuery, filters: FilterList, page: u32) -> LoadFuture {
            self.requests.lock().expect("lock").push((query, filters, page));
            let next = {
                let mut results = self.results.lock().expect("lock");
                if results.is_empty() { Ok(ListedPage { items: vec![ListItem { title: "x".into(), ..ListItem::default() }], has_next: false }) } else { results.remove(0) }
            };
            Box::pin(async move { next })
        }
    }

    struct Rig {
        page: SourcePage,
        script: Arc<Script>,
        prefs: Rc<super::super::prefs::BrowsePrefs>,
        log: Rc<RefCell<Vec<String>>>,
        _runtime: tokio::runtime::EnterGuard<'static>,
    }

    fn defaults() -> FilterList {
        FilterList(vec![Filter::check_box("Completed")])
    }

    fn source_rig(stub: bool, incognito: bool, filters: FilterList, results: Vec<Result<ListedPage, String>>) -> Rig {
        let rt: &'static tokio::runtime::Runtime =
            Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime")));
        let entered = rt.enter();
        let script = Arc::new(Script { results: Mutex::new(results), requests: Mutex::default() });
        let prefs = Rc::new(super::super::prefs::BrowsePrefs::new(PrefStore::in_memory()));
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (back, action, overflow, url) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let (_tx, ready) = watch::channel(true);
        std::mem::forget(_tx);
        let page = browse_source(BrowseSourceDeps {
            supports_latest: true,
            configurable: false,
            incognito,
            prefs: Rc::clone(&prefs),
            listing: ListingDeps {
                loader: Arc::clone(&script) as Arc<dyn PageLoader>,
                prefs: Rc::clone(&prefs),
                ready,
                stub,
                is_local: false,
                source_id: 42,
                source_name: "Gone".into(),
                source_lang: "en".into(),
                load_cover: Rc::new(|_, _| {}),
                open_entry: Rc::new(|_| {}),
                on_long_press: Rc::new(|_| {}),
                favorite_urls: None,
                on_action: Rc::new(move |a| action.borrow_mut().push(format!("action {a:?}"))),
            },
            filters: Rc::new(SourceFilters::new(filters)),
            favorites: None,
            on_back: Rc::new(move || back.borrow_mut().push("back".into())),
            on_overflow: Rc::new(move |i| overflow.borrow_mut().push(format!("overflow {i:?}"))),
            open_url: Rc::new(move |u| url.borrow_mut().push(format!("url {u}"))),
        });
        Rig { page, script, prefs, log, _runtime: entered }
    }

    fn pump_until(mut done: impl FnMut() -> bool) -> bool {
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

    fn all_buttons(root: &impl IsA<gtk::Widget>) -> Vec<gtk::Button> {
        fn go(w: &gtk::Widget, out: &mut Vec<gtk::Button>) {
            if let Some(b) = w.downcast_ref::<gtk::Button>() {
                out.push(b.clone());
            }
            let mut child = w.first_child();
            while let Some(c) = child {
                go(&c, out);
                child = c.next_sibling();
            }
        }
        let mut out = Vec::new();
        go(root.upcast_ref(), &mut out);
        out
    }

    fn a_stub_source_shows_only_its_title_back_and_the_missing_message() {
        let rig = source_rig(true, false, defaults(), vec![]);
        let SourcePage::Stub(root) = &rig.page else { panic!("a stub source gets the missing-source page") };
        let text = all_labels(root);
        assert!(text.contains(&"Gone".to_owned()), "the title is the stub's name: {text:?}");
        let expected = trf(MsgKey::source_not_installed, &[("arg1", Arg::Str("Gone (EN)"))]);
        assert!(text.contains(&expected), "{text:?}");
        let mut searches = Vec::new();
        fn find(w: &gtk::Widget, out: &mut Vec<gtk::SearchEntry>) {
            if let Some(e) = w.downcast_ref::<gtk::SearchEntry>() {
                out.push(e.clone());
            }
            let mut child = w.first_child();
            while let Some(c) = child {
                find(&c, out);
                child = c.next_sibling();
            }
        }
        find(root.upcast_ref(), &mut searches);
        assert!(searches.is_empty(), "no search bar");
        let buttons = all_buttons(root);
        assert_eq!(buttons.len(), 1, "back is the only control: no chips, no filter, no retry");
        buttons[0].emit_clicked();
        assert_eq!(*rig.log.borrow(), ["back"]);
        assert_eq!(rig.prefs.sources.last_used_manga_source.get(), -1, "a stub is not stored as the last used source");
        assert!(rig.script.requests.lock().expect("lock").is_empty(), "and never loads");
        let label = stub_source_label("", "en", 42);
        assert_eq!(label, "42", "a blank name falls back to the id");
    }

    fn opening_a_source_stores_it_as_the_last_used_unless_incognito_applies() {
        let rig = source_rig(false, false, FilterList::default(), vec![]);
        assert_eq!(rig.prefs.sources.last_used_manga_source.get(), 42);
        let incognito = source_rig(false, true, FilterList::default(), vec![]);
        assert_eq!(incognito.prefs.sources.last_used_manga_source.get(), -1, "incognito leaves it alone");
        let SourcePage::Browse(page) = &rig.page else { panic!("browse page") };
        page.popular.set_active(true);
        page.latest.as_ref().expect("latest").set_active(true);
        assert_eq!(rig.prefs.sources.last_used_manga_source.get(), 42, "switching listings does not write it again");
    }

    fn back_clears_the_search_text_first_and_then_leaves() {
        let rig = source_rig(false, false, FilterList::default(), vec![]);
        let SourcePage::Browse(page) = &rig.page else { panic!("browse page") };
        let back = all_buttons(&page.header.bar).into_iter().find(|b| b.icon_name().as_deref() == Some("go-previous-symbolic")).expect("back");
        back.emit_clicked();
        assert_eq!(*rig.log.borrow(), ["back"], "no search: Back leaves");
        rig.log.borrow_mut().clear();
        page.header.search.set_active(true);
        page.search.set_text("one piece");
        back.emit_clicked();
        assert!(rig.log.borrow().is_empty(), "an active search with text clears the text and stays");
        assert_eq!(page.search.text().as_str(), "");
        back.emit_clicked();
        assert_eq!(*rig.log.borrow(), ["back"], "the second Back leaves");
    }

    fn popular_latest_and_a_search_from_them_start_from_fresh_defaults() {
        let rig = source_rig(false, false, defaults(), vec![]);
        let SourcePage::Browse(page) = &rig.page else { panic!("browse page") };
        assert!(pump_until(|| !rig.script.requests.lock().expect("lock").is_empty()));
        // a search from Popular starts from the defaults
        page.search.set_text("berserk");
        page.search.emit_by_name::<()>("activate", &[]);
        assert!(pump_until(|| rig.script.requests.lock().expect("lock").len() == 2));
        let (query, filters, _) = rig.script.requests.lock().expect("lock")[1].clone();
        assert_eq!((query, filters), (BrowseQuery::Search("berserk".into()), defaults()));
        page.latest.as_ref().expect("latest").set_active(true);
        assert!(pump_until(|| rig.script.requests.lock().expect("lock").len() == 3));
        let (query, filters, _) = rig.script.requests.lock().expect("lock")[2].clone();
        assert_eq!((query, filters), (BrowseQuery::Latest, defaults()), "Latest takes a fresh clone of the defaults");
    }

    fn the_filter_button_of_the_sheet_searches_with_the_sheet_values() {
        let rig = source_rig(false, false, defaults(), vec![]);
        let SourcePage::Browse(page) = &rig.page else { panic!("browse page") };
        assert!(pump_until(|| !rig.script.requests.lock().expect("lock").is_empty()));
        page.search.set_text("naruto");
        page.filter.as_ref().expect("filter chip").emit_clicked();
        let sheet = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.title().as_deref() == Some(tr(MsgKey::action_filter).as_str()))
            .expect("the filter sheet window");
        let buttons = all_buttons(&sheet);
        let check = {
            fn go(w: &gtk::Widget) -> Option<gtk::CheckButton> {
                if let Some(c) = w.downcast_ref::<gtk::CheckButton>() {
                    return Some(c.clone());
                }
                let mut child = w.first_child();
                while let Some(c) = child {
                    if let Some(found) = go(&c) {
                        return Some(found);
                    }
                    child = c.next_sibling();
                }
                None
            }
            go(sheet.child().expect("child").upcast_ref()).expect("check box")
        };
        check.set_active(true);
        let filter = buttons.iter().find(|b| b.label().as_deref() == Some(tr(MsgKey::action_filter).as_str())).expect("Filter button");
        filter.emit_clicked();
        assert!(pump_until(|| rig.script.requests.lock().expect("lock").len() == 2));
        let (query, filters, _) = rig.script.requests.lock().expect("lock")[1].clone();
        assert_eq!(query, BrowseQuery::Search("naruto".into()), "the text in the search box goes with it");
        assert!(matches!(&filters.0[0], Filter::CheckBox { state: true, .. }), "and the sheet's values");
    }

    fn there_is_no_help_action_and_other_actions_go_to_the_caller() {
        let rig = source_rig(false, false, FilterList::default(), vec![Err("offline".into())]);
        let SourcePage::Browse(page) = &rig.page else { panic!("browse page") };
        assert!(pump_until(|| page.listing.widget().first_child().is_some() && !all_buttons(page.listing.widget()).is_empty()));
        let tips = |tip: String| all_buttons(page.listing.widget()).into_iter().find(|b| b.tooltip_text().as_deref() == Some(tip.as_str()));
        assert!(tips(EmptyAction::Help.label()).is_none(), "no help page, so no Help button (T-14a-047)");
        tips(EmptyAction::OpenInWebView.label()).expect("button").emit_clicked();
        assert_eq!(rig.log.borrow().last().map(String::as_str), Some("action OpenInWebView"));
    }

    pub(crate) fn smoke() {
        a_stub_source_shows_only_its_title_back_and_the_missing_message();
        opening_a_source_stores_it_as_the_last_used_unless_incognito_applies();
        back_clears_the_search_text_first_and_then_leaves();
        popular_latest_and_a_search_from_them_start_from_fresh_defaults();
        the_filter_button_of_the_sheet_searches_with_the_sheet_values();
        there_is_no_help_action_and_other_actions_go_to_the_caller();
        no_sample_rows_in_an_empty_snapshot();
        migrate_sources_lists_the_snapshot_with_counts_and_marks_stubs();
        global_search_shows_each_state_of_a_source();
        migrate_entries_sorts_by_title_without_case_and_shows_a_load_error();
    }
}
