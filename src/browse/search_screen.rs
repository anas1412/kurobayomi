//! The global search and migrate search screens (`global_search`, `migrate_search`; T-40b-060 to T-40b-067): the page of
//! `pages::global_search` over a `SearchRunner`, started from what the caller parked. The start, the sources a search can
//! ask and the routes of its rows are pure and tested here; the page is thin.
//!
//! The Has results toggle is stored as `has_filters_toggle_state` and read again when a page opens, and the migrate search is
//! this same page, so both screens share the key (T-40b-063).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use gtk::prelude::*;
use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::pin::Pins;
use kuro_i18n::{MsgKey, tr};
use tokio::sync::watch;

use super::directory::SourceDirectory;
use kuro_data::anime::AnimeRepo;
use kuro_domain::kind::EntryKind;
use super::global_search::{Candidate, GlobalSearch, MigrateStart, deep_link_fallback, migrate_start, row_text_route};
use super::pages::{GlobalSearchNav, SearchChips, SourceFilter, global_search as search_page_widget};
use super::pending::{self, SearchIntent};
use super::prefs::BrowsePrefs;
use super::route::{BrowseKind, BrowseRoute};
use super::search_runner::{Loaders, RunnerHooks, SearchRunner};
use super::view::file_cover_loader;
use crate::screens::Built;

/// What a search page starts with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// The query to search (blank searches nothing), the extension filter of an intent, and the source a migrate search
    /// started from.
    Search { query: String, extension: Option<String>, origin: Option<i64> },
    /// A migrate search whose manga is not in the library (E-MG-02): the page is an error state.
    MigrateMissing,
}

/// The start of a page. A migrate search starts from its manga's title and puts that manga's source first (E-MG-01); a
/// missing manga is an error, not a panic (E-MG-02), and a blank title searches nothing (E-MG-03). Otherwise an intent
/// wins over a parked route, which wins over the text an entry page parked.
pub fn start_for(migrating: bool, migrate_manga: Option<(i64, String)>, intent: Option<SearchIntent>, route: Option<BrowseRoute>, entry_text: Option<String>) -> Start {
    if migrating {
        return match migrate_start(migrate_manga) {
            MigrateStart::Missing => Start::MigrateMissing,
            MigrateStart::Ready { origin, title } => Start::Search { query: title, extension: None, origin: Some(origin) },
        };
    }
    if let Some(SearchIntent { query, extension }) = intent {
        return Start::Search { query, extension, origin: None };
    }
    let query = match route {
        Some(BrowseRoute::GlobalSearch { query, .. }) => query,
        _ => entry_text.unwrap_or_default(),
    };
    Start::Search { query, extension: None, origin: None }
}

/// The sources a search can ask, with the pinned ones marked from `pinned_catalogues` (decimal ids).
pub fn candidates(directory: Option<&dyn SourceDirectory>, pinned: &BTreeSet<String>) -> Vec<Candidate> {
    directory
        .map(|directory| {
            directory
                .sources()
                .into_iter()
                .map(|entry| {
                    let mut source = entry.source;
                    source.pin = if pinned.contains(&source.id.to_string()) { Pins::PINNED } else { Pins::UNPINNED };
                    Candidate { source, package: entry.package }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `hidden_catalogues` as source ids; text that is not an id is skipped.
pub fn hidden_ids(disabled: &BTreeSet<String>) -> BTreeSet<i64> {
    disabled.iter().filter_map(|id| id.parse().ok()).collect()
}

/// The arrow of a source row: that source's listing, opened with the text in the search box whether or not it was submitted
/// (GS-EC-15). With no text it opens on Popular. A migrate search opens the migrate listing.
pub fn source_route(source_id: i64, typed: &str, migrating: bool) -> BrowseRoute {
    if typed.trim().is_empty() {
        return BrowseRoute::BrowseSource { source_id, query: BrowseQuery::Popular, migrating };
    }
    match row_text_route(source_id, typed) {
        BrowseRoute::BrowseSource { source_id, query, .. } => BrowseRoute::BrowseSource { source_id, query, migrating },
        other => other,
    }
}

/// The arrow of a source row of either catalogue: an anime row opens the anime browse page on the same listing.
pub fn source_route_for(kind: BrowseKind, source_id: i64, typed: &str, migrating: bool) -> BrowseRoute {
    match (kind, source_route(source_id, typed, migrating)) {
        (BrowseKind::Anime, BrowseRoute::BrowseSource { source_id, query, .. }) => BrowseRoute::AnimeBrowseSource { source_id, query },
        (_, route) => route,
    }
}

/// A deep link that found nothing replaces its page with global search for the query (GS-EC-16).
pub fn replace_with_global_search(query: &str) {
    crate::ui::pop();
    pending::open(deep_link_fallback(query));
}

/// What a search page needs of the app.
pub struct SearchEnv {
    /// Which catalogue the page searches: it reads that kind's pinned and hidden sources and routes its rows.
    pub kind: BrowseKind,
    pub prefs: Rc<BrowsePrefs>,
    pub directory: Option<Arc<dyn SourceDirectory>>,
    /// A card: the manga's page (library id).
    pub open_entry: Rc<dyn Fn(i64)>,
    /// A card of a migrate search: the anime page asks what to migrate (the dialog). `None` opens the entry, as a long
    /// press does.
    pub pick_entry: Option<Rc<dyn Fn(i64)>>,
    /// The single-result auto-open: the manga's page replaces the search.
    pub replace_with_entry: Rc<dyn Fn(i64)>,
    pub open_route: Rc<dyn Fn(BrowseRoute)>,
}

/// The error state of a page that cannot search.
fn error_state(text: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_valign(gtk::Align::Center);
    label.set_vexpand(true);
    label.upcast()
}

/// The search page for `start`. The runner lives as long as the page; popping it aborts the fetches.
pub fn search_widget(env: &SearchEnv, start: &Start, migrating: bool) -> gtk::Widget {
    let Start::Search { query, extension, origin } = start else { return error_state(&tr(MsgKey::unknown_error)) };
    let sources = &env.prefs.sources;
    let only_has_results = sources.global_search_filter_state.get();
    let directory = env.directory.clone();
    let (pinned, hidden) = match env.kind {
        BrowseKind::Manga => (sources.pinned_manga_sources.get(), sources.disabled_manga_sources.get()),
        BrowseKind::Anime => (sources.pinned_anime_sources.get(), sources.disabled_anime_sources.get()),
    };
    let engine = GlobalSearch::new(candidates(directory.as_deref(), &pinned), sources.enabled_languages.get(), hidden_ids(&hidden), *origin, only_has_results);
    // the page and the runner reach each other: the page calls the runner through a weak slot
    let slot: Rc<RefCell<Weak<SearchRunner>>> = Rc::default();
    let with_runner = |action: fn(&Rc<SearchRunner>, &str)| {
        let slot = Rc::clone(&slot);
        move |text: &str| {
            let runner = slot.borrow().upgrade();
            if let Some(runner) = runner {
                action(&runner, text);
            }
        }
    };
    let submit = with_runner(|runner, text| runner.submit(text));
    let (typed_slot, open_route, prefs) = (Rc::clone(&slot), Rc::clone(&env.open_route), Rc::clone(&env.prefs));
    let (open_entry, preview) = (env.pick_entry.clone().unwrap_or_else(|| Rc::clone(&env.open_entry)), Rc::clone(&env.open_entry));
    let kind = env.kind;
    let nav = GlobalSearchNav {
        search: Rc::new(submit),
        open_source: Rc::new(move |id, typed| open_route(source_route_for(kind, id, typed, migrating))),
        open_entry: Rc::new(move |_, item| open_entry(item.id)),
        // there is no entry preview in the app: a long press opens the entry, as a tap does
        preview: Rc::new(move |_, item| preview(item.id)),
        load_cover: file_cover_loader(),
        chips: SearchChips {
            filter: if extension.is_some() { SourceFilter::All } else { SourceFilter::PinnedOnly },
            only_has_results,
        },
        on_chips: Rc::new(move |chips| {
            // the toggle is stored for the next page (the same key serves the migrate search); the filter searches again
            if let Err(error) = prefs.sources.global_search_filter_state.set(chips.only_has_results) {
                tracing::warn!(%error, "the has-results toggle could not be stored");
            }
            let runner = typed_slot.borrow().upgrade();
            if let Some(runner) = runner {
                runner.set_only_has_results(chips.only_has_results);
                runner.set_filter(chips.filter);
            }
        }),
    };
    let (_never_sent, ready) = watch::channel(true);
    let page = search_page_widget(nav, ready);
    if env.kind == BrowseKind::Anime {
        page.set_hint(super::texts::GLOBAL_SEARCH_EMPTY_HINT);
    }
    let hooks = RunnerHooks { changed: Rc::new(page.updater()), open_entry: Rc::clone(&env.replace_with_entry) };
    let loaders: Loaders = match directory.clone() {
        Some(directory) => Arc::new(move |id| directory.loader(id)),
        None => Arc::new(|_| None),
    };
    let runner = SearchRunner::new(engine, loaders, hooks);
    if let Some(directory) = directory {
        runner.set_default_filters(Arc::new(move |id| directory.filters(id)));
    }
    // a pasted link resolves through the sources' `uri_type` before any catalogue search (T-20a-031)
    if let Some(links) = env.directory.clone() {
        runner.set_link_resolver(Arc::new(move |uri| links.resolve_link(uri)));
    }
    *slot.borrow_mut() = Rc::downgrade(&runner);
    runner.attach(page.widget());
    if query.trim().is_empty() {
        runner.refresh();
    } else {
        runner.request(query, extension.as_deref());
    }
    page.widget().clone()
}

/// The app side of a search page of `kind`. `migrate_from` is the entry a migrate search started from: a card of it opens
/// the migrate dialog (T-41a-041, T-40a-015) instead of the entry.
fn production_env(kind: BrowseKind, migrate_from: Option<i64>) -> SearchEnv {
    let prefs = Rc::new(BrowsePrefs::new(crate::data::app().base.store().clone()));
    match kind {
        BrowseKind::Manga => SearchEnv {
            kind,
            prefs,
            directory: super::directory::current(),
            open_entry: Rc::new(crate::entry_core::route::open),
            pick_entry: migrate_from.map(manga_migrate_dialog),
            replace_with_entry: Rc::new(|id| {
                crate::ui::pop();
                crate::entry_core::route::open(id);
            }),
            open_route: Rc::new(pending::open),
        },
        BrowseKind::Anime => {
            let repo = AnimeRepo::new(crate::data::app().db(EntryKind::Anime));
            SearchEnv {
                kind,
                prefs,
                directory: Some(Arc::new(super::anime::directory::AnimeDirectory::current(repo.clone()))),
                open_entry: Rc::new(crate::entry_core::anime::route::open),
                pick_entry: migrate_from.map(|old| anime_migrate_dialog(repo, old)),
                replace_with_entry: Rc::new(|id| {
                    crate::ui::pop();
                    crate::entry_core::anime::route::open(id);
                }),
                open_route: Rc::new(pending::open),
            }
        }
    }
}

/// A card of an anime migrate search: the dialog of what to include for moving `old` to the card's anime. Show anime and
/// Show seasons open the card's page, where a seasons title lists its seasons. The options come from the old title (its
/// custom cover and background) and start from the saved `anime_migrate_flags`, which every run stores before it starts.
/// Copy and Migrate need the anime migration engine, which the app does not have yet, so they say that the operation
/// failed instead of doing nothing.
fn anime_migrate_dialog(repo: AnimeRepo, old: i64) -> Rc<dyn Fn(i64)> {
    use super::anime::migrate_dialog::{Facts, MigrateHandlers, show};
    use super::anime::prefs::AnimeBrowsePrefs;
    let slot = super::dialogs::DialogSlot::new();
    let prefs = Rc::new(AnimeBrowsePrefs::new(crate::data::app().base.store().clone()));
    Rc::new(move |new: i64| {
        let (Ok(old_anime), Ok(new_anime)) = (repo.by_id(old), repo.by_id(new)) else {
            tracing::error!(old, new, "an anime of the migrate dialog could not be read");
            crate::toast::notify(tr(MsgKey::internal_error));
            return;
        };
        let (save, stored) = (Rc::clone(&prefs), Rc::clone(&prefs));
        let handlers = MigrateHandlers {
            show_anime: Rc::new(move || crate::entry_core::anime::route::open(new)),
            show_seasons: Rc::new(move || crate::entry_core::anime::route::open(new)),
            save_flags: Rc::new(move |bitmap| {
                if let Err(error) = save.save_migrate_flags(bitmap) {
                    tracing::warn!(%error, "the anime migrate flags could not be stored");
                }
            }),
            run: Rc::new(|run| {
                tracing::warn!(?run, "no anime migration engine is installed, nothing was migrated");
                crate::toast::notify(tr(MsgKey::internal_error));
            }),
        };
        // the library stores the fetch type as the database's enum; the dialog's rules take the source API's
        let fetch = |stored: kuro_db::schema_anime::FetchType| kuro_source::anime::model::FetchType::from(stored as i64);
        // no anime download index is published yet, so Delete downloaded is not offered
        let has = |family| {
            kuro_platform::linux::LinuxPaths::system().is_ok_and(|paths| kuro_images::cover_paths::custom_file(&paths, family, old).is_file())
        };
        let facts = Facts {
            custom_cover: has(kuro_images::cover_paths::Family::AnimeCover),
            custom_background: has(kuro_images::cover_paths::Family::AnimeBackground),
            downloads: false,
        };
        show(&slot, &crate::ui::window(), fetch(old_anime.fetch_type), fetch(new_anime.fetch_type), facts, stored.migrate_flags(), handlers);
    })
}

/// A card of a manga migrate search: the dialog of what to include for moving `old` to the card's manga. Show manga opens
/// the card's page; Copy and Migrate run the migration engine on the main context and pop the search when it succeeds.
pub(super) fn manga_migrate_dialog(old: i64) -> Rc<dyn Fn(i64)> {
    use kuro_data::manga::MangaRepo;
    use super::manga_migrate_dialog::{Handlers, show};
    use super::migrate_engine::migrate_manga;
    use super::migrate_live::LiveSteps;
    Rc::new(move |new: i64| {
        let app = crate::data::app();
        let repo = MangaRepo::new(app.db(EntryKind::Manga));
        let (Ok(old_manga), Ok(new_manga)) = (repo.by_id(old), repo.by_id(new)) else {
            tracing::error!(old, new, "a manga of the migrate dialog could not be read");
            crate::toast::notify(tr(MsgKey::internal_error));
            return;
        };
        let downloads = crate::shell::host::Shell::get().map(|shell| shell.downloads().clone());
        let steps = Rc::new(LiveSteps::new(app.db(EntryKind::Manga), super::directory::current(), downloads));
        let prefs = Rc::new(BrowsePrefs::new(app.base.store().clone()));
        let (save, ran) = (Rc::clone(&prefs), Rc::clone(&steps));
        let (old_row, new_row) = (old_manga.clone(), new_manga.clone());
        let handlers = Handlers {
            show_manga: Rc::new(move || crate::entry_core::route::open(new)),
            save_flags: Rc::new(move |bitmap| {
                if let Err(error) = save.migrate_flags.set(bitmap) {
                    tracing::warn!(%error, "the migrate flags could not be stored");
                }
            }),
            run: Rc::new(move |plan, done| {
                let (steps, old_manga, new_manga) = (Rc::clone(&ran), old_row.clone(), new_row.clone());
                gtk::glib::spawn_future_local(async move { done(migrate_manga(&*steps, &old_manga, &new_manga, plan).await) });
            }),
            finished: Rc::new(crate::ui::pop),
            toast: Rc::new(|text| crate::toast::notify(text)),
        };
        show(&crate::ui::window(), LiveSteps::has_custom_cover(&old_manga), steps.has_downloads(&old_manga), prefs.migrate_flags.get(), handlers);
    })
}

/// The anime entry a migrate request names: its source and title. An anime that is not in the library is `None`.
fn migrate_anime(entry_id: i64) -> Option<(i64, String)> {
    AnimeRepo::new(crate::data::app().db(EntryKind::Anime)).by_id(entry_id).ok().map(|anime| (anime.source, anime.title))
}

/// The manga a migrate request names: its source and title. A manga that is not in the library is `None`.
fn migrate_manga(entry_id: i64) -> Option<(i64, String)> {
    use kuro_data::manga::MangaRepo;
    MangaRepo::new(crate::data::app().db(EntryKind::Manga)).by_id(entry_id).ok().map(|manga| (manga.source, manga.title))
}

/// The body of `global_search` and `migrate_search`, as `screens::body` asks for it. `None` leaves the screen to its sample
/// page: no app data, or nothing was parked for this screen. The catalogue is the migrate request's, the parked route's, or
/// the entry page's that asked for a title or genre search; an intent is a manga one.
pub fn body(migrating: bool) -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    let screen = if migrating { "migrate_search" } else { "global_search" };
    let (kind, migrate_from, start) = if migrating {
        let request = crate::entry_core::handoff::take_migrate()?;
        let (kind, found) = if request.anime { (BrowseKind::Anime, migrate_anime(request.entry_id)) } else { (BrowseKind::Manga, migrate_manga(request.entry_id)) };
        (kind, Some(request.entry_id), start_for(true, found, None, None, None))
    } else {
        let intent = pending::take_intent();
        let route = pending::take_for(screen);
        let caller = super::browse_screen::search_caller();
        let entry_text = (intent.is_none() && route.is_none() && caller.is_some())
            .then(crate::entry_core::handoff::take_search)
            .flatten()
            .map(|request| request.query);
        if intent.is_none() && route.is_none() && entry_text.is_none() {
            return None;
        }
        let kind = match (&intent, &route) {
            (Some(_), _) => BrowseKind::Manga,
            (None, Some(BrowseRoute::GlobalSearch { kind, .. })) => *kind,
            _ => caller.unwrap_or(BrowseKind::Manga),
        };
        (kind, None, start_for(false, None, intent, route, entry_text))
    };
    Some(Built { content: search_widget(&production_env(kind, migrate_from), &start, migrating), end: Vec::new() })
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;

    use gtk::glib;
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::directory::tests::{FakeDirectory, entry};
    use crate::browse::listing::ListItem;

    fn pump_until(mut done: impl FnMut() -> bool) -> bool {
        let context = glib::MainContext::default();
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

    #[test]
    fn a_migrate_search_starts_from_its_manga_and_a_missing_one_is_an_error_state() {
        assert_eq!(
            start_for(true, Some((5, "Berserk".into())), None, None, None),
            Start::Search { query: "Berserk".into(), extension: None, origin: Some(5) },
            "E-MG-01: the title is searched and its source is the origin"
        );
        assert_eq!(start_for(true, None, None, None, None), Start::MigrateMissing, "E-MG-02: a missing manga is an error, not a panic");
        assert_eq!(
            start_for(true, Some((5, String::new())), None, None, None),
            Start::Search { query: String::new(), extension: None, origin: Some(5) },
            "E-MG-03: a blank title is a blank query, which searches nothing"
        );
    }

    #[test]
    fn an_intent_wins_over_a_route_and_a_route_over_the_text_an_entry_parked() {
        let intent = SearchIntent { query: "one".into(), extension: Some("eu.a".into()) };
        let route = BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: "two".into() };
        assert_eq!(
            start_for(false, None, Some(intent), Some(route.clone()), Some("three".into())),
            Start::Search { query: "one".into(), extension: Some("eu.a".into()), origin: None }
        );
        assert_eq!(start_for(false, None, None, Some(route), Some("three".into())), Start::Search { query: "two".into(), extension: None, origin: None });
        assert_eq!(start_for(false, None, None, None, Some("three".into())), Start::Search { query: "three".into(), extension: None, origin: None });
        assert_eq!(start_for(false, None, None, None, None), Start::Search { query: String::new(), extension: None, origin: None });
    }

    #[test]
    fn a_source_row_opens_its_listing_with_the_typed_text_or_on_popular() {
        assert_eq!(
            source_route(7, "typed", false),
            BrowseRoute::BrowseSource { source_id: 7, query: BrowseQuery::Search("typed".into()), migrating: false },
            "GS-EC-15: unsubmitted text goes with the tap"
        );
        assert_eq!(source_route(7, "  ", false), BrowseRoute::BrowseSource { source_id: 7, query: BrowseQuery::Popular, migrating: false });
        assert!(matches!(source_route(7, "x", true), BrowseRoute::BrowseSource { migrating: true, .. }), "a migrate search opens the migrate listing");
    }

    #[test]
    fn candidates_carry_the_pinned_flag_and_the_hidden_ids_skip_text_that_is_no_id() {
        let directory = FakeDirectory { entries: vec![entry(1, "A", "en", "ext.a"), entry(2, "B", "ja", "")], answers: Arc::new(|_, _| Ok(Vec::new())) };
        let found = candidates(Some(&directory), &BTreeSet::from(["2".to_owned()]));
        assert_eq!(found.iter().map(|c| (c.source.id, c.source.pin.contains(kuro_domain::source::pin::Pin::Pinned))).collect::<Vec<_>>(), [(1, false), (2, true)]);
        assert_eq!(found[0].package, "ext.a");
        assert!(candidates(None, &BTreeSet::new()).is_empty(), "no directory: nothing to search");
        assert_eq!(hidden_ids(&BTreeSet::from(["3".to_owned(), "x".to_owned()])), BTreeSet::from([3]));
    }

    struct Rig {
        env: SearchEnv,
        asked: Arc<Mutex<Vec<BrowseQuery>>>,
        log: Rc<RefCell<Vec<String>>>,
        _entered: tokio::runtime::EnterGuard<'static>,
    }

    /// Two pinned sources, 1 (ext.a) and 2 (ext.b), that each answer a search with one entry.
    fn rig() -> Rig {
        let rt: &'static tokio::runtime::Runtime =
            Box::leak(Box::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime")));
        let entered = rt.enter();
        let asked: Arc<Mutex<Vec<BrowseQuery>>> = Arc::default();
        let record = Arc::clone(&asked);
        let directory = Arc::new(FakeDirectory {
            entries: vec![entry(1, "Alpha", "en", "ext.a"), entry(2, "Beta", "en", "ext.b")],
            answers: Arc::new(move |id, query| {
                record.lock().expect("lock").push(query.clone());
                Ok(vec![ListItem { id: 40 + id, source_id: id, title: format!("Found {id}"), url: format!("/{id}"), ..ListItem::default() }])
            }),
        });
        let prefs = Rc::new(BrowsePrefs::new(PrefStore::in_memory()));
        prefs.sources.pinned_manga_sources.set(BTreeSet::from(["1".to_owned(), "2".to_owned()])).expect("pins");
        let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let (entry_log, replace_log, route_log) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let env = SearchEnv {
            kind: BrowseKind::Manga,
            prefs,
            directory: Some(directory),
            pick_entry: None,
            open_entry: Rc::new(move |id| entry_log.borrow_mut().push(format!("entry {id}"))),
            replace_with_entry: Rc::new(move |id| replace_log.borrow_mut().push(format!("replace {id}"))),
            open_route: Rc::new(move |route| route_log.borrow_mut().push(format!("route {route:?}"))),
        };
        Rig { env, asked, log, _entered: entered }
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

    fn toggles(root: &gtk::Widget) -> Vec<gtk::ToggleButton> {
        fn walk(widget: &gtk::Widget, out: &mut Vec<gtk::ToggleButton>) {
            if let Some(toggle) = widget.downcast_ref::<gtk::ToggleButton>() {
                out.push(toggle.clone());
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

    fn a_page_with_a_query_searches_the_pinned_sources_and_draws_their_rows() {
        let rig = rig();
        let start = Start::Search { query: "one".into(), extension: None, origin: None };
        let page = search_widget(&rig.env, &start, false);
        assert!(pump_until(|| rig.asked.lock().expect("lock").len() == 2), "both pinned sources are asked");
        assert!(rig.asked.lock().expect("lock").iter().all(|q| *q == BrowseQuery::Search("one".into())));
        assert!(pump_until(|| {
            let text = labels(&page);
            text.iter().any(|t| t == "Found 1") && text.iter().any(|t| t == "Found 2")
        }), "the results are drawn: {:?}", labels(&page));
    }

    fn the_has_results_toggle_is_stored_and_read_again_by_the_next_page() {
        let rig = rig();
        assert!(!rig.env.prefs.sources.global_search_filter_state.get());
        let start = Start::Search { query: String::new(), extension: None, origin: None };
        let page = search_widget(&rig.env, &start, false);
        assert!(pump_until(|| toggles(&page).len() == 3), "Pinned, All and Has results");
        let chips = toggles(&page);
        chips[2].set_active(true);
        assert!(rig.env.prefs.sources.global_search_filter_state.get(), "T-40b-063: the toggle is stored");
        let again = search_widget(&rig.env, &start, true);
        assert!(pump_until(|| toggles(&again).len() == 3));
        assert!(toggles(&again)[2].is_active(), "the migrate search reads the same key");
        chips[2].set_active(false);
        assert!(!rig.env.prefs.sources.global_search_filter_state.get());
    }

    fn a_source_arrow_opens_its_listing_with_the_typed_text() {
        let rig = rig();
        let start = Start::Search { query: "one".into(), extension: None, origin: None };
        let page = search_widget(&rig.env, &start, false);
        assert!(pump_until(|| labels(&page).iter().any(|t| t == "Found 1")));
        let search = {
            fn find(widget: &gtk::Widget) -> Option<gtk::SearchEntry> {
                if let Some(entry) = widget.downcast_ref::<gtk::SearchEntry>() {
                    return Some(entry.clone());
                }
                let mut child = widget.first_child();
                while let Some(next) = child {
                    if let Some(found) = find(&next) {
                        return Some(found);
                    }
                    child = next.next_sibling();
                }
                None
            }
            find(&page).expect("search entry")
        };
        search.set_text("typed but never submitted");
        let arrow = {
            fn find(widget: &gtk::Widget) -> Option<gtk::Button> {
                if let Some(button) = widget.downcast_ref::<gtk::Button>() {
                    if button.icon_name().as_deref() == Some("go-next-symbolic") {
                        return Some(button.clone());
                    }
                }
                let mut child = widget.first_child();
                while let Some(next) = child {
                    if let Some(found) = find(&next) {
                        return Some(found);
                    }
                    child = next.next_sibling();
                }
                None
            }
            find(&page).expect("source arrow")
        };
        arrow.emit_clicked();
        let last = rig.log.borrow().last().cloned().unwrap_or_default();
        assert!(last.contains("Search(\"typed but never submitted\")"), "GS-EC-15: {last}");
    }

    fn a_card_opens_its_entry_and_one_result_with_an_extension_filter_replaces_the_page() {
        let rig = rig();
        let start = Start::Search { query: "one".into(), extension: Some("ext.a".into()), origin: None };
        let _page = search_widget(&rig.env, &start, false);
        assert!(pump_until(|| rig.log.borrow().contains(&"replace 41".to_owned())), "GS-EC-13: one source, one result, replaced by its page: {:?}", rig.log.borrow());
        assert_eq!(rig.asked.lock().expect("lock").len(), 1, "only the extension's source is asked");
    }

    fn a_migrate_search_with_a_missing_manga_is_an_error_state_that_searches_nothing() {
        let rig = rig();
        let page = search_widget(&rig.env, &Start::MigrateMissing, true);
        assert!(labels(&page).contains(&tr(MsgKey::unknown_error)), "E-MG-02");
        assert!(rig.asked.lock().expect("lock").is_empty());
    }

    fn an_anime_page_reads_the_anime_pins_hints_what_to_search_and_sends_its_rows_to_the_anime_page() {
        let mut rig = rig();
        rig.env.kind = BrowseKind::Anime;
        // the manga pins of the rig do not count for anime: only source 2 is pinned there
        rig.env.prefs.sources.pinned_anime_sources.set(BTreeSet::from(["2".to_owned()])).expect("pins");
        let page = search_widget(&rig.env, &Start::Search { query: "one".into(), extension: None, origin: None }, false);
        assert!(pump_until(|| labels(&page).iter().any(|t| t == "Found 2")), "the pinned anime source answers: {:?}", labels(&page));
        assert_eq!(rig.asked.lock().expect("lock").len(), 1, "only the anime-pinned source is asked, not the manga-pinned ones");
        let search = {
            fn find(widget: &gtk::Widget) -> Option<gtk::SearchEntry> {
                if let Some(entry) = widget.downcast_ref::<gtk::SearchEntry>() {
                    return Some(entry.clone());
                }
                let mut child = widget.first_child();
                while let Some(next) = child {
                    if let Some(found) = find(&next) {
                        return Some(found);
                    }
                    child = next.next_sibling();
                }
                None
            }
            find(&page).expect("search entry")
        };
        assert_eq!(search.placeholder_text().as_deref(), Some(crate::browse::texts::GLOBAL_SEARCH_EMPTY_HINT), "T-41a-027: the anime hint");
        assert_eq!(
            source_route_for(BrowseKind::Anime, 2, "typed", false),
            BrowseRoute::AnimeBrowseSource { source_id: 2, query: BrowseQuery::Search("typed".into()) },
            "an anime row opens the anime page on the typed text"
        );
    }

    #[test]
    fn the_row_route_of_each_catalogue_is_its_own_browse_page() {
        assert_eq!(source_route_for(BrowseKind::Anime, 3, " ", false), BrowseRoute::AnimeBrowseSource { source_id: 3, query: BrowseQuery::Popular });
        assert_eq!(source_route_for(BrowseKind::Manga, 3, " ", false), BrowseRoute::BrowseSource { source_id: 3, query: BrowseQuery::Popular, migrating: false });
        assert!(matches!(source_route_for(BrowseKind::Manga, 3, "x", true), BrowseRoute::BrowseSource { migrating: true, .. }));
    }

    pub(crate) fn smoke() {
        an_anime_page_reads_the_anime_pins_hints_what_to_search_and_sends_its_rows_to_the_anime_page();
        a_page_with_a_query_searches_the_pinned_sources_and_draws_their_rows();
        the_has_results_toggle_is_stored_and_read_again_by_the_next_page();
        a_source_arrow_opens_its_listing_with_the_typed_text();
        a_card_opens_its_entry_and_one_result_with_an_extension_filter_replaces_the_page();
        a_migrate_search_with_a_missing_manga_is_an_error_state_that_searches_nothing();
    }
}
