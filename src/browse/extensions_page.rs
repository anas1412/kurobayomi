//! The Extensions tab: a grouped list with headers, 40 px rows with a busy spinner and action buttons, a refresh that
//! keeps its spinner for a second, and the repos button (T-40a-017). The install, update, cancel and uninstall rules
//! are `installs::Installs`; this file runs its effects on the runtime and draws the rows.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_channel::Sender;
use gtk::glib;
use gtk::prelude::*;
use kuro_extensions::InstallStep;
use kuro_i18n::{MsgKey, tr};
use tokio::sync::watch;
use tokio::task::AbortHandle;

use super::dialogs::{ButtonStyle, DialogSlot, DialogSpec};
use super::extensions::{
    Ext, ExtEmpty, ExtState, GroupKey, Refresh, RefreshGuard, RowAction, fetch_failed_banner, language_header, language_line,
    real_pkg, refresh_remaining_ms, row_actions, split_available, status_text, update_all_queue, version_line, warning,
    warning_text,
};
use super::glue::{consume_events, keep_alive, own_tasks};
use super::installs::{Effect, Event, Input, Installs};
use super::tasks::{PageTasks, Tagged};
use super::trust::{TrustAnswer, TrustEffect, answer as trust_answer, trust_dialog};
use super::icons::resolve_for;
use super::view::{icon_image, spinner_row};

pub type JobFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
/// A refresh that can fail: the error text is not shown, the page shows its fetch-failed banner (T-41a-013).
pub type RefreshFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
/// How a running job reports its step to the page.
pub type Reporter = Arc<dyn Fn(InstallStep) + Send + Sync>;

/// What the tab needs from the extension manager. Every call runs on the tokio runtime.
pub trait ExtensionBackend: Send + Sync + 'static {
    /// Downloads and installs (or updates) `pkg`, reporting each step. Returns when the job is over.
    fn install(&self, pkg: String, report: Reporter) -> JobFuture;
    /// Removes the package.
    fn uninstall(&self, pkg: String) -> JobFuture;
    /// `findAvailableExtensions`: reloads the lists of the repos. An error keeps the previous snapshot on screen.
    fn refresh(&self) -> RefreshFuture;
}

/// What the buttons do outside the tab.
pub struct ExtNav {
    /// An installed extension's row was activated, or its settings button pressed.
    pub open_details: Rc<dyn Fn(&str)>,
    /// The trust button of an untrusted extension.
    pub trust: Rc<dyn Fn(&str)>,
    /// The web view button of an available extension.
    pub web_view: Rc<dyn Fn(&str)>,
    /// The repos button.
    pub open_repos: Rc<dyn Fn()>,
    /// The NSFW setting, read on every rebuild.
    pub show_nsfw: Rc<dyn Fn() -> bool>,
}

enum ExtEvent {
    Job { pkg: String, job: u64, event: Event },
    Refreshed { failed: bool },
}

/// Sends the end of a job when the task is dropped, so an aborted or panicking job still ends.
struct EndOnDrop {
    tx: Sender<Tagged<ExtEvent>>,
    generation: u64,
    pkg: String,
    job: u64,
}

impl Drop for EndOnDrop {
    fn drop(&mut self) {
        let event = ExtEvent::Job { pkg: std::mem::take(&mut self.pkg), job: self.job, event: Event::End };
        let _ = self.tx.try_send(Tagged { generation: self.generation, event });
    }
}

/// Reports the end of a refresh however the task ends, so the spinner always stops (E-40-28).
struct RefreshedOnDrop {
    tx: Sender<Tagged<ExtEvent>>,
    generation: u64,
    /// True until the fetch reports success, so an aborted or panicking fetch counts as failed.
    failed: bool,
}

impl Drop for RefreshedOnDrop {
    fn drop(&mut self) {
        let _ = self.tx.try_send(Tagged { generation: self.generation, event: ExtEvent::Refreshed { failed: self.failed } });
    }
}

struct RowWidgets {
    shown: Cell<InstallStep>,
    ext: Ext,
    spinner: gtk::Spinner,
    status: gtk::Label,
    actions: gtk::Box,
}

struct Inner {
    backend: Arc<dyn ExtensionBackend>,
    nav: ExtNav,
    tasks: Rc<RefCell<PageTasks<ExtEvent>>>,
    installs: RefCell<Installs>,
    handles: RefCell<BTreeMap<u64, AbortHandle>>,
    /// The models as given.
    exts: RefCell<Vec<Ext>>,
    /// `exts` after the per-source split; its rows are what the list shows and what the install steps are keyed by.
    shown: RefCell<Vec<Ext>>,
    /// `enabled_languages`; `None` shows every language.
    enabled: RefCell<Option<BTreeSet<String>>>,
    query: RefCell<String>,
    /// One entry per row on screen.
    rows: RefCell<Vec<RowWidgets>>,
    dialogs: DialogSlot,
    /// The load problems of the packages, by folder; the list ends with them (T-18a-054).
    problems: RefCell<Vec<(String, String)>>,
    /// The page was shown once, so its first fetch has started.
    opened: Cell<bool>,
    banner: gtk::Box,
    refresh: Refresh,
    refresh_guard: RefCell<Option<RefreshGuard>>,
    refresh_started: Cell<Option<Instant>>,
    refresh_spinner: gtk::Spinner,
    refresh_button: gtk::Button,
    column: gtk::Box,
    stack: gtk::Stack,
    empty_label: gtk::Label,
}

/// The tab. Cloning shares it; the widget keeps it alive, and destroying the widget aborts every job.
#[derive(Clone)]
pub struct ExtensionsPage {
    inner: Rc<Inner>,
    root: gtk::Box,
}

impl ExtensionsPage {
    pub fn new(backend: Arc<dyn ExtensionBackend>, nav: ExtNav) -> Self {
        let refresh_spinner = gtk::Spinner::new();
        let refresh_button = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh_button.set_tooltip_text(Some(&tr(MsgKey::action_webview_refresh)));
        let repos = gtk::Button::from_icon_name("folder-download-symbolic");
        repos.set_tooltip_text(Some(&tr(MsgKey::label_extension_repos)));
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_margin_start(16);
        toolbar.set_margin_end(16);
        toolbar.set_margin_top(8);
        toolbar.append(&gtk::Label::builder().label(tr(MsgKey::label_extensions)).hexpand(true).xalign(0.0).css_classes(["title-4"]).build());
        toolbar.append(&refresh_spinner);
        toolbar.append(&refresh_button);
        toolbar.append(&repos);

        let banner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        banner.set_margin_start(16);
        banner.set_margin_end(16);
        banner.set_margin_top(8);
        banner.add_css_class("toast");
        banner.append(&gtk::Label::builder().label(fetch_failed_banner().message).hexpand(true).xalign(0.0).wrap(true).build());
        let retry = gtk::Button::with_label(&tr(MsgKey::action_retry));
        banner.append(&retry);
        banner.set_visible(false);

        let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroller = gtk::ScrolledWindow::builder().child(&column).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
        let empty_label = gtk::Label::new(None);
        let empty = gtk::Box::new(gtk::Orientation::Vertical, 12);
        empty.set_valign(gtk::Align::Center);
        empty.append(&empty_label);
        let repos_in_empty = gtk::Button::with_label(&tr(MsgKey::label_extension_repos));
        repos_in_empty.set_halign(gtk::Align::Center);
        empty.append(&repos_in_empty);
        let stack = gtk::Stack::new();
        stack.add_named(&spinner_row(), Some("loading"));
        stack.add_named(&empty, Some("empty"));
        stack.add_named(&scroller, Some("list"));
        stack.set_visible_child_name("loading");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&toolbar);
        root.append(&banner);
        root.append(&stack);
        stack.set_vexpand(true);

        let inner = Rc::new(Inner {
            backend,
            nav,
            tasks: Rc::new(RefCell::new(PageTasks::new())),
            installs: RefCell::new(Installs::default()),
            handles: RefCell::new(BTreeMap::new()),
            exts: RefCell::new(Vec::new()),
            shown: RefCell::new(Vec::new()),
            enabled: RefCell::new(None),
            dialogs: DialogSlot::new(),
            problems: RefCell::new(Vec::new()),
            opened: Cell::new(false),
            banner,
            query: RefCell::new(String::new()),
            rows: RefCell::new(Vec::new()),
            refresh: Refresh::default(),
            refresh_guard: RefCell::new(None),
            refresh_started: Cell::new(None),
            refresh_spinner,
            refresh_button: refresh_button.clone(),
            column,
            stack,
            empty_label,
        });
        // the root widget owns the state; the state never owns the root, or neither would be freed
        keep_alive(&root, &inner);
        own_tasks(&root, &inner.tasks);
        let weak = Rc::downgrade(&inner);
        consume_events(&inner.tasks, move |event| {
            if let Some(inner) = weak.upgrade() {
                inner.event(event);
            }
        });
        let weak = Rc::downgrade(&inner);
        refresh_button.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.start_refresh();
            }
        });
        let weak = Rc::downgrade(&inner);
        retry.connect_clicked(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.start_refresh();
            }
        });
        // F5 starts one fetch from anywhere in the page (T-41a-013)
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(&inner);
        keys.connect_key_pressed(move |_, key, _, _| match (key == gtk::gdk::Key::F5, weak.upgrade()) {
            (true, Some(inner)) => {
                inner.start_refresh();
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        });
        root.add_controller(keys);
        // the fetch of the Kotlin `init` runs when the page is first shown
        let weak = Rc::downgrade(&inner);
        root.connect_map(move |_| {
            if let Some(inner) = weak.upgrade() {
                inner.opened();
            }
        });
        for button in [repos, repos_in_empty] {
            let weak = Rc::downgrade(&inner);
            button.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    (inner.nav.open_repos)();
                }
            });
        }
        Self { inner, root }
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.root
    }

    /// The extensions from the models, replacing the list. Rows keep the install steps of their jobs.
    pub fn set_exts(&self, exts: Vec<Ext>) {
        *self.inner.exts.borrow_mut() = exts;
        self.inner.rebuild();
    }

    /// The load problems to list under the extensions, as `(folder, message)`.
    pub fn set_problems(&self, problems: Vec<(String, String)>) {
        *self.inner.problems.borrow_mut() = problems;
        self.inner.rebuild();
    }

    /// The enabled languages that available extensions are split and filtered by. `None` shows every language.
    pub fn set_enabled_languages(&self, enabled: Option<BTreeSet<String>>) {
        *self.inner.enabled.borrow_mut() = enabled;
        self.inner.rebuild();
    }

    /// The page was shown. The first call starts the fetch of the available extensions, later calls do nothing.
    pub fn opened(&self) {
        self.inner.opened();
    }

    /// True while the fetch-failed banner is up.
    pub fn banner_visible(&self) -> bool {
        self.inner.banner.is_visible()
    }

    /// The text of the shared search entry, already debounced.
    pub fn set_query(&self, query: &str) {
        *self.inner.query.borrow_mut() = query.to_owned();
        self.inner.rebuild();
    }

    /// Runs `findAvailableExtensions`. The spinner stays up for at least a second and always stops.
    pub fn refresh(&self) {
        self.inner.start_refresh();
    }

    pub fn is_refreshing(&self) -> bool {
        self.inner.refresh.is_running()
    }

    /// The step shown for a package.
    pub fn step(&self, pkg: &str) -> InstallStep {
        self.inner.installs.borrow().step(pkg)
    }

    /// A button of a row, as the row's own button would do it.
    pub fn press(&self, pkg: &str, action: RowAction) {
        self.inner.press(pkg, action);
    }

    pub fn update_all(&self) {
        self.inner.update_all();
    }

    /// Cancels the update-all queue and its running package.
    pub fn cancel_update_all(&self) {
        self.inner.input(Input::CancelUpdateAll);
    }

    /// The package was removed from the registry elsewhere: drop its job and row state.
    pub fn uninstall(&self, pkg: &str) {
        self.inner.input(Input::Row { pkg: pkg.to_owned(), event: Event::Uninstall });
    }
}

impl Inner {
    fn opened(self: &Rc<Self>) {
        if !self.opened.replace(true) {
            self.start_refresh();
        }
    }

    fn start_refresh(self: &Rc<Self>) {
        if self.refresh.is_running() {
            return;
        }
        self.banner.set_visible(false);
        *self.refresh_guard.borrow_mut() = Some(self.refresh.begin());
        self.refresh_started.set(Some(Instant::now()));
        self.refresh_spinner.start();
        self.refresh_button.set_sensitive(false);
        let (generation, tx) = self.tasks.borrow().sender();
        let job = self.backend.refresh();
        self.tasks.borrow_mut().spawn(async move {
            let mut done = RefreshedOnDrop { tx, generation, failed: true };
            done.failed = job.await.is_err();
        });
    }

    fn event(self: &Rc<Self>, event: ExtEvent) {
        match event {
            ExtEvent::Job { pkg, job, event } => {
                if event == Event::End {
                    self.handles.borrow_mut().remove(&job);
                }
                self.input(Input::Job { pkg, job, event });
            }
            ExtEvent::Refreshed { failed } => {
                let elapsed = self.refresh_started.take().map_or(u64::MAX, |at| at.elapsed().as_millis() as u64);
                let weak = Rc::downgrade(self);
                glib::timeout_add_local_once(Duration::from_millis(refresh_remaining_ms(elapsed)), move || {
                    let Some(inner) = weak.upgrade() else { return };
                    inner.refresh_guard.borrow_mut().take();
                    inner.refresh_spinner.stop();
                    inner.refresh_button.set_sensitive(true);
                    inner.banner.set_visible(failed);
                });
            }
        }
    }

    fn press(self: &Rc<Self>, pkg: &str, action: RowAction) {
        match action {
            RowAction::Cancel => self.input(Input::Row { pkg: pkg.to_owned(), event: Event::Cancel }),
            RowAction::Install if super::ext_badges::of(pkg).is_some_and(|b| !b.install_enabled()) => {}
            RowAction::Retry | RowAction::Update | RowAction::Install => {
                self.input(Input::Row { pkg: pkg.to_owned(), event: Event::Request });
            }
            RowAction::Settings => (self.nav.open_details)(pkg),
            RowAction::Trust => self.ask_trust(pkg),
            RowAction::WebView => (self.nav.web_view)(pkg),
        }
    }

    /// The untrusted-extension dialog: Trust, Uninstall, or a dismissal that changes nothing (T-41a-012).
    fn ask_trust(self: &Rc<Self>, pkg: &str) {
        let text = trust_dialog();
        let spec = DialogSpec {
            title: text.title,
            body: Some(text.body),
            buttons: vec![(text.trust, ButtonStyle::Suggested), (text.uninstall, ButtonStyle::Destructive)],
        };
        let (weak, pkg) = (Rc::downgrade(self), pkg.to_owned());
        self.dialogs.show(&self.column, spec, move |chosen| {
            let answer = match chosen {
                Some(0) => TrustAnswer::Trust,
                Some(_) => TrustAnswer::Uninstall,
                None => TrustAnswer::Dismiss,
            };
            if let Some(inner) = weak.upgrade() {
                inner.answer_trust(&pkg, answer);
            }
        });
    }

    /// Applies a dialog answer. The entry may have vanished while the dialog was open, which makes it a no-op.
    fn answer_trust(self: &Rc<Self>, pkg: &str, answer: TrustAnswer) {
        let still_untrusted = self.shown.borrow().iter().any(|e| e.pkg == pkg && e.state == ExtState::Untrusted);
        match trust_answer(answer, still_untrusted) {
            TrustEffect::Trust => (self.nav.trust)(pkg),
            TrustEffect::Uninstall => self.input(Input::Row { pkg: pkg.to_owned(), event: Event::Uninstall }),
            TrustEffect::Nothing => {}
        }
    }

    fn update_all(self: &Rc<Self>) {
        let exts = self.exts.borrow();
        let pkgs = update_all_queue(&exts).into_iter().map(|i| exts[i].pkg.clone()).collect();
        drop(exts);
        self.input(Input::UpdateAll(pkgs));
    }

    /// Applies one input to the state machine, runs its effects, and redraws the rows whose step changed.
    fn input(self: &Rc<Self>, input: Input) {
        let effects = self.installs.borrow_mut().apply(input);
        for effect in effects {
            self.run(effect);
        }
        self.sync_rows();
    }

    fn run(self: &Rc<Self>, effect: Effect) {
        match effect {
            Effect::Start { pkg, job } => {
                let (generation, tx) = self.tasks.borrow().sender();
                let report: Reporter = {
                    let (tx, pkg) = (tx.clone(), pkg.clone());
                    Arc::new(move |step| {
                        let event = ExtEvent::Job { pkg: pkg.clone(), job, event: Event::Report(step) };
                        let _ = tx.try_send(Tagged { generation, event });
                    })
                };
                // a split row is keyed `<pkg>-<source id>`; the manager installs the real package
                let real = real_pkg(&self.shown.borrow(), &pkg);
                let work = self.backend.install(real, report);
                let handle = self.tasks.borrow_mut().spawn(async move {
                    let _end = EndOnDrop { tx, generation, pkg, job };
                    work.await;
                });
                self.handles.borrow_mut().insert(job, handle);
            }
            Effect::Abort { job, .. } => {
                if let Some(handle) = self.handles.borrow_mut().remove(&job) {
                    handle.abort();
                }
            }
            Effect::Uninstall { pkg } => {
                let work = self.backend.uninstall(pkg);
                self.tasks.borrow_mut().spawn(work);
            }
        }
    }

    /// Redraws the spinner, status and buttons of every row whose shown step is not the machine's step.
    fn sync_rows(self: &Rc<Self>) {
        let installs = self.installs.borrow();
        let rows = self.rows.borrow();
        for row in rows.iter() {
            let step = installs.step(&row.ext.pkg);
            if row.shown.get() == step {
                continue;
            }
            row.shown.set(step);
            self.fill_row(row, step);
        }
    }

    fn fill_row(self: &Rc<Self>, row: &RowWidgets, step: InstallStep) {
        let busy = !step.is_completed();
        row.spinner.set_visible(busy);
        row.spinner.set_spinning(busy);
        let text = status_text(step);
        row.status.set_visible(text.is_some());
        row.status.set_label(text.as_deref().unwrap_or(""));
        while let Some(child) = row.actions.first_child() {
            row.actions.remove(&child);
        }
        for action in row_actions(&row.ext, step) {
            let button = gtk::Button::from_icon_name(action.icon());
            let label = action.label();
            button.set_tooltip_text(Some(&label));
            button.update_property(&[gtk::accessible::Property::Label(&label)]);
            button.add_css_class("flat");
            // install is off for a listing that cannot run here
            if action == RowAction::Install && super::ext_badges::of(&row.ext.pkg).is_some_and(|b| !b.install_enabled()) {
                button.set_sensitive(false);
            }
            let (weak, pkg) = (Rc::downgrade(self), row.ext.pkg.clone());
            button.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    inner.press(&pkg, action);
                }
            });
            row.actions.append(&button);
        }
    }

    /// Rebuilds the whole list from the models, the query and the current steps.
    fn rebuild(self: &Rc<Self>) {
        while let Some(child) = self.column.first_child() {
            self.column.remove(&child);
        }
        self.rows.borrow_mut().clear();
        let exts = split_available(&self.exts.borrow(), self.enabled.borrow().as_ref());
        *self.shown.borrow_mut() = exts.clone();
        let query = self.query.borrow().clone();
        let groups = super::extensions::group(&exts, &query, (self.nav.show_nsfw)());
        if let Some(empty) = super::extensions::empty_state(&query, !groups.is_empty()) {
            self.empty_label.set_label(&tr(match empty {
                ExtEmpty::NoResults => MsgKey::no_results_found,
                ExtEmpty::Nothing => MsgKey::empty_screen,
            }));
            self.stack.set_visible_child_name("empty");
            return;
        }
        for group in groups {
            let (title, updates) = match &group.key {
                GroupKey::Updates => (tr(MsgKey::ext_updates_pending), true),
                GroupKey::Installed => (tr(MsgKey::ext_installed), false),
                GroupKey::Language(lang) => (language_header(lang), false),
            };
            self.column.append(&self.header(&title, updates));
            for index in group.rows {
                let row = self.row(&exts[index]);
                self.column.append(&row);
            }
        }
        if let Some(problems) = super::ext_problems::section(&self.problems.borrow()) {
            self.column.append(&problems);
        }
        self.stack.set_visible_child_name("list");
    }

    fn header(self: &Rc<Self>, title: &str, with_update_all: bool) -> gtk::Box {
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        header.set_margin_start(16);
        header.set_margin_end(16);
        header.set_margin_top(16);
        header.set_margin_bottom(8);
        header.append(&gtk::Label::builder().label(title).hexpand(true).xalign(0.0).css_classes(["heading"]).build());
        if with_update_all {
            let button = gtk::Button::with_label(&tr(MsgKey::ext_update_all));
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(inner) = weak.upgrade() {
                    inner.update_all();
                }
            });
            header.append(&button);
        }
        header
    }

    /// One 40 px row: the icon with a spinner over it, the name, the secondary line and the buttons.
    fn row(self: &Rc<Self>, ext: &Ext) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        row.set_margin_start(16);
        row.set_margin_end(16);
        row.set_margin_top(8);
        row.set_margin_bottom(8);

        // an untrusted extension shows the dangerous glyph in the error colour; an installed one its own icon
        let icon = icon_image(resolve_for(ext.state == ExtState::Untrusted, false, ext.icon_file.is_some(), false), ext.icon_file.as_deref());
        let spinner = gtk::Spinner::new();
        spinner.set_size_request(super::icons::ICON_HEIGHT_PX / 2, super::icons::ICON_HEIGHT_PX / 2);
        spinner.set_halign(gtk::Align::Center);
        spinner.set_valign(gtk::Align::Center);
        let stage = gtk::Overlay::new();
        stage.set_child(Some(&icon));
        stage.add_overlay(&spinner);
        row.append(&stage);

        let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
        text.set_hexpand(true);
        let name = gtk::Label::new(Some(&ext.name));
        name.set_xalign(0.0);
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        text.append(&name);
        let mut parts: Vec<String> = Vec::new();
        parts.extend(language_line(ext));
        parts.extend(version_line(ext).map(str::to_owned));
        if let Some(w) = warning(ext) {
            parts.push(warning_text(w));
        }
        let secondary = gtk::Label::new(Some(&parts.join(" \u{2022} ")));
        secondary.set_xalign(0.0);
        secondary.add_css_class("dim-label");
        secondary.set_ellipsize(gtk::pango::EllipsizeMode::End);
        secondary.set_visible(!parts.is_empty());
        text.append(&secondary);
        // a listing that cannot run here carries its badges (T-19a-066)
        if let Some(badges) = super::ext_badges::of(&ext.pkg) {
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            for label in super::ext_badges::labels(badges) {
                line.append(&gtk::Label::builder().label(label).css_classes(["caption", "error"]).build());
            }
            text.append(&line);
        }
        let status = gtk::Label::new(None);
        status.set_xalign(0.0);
        status.add_css_class("dim-label");
        text.append(&status);
        row.append(&text);

        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        row.append(&actions);

        let step = self.installs.borrow().step(&ext.pkg);
        let widgets = RowWidgets { shown: Cell::new(step), ext: ext.clone(), spinner, status, actions };
        self.fill_row(&widgets, step);
        self.rows.borrow_mut().push(widgets);

        // a click does what the row's main button does: install, open the details, or ask for trust
        let primary = match ext.state {
            ExtState::Available => RowAction::Install,
            ExtState::Installed { .. } => RowAction::Settings,
            ExtState::Untrusted => RowAction::Trust,
        };
        let (weak, pkg) = (Rc::downgrade(self), ext.pkg.clone());
        let click = gtk::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            if let Some(inner) = weak.upgrade() {
                if inner.installs.borrow().step(&pkg).is_completed() {
                    inner.press(&pkg, primary);
                }
            }
        });
        row.add_controller(click);
        row
    }
}

/// The details page leaves when its extension is no longer installed (B§7.5). `installed` carries the package names
/// of the registry; `pop` runs once, on the main loop, and the follower ends.
pub fn pop_details_on_uninstall(mut installed: watch::Receiver<Vec<String>>, pkg: String, pop: impl Fn() + 'static) {
    glib::spawn_future_local(async move {
        loop {
            let gone = {
                let names = installed.borrow_and_update();
                let refs: Vec<&str> = names.iter().map(String::as_str).collect();
                super::extensions::details_should_pop(&refs, &pkg)
            };
            if gone {
                pop();
                return;
            }
            if installed.changed().await.is_err() {
                return;
            }
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::browse::extensions::ExtSource;

    /// Records the calls; installs report every step and end at once.
    #[derive(Default)]
    struct Fake {
        installs: Mutex<Vec<String>>,
        uninstalls: Mutex<Vec<String>>,
        refreshes: AtomicUsize,
        /// Makes the next refreshes fail.
        fail_refresh: std::sync::atomic::AtomicBool,
    }

    impl ExtensionBackend for Arc<Fake> {
        fn install(&self, pkg: String, report: Reporter) -> JobFuture {
            let fake = Arc::clone(self);
            Box::pin(async move {
                fake.installs.lock().expect("lock").push(pkg);
                for step in [InstallStep::Downloading, InstallStep::Installing, InstallStep::Installed] {
                    report(step);
                }
            })
        }
        fn uninstall(&self, pkg: String) -> JobFuture {
            let fake = Arc::clone(self);
            Box::pin(async move { fake.uninstalls.lock().expect("lock").push(pkg) })
        }
        fn refresh(&self) -> RefreshFuture {
            let fake = Arc::clone(self);
            Box::pin(async move {
                fake.refreshes.fetch_add(1, Ordering::SeqCst);
                if fake.fail_refresh.load(Ordering::SeqCst) { Err("offline".to_owned()) } else { Ok(()) }
            })
        }
    }

    fn ext(name: &str, state: ExtState, lang: &str) -> Ext {
        Ext {
            name: name.into(),
            pkg: format!("eu.example.{}", name.to_lowercase()),
            version: "1.4.1".into(),
            lang: Some(lang.into()),
            nsfw: false,
            sources: vec![ExtSource { id: 1, name: name.into(), lang: lang.into(), base_url: None }],
            state,
            icon_file: None,
        }
    }

    fn nav() -> ExtNav {
        ExtNav { open_details: Rc::new(|_| {}), trust: Rc::new(|_| {}), web_view: Rc::new(|_| {}), open_repos: Rc::new(|| {}), show_nsfw: Rc::new(|| true) }
    }

    fn pump(context: &glib::MainContext, mut done: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(3) {
            while context.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        done()
    }

    fn texts(column: &gtk::Box) -> Vec<String> {
        let mut out = Vec::new();
        let mut child = column.first_child();
        while let Some(widget) = child {
            let mut inner = widget.first_child();
            let mut label = None;
            while let Some(w) = inner {
                if let Some(l) = w.downcast_ref::<gtk::Label>() {
                    label = Some(l.label().to_string());
                    break;
                }
                inner = w.next_sibling();
            }
            out.push(label.unwrap_or_default());
            child = widget.next_sibling();
        }
        out
    }

    fn the_list_has_headers_in_order_with_a_row_under_each() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let page = ExtensionsPage::new(Arc::new(Arc::new(Fake::default())), nav());
        page.set_exts(vec![
            ext("Alpha", ExtState::Installed { has_update: true, obsolete: false }, "en"),
            ext("Beta", ExtState::Installed { has_update: false, obsolete: false }, "en"),
            ext("Gamma", ExtState::Available, "ja"),
            ext("Delta", ExtState::Available, "en"),
        ]);
        let headers: Vec<String> = texts(&page.inner.column).into_iter().filter(|t| !t.is_empty()).collect();
        assert_eq!(headers, [tr(MsgKey::ext_updates_pending), tr(MsgKey::ext_installed), "English".into(), crate::browse::names::localized_display_name("ja")]);
        assert_eq!(page.inner.rows.borrow().len(), 4, "an extension with an update has one row, under Updates");
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("list"));
        page.set_query("zzz");
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("empty"));
        assert_eq!(page.inner.empty_label.label().as_str(), tr(MsgKey::no_results_found));
        page.set_exts(Vec::new());
        page.set_query("");
        assert_eq!(page.inner.empty_label.label().as_str(), tr(MsgKey::empty_screen));
    }

    fn pressing_install_runs_the_job_and_the_row_walks_to_installed_then_clears() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let context = glib::MainContext::default();
        let fake = Arc::new(Fake::default());
        let page = ExtensionsPage::new(Arc::new(Arc::clone(&fake)), nav());
        page.set_exts(vec![ext("Delta", ExtState::Available, "en")]);
        let pkg = "eu.example.delta";
        page.press(pkg, RowAction::Install);
        assert_eq!(page.step(pkg), InstallStep::Pending, "the row is busy at once");
        assert!(pump(&context, || page.step(pkg) == InstallStep::Idle), "the job ends and its entry is cleared");
        assert_eq!(*fake.installs.lock().expect("lock"), [pkg]);
    }

    fn cancel_aborts_and_update_all_runs_one_after_another() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let context = glib::MainContext::default();
        let fake = Arc::new(Fake::default());
        let page = ExtensionsPage::new(Arc::new(Arc::clone(&fake)), nav());
        page.set_exts(vec![
            ext("One", ExtState::Installed { has_update: true, obsolete: false }, "en"),
            ext("Two", ExtState::Installed { has_update: true, obsolete: false }, "en"),
        ]);
        page.update_all();
        assert_eq!(page.step("eu.example.one"), InstallStep::Pending);
        assert_eq!(page.step("eu.example.two"), InstallStep::Idle, "the second waits for the first");
        assert!(pump(&context, || fake.installs.lock().expect("lock").len() == 2));
        assert_eq!(*fake.installs.lock().expect("lock"), ["eu.example.one", "eu.example.two"], "in list order");
        assert!(pump(&context, || !page.inner.installs.borrow().updating_all()));
        assert!(page.inner.installs.borrow().is_empty(), "the map is clear when update-all completes");
    }

    fn refresh_keeps_its_spinner_for_a_second_and_always_stops() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let context = glib::MainContext::default();
        let fake = Arc::new(Fake::default());
        let page = ExtensionsPage::new(Arc::new(Arc::clone(&fake)), nav());
        let started = Instant::now();
        page.refresh();
        assert!(page.is_refreshing());
        page.refresh();
        assert!(pump(&context, || !page.is_refreshing()), "the spinner stops");
        assert!(started.elapsed() >= Duration::from_millis(990), "but not before one second");
        assert_eq!(fake.refreshes.load(Ordering::SeqCst), 1, "a second press while refreshing does nothing");
    }

    fn destroying_the_page_aborts_its_jobs() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        /// Counts live jobs: incremented when built, decremented when its future is dropped.
        struct Live(Arc<AtomicUsize>);
        impl Drop for Live {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        struct Hang(Arc<AtomicUsize>);
        impl ExtensionBackend for Hang {
            fn install(&self, _pkg: String, _report: Reporter) -> JobFuture {
                self.0.fetch_add(1, Ordering::SeqCst);
                let guard = Live(Arc::clone(&self.0));
                Box::pin(async move {
                    let _guard = guard;
                    tokio::time::sleep(Duration::from_secs(3600)).await;
                })
            }
            fn uninstall(&self, _pkg: String) -> JobFuture {
                Box::pin(async {})
            }
            fn refresh(&self) -> RefreshFuture {
                Box::pin(async { Ok(()) })
            }
        }
        let live = Arc::new(AtomicUsize::new(0));
        let page = ExtensionsPage::new(Arc::new(Hang(Arc::clone(&live))), nav());
        page.set_exts(vec![ext("Delta", ExtState::Available, "en")]);
        page.press("eu.example.delta", RowAction::Install);
        assert_eq!(live.load(Ordering::SeqCst), 1);
        let tasks_alive = Rc::clone(&page.inner.tasks);
        drop(page);
        assert!(tasks_alive.borrow().live_tasks() <= 1);
        let start = Instant::now();
        while live.load(Ordering::SeqCst) != 0 && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(live.load(Ordering::SeqCst), 0, "popping the page aborts its loaders");
    }

    fn texts_of_buttons(window: &gtk::Window) -> Vec<String> {
        crate::browse::dialogs::buttons_of(window).iter().map(|b| b.label().map(|l| l.to_string()).unwrap_or_default()).collect()
    }

    fn pressing_trust_asks_first_and_each_answer_does_what_it_says() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let context = glib::MainContext::default();
        let fake = Arc::new(Fake::default());
        let trusted: Rc<RefCell<Vec<String>>> = Rc::default();
        let log = Rc::clone(&trusted);
        let nav = ExtNav { trust: Rc::new(move |pkg| log.borrow_mut().push(pkg.to_owned())), ..nav() };
        let page = ExtensionsPage::new(Arc::new(Arc::clone(&fake)), nav);
        let mut secret = ext("Secret", ExtState::Untrusted, "en");
        secret.lang = None;
        page.set_exts(vec![secret]);
        let pkg = "eu.example.secret";
        page.press(pkg, RowAction::Trust);
        assert!(trusted.borrow().is_empty(), "nothing happens before the dialog is answered");
        let window = page.inner.dialogs.current().expect("the trust dialog is open");
        let expected = trust_dialog();
        assert_eq!(window.title().map(|t| t.to_string()), Some(expected.title));
        assert_eq!(texts_of_buttons(&window), [expected.trust, expected.uninstall]);
        window.close();
        assert!(trusted.borrow().is_empty() && fake.uninstalls.lock().expect("lock").is_empty(), "dismissing changes nothing");

        page.press(pkg, RowAction::Trust);
        crate::browse::dialogs::buttons_of(&page.inner.dialogs.current().expect("dialog"))[0].emit_clicked();
        assert_eq!(*trusted.borrow(), [pkg]);

        page.press(pkg, RowAction::Trust);
        crate::browse::dialogs::buttons_of(&page.inner.dialogs.current().expect("dialog"))[1].emit_clicked();
        assert!(pump(&context, || *fake.uninstalls.lock().expect("lock") == [pkg]), "Uninstall removes the package");

        // the entry vanished while the dialog was open: Trust is a no-op
        page.press(pkg, RowAction::Trust);
        let window = page.inner.dialogs.current().expect("dialog");
        page.set_exts(Vec::new());
        crate::browse::dialogs::buttons_of(&window)[0].emit_clicked();
        assert_eq!(trusted.borrow().len(), 1, "no second trust");
    }

    fn f5_and_the_first_open_start_one_fetch_each_and_a_failure_shows_the_banner_with_retry() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let context = glib::MainContext::default();
        let fake = Arc::new(Fake::default());
        fake.fail_refresh.store(true, Ordering::SeqCst);
        let page = ExtensionsPage::new(Arc::new(Arc::clone(&fake)), nav());
        page.set_exts(vec![ext("Delta", ExtState::Available, "en")]);
        page.opened();
        page.opened();
        assert!(page.is_refreshing());
        assert!(pump(&context, || !page.is_refreshing()));
        assert_eq!(fake.refreshes.load(Ordering::SeqCst), 1, "the first open fetches once");
        assert!(page.banner_visible(), "a failed fetch shows the banner");
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("list"), "the previous snapshot stays on screen");

        fake.fail_refresh.store(false, Ordering::SeqCst);
        let keys = page.root.observe_controllers();
        let key = (0..keys.n_items()).filter_map(|i| keys.item(i)).find_map(|c| c.downcast::<gtk::EventControllerKey>().ok()).expect("key controller");
        let stopped = key.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::F5, &0_u32, &gtk::gdk::ModifierType::empty()]);
        assert!(stopped);
        assert!(!page.banner_visible(), "a new fetch hides the banner");
        assert!(pump(&context, || !page.is_refreshing() && fake.refreshes.load(Ordering::SeqCst) == 2));
        assert!(!page.banner_visible(), "and a good fetch leaves it hidden");
    }

    fn an_available_row_installs_the_real_package_of_its_synthetic_key() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let context = glib::MainContext::default();
        let fake = Arc::new(Fake::default());
        let page = ExtensionsPage::new(Arc::new(Arc::clone(&fake)), nav());
        page.set_exts(vec![ext("Delta", ExtState::Available, "en")]);
        page.set_enabled_languages(Some(["en".to_owned()].into()));
        assert_eq!(page.inner.rows.borrow()[0].ext.pkg, "eu.example.delta-1", "the row key is <pkg>-<source id>");
        page.press("eu.example.delta-1", RowAction::Install);
        assert!(pump(&context, || !fake.installs.lock().expect("lock").is_empty()));
        assert_eq!(*fake.installs.lock().expect("lock"), ["eu.example.delta"]);
        page.set_enabled_languages(Some(["ja".to_owned()].into()));
        assert_eq!(page.inner.stack.visible_child_name().as_deref(), Some("empty"), "no enabled language leaves nothing");
    }

    pub(crate) fn smoke() {
        pressing_trust_asks_first_and_each_answer_does_what_it_says();
        f5_and_the_first_open_start_one_fetch_each_and_a_failure_shows_the_banner_with_retry();
        an_available_row_installs_the_real_package_of_its_synthetic_key();
        the_list_has_headers_in_order_with_a_row_under_each();
        pressing_install_runs_the_job_and_the_row_walks_to_installed_then_clears();
        cancel_aborts_and_update_all_runs_one_after_another();
        refresh_keeps_its_spinner_for_a_second_and_always_stops();
        destroying_the_page_aborts_its_jobs();
    }
}
