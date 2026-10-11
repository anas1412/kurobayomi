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
use kuro_source::filter_list::FilterList;
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
use super::names::{localized_display_name, subtitle_display_name};
use super::primitives::{ToastSpec, stub_source_label};
use super::route::search_listing;
use super::sources::LOCAL_SOURCE_ID;
use super::sources_page::{SourcesNav, SourcesPage};
use super::view::{CoverLoader, list_row, source_row, spinner_row};

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
    /// What the page says when no source has a favourite: the manga picker says `information_empty_library`, the anime
    /// picker `texts::MIGRATE_SOURCES_EMPTY` (T-41a-036).
    pub empty_text: String,
    /// Set when the favourite counts could not be read: the page shows an error row with Retry instead of the list
    /// (E-MS-02).
    pub error: Option<String>,
}

pub struct MigrateSourcesNav {
    /// A row: the favourites of that source.
    pub open: Rc<dyn Fn(i64)>,
    /// A long press copies the id.
    pub copy_id: Rc<dyn Fn(i64)>,
    pub toggle_mode: Rc<dyn Fn()>,
    pub toggle_direction: Rc<dyn Fn()>,
    /// The header's help action: the source migration guide.
    pub open_help: Rc<dyn Fn()>,
    /// The error row's Retry: reads the counts again.
    pub retry: Rc<dyn Fn()>,
}

pub fn migrate_sources(snapshot: &MigrateSourcesSnapshot, nav: MigrateSourcesNav) -> gtk::Box {
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    // the prompt and the sort controls head the list column, as a section heading would
    let sort = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    sort.set_margin_start(16);
    sort.set_margin_end(16);
    sort.set_margin_top(12);
    sort.append(&gtk::Label::builder().label(tr(MsgKey::migration_selection_prompt)).hexpand(true).xalign(0.0).wrap(true).css_classes(["dim-label"]).build());
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
    let help = gtk::Button::from_icon_name("help-browser-symbolic");
    help.set_tooltip_text(Some(&tr(MsgKey::label_help)));
    let open_help = Rc::clone(&nav.open_help);
    help.connect_clicked(move |_| open_help());
    sort.append(&help);
    root.append(&sort);

    if let Some(message) = &snapshot.error {
        let retry = gtk::Button::with_label(&tr(MsgKey::action_retry));
        let on_retry = Rc::clone(&nav.retry);
        retry.connect_clicked(move |_| on_retry());
        root.append(&super::look::empty_state("dialog-warning-symbolic", message, Some(&retry)).0);
        return root;
    }
    if snapshot.sources.is_empty() {
        root.append(&super::look::empty_state("view-list-symbolic", &snapshot.empty_text, None).0);
        return root;
    }
    let column = crate::ui::column();
    let list = crate::ui::boxed_list();
    for (source, count) in &snapshot.sources {
        let language = (!source.lang.is_empty()).then(|| localized_display_name(&source.lang));
        let badge = super::look::badge(&migrate_count(*count), false);
        let icon = resolve(source.is_stub, false, source.id == LOCAL_SOURCE_ID);
        let row = source_row(&source.name, language.as_deref(), icon, None, source.is_stub, Some(badge.upcast_ref()));
        let (open, copy, id) = (Rc::clone(&nav.open), Rc::clone(&nav.copy_id), source.id);
        list.append(&list_row(&row, move || open(id), move || copy(id)));
    }
    column.append(&list);
    root.append(&crate::ui::scrolled(&column));
    root
}

/// The badge of a migrate source row, in words: how many library entries come from it.
pub fn migrate_count(count: impl std::fmt::Display) -> String {
    format!("{count} in library")
}

// ---------- browse_source ----------

/// The library side of a browse page: the store its covers read and write, and what the flows report back.
pub struct FavoritesDeps {
    pub store: Arc<dyn FavoriteStore>,
    pub toast: Rc<dyn Fn(ToastSpec)>,
    /// The duplicate dialog's Show manga.
    pub open_entry: Rc<dyn Fn(i64)>,
    /// The duplicate dialog's Migrate existing entry: the duplicate (old) and the browsed entry (new).
    pub migrate: Rc<dyn Fn(i64, i64)>,
    pub now_ms: Rc<dyn Fn() -> i64>,
    /// The category chooser's Edit categories: the Categories page on this kind's section.
    pub edit_categories: Rc<dyn Fn()>,
    /// An anime page: the duplicate dialog's wording.
    pub anime: bool,
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
    /// The hide-in-library preference of the source's kind, read when the page is made (`BrowsePrefs::hide_in_library`
    /// for manga): favourites are then left out of the listing (T-40b-034).
    pub hide_in_library: bool,
    /// What the page opens on: Popular, or a search (a browse page opened with a query, B§4.6).
    pub initial_query: BrowseQuery,
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
    let BrowseSourceDeps { supports_latest, configurable, incognito, prefs, mut listing, filters, favorites, hide_in_library, initial_query, on_back, on_overflow, open_url } = deps;
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
    // a page opened on a search starts from the source's defaults, like a search made from Popular
    let start_filters = if matches!(initial_query, BrowseQuery::Search(_)) { filters.for_search(false) } else { FilterList::default() };
    let listing_view = ListingView::with_filters(listing, initial_query.clone(), start_filters);
    listing_view.set_hide_in_library(hide_in_library);

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

    if let BrowseQuery::Search(text) = &initial_query {
        search.set_text(text);
        header.search.set_active(true);
    }

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
    popular.set_active(matches!(initial_query, BrowseQuery::Popular));
    chips.append(&popular);
    let mut latest_chip = None;
    if supports_latest {
        let latest = gtk::ToggleButton::with_label(&tr(MsgKey::latest));
        latest.set_group(Some(&popular));
        latest.set_active(matches!(initial_query, BrowseQuery::Latest));
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
    // Escape closes the open search first (the same step as Back, B-EC-10) and then the window's handler leaves
    {
        let (text, active, clear) = (search.clone(), Rc::clone(&search_active), search.clone());
        super::keys::attach_escape_step(
            &root,
            move || back_action(active.get(), text.text().as_str()) == BackAction::ClearQuery,
            move || clear.set_text(""),
        );
    }
    root.append(&chips);
    root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    listing_view.widget().set_vexpand(true);
    root.append(listing_view.widget());

    // the library flows: a long press asks, a flag flips at once and a failed write puts it back
    let flow = favorites.map(|FavoritesDeps { store, toast, open_entry, migrate, now_ms, edit_categories, anime }| {
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
            edit_categories,
        };
        FavoriteFlow::new_for(store, &root, hooks, anime)
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
    /// The source the search started from (a migrate search): its header carries a "\u{25B6} " prefix.
    pub origin: bool,
}

/// Global search: the query, how many sources have answered of how many, and one row per source in the order the model
/// chose.
#[derive(Clone, Debug)]
pub struct GlobalSearchSnapshot {
    pub query: String,
    /// The sources that have answered, with an error counting as an answer.
    pub answered: usize,
    /// The sources asked.
    pub total: usize,
    pub rows: Vec<SearchRow>,
}

/// Which sources the rows are limited to: the pinned ones (the default) or all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceFilter {
    #[default]
    PinnedOnly,
    All,
}

/// The chip row of the search toolbar: one of the two source filters, and the Has results toggle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchChips {
    pub filter: SourceFilter,
    pub only_has_results: bool,
}

/// A card of a result row is 96 px wide (T-40a-011).
pub const SEARCH_CARD_WIDTH_PX: i32 = 96;

/// The progress bar shows while at least one source has answered and not all of them have (E-40-57..59).
pub fn progress_visible(answered: usize, total: usize) -> bool {
    answered >= 1 && answered < total
}

/// The rows the chips leave: pinned-only keeps pinned sources and the origin source; Has results keeps rows whose
/// search found something. A chip change filters what is already here and never runs the search again (D-UI-10).
pub fn visible_rows(rows: &[SearchRow], chips: SearchChips) -> Vec<&SearchRow> {
    use kuro_domain::source::pin::Pin;
    rows.iter()
        .filter(|row| match chips.filter {
            SourceFilter::All => true,
            SourceFilter::PinnedOnly => row.origin || row.source.pin.contains(Pin::Pinned),
        })
        .filter(|row| !chips.only_has_results || matches!(&row.state, SearchState::Results(items) if !items.is_empty()))
        .collect()
}

/// The title of a source header: the name, with a prefix on the origin source.
pub fn row_title(row: &SearchRow) -> String {
    if row.origin { format!("\u{25B6} {}", row.source.name) } else { row.source.name.clone() }
}

pub struct GlobalSearchNav {
    /// Enter in the search entry.
    pub search: Rc<dyn Fn(&str)>,
    /// The arrow of a source's header, with the text in the search box now (submitted or not): its browse listing with
    /// that text (GS-EC-15).
    pub open_source: Rc<dyn Fn(i64, &str)>,
    /// A card.
    pub open_entry: Rc<dyn Fn(i64, &ListItem)>,
    /// A long press on a card: the entry's preview.
    pub preview: Rc<dyn Fn(i64, &ListItem)>,
    pub load_cover: CoverLoader,
    /// The chips as the page starts with them (the stored filter and toggle).
    pub chips: SearchChips,
    /// A chip changed: the page already filters its rows; this is for storing the choice. It runs no search.
    pub on_chips: Rc<dyn Fn(SearchChips)>,
}

/// What the page redraws from.
struct SearchView {
    column: gtk::Box,
    progress: gtk::ProgressBar,
    nav: GlobalSearchNav,
    chips: Cell<SearchChips>,
    last: RefCell<Option<GlobalSearchSnapshot>>,
    /// The text in the search box, as typed.
    typed: Rc<RefCell<String>>,
}

/// The global search page. It shows a spinner until the source registry is initialised, then the rows of the snapshot;
/// `update` replaces the rows and the progress as results arrive, and leaves the entry alone.
pub struct GlobalSearchPage {
    root: gtk::Widget,
    entry: gtk::SearchEntry,
    view: Rc<SearchView>,
}

pub fn global_search(nav: GlobalSearchNav, ready: watch::Receiver<bool>) -> GlobalSearchPage {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let entry = gtk::SearchEntry::new();
    entry.set_placeholder_text(Some(&tr(MsgKey::action_global_search_hint)));
    entry.set_margin_start(12);
    entry.set_margin_end(12);
    entry.set_margin_top(6);
    body.append(&entry);
    // Escape clears the typed query first; with none, it goes on and leaves the page
    {
        let (text, clear) = (entry.clone(), entry.clone());
        super::keys::attach_escape_step(&body, move || !text.text().is_empty(), move || clear.set_text(""));
    }
    let progress = gtk::ProgressBar::new();
    progress.set_visible(false);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_start(12);
    column.set_margin_end(12);
    let view = Rc::new(SearchView { column, progress, chips: Cell::new(nav.chips), nav, last: RefCell::new(None), typed: Rc::default() });
    let search = Rc::clone(&view);
    entry.connect_activate(move |entry| (search.nav.search)(entry.text().as_str()));
    let typing = Rc::clone(&view);
    entry.connect_changed(move |entry| *typing.typed.borrow_mut() = entry.text().to_string());
    body.append(&search_chips(&view));
    body.append(&view.progress);
    body.append(&gtk::ScrolledWindow::builder().child(&view.column).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build());
    // the registry's readiness gates the whole page: the existing source gate shows the spinner, then crossfades in
    let root = crate::source_gate::when_sources_ready(&kuro_core::sources::SourceGate::new(ready), &body);
    GlobalSearchPage { root, entry, view }
}

/// The chips: Pinned and All pick the source filter, a separator, then the Has results toggle (B§10.1).
fn search_chips(view: &Rc<SearchView>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.set_margin_start(12);
    row.set_margin_end(12);
    let chips = view.chips.get();
    let chip = |label: MsgKey, active: bool| {
        let button = gtk::ToggleButton::with_label(&tr(label));
        button.add_css_class("chip");
        button.set_active(active);
        button
    };
    let pinned = chip(MsgKey::pinned_sources, chips.filter == SourceFilter::PinnedOnly);
    let all = chip(MsgKey::all, chips.filter == SourceFilter::All);
    all.set_group(Some(&pinned));
    let has_results = chip(MsgKey::has_results, chips.only_has_results);
    for (button, filter) in [(&pinned, SourceFilter::PinnedOnly), (&all, SourceFilter::All)] {
        let view = Rc::clone(view);
        button.connect_toggled(move |button| {
            if button.is_active() {
                view.set_chips(SearchChips { filter, ..view.chips.get() });
            }
        });
    }
    let toggled = Rc::clone(view);
    has_results.connect_toggled(move |button| toggled.set_chips(SearchChips { only_has_results: button.is_active(), ..toggled.chips.get() }));
    row.append(&pinned);
    row.append(&all);
    row.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    row.append(&has_results);
    row
}

impl SearchView {
    fn set_chips(&self, chips: SearchChips) {
        if self.chips.replace(chips) == chips {
            return;
        }
        (self.nav.on_chips)(chips);
        self.redraw();
    }

    fn redraw(&self) {
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        let last = self.last.borrow();
        let Some(snapshot) = last.as_ref() else { return };
        self.progress.set_fraction(if snapshot.total == 0 { 0.0 } else { (snapshot.answered as f64 / snapshot.total as f64).clamp(0.0, 1.0) });
        self.progress.set_visible(progress_visible(snapshot.answered, snapshot.total));
        for row in visible_rows(&snapshot.rows, self.chips.get()) {
            self.column.append(&search_row(row, &self.nav, Rc::clone(&self.typed)));
        }
    }
}

impl GlobalSearchPage {
    pub fn widget(&self) -> &gtk::Widget {
        &self.root
    }

    /// The progress bar's fraction while it shows, `None` while it is hidden.
    pub fn progress(&self) -> Option<f64> {
        self.view.progress.is_visible().then(|| self.view.progress.fraction())
    }

    pub fn update(&self, snapshot: &GlobalSearchSnapshot) {
        draw(&self.entry, &self.view, snapshot);
    }

    /// The hint of the empty search box. The anime page says "Search a source by title" (T-41a-027).
    pub fn set_hint(&self, hint: &str) {
        self.entry.set_placeholder_text(Some(hint));
    }

    /// The text in the search box, as typed: a source row opens its browse page with it (GS-EC-15).
    pub fn typed(&self) -> String {
        self.view.typed.borrow().clone()
    }

    /// `update` as a closure that holds the page's contents but not the page widget itself, so a model that owns it does not
    /// keep the page alive.
    pub fn updater(&self) -> impl Fn(&GlobalSearchSnapshot) + 'static {
        let (entry, view) = (self.entry.clone(), Rc::clone(&self.view));
        move |snapshot| draw(&entry, &view, snapshot)
    }
}

fn draw(entry: &gtk::SearchEntry, view: &SearchView, snapshot: &GlobalSearchSnapshot) {
    if entry.text().as_str() != snapshot.query && !entry.has_focus() {
        entry.set_text(&snapshot.query);
    }
    *view.last.borrow_mut() = Some(snapshot.clone());
    view.redraw();
}

fn search_row(row: &SearchRow, nav: &GlobalSearchNav, typed: Rc<RefCell<String>>) -> gtk::Box {
    let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let titles = gtk::Box::new(gtk::Orientation::Vertical, 0);
    titles.set_hexpand(true);
    titles.append(&gtk::Label::builder().label(row_title(row)).xalign(0.0).css_classes(["heading"]).build());
    if !row.source.lang.is_empty() {
        titles.append(&gtk::Label::builder().label(subtitle_display_name(&row.source.lang)).xalign(0.0).css_classes(["dim-label"]).build());
    }
    header.append(&titles);
    let go = gtk::Button::from_icon_name("go-next-symbolic");
    go.set_tooltip_text(Some(&row.source.name));
    let (open, id) = (Rc::clone(&nav.open_source), row.source.id);
    // the text is read when the arrow is pressed, not when the row was drawn
    go.connect_clicked(move |_| open(id, &typed.borrow().clone()));
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
                let cell = super::view::grid_cell_lines(
                    Some(item),
                    kuro_prefs::library::LibraryDisplayMode::ComfortableGrid,
                    &nav.load_cover,
                    &super::view::always(),
                    SEARCH_CARD_WIDTH_PX,
                    3,
                );
                let button = gtk::Button::builder().child(&cell).css_classes(["flat"]).build();
                let (open, entry, id) = (Rc::clone(&nav.open_entry), item.clone(), row.source.id);
                button.connect_clicked(move |_| open(id, &entry));
                let (preview, entry) = (Rc::clone(&nav.preview), item.clone());
                super::view::attach_long_press(&button, move || preview(id, &entry));
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
    let column = crate::ui::column();
    let list = crate::ui::boxed_list();
    for entry in sorted {
        let item = ListItem { id: entry.id, title: entry.title.clone(), cover: entry.cover.clone(), ..ListItem::default() };
        let row = super::view::list_cell(Some(&item), &nav.load_cover, &super::view::always());
        let (open_search, id) = (Rc::clone(&nav.open_search), entry.id);
        // a click on the row opens the migrate search; a click on the cover opens the entry and claims the click
        let cell = row.clone();
        let row = list_row(&cell, move || open_search(id), || {});
        if let Some(cover) = cell.first_child() {
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
    // an empty boxed list would still draw its frame
    list.set_visible(list.first_child().is_some());
    column.append(&list);
    root.append(&crate::ui::scrolled(&column));
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
            &MigrateSourcesSnapshot { sources: vec![], mode: MigrationSortingMode::ALPHABETICAL, direction: MigrationDirection::ASCENDING, empty_text: tr(MsgKey::information_empty_library), error: None },
            MigrateSourcesNav {
                open: Rc::new(move |id| a.borrow_mut().push(format!("open {id}"))),
                copy_id: Rc::new(move |id| b.borrow_mut().push(format!("copy {id}"))),
                toggle_mode: Rc::new(move || c.borrow_mut().push("mode".into())),
                toggle_direction: Rc::new(move || d.borrow_mut().push("direction".into())),
                open_help: Rc::new(|| {}),
                retry: Rc::new(|| {}),
            },
        );
        let text = all_labels(&migrate);
        assert!(text.contains(&tr(MsgKey::information_empty_library)), "an empty list says so");
        assert!(!text.iter().any(|t| t.contains("Source A") || t.contains("Extension A")), "{text:?}");

        let (_tx, ready) = watch::channel(true);
        let search = global_search(
            search_nav(SearchChips::default()).0,
            ready,
        );
        search.update(&GlobalSearchSnapshot { query: String::new(), answered: 0, total: 0, rows: vec![] });
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
            empty_text: String::new(),
            error: None,
        };
        let page = migrate_sources(
            &snapshot,
            MigrateSourcesNav {
                open: Rc::new(move |id| a.borrow_mut().push(format!("open {id}"))),
                copy_id: Rc::new(move |id| b.borrow_mut().push(format!("copy {id}"))),
                toggle_mode: Rc::new(|| {}),
                toggle_direction: Rc::new(|| {}),
                open_help: Rc::new(|| {}),
                retry: Rc::new(|| {}),
            },
        );
        let text = all_labels(&page);
        for expected in ["Alpha", "12 in library", "Gone", "3 in library", "Nolang", "1 in library", &tr(MsgKey::not_installed), &tr(MsgKey::action_sort_count)] {
            assert!(text.iter().any(|t| t == expected), "{expected} in {text:?}");
        }
        assert_eq!(text.iter().filter(|t| **t == tr(MsgKey::not_installed)).count(), 1, "only the stub is marked");
    }

    fn global_search_shows_each_state_of_a_source() {
        let source = |id, name: &str| Source::new(id, name, "en");
        let item = ListItem { title: "Berserk".into(), ..ListItem::default() };
        let (ready_tx, ready) = watch::channel(false);
        let page = global_search(search_nav(SearchChips { filter: SourceFilter::All, only_has_results: false }).0, ready);
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
            answered: 2,
            total: 4,
            rows: vec![
                SearchRow { source: source(1, "Alpha"), state: SearchState::Results(vec![item]), origin: false },
                SearchRow { source: source(2, "Beta"), state: SearchState::Results(vec![]), origin: false },
                SearchRow { source: source(3, "Gamma"), state: SearchState::Error("offline".into()), origin: false },
                SearchRow { source: source(4, "Delta"), state: SearchState::Loading, origin: false },
            ],
        });
        let text = all_labels(page.widget());
        for expected in ["Alpha", "Berserk", "Beta", "Gamma", "offline", "Delta", &tr(MsgKey::no_results_found)] {
            assert!(text.iter().any(|t| t == expected), "{expected} in {text:?}");
        }
    }

    /// Everything the search page called back, as text.
    type SearchLog = Rc<RefCell<Vec<String>>>;

    fn search_nav(chips: SearchChips) -> (GlobalSearchNav, SearchLog) {
        let log: SearchLog = Rc::default();
        let (searched, opened, previewed, chipped) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let nav = GlobalSearchNav {
            search: Rc::new(move |q| searched.borrow_mut().push(format!("search {q}"))),
            open_source: Rc::new(|_, _| {}),
            open_entry: Rc::new(move |id, item| opened.borrow_mut().push(format!("open {id} {}", item.title))),
            preview: Rc::new(move |id, item| previewed.borrow_mut().push(format!("preview {id} {}", item.title))),
            load_cover: Rc::new(|_, _| {}),
            chips,
            on_chips: Rc::new(move |c| chipped.borrow_mut().push(format!("chips {c:?}"))),
        };
        (nav, log)
    }

    fn descendants<T: IsA<gtk::Widget>>(widget: &impl IsA<gtk::Widget>) -> Vec<T> {
        fn walk<T: IsA<gtk::Widget>>(widget: &gtk::Widget, out: &mut Vec<T>) {
            if let Some(found) = widget.downcast_ref::<T>() {
                out.push(found.clone());
            }
            let mut child = widget.first_child();
            while let Some(w) = child {
                walk(&w, out);
                child = w.next_sibling();
            }
        }
        let mut out = Vec::new();
        walk(widget.upcast_ref(), &mut out);
        out
    }

    fn global_search_has_chips_a_progress_bar_that_hides_at_the_ends_and_96_px_cards() {
        let (nav, log) = search_nav(SearchChips::default());
        let page = global_search(nav, watch::channel(true).1);
        let pinned = {
            let mut source = Source::new(1, "Alpha", "en");
            source.pin = kuro_domain::source::pin::Pins::PINNED;
            source
        };
        let item = ListItem { title: "Berserk".into(), ..ListItem::default() };
        let rows = vec![
            SearchRow { source: pinned, state: SearchState::Results(vec![item]), origin: false },
            SearchRow { source: Source::new(2, "Beta", "en"), state: SearchState::Results(vec![]), origin: false },
            SearchRow { source: Source::new(3, "Gamma", "en"), state: SearchState::Loading, origin: true },
        ];
        let snapshot = |answered| GlobalSearchSnapshot { query: "ber".into(), answered, total: 3, rows: rows.clone() };

        let context = gtk::glib::MainContext::default();
        let start = std::time::Instant::now();
        while page.widget().downcast_ref::<gtk::Stack>().and_then(|s| s.visible_child_name()).as_deref() != Some("content")
            && start.elapsed().as_secs() < 2
        {
            while context.iteration(false) {}
        }
        page.update(&snapshot(0));
        assert!(!page.view.progress.is_visible(), "no source has answered yet");
        page.update(&snapshot(1));
        assert!(page.view.progress.is_visible(), "one of three");
        page.update(&snapshot(3));
        assert!(!page.view.progress.is_visible(), "all answered");

        let text = all_labels(page.widget());
        for expected in ["Alpha", &format!("\u{25B6} Gamma")] {
            assert!(text.iter().any(|t| t == expected), "{expected} in {text:?}: pinned-only keeps the pinned and the origin source");
        }
        assert!(!text.iter().any(|t| t == "Beta"), "an unpinned source is out under Pinned");

        // the chips: Pinned, All, a separator, Has results
        let chips: Vec<gtk::ToggleButton> = descendants(page.widget());
        let names: Vec<String> = chips.iter().map(|c| c.label().map(|l| l.to_string()).unwrap_or_default()).collect();
        assert_eq!(names, [tr(MsgKey::pinned_sources), tr(MsgKey::all), tr(MsgKey::has_results)]);
        assert!(chips[0].is_active() && !chips[1].is_active() && !chips[2].is_active());
        chips[1].set_active(true);
        assert!(!chips[0].is_active(), "Pinned and All are one choice");
        assert!(all_labels(page.widget()).iter().any(|t| t == "Beta"), "All shows every source again, from the rows already here");
        chips[2].set_active(true);
        let text = all_labels(page.widget());
        assert!(text.iter().any(|t| t == "Alpha") && !text.iter().any(|t| t == "Beta" || t == "\u{25B6} Gamma"), "Has results keeps only rows with results: {text:?}");
        assert!(log.borrow().iter().all(|entry| entry.starts_with("chips ")), "a chip change never runs the search: {:?}", log.borrow());
        assert_eq!(log.borrow().len(), 2);

        // cards are 96 px wide and a long press previews
        let card = descendants::<gtk::Button>(page.widget()).into_iter().find(|b| b.has_css_class("flat") && b.child().is_some_and(|c| c.is::<gtk::Box>())).expect("card");
        assert!(descendants::<gtk::Widget>(&card).iter().any(|w| w.width_request() == SEARCH_CARD_WIDTH_PX), "the cover is 96 px wide");
        let controllers = card.observe_controllers();
        let long = (0..controllers.n_items()).filter_map(|i| controllers.item(i)).find_map(|c| c.downcast::<gtk::GestureLongPress>().ok()).expect("long press");
        long.emit_by_name::<()>("pressed", &[&0.0_f64, &0.0_f64]);
        assert_eq!(log.borrow().last().map(String::as_str), Some("preview 1 Berserk"));
        card.emit_clicked();
        assert_eq!(log.borrow().last().map(String::as_str), Some("open 1 Berserk"));
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
        // each cover shows the title's initials until its picture loads
        assert_eq!(text, ["could not load", "A", "Apple", "B", "banana", "Z", "zebra"]);
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
        source_rig_hiding(stub, incognito, filters, results, false)
    }

    fn source_rig_hiding(stub: bool, incognito: bool, filters: FilterList, results: Vec<Result<ListedPage, String>>, hide: bool) -> Rig {
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
            hide_in_library: hide,
            initial_query: BrowseQuery::Popular,
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

    fn hide_in_library_keeps_favourites_out_of_a_page_made_with_the_flag_on() {
        let page = |favorite: bool, title: &str| ListItem { title: title.into(), url: format!("/{title}"), favorite, ..ListItem::default() };
        let listed = || Ok(ListedPage { items: vec![page(true, "mine"), page(false, "new")], has_next: false });
        let prefs = super::super::prefs::BrowsePrefs::new(PrefStore::in_memory());
        assert!(!prefs.hide_in_library(), "off by default");
        prefs.sources.hide_in_manga_library_items.set(true).expect("set");
        assert!(prefs.hide_in_library(), "the page reads the stored preference");

        let on = source_rig_hiding(false, false, FilterList::default(), vec![listed()], true);
        let SourcePage::Browse(shown) = &on.page else { panic!("browse page") };
        assert!(pump_until(|| shown.listing.loaded_count() == 1), "only the entry that is not in the library is kept");
        let off = source_rig_hiding(false, false, FilterList::default(), vec![listed()], false);
        let SourcePage::Browse(shown) = &off.page else { panic!("browse page") };
        assert!(pump_until(|| shown.listing.loaded_count() == 2), "the flag off keeps both");
    }

    fn escape_closes_an_open_search_first_and_otherwise_goes_on() {
        let (_tx, ready) = watch::channel(true);
        let page = global_search(search_nav(SearchChips::default()).0, ready);
        let entry = descendants::<gtk::SearchEntry>(page.widget()).into_iter().next().expect("entry");
        entry.set_text("one piece");
        let controller = |widget: &gtk::Widget| {
            let all = widget.observe_controllers();
            (0..all.n_items()).filter_map(|i| all.item(i)).filter_map(|c| c.downcast::<gtk::EventControllerKey>().ok()).count()
        };
        let body = entry.parent().expect("body");
        assert_eq!(controller(&body), 1, "the page carries the Escape step");
        let all = body.observe_controllers();
        let keys = (0..all.n_items()).filter_map(|i| all.item(i)).find_map(|c| c.downcast::<gtk::EventControllerKey>().ok()).expect("controller");
        let none = gtk::gdk::ModifierType::empty();
        assert!(keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::Escape, &0_u32, &none]), "the first Escape closes the search");
        assert_eq!(entry.text().as_str(), "", "the typed text is gone");
        assert!(!keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::Escape, &0_u32, &none]), "the second one goes on, to leave the page");
    }

    pub(crate) fn smoke() {
        escape_closes_an_open_search_first_and_otherwise_goes_on();
        hide_in_library_keeps_favourites_out_of_a_page_made_with_the_flag_on();
        a_stub_source_shows_only_its_title_back_and_the_missing_message();
        opening_a_source_stores_it_as_the_last_used_unless_incognito_applies();
        back_clears_the_search_text_first_and_then_leaves();
        popular_latest_and_a_search_from_them_start_from_fresh_defaults();
        the_filter_button_of_the_sheet_searches_with_the_sheet_values();
        there_is_no_help_action_and_other_actions_go_to_the_caller();
        no_sample_rows_in_an_empty_snapshot();
        migrate_sources_lists_the_snapshot_with_counts_and_marks_stubs();
        global_search_shows_each_state_of_a_source();
        global_search_has_chips_a_progress_bar_that_hides_at_the_ends_and_96_px_cards();
        migrate_entries_sorts_by_title_without_case_and_shows_a_load_error();
    }
}

#[cfg(test)]
mod pure_tests {
    use kuro_domain::source::pin::Pins;

    use super::*;

    fn row(id: i64, pinned: bool, origin: bool, state: SearchState) -> SearchRow {
        let mut source = Source::new(id, &format!("S{id}"), "en");
        if pinned {
            source.pin = Pins::PINNED;
        }
        SearchRow { source, state, origin }
    }

    fn found() -> SearchState {
        SearchState::Results(vec![ListItem::default()])
    }

    fn ids(rows: &[&SearchRow]) -> Vec<i64> {
        rows.iter().map(|r| r.source.id).collect()
    }

    #[test]
    fn the_progress_bar_shows_only_from_one_answer_until_all_have_answered() {
        assert!(!progress_visible(0, 5), "nobody has answered");
        assert!(progress_visible(1, 5));
        assert!(progress_visible(4, 5));
        assert!(!progress_visible(5, 5), "everybody has");
        assert!(!progress_visible(0, 0));
    }

    #[test]
    fn the_chips_filter_the_rows_they_are_given_and_the_origin_is_never_pinned_out() {
        let rows = vec![
            row(1, true, false, found()),
            row(2, false, false, found()),
            row(3, false, true, SearchState::Results(vec![])),
            row(4, true, false, SearchState::Loading),
        ];
        let pinned = SearchChips { filter: SourceFilter::PinnedOnly, only_has_results: false };
        assert_eq!(ids(&visible_rows(&rows, pinned)), [1, 3, 4]);
        let all = SearchChips { filter: SourceFilter::All, only_has_results: false };
        assert_eq!(ids(&visible_rows(&rows, all)), [1, 2, 3, 4]);
        let results = SearchChips { filter: SourceFilter::All, only_has_results: true };
        assert_eq!(ids(&visible_rows(&rows, results)), [1, 2], "an empty result or a loading row has none");
        assert_eq!(SearchChips::default().filter, SourceFilter::PinnedOnly, "Pinned is the default chip");
    }

    #[test]
    fn the_origin_source_carries_the_arrow_prefix_and_a_card_is_96_px() {
        assert_eq!(row_title(&row(1, false, true, found())), "\u{25B6} S1");
        assert_eq!(row_title(&row(2, false, false, found())), "S2");
        assert_eq!(SEARCH_CARD_WIDTH_PX, 96);
    }
}
