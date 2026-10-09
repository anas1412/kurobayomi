//! The reader screen: the initial load behind the source gate, the failure card with Retry, adjacent chapter
//! navigation and the crop toggle. T-43a-023, T-43a-024, T-43a-026, T-43a-037 (wiring), part of T-43a-077.
//!
//! [`ReaderCore`] holds the behaviour and has no widgets, so it is tested without a display. [`open`] builds the
//! widgets over it. The page viewers (area 44) fill the `viewer` slot of the returned [`ReaderScreen`].
//!
//! ponytail: a chapter load runs on the GTK thread (a `ReaderChapter` is `Rc`, so it cannot move to a worker). The
//! spinner is shown and one frame is let through before the load starts, so the screen is never modal, but it does
//! not animate while an HTTP page list is fetched. Move the page-list fetch to the runtime when the viewers land.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};

use gtk::glib;
use gtk::prelude::*;
use kuro_core::sources::SourceGate;
use kuro_data::chapter::Chapter;
use kuro_db::Db;
use kuro_platform::toast::{ToastLength, Toaster};
use kuro_prefs::reader::{ReaderOrientation, ReaderPrefs, ReadingMode};
use tokio::sync::watch;

use super::bottom_bar::BottomBar;
use super::chapters::ListOptions;
use super::chrome::{self, TopBar, TopBarActions, build_error_card, build_top_bar};
use super::flags::{self, CropPref};
use super::indicator::PageIndicator;
use super::keep_awake::{self, KeepAwake};
use super::loader::{LoadFailure, Loader, load_chapter};
use super::mode_sheet::{self, Outcome};
use super::model::{LoadError, ReaderChapter, ReaderPage, ViewerChapters};
use super::nav::{self, Step};
use super::navigator::{Navigator, NavigatorState};
use super::overlay::Tint;
use super::session::{AdjacentOutcome, ReaderEvent, ReaderSession, ReaderStore, open_list};
use super::settings_window::{self, MangaChoices};
use super::sheet::{self, Sheet};
use super::sync::{self, Effect};
use super::state::{self, Dialog, EscapeAction, UiState};
use super::strings;
use super::theme;
use super::viewer::{self, Viewer, ViewerEvent, ViewerSlot};

/// Picks the loader of a chapter (see `loader::choose_loader`).
pub type BuildLoader = Rc<dyn Fn(&Rc<ReaderChapter>) -> Result<Loader, LoadError>>;

/// What the reader needs from the app.
pub struct ReaderDeps {
    pub db: Arc<Db>,
    pub manga_id: i64,
    pub chapter_id: i64,
    pub prefs: ReaderPrefs,
    pub store: Rc<dyn ReaderStore>,
    pub build: BuildLoader,
    pub is_downloaded: Rc<dyn Fn(&Chapter) -> bool>,
    pub downloaded_only: bool,
    pub incognito: bool,
    /// The store behind `prefs`. Its change feed applies live settings to the open reader.
    pub prefs_store: kuro_prefs::store::PrefStore,
}

/// The crop button after a toggle: the new value, the icon that reflects it and the toast text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CropView {
    pub value: bool,
    pub icon: &'static str,
    pub toast: &'static str,
}

pub const CROP_ICON_ON: &str = "image-crop-symbolic";
pub const CROP_ICON_OFF: &str = "image-x-generic-symbolic";

/// The crop icon for a state: crop when on, crop-off when off.
pub fn crop_icon(on: bool) -> &'static str {
    if on { CROP_ICON_ON } else { CROP_ICON_OFF }
}

/// The toast for a failed adjacent load. A blank message never gives an empty toast (E-TOAST-1).
pub fn failure_toast(message: &str) -> String {
    if message.trim().is_empty() { strings::UNKNOWN_ERROR.to_owned() } else { message.to_owned() }
}

/// Resolves once the source manager has finished initialising. `false` when it went away without doing so.
pub async fn wait_ready(mut ready: watch::Receiver<bool>) -> bool {
    while !*ready.borrow_and_update() {
        if ready.changed().await.is_err() {
            return false;
        }
    }
    true
}

/// The reader's behaviour without widgets.
pub struct ReaderCore {
    deps: ReaderDeps,
    session: RefCell<Option<ReaderSession>>,
    events: RefCell<Option<Receiver<ReaderEvent>>>,
    manga_title: RefCell<String>,
}

impl ReaderCore {
    pub fn new(deps: ReaderDeps) -> Self {
        Self { deps, session: RefCell::new(None), events: RefCell::new(None), manga_title: RefCell::new(String::new()) }
    }

    /// The reader's event feed for the viewers, once the initial load has made it. Taken once.
    pub fn take_events(&self) -> Option<Receiver<ReaderEvent>> {
        self.events.borrow_mut().take()
    }

    /// Loads the manga, builds the reader list and loads the requested chapter (T-43a-023). A failure leaves no
    /// session behind, so a retry repeats the whole load (T-43a-024).
    pub fn initial_load(&self) -> Result<(), LoadError> {
        *self.session.borrow_mut() = None;
        let prefs = &self.deps.prefs;
        let opened = open_list(
            Arc::clone(&self.deps.db),
            self.deps.manga_id,
            self.deps.chapter_id,
            |manga| ListOptions {
                skip_read: prefs.skip_read.get(),
                skip_filtered: prefs.skip_filtered.get(),
                skip_dupe: prefs.skip_dupe.get(),
                downloaded_only: self.deps.downloaded_only,
                chapter_flags: manga.chapter_flags,
            },
            |chapter| (self.deps.is_downloaded)(chapter),
        )
        .map_err(LoadError::new)?;
        let preserve = prefs.preserve_reading_position.get();
        let chapter = Rc::clone(&opened.chapters[opened.index]);
        load_chapter(&chapter, || (self.deps.build)(&chapter), preserve).map_err(|failure| match failure {
            LoadFailure::Failed(error) => error,
            LoadFailure::Cancelled => LoadError::new(strings::UNKNOWN_ERROR),
        })?;
        *self.manga_title.borrow_mut() = opened.manga.title.clone();
        let (tx, rx) = mpsc::channel();
        let session = ReaderSession::new(
            self.deps.manga_id,
            opened.manga.viewer_flags,
            opened.chapters,
            opened.index,
            self.deps.incognito,
            preserve,
            Rc::clone(&self.deps.store),
            tx,
        );
        *self.session.borrow_mut() = Some(session);
        *self.events.borrow_mut() = Some(rx);
        Ok(())
    }

    /// Whether the previous and next buttons are enabled: off at the ends and before the session exists.
    pub fn nav_enabled(&self) -> (bool, bool) {
        self.session.borrow().as_ref().map_or((false, false), |s| {
            let (previous, next) = s.neighbours();
            (previous.is_some(), next.is_some())
        })
    }

    /// Moves to the next or previous chapter. A failure keeps the current chapter (E-NAV-1).
    pub fn step(&self, step: Step) -> AdjacentOutcome {
        match self.session.borrow_mut().as_mut() {
            Some(session) => session.load_adjacent(step, |chapter| (self.deps.build)(chapter)),
            None => AdjacentOutcome::NoNeighbour,
        }
    }

    /// What the top bar shows for the current chapter: the manga title, the chapter name, the bookmark state, and
    /// whether the chapter comes from an HTTP source (which gets the web entries in the overflow menu).
    pub fn top_bar(&self) -> Option<TopBar> {
        let session = self.session.borrow();
        let chapter = session.as_ref()?.current_chapter()?;
        Some(TopBar {
            manga_title: self.manga_title.borrow().clone(),
            chapter_name: chapter.chapter.name.clone(),
            bookmarked: chapter.bookmark.get(),
            is_http_source: chapter.attached_is_local() == Some(false),
        })
    }

    /// The id of the chapter being read.
    pub fn current_chapter_id(&self) -> Option<i64> {
        self.session.borrow().as_ref()?.current_chapter().map(|c| c.chapter.id)
    }

    /// The source url of the chapter being read.
    pub fn chapter_url(&self) -> Option<String> {
        self.session.borrow().as_ref()?.current_chapter().map(|c| c.chapter.url.clone())
    }

    /// Flips the bookmark of the current chapter and writes it. Returns the new state.
    pub fn toggle_bookmark(&self) -> Option<bool> {
        self.session.borrow().as_ref()?.toggle_bookmark()
    }

    /// The current chapter and its neighbours, for a viewer that is attached.
    pub fn viewer_chapters(&self) -> Option<ViewerChapters> {
        self.session.borrow().as_ref()?.viewer_chapters()
    }

    /// The slider's move: asks `viewer` to show the page at the 0-based `index` of the current chapter. `false`
    /// when there is no such page.
    pub fn move_viewer_to(&self, viewer: &dyn Viewer, index: usize) -> bool {
        let page = self
            .session
            .borrow()
            .as_ref()
            .and_then(|s| s.current_chapter().and_then(|c| c.pages()))
            .and_then(|pages| pages.get(index).cloned());
        match page {
            Some(page) => {
                viewer.move_to_page(&page);
                true
            }
            None => false,
        }
    }

    /// The current chapter's position in the reader list.
    pub fn current_index(&self) -> Option<usize> {
        self.session.borrow().as_ref().and_then(|s| s.current_index())
    }

    /// The crop state for the current reading mode, for the button icon when the screen opens.
    pub fn crop_on(&self) -> bool {
        let pref = self.crop_pref();
        let prefs = &self.deps.prefs;
        match pref {
            CropPref::Pager => prefs.crop_borders.get(),
            CropPref::Webtoon => prefs.crop_borders_webtoon.get(),
        }
    }

    /// Flips the crop preference of the current mode and stores it (T-43a-037).
    pub fn toggle_crop(&self) -> CropView {
        let prefs = &self.deps.prefs;
        let toggle = flags::toggle_crop(self.mode(), prefs.crop_borders.get(), prefs.crop_borders_webtoon.get());
        let stored = match toggle.pref {
            CropPref::Pager => prefs.crop_borders.set(toggle.value),
            CropPref::Webtoon => prefs.crop_borders_webtoon.set(toggle.value),
        };
        if let Err(e) = stored {
            tracing::warn!(error = %e, "the crop preference could not be saved");
        }
        CropView { value: toggle.value, icon: crop_icon(toggle.value), toast: strings::on_off(toggle.value) }
    }

    /// The resolved reading mode of the current manga (the default preference applies to a Default manga).
    pub fn reading_mode(&self) -> ReadingMode {
        self.mode()
    }

    /// The raw reading mode of the manga, Default included. The bottom bar shows this value.
    pub fn manga_mode(&self) -> ReadingMode {
        self.session.borrow().as_ref().map_or(ReadingMode::Default, |s| flags::reading_mode_of(s.viewer_flags()))
    }

    /// The raw orientation of the manga, Default included.
    pub fn manga_orientation(&self) -> ReaderOrientation {
        self.session.borrow().as_ref().map_or(ReaderOrientation::Default, |s| flags::orientation_of(s.viewer_flags()))
    }

    /// The visible page (1-based, `-1` before any page) and the page total of the current chapter.
    pub fn page_counts(&self) -> (i32, i32) {
        self.session.borrow().as_ref().map_or((-1, 0), |s| {
            let total = s.current_index().and_then(|i| s.chapters().get(i)).and_then(|c| c.pages()).map_or(0, |p| p.len());
            (s.current_page() as i32, total as i32)
        })
    }

    /// Writes the manga's reading mode and reloads the viewers (T-043a-036).
    pub fn apply_mode(&self, mode: ReadingMode) {
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.set_reading_mode(mode);
        }
    }

    /// Writes the manga's orientation and reloads the viewers (T-043a-036).
    pub fn apply_orientation(&self, orientation: ReaderOrientation) {
        let default_pref = self.deps.prefs.default_orientation_type.get();
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.set_orientation(orientation, default_pref);
        }
    }

    pub fn show_page_number(&self) -> bool {
        self.deps.prefs.show_page_number.get()
    }

    pub fn theme_pref(&self) -> i32 {
        self.deps.prefs.reader_theme.get()
    }

    /// `on_page_selected` for a page a viewer shows (T-043a-093). Download-ahead and the flash are the hooks the
    /// caller passes in.
    pub fn page_selected(
        &self,
        page: &Rc<ReaderPage>,
        download_ahead: &mut dyn FnMut(usize),
        flash: &mut dyn FnMut(),
    ) -> Option<usize> {
        let build = Rc::clone(&self.deps.build);
        self.session.borrow_mut().as_mut().and_then(|s| s.on_page_selected(page, |c| build(c), download_ahead, flash))
    }

    /// A viewer asked for a chapter to be loaded in the background (T-043a-093).
    pub fn request_preload(&self, chapter: &Rc<ReaderChapter>) {
        let build = Rc::clone(&self.deps.build);
        let downloaded = (self.deps.is_downloaded)(&chapter.chapter);
        if let Some(session) = self.session.borrow_mut().as_mut() {
            session.on_request_preload(chapter, downloaded, |c| build(c));
        }
    }

    pub fn show_reading_mode(&self) -> bool {
        self.deps.prefs.show_reading_mode.get()
    }

    pub fn fullscreen_pref(&self) -> bool {
        self.deps.prefs.fullscreen.get()
    }

    pub fn keep_awake_pref(&self) -> bool {
        self.deps.prefs.keep_screen_on.get()
    }

    fn mode(&self) -> kuro_prefs::reader::ReadingMode {
        let flags_value = self.session.borrow().as_ref().map_or(0, |s| s.viewer_flags());
        flags::resolve_reading_mode(flags_value, self.deps.prefs.default_reading_mode.get())
    }

    fn crop_pref(&self) -> CropPref {
        flags::crop_pref_for(self.mode())
    }
}

/// The reader widget and the slot the viewers fill. `chrome` is what the viewers call: taps toggle the menu.
pub struct ReaderScreen {
    pub root: gtk::Box,
    pub viewer: gtk::Box,
    pub core: Rc<ReaderCore>,
    pub chrome: ReaderChrome,
}

/// The chrome controls for the viewers (area 44): a tap toggles the menu, and the menu drives the fullscreen state.
#[derive(Clone)]
pub struct ReaderChrome {
    screen: Weak<Screen>,
}

impl ReaderChrome {
    /// Routes a viewer event to the session and the chrome (T-043a-093). A long press waits for the page sheet
    /// (T-043a-045).
    pub fn route(&self, event: ViewerEvent) {
        let Some(screen) = self.screen.upgrade() else { return };
        match event {
            ViewerEvent::ToggleMenu => self.toggle_menu(),
            ViewerEvent::ShowMenu => self.show_menu(),
            ViewerEvent::HideMenu => self.hide_menu(),
            ViewerEvent::PageSelected(page) => {
                let (prefs, tint) = (&screen.core.deps.prefs, &screen.tint);
                // the flash counts only while the feature is on, so enabling it later does not flash at once
                let mut flash = || {
                    if prefs.flash_on_page_change.get() {
                        tint.on_page_changed(prefs);
                    }
                };
                let _ = screen.core.page_selected(&page, &mut |_| {}, &mut flash);
                sync_top_bar(&screen);
                refresh(&screen);
            }
            ViewerEvent::RequestPreload(chapter) => screen.core.request_preload(&chapter),
            ViewerEvent::LongTap(_) => {}
        }
    }

    /// Attaches a page viewer (area 44): the previous viewer is destroyed, `ReaderScreen::viewer` is cleared, and the
    /// new viewer gets the current chapters through `set_chapters`, where it adds its widget to the container. From
    /// then on the slider moves it.
    pub fn attach_viewer(&self, next: Box<dyn Viewer>) {
        let Some(screen) = self.screen.upgrade() else { return };
        let container = screen.viewer_box.clone();
        let chapters = screen.core.viewer_chapters();
        let mut slot = screen.viewer_slot.borrow_mut();
        viewer::swap(
            &mut slot,
            next,
            || {
                while let Some(child) = container.first_child() {
                    container.remove(&child);
                }
            },
            |v| {
                if let Some(chapters) = &chapters {
                    v.set_chapters(chapters);
                }
            },
        );
    }

    pub fn toggle_menu(&self) {
        if let Some(screen) = self.screen.upgrade() {
            let visible = !screen.ui.borrow().menu_visible;
            set_menu(&screen, visible);
        }
    }

    pub fn show_menu(&self) {
        if let Some(screen) = self.screen.upgrade() {
            set_menu(&screen, true);
        }
    }

    pub fn hide_menu(&self) {
        if let Some(screen) = self.screen.upgrade() {
            set_menu(&screen, false);
        }
    }
}

/// The widget state the reader's handlers share. The root is held weakly, so the screen does not keep its own
/// widget alive.
struct Screen {
    core: Rc<ReaderCore>,
    ready: watch::Receiver<bool>,
    toaster: Toaster,
    on_close: Rc<dyn Fn()>,
    root: glib::WeakRef<gtk::Box>,
    stack: gtk::Stack,
    /// The widget the viewers fill (`ReaderScreen::viewer`).
    viewer_box: gtk::Box,
    /// The viewer attached by area 44. The slider moves it; with none attached the slider has nothing to move.
    viewer_slot: RefCell<ViewerSlot>,
    tint: Tint,
    /// Holds the top bar, which is rebuilt when the chapter or its bookmark changes.
    top_slot: gtk::Box,
    /// The id of the chapter the top bar shows, `-1` before the first build.
    shown_chapter: Cell<i64>,
    top: gtk::Revealer,
    bottom: gtk::Revealer,
    navigator: Rc<Navigator>,
    bottom_bar: Rc<BottomBar>,
    indicator: PageIndicator,
    sheet: Rc<Sheet>,
    ui: RefCell<UiState>,
    keep_awake: KeepAwake,
    /// An adjacent load is waiting for its frame or running; further presses are ignored.
    stepping: Cell<bool>,
}

const PAGE_LOADING: &str = "loading";
const PAGE_CARD: &str = "card";
const PAGE_VIEWER: &str = "viewer";

/// `reader::open`: the reader page. It waits for the source manager, loads the manga and the requested chapter,
/// and shows the failure card with Retry and Close when that fails. The root widget is named `screen:reader`.
pub fn open(deps: ReaderDeps, gate: &SourceGate, toaster: Toaster, on_close: impl Fn() + 'static) -> ReaderScreen {
    theme::install_css();
    let theme_value = deps.prefs.reader_theme.get();
    let prefs_store = deps.prefs_store.clone();
    let core = Rc::new(ReaderCore::new(deps));

    let viewer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    viewer.set_vexpand(true);
    let loading = gtk::Spinner::builder().halign(gtk::Align::Center).valign(gtk::Align::Center).spinning(true).build();
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.add_named(&loading, Some(PAGE_LOADING));
    stack.add_named(&viewer, Some(PAGE_VIEWER));

    // the overlays sit over the pages only, below the chrome and the sheets
    let page_layer = gtk::Overlay::new();
    let tint = Tint::new(&page_layer, &stack);

    let navigator = Navigator::new();
    let top_slot = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let top_column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    top_column.append(&top_slot);
    top_column.append(&navigator.root);
    let top = gtk::Revealer::new();
    top.set_transition_type(gtk::RevealerTransitionType::SlideDown);
    top.set_transition_duration(200);
    top.set_child(Some(&top_column));
    top.set_reveal_child(true);
    let bottom_bar = BottomBar::new();
    let bottom = gtk::Revealer::new();
    bottom.set_transition_type(gtk::RevealerTransitionType::SlideUp);
    bottom.set_transition_duration(200);
    bottom.set_child(Some(&bottom_bar.root));
    bottom.set_reveal_child(true);

    let sheet = Sheet::new(&page_layer);
    let indicator = PageIndicator::new();
    sheet.root().add_overlay(&indicator.label);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    column.append(&top);
    column.append(sheet.root());
    column.append(&bottom);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_widget_name("screen:reader");
    root.set_focusable(true);
    root.append(&column);
    theme::apply_background(&root, theme_value);
    // the pages sit on their own background, so the colour filter blends with it and not with what is behind
    theme::apply_background(&stack, theme_value);

    let root_ref = glib::WeakRef::new();
    root_ref.set(Some(&root));
    let screen = Rc::new(Screen {
        core: Rc::clone(&core),
        ready: gate.ready_watch(),
        toaster,
        on_close: Rc::new(on_close),
        root: root_ref,
        stack,
        viewer_box: viewer.clone(),
        viewer_slot: RefCell::new(None),
        tint,
        top_slot,
        shown_chapter: Cell::new(-1),
        top,
        bottom,
        navigator: Rc::clone(&navigator),
        bottom_bar: Rc::clone(&bottom_bar),
        indicator,
        sheet: Rc::clone(&sheet),
        ui: RefCell::new(UiState::default()),
        keep_awake: KeepAwake::default(),
        stepping: Cell::new(false),
    });

    let weak = Rc::downgrade(&screen);
    let (step_weak, slider_weak) = (weak.clone(), weak.clone());
    navigator.connect(
        move |step| {
            if let Some(s) = step_weak.upgrade() {
                press(&s, step);
            }
        },
        move |index| {
            if let Some(s) = slider_weak.upgrade() {
                move_viewer(&s, index);
            }
        },
    );
    let (mode_weak, crop_weak, settings_weak) = (weak.clone(), weak.clone(), weak.clone());
    bottom_bar.connect(
        move || {
            if let Some(s) = mode_weak.upgrade() {
                open_mode_sheet(&s);
            }
        },
        // The rotation sheet is T-043a-044, which is not part of this change.
        || {},
        move || {
            if let Some(s) = crop_weak.upgrade() {
                toggle_crop(&s);
            }
        },
        move || {
            if let Some(s) = settings_weak.upgrade() {
                open_settings(&s);
            }
        },
    );
    let dismissed = weak.clone();
    sheet.connect_dismissed(move || {
        if let Some(s) = dismissed.upgrade() {
            s.ui.borrow_mut().dialog = None;
            // the focus came into the sheet, so it goes back to the reader for the keys
            if let Some(root) = s.root.upgrade() {
                root.grab_focus();
            }
        }
    });

    let keys = gtk::EventControllerKey::new();
    {
        let screen = Rc::clone(&screen);
        keys.connect_key_pressed(move |_, key, _, _| handle_key(&screen, key));
    }
    root.add_controller(keys);
    {
        let screen = Rc::clone(&screen);
        root.connect_map(move |root| {
            root.grab_focus();
            sync_keep_awake(&screen, true);
        });
    }
    {
        let screen = Rc::clone(&screen);
        root.connect_unmap(move |_| sync_keep_awake(&screen, false));
    }

    screen.tint.apply(&screen.core.deps.prefs, &screen.stack);
    sync::follow(&prefs_store, weak.clone(), apply_effect);
    start(&screen);
    ReaderScreen { root, viewer, core, chrome: ReaderChrome { screen: weak } }
}

fn window_of(screen: &Screen) -> Option<gtk::Window> {
    screen.root.upgrade().and_then(|root| root.root()).and_then(|root| root.downcast::<gtk::Window>().ok())
}

fn width_of(screen: &Screen) -> i32 {
    screen.root.upgrade().map_or(0, |root| root.width())
}

/// Takes or releases the keep-awake inhibit for the open reader (T-043a-051).
fn sync_keep_awake(screen: &Screen, reader_open: bool) {
    let Some(window) = window_of(screen) else { return };
    let want = keep_awake::should_inhibit(screen.core.keep_awake_pref(), reader_open);
    screen.keep_awake.sync(&window, want);
}

/// Applies one live setting change to the open reader (T-043a-080). The brightness, colour filter, grayscale,
/// invert and flash layers are the reader's own (`overlay`), over the page area only.
fn apply_effect(screen: &Screen, effect: Effect) {
    match effect {
        Effect::Theme => {
            if let Some(root) = screen.root.upgrade() {
                theme::apply_background(&root, screen.core.theme_pref());
                theme::apply_background(&screen.stack, screen.core.theme_pref());
            }
        }
        Effect::PageNumber | Effect::Fullscreen => refresh(screen),
        Effect::KeepAwake => {
            let open = screen.root.upgrade().is_some_and(|root| root.is_mapped());
            sync_keep_awake(screen, open);
        }
        Effect::Flash => {
            if !screen.core.deps.prefs.flash_on_page_change.get() {
                screen.tint.reset_flash();
            }
        }
        Effect::Brightness | Effect::ColorFilter => screen.tint.apply(&screen.core.deps.prefs, &screen.stack),
    }
}

/// Applies the chrome state: navigator, bottom bar, page indicator and the window fullscreen state.
fn refresh(screen: &Screen) {
    let (previous, next) = screen.core.nav_enabled();
    let (page, total) = screen.core.page_counts();
    let state = NavigatorState {
        total,
        current_page: page,
        previous_enabled: previous,
        next_enabled: next,
        rtl: screen.core.reading_mode() == ReadingMode::RightToLeft,
        loading: screen.stepping.get(),
    };
    screen.navigator.update(&state, width_of(screen));
    let ui = *screen.ui.borrow();
    screen.indicator.update(page, total, ui.menu_visible, screen.core.show_page_number());
    screen.bottom_bar.update(screen.core.manga_mode(), screen.core.manga_orientation(), screen.core.crop_on());
    apply_fullscreen(screen, ui.menu_visible);
}

fn apply_fullscreen(screen: &Screen, menu_visible: bool) {
    let Some(window) = window_of(screen) else { return };
    let want = state::fullscreen_for_menu(menu_visible, screen.core.fullscreen_pref());
    if window.is_fullscreen() != want {
        if want { window.fullscreen() } else { window.unfullscreen() }
    }
}

fn set_menu(screen: &Screen, visible: bool) {
    screen.ui.borrow_mut().set_menu_visible(visible);
    screen.top.set_reveal_child(visible);
    screen.bottom.set_reveal_child(visible);
    refresh(screen);
}

fn handle_key(screen: &Rc<Screen>, key: gtk::gdk::Key) -> glib::Propagation {
    if key == gtk::gdk::Key::F11 {
        if let Some(window) = window_of(screen) {
            if state::toggle_fullscreen(window.is_fullscreen()) { window.fullscreen() } else { window.unfullscreen() }
        }
        return glib::Propagation::Stop;
    }
    if key == gtk::gdk::Key::Escape {
        // first on the root, wherever the focus is: an open sheet is dismissed before anything else (T-043a-079)
        match state::escape_action(screen.sheet.is_open(), window_of(screen).is_some_and(|w| w.is_fullscreen())) {
            EscapeAction::DismissSheet => screen.sheet.dismiss(),
            EscapeAction::LeaveFullscreen => {
                if let Some(window) = window_of(screen) {
                    window.unfullscreen();
                }
            }
            EscapeAction::CloseReader => (screen.on_close)(),
        }
        return glib::Propagation::Stop;
    }
    // the keys of the sheet that is open stay with it
    if screen.sheet.is_open() {
        return glib::Propagation::Proceed;
    }
    match key.to_unicode().and_then(nav::key_step) {
        Some(step) => {
            press(screen, step);
            glib::Propagation::Stop
        }
        None => glib::Propagation::Proceed,
    }
}

fn open_mode_sheet(screen: &Rc<Screen>) {
    let weak = Rc::downgrade(screen);
    let done: Rc<dyn Fn(Outcome)> = Rc::new(move |outcome| {
        let Some(s) = weak.upgrade() else { return };
        let mode = match outcome {
            Outcome::Apply { mode, toast } => {
                if let Some(text) = toast {
                    s.toaster.show(text, ToastLength::Short);
                }
                mode
            }
            Outcome::Revert => ReadingMode::Default,
        };
        s.core.apply_mode(mode);
        s.sheet.dismiss();
        refresh(&s);
    });
    let content = mode_sheet::build(screen.core.manga_mode(), screen.core.show_reading_mode(), done);
    screen.ui.borrow_mut().open(Dialog::ReadingModeSelect);
    screen.sheet.show(&content);
}

fn toggle_crop(screen: &Screen) {
    let view = screen.core.toggle_crop();
    screen.toaster.show(view.toast, ToastLength::Short);
    refresh(screen);
}

/// The reader settings window (T-043a-046, T-043a-047). Its chips apply to this manga at once.
fn open_settings(screen: &Rc<Screen>) {
    let Some(window) = window_of(screen) else { return };
    let (mode_weak, orientation_weak) = (Rc::downgrade(screen), Rc::downgrade(screen));
    let choices = MangaChoices {
        on_mode: Rc::new(move |mode| {
            if let Some(s) = mode_weak.upgrade() {
                s.core.apply_mode(mode);
                refresh(&s);
            }
        }),
        on_orientation: Rc::new(move |orientation| {
            if let Some(s) = orientation_weak.upgrade() {
                s.core.apply_orientation(orientation);
                refresh(&s);
            }
        }),
    };
    let body = settings_window::notebook(&screen.core.deps.prefs, screen.core.manga_mode(), screen.core.manga_orientation(), choices);
    let dialog = sheet::modal_window(&window, strings::ACTION_SETTINGS, &body);
    screen.ui.borrow_mut().open(Dialog::Settings);
    let closed = Rc::downgrade(screen);
    dialog.connect_close_request(move |_| {
        if let Some(s) = closed.upgrade() {
            s.ui.borrow_mut().close(Dialog::Settings);
            refresh(&s);
        }
        glib::Propagation::Proceed
    });
    dialog.present();
}

/// The initial load. Clears a failure card, shows the spinner, waits for the source manager, then loads.
fn start(screen: &Rc<Screen>) {
    if let Some(card) = screen.stack.child_by_name(PAGE_CARD) {
        screen.stack.remove(&card);
    }
    screen.stack.set_visible_child_name(PAGE_LOADING);
    refresh(screen);
    let screen = Rc::clone(screen);
    glib::spawn_future_local(async move {
        let result = if wait_ready(screen.ready.clone()).await {
            // one frame, so the spinner is on screen before the load takes the thread
            glib::timeout_future(std::time::Duration::ZERO).await;
            screen.core.initial_load()
        } else {
            Err(LoadError::new(strings::UNKNOWN_ERROR))
        };
        match result {
            Ok(()) => {
                screen.stack.set_visible_child_name(PAGE_VIEWER);
                sync_top_bar(&screen);
                refresh(&screen);
            }
            Err(error) => show_card(&screen, &error),
        }
    });
}

fn show_card(screen: &Rc<Screen>, error: &LoadError) {
    let (retry_screen, close) = (Rc::clone(screen), Rc::clone(&screen.on_close));
    let card = build_error_card(error, move || start(&retry_screen), move || close());
    screen.stack.add_named(&card, Some(PAGE_CARD));
    screen.stack.set_visible_child_name(PAGE_CARD);
}

/// The slider's page index goes to the attached viewer's `move_to_page`. Until area 44 attaches a viewer with
/// `ReaderChrome::attach_viewer` there is nothing to move, and the slider does nothing (blocked on area 44).
fn move_viewer(screen: &Screen, index: usize) {
    if let Some(viewer) = screen.viewer_slot.borrow().as_ref() {
        screen.core.move_viewer_to(viewer.as_ref(), index);
    }
}

/// Rebuilds the top bar when the chapter shown is not the current one.
fn sync_top_bar(screen: &Rc<Screen>) {
    if screen.core.current_chapter_id() != Some(screen.shown_chapter.get()) {
        rebuild_top_bar(screen);
    }
}

/// Builds the top bar for the current chapter (T-043a-040): back, the manga and chapter title, the bookmark toggle
/// and the overflow menu. Back and the title return to the page below, which is the manga's page.
fn rebuild_top_bar(screen: &Rc<Screen>) {
    let Some(info) = screen.core.top_bar() else { return };
    screen.shown_chapter.set(screen.core.current_chapter_id().unwrap_or(-1));
    while let Some(child) = screen.top_slot.first_child() {
        screen.top_slot.remove(&child);
    }
    let (back, open_manga) = (Rc::clone(&screen.on_close), Rc::clone(&screen.on_close));
    let (bookmark, overflow) = (Rc::downgrade(screen), Rc::downgrade(screen));
    let actions = TopBarActions {
        on_back: Box::new(move || back()),
        on_open_manga: Box::new(move || open_manga()),
        on_bookmark: Box::new(move || {
            let Some(screen) = bookmark.upgrade() else { return };
            screen.core.toggle_bookmark();
            // the bar is rebuilt after the click has finished, so its icon and label follow the new state
            glib::idle_add_local_once(move || rebuild_top_bar(&screen));
        }),
        on_overflow: Box::new(move |index| {
            if let Some(screen) = overflow.upgrade() {
                open_overflow(&screen, index);
            }
        }),
    };
    screen.top_slot.append(&build_top_bar(&info, actions));
}

/// An overflow entry: open in the web view or the browser, or share (copy) the chapter's web address.
fn open_overflow(screen: &Rc<Screen>, index: usize) {
    let Some(url) = screen.core.chapter_url().and_then(|u| chrome::web_url(&u)) else {
        screen.toaster.show(strings::UNKNOWN_ERROR, ToastLength::Short);
        return;
    };
    if chrome::overflow_items(true).get(index) == Some(&strings::ACTION_SHARE) {
        if let Some(root) = screen.root.upgrade() {
            root.clipboard().set_text(&url);
        }
        return;
    }
    gtk::UriLauncher::new(&url).launch(window_of(screen).as_ref(), None::<&gtk::gio::Cancellable>, |_| {});
}

/// A press of a chapter button or of N or P. At the ends, or while a load runs, it does nothing.
fn press(screen: &Rc<Screen>, step: Step) {
    let (previous, next) = screen.core.nav_enabled();
    let enabled = match step {
        Step::Previous => previous,
        Step::Next => next,
    };
    if !enabled || screen.stepping.replace(true) {
        return;
    }
    refresh(screen);
    let screen = Rc::clone(screen);
    glib::idle_add_local_once(move || {
        let outcome = screen.core.step(step);
        screen.stepping.set(false);
        sync_top_bar(&screen);
        refresh(&screen);
        if let AdjacentOutcome::Failed(message) = outcome {
            screen.toaster.show(failure_toast(&message), ToastLength::Short);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::cache::{CACHE_LIMIT_BYTES, DiskCache};
    use crate::reader::loader::PageEntry;
    use crate::reader::loader::http::{HttpLoader, HttpPageSource};
    use kuro_data::chapter::ChapterRepo;
    use kuro_data::manga::{Manga, MangaRepo};
    use kuro_prefs::store::PrefStore;
    use kuro_prefs::reader::ReadingMode;

    struct Pages;

    impl HttpPageSource for Pages {
        fn page_list(&self, _chapter_url: &str) -> Result<Vec<PageEntry>, String> {
            Ok(vec![PageEntry { url: "p0".into(), image_url: None }, PageEntry { url: "p1".into(), image_url: None }])
        }
        fn image_url(&self, page_url: &str) -> Result<String, String> {
            Ok(format!("https://img/{page_url}"))
        }
        fn image(&self, _image_url: &str) -> Result<Vec<u8>, String> {
            Ok(b"img".to_vec())
        }
    }

    #[derive(Default)]
    struct NoStore;

    impl ReaderStore for NoStore {
        fn save_progress(&self, _: i64, _: i64, _: bool) {}
        fn set_bookmark(&self, _: i64, _: bool) {}
        fn set_viewer_flags(&self, _: i64, _: i64, _: i64) {}
        fn flush(&self) {}
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        db: Arc<Db>,
        manga_id: i64,
        chapter_ids: Vec<i64>,
    }

    fn fixture(chapters: usize) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let path = kuro_db::schema_manga::db_path(dir.path());
        let writer = kuro_db::schema_manga::open(&path).unwrap();
        let db = Arc::new(Db::new(&path, writer).unwrap());
        let manga_id = MangaRepo::new(Arc::clone(&db)).insert(&Manga::create()).unwrap();
        let new: Vec<Chapter> = (0..chapters)
            .map(|i| {
                let mut c = Chapter::create();
                c.manga_id = manga_id;
                c.url = format!("/c/{i}");
                c.chapter_number = (i + 1) as f64;
                c.source_order = (chapters - i) as i64;
                c
            })
            .collect();
        let chapter_ids = ChapterRepo::new(Arc::clone(&db)).add_all(&new).unwrap().iter().map(|c| c.id).collect();
        Fixture { _dir: dir, db, manga_id, chapter_ids }
    }

    fn http_build(tag: &'static str) -> BuildLoader {
        Rc::new(move |chapter| {
            let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/reader-screen-tests").join(tag);
            let cache = Arc::new(DiskCache::open(root, CACHE_LIMIT_BYTES).unwrap());
            Ok(Loader::Http(HttpLoader::new(Arc::new(Pages), 1, chapter.chapter.url.clone(), cache)))
        })
    }

    fn core(f: &Fixture, requested: usize, build: BuildLoader) -> ReaderCore {
        ReaderCore::new(ReaderDeps {
            db: Arc::clone(&f.db),
            manga_id: f.manga_id,
            chapter_id: f.chapter_ids[requested],
            prefs: ReaderPrefs::new(PrefStore::in_memory()),
            store: Rc::new(NoStore),
            build,
            is_downloaded: Rc::new(|_| false),
            downloaded_only: false,
            incognito: false,
            prefs_store: PrefStore::in_memory(),
        })
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap()
    }

    #[test]
    fn opening_waits_for_the_source_manager_then_proceeds() {
        runtime().block_on(async {
            let (registry, ready) = watch::channel(false);
            let waiting = tokio::spawn(wait_ready(ready.clone()));
            tokio::task::yield_now().await;
            assert!(!waiting.is_finished(), "the open must not go on while the gate is closed");
            registry.send_replace(true);
            assert!(waiting.await.unwrap());
            assert!(wait_ready(ready).await, "an open gate does not wait");
        });
    }

    #[test]
    fn a_source_manager_that_goes_away_unready_ends_the_wait() {
        runtime().block_on(async {
            let (registry, ready) = watch::channel(false);
            drop(registry);
            assert!(!wait_ready(ready).await);
        });
    }

    #[test]
    fn the_initial_load_opens_the_requested_chapter() {
        let f = fixture(3);
        let core = core(&f, 1, http_build("open"));
        assert_eq!(core.nav_enabled(), (false, false), "nothing to navigate before the load");
        core.initial_load().unwrap();
        assert!(core.take_events().is_some());
        assert_eq!(core.nav_enabled(), (true, true));
    }

    #[test]
    fn a_missing_chapter_or_manga_gives_the_card_message() {
        let f = fixture(1);
        let mut other = core(&f, 0, http_build("missing"));
        other.deps.chapter_id = 9999;
        assert_eq!(other.initial_load().unwrap_err().message, "Requested chapter of id 9999 not found in chapter list");
        other.deps.manga_id = 9999;
        assert_eq!(other.initial_load().unwrap_err().message, "Unknown err");
    }

    #[test]
    fn retry_repeats_the_whole_initial_load_and_can_succeed() {
        let f = fixture(2);
        let fail = Rc::new(Cell::new(true));
        let flag = Rc::clone(&fail);
        let inner = http_build("retry");
        let core = core(&f, 0, Rc::new(move |chapter| if flag.get() { Err(LoadError::new("offline")) } else { inner(chapter) }));
        let error = core.initial_load().unwrap_err();
        assert_eq!(super::super::chrome::error_card_message(&error), "Failed to load pages: offline");
        assert_eq!(core.nav_enabled(), (false, false), "a failed load leaves no session");
        fail.set(false);
        core.initial_load().unwrap();
        assert_eq!(core.nav_enabled(), (false, true));
    }

    #[test]
    fn next_and_previous_are_disabled_at_the_ends_and_the_buttons_follow_the_move() {
        let f = fixture(3);
        let core = core(&f, 0, http_build("nav"));
        core.initial_load().unwrap();
        assert_eq!(core.nav_enabled(), (false, true));
        assert_eq!(core.step(Step::Previous), AdjacentOutcome::NoNeighbour);
        assert_eq!(core.step(Step::Next), AdjacentOutcome::Moved(1));
        assert_eq!(core.nav_enabled(), (true, true));
        assert_eq!(core.step(Step::Next), AdjacentOutcome::Moved(2));
        assert_eq!(core.nav_enabled(), (true, false));
        assert_eq!(core.current_index(), Some(2));
    }

    #[test]
    fn a_failed_adjacent_load_reports_its_message_and_keeps_the_chapter() {
        let f = fixture(2);
        let fail = Rc::new(Cell::new(false));
        let flag = Rc::clone(&fail);
        let inner = http_build("adjacent-fail");
        let core = core(&f, 0, Rc::new(move |chapter| if flag.get() { Err(LoadError::new("offline")) } else { inner(chapter) }));
        core.initial_load().unwrap();
        fail.set(true);
        assert_eq!(core.step(Step::Next), AdjacentOutcome::Failed("offline".into()));
        assert_eq!(core.current_index(), Some(0));
        assert_eq!(failure_toast("offline"), "offline");
        assert_eq!(failure_toast("  "), "Unknown error");
    }

    #[test]
    fn the_crop_toggle_writes_the_preference_of_the_mode_and_reports_on_then_off() {
        let f = fixture(1);
        let core = core(&f, 0, http_build("crop"));
        core.initial_load().unwrap();
        // the default mode preference is right to left, a pager mode
        assert_eq!(core.mode(), ReadingMode::RightToLeft);
        assert!(!core.crop_on());
        let on = core.toggle_crop();
        assert_eq!(on, CropView { value: true, icon: CROP_ICON_ON, toast: "On" });
        assert!(core.deps.prefs.crop_borders.get());
        assert!(!core.deps.prefs.crop_borders_webtoon.get(), "only the pager preference changes");
        assert!(core.crop_on());
        let off = core.toggle_crop();
        assert_eq!(off, CropView { value: false, icon: CROP_ICON_OFF, toast: "Off" });
        assert!(!core.deps.prefs.crop_borders.get());
    }

    #[test]
    fn a_webtoon_manga_toggles_the_webtoon_crop_preference() {
        let f = fixture(1);
        MangaRepo::new(Arc::clone(&f.db)).set_viewer_bits(f.manga_id, i64::from(ReadingMode::Webtoon.flag_value()), 0x7).unwrap();
        let core = core(&f, 0, http_build("crop-webtoon"));
        core.initial_load().unwrap();
        assert_eq!(core.toggle_crop().toast, "On");
        assert!(core.deps.prefs.crop_borders_webtoon.get());
        assert!(!core.deps.prefs.crop_borders.get());
    }

    #[derive(Default)]
    struct MoveLog(RefCell<Vec<u32>>);

    impl Viewer for MoveLog {
        fn set_chapters(&self, _: &ViewerChapters) {}
        fn move_to_page(&self, page: &Rc<ReaderPage>) {
            self.0.borrow_mut().push(page.index);
        }
        fn destroy(&self) {}
        fn handle_key(&self, _: gtk::gdk::Key, _: gtk::gdk::ModifierType) -> bool {
            false
        }
        fn handle_motion(&self, _: f64, _: f64) -> bool {
            false
        }
    }

    #[test]
    fn the_slider_asks_the_viewer_for_the_page_at_its_index() {
        let f = fixture(1);
        let core = core(&f, 0, http_build("slider"));
        let viewer = MoveLog::default();
        assert!(!core.move_viewer_to(&viewer, 0), "nothing is loaded before the initial load");
        core.initial_load().unwrap();
        assert!(core.move_viewer_to(&viewer, 1));
        assert!(!core.move_viewer_to(&viewer, 99), "an index past the last page moves nothing");
        assert_eq!(viewer.0.borrow().as_slice(), [1]);
    }

    #[test]
    fn the_top_bar_shows_the_manga_the_chapter_the_bookmark_and_the_source_kind() {
        let f = fixture(2);
        MangaRepo::new(Arc::clone(&f.db))
            .update(&kuro_data::manga::MangaUpdate { title: Some("My Manga".into()), ..kuro_data::manga::MangaUpdate::new(f.manga_id) })
            .unwrap();
        let core = core(&f, 1, http_build("top-bar"));
        assert!(core.top_bar().is_none(), "no bar before the initial load");
        core.initial_load().unwrap();
        let bar = core.top_bar().unwrap();
        assert_eq!((bar.manga_title.as_str(), bar.bookmarked, bar.is_http_source), ("My Manga", false, true));
        assert_eq!(core.current_chapter_id(), Some(f.chapter_ids[1]));
        assert_eq!(core.toggle_bookmark(), Some(true));
        assert!(core.top_bar().unwrap().bookmarked);
        assert_eq!(core.step(Step::Previous), AdjacentOutcome::Moved(0));
        assert_eq!(core.current_chapter_id(), Some(f.chapter_ids[0]), "the bar follows the chapter");
        assert!(!core.top_bar().unwrap().bookmarked, "the bookmark belongs to the chapter, not the reader");
    }
}
