//! The rows of Advanced settings this module binds (tasks T-47a-076, T-47a-077, T-47a-078 and T-47a-080): the
//! networking group (clear cookies, clear WebView data, DNS over HTTPS, the user agent and its reset) and the reader
//! reset under Library. The rest of the page stays as the spec builds it.
use std::rc::Rc;

use gtk::prelude::*;
use kuro_data::manga::MangaRepo;
use kuro_domain::kind::EntryKind;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::pref_model::page;
use crate::screens::{built, Built};
use crate::settings_ui::merge::merge;
use crate::pref_model::bind;
use crate::settings_ui::rows::{edit_text, group, info, int_choice, item, switch_with, text, REQUIRES_RESTART};
use crate::settings_ui::route::{prefs as app_prefs, spec_tree, toolbar};

/// DNS-over-HTTPS providers by `doh_provider` code, in list order (`DohProviders.kt`). `-1` is off.
pub const DOH_PROVIDERS: [(i32, &str); 14] = [
    (-1, "Disabled"),
    (1, "Cloudflare"),
    (2, "Google"),
    (3, "AdGuard"),
    (4, "Quad9"),
    (5, "AliDNS"),
    (6, "DNSPod"),
    (7, "360"),
    (8, "Quad 101"),
    (9, "Mullvad"),
    (10, "Control D"),
    (11, "Njalla"),
    (12, "Shecan"),
    (13, "LibreDNS"),
];

/// The message for a user agent that cannot be sent as a header value.
pub const INVALID_USER_AGENT: &str = "Invalid user agent string";

/// A user agent is a header value: a control character (CR and LF among them) makes it invalid. A tab is allowed.
pub fn validate_user_agent(text: &str) -> Result<(), String> {
    if text.chars().any(|c| c.is_control() && c != '\t') {
        Err(INVALID_USER_AGENT.to_owned())
    } else {
        Ok(())
    }
}

/// The "Reset default user agent string" row is enabled only while the stored value differs from the default.
pub fn user_agent_is_custom(prefs: &Prefs) -> bool {
    let pref = &prefs.network.default_user_agent;
    pref.get() != pref.default_value()
}

/// The user agent edit row: single line, never blank, validated as a header value.
pub fn user_agent_row(prefs: &Prefs) -> PreferenceItem {
    edit_text("Default user agent string", &prefs.network.default_user_agent, false, validate_user_agent, true)
}

/// Deletes the stored user agent, which brings back the default, and runs `changed` to redraw the page.
pub fn reset_user_agent_row(prefs: &Prefs, changed: Rc<dyn Fn()>) -> PreferenceItem {
    let mut row = text("Reset default user agent string", None, user_agent_is_custom(prefs));
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        let pref = prefs.network.default_user_agent.clone();
        *on_click = Some(Rc::new(move || {
            if let Err(err) = pref.delete() {
                tracing::warn!(%err, "the user agent could not be reset");
            }
            changed();
        }));
    }
    row
}

/// The DNS-over-HTTPS list. The HTTP client reads the provider once when it is built, so a change takes effect at the
/// next launch: the change toasts "Requires app restart to take effect" (T-12a-031) and runs `changed`, which redraws
/// the page so the local-name hint under the row follows the new provider.
pub fn doh_row(prefs: &Prefs, changed: Rc<dyn Fn()>) -> PreferenceItem {
    let mut row = int_choice("DNS over HTTPS (DoH)", &prefs.network.doh_provider, &DOH_PROVIDERS, true);
    if let PreferenceItem::BasicList { on_change, .. } = &mut row {
        let inner = Rc::clone(on_change);
        *on_change = bind::veto(move |key: String| {
            let applied = inner(key);
            let changed = Rc::clone(&changed);
            async move {
                let result = applied.await;
                if matches!(result, Ok(true)) {
                    crate::toast::notify(REQUIRES_RESTART);
                    changed();
                }
                result
            }
        });
    }
    row
}

/// The helper line under the DoH row while a provider is selected (T-12a-044).
pub fn doh_hint_row(prefs: &Prefs) -> Option<PreferenceItem> {
    kuro_net::settings::local_name_hint_visible(prefs.network.doh_provider.get())
        .then(|| info(kuro_net::settings::LOCAL_NAME_HINT))
}

/// The jar file the HTTP client persists its cookies in, under the platform data directory.
pub const COOKIE_FILE: &str = "cookies.json";

/// Empties the cookie jar stored at `file` (memory and file) and returns the toast text on success.
pub fn clear_cookies_at(file: std::path::PathBuf) -> std::io::Result<&'static str> {
    kuro_net::cookies::CookieJar::load(file).remove_all()?;
    Ok("Cookies cleared")
}

fn clear_cookies() {
    let file = kuro_platform::linux::LinuxPaths::system().map(|p| kuro_platform::paths::Paths::data_dir(&p).join(COOKIE_FILE));
    match file.map_err(std::io::Error::other).and_then(clear_cookies_at) {
        Ok(text) => crate::toast::notify(text),
        Err(err) => tracing::warn!(%err, "the cookies could not be cleared"),
    }
}

/// The "Clear cookies" row: empties the HTTP cookie jar and says "Cookies cleared".
pub fn clear_cookies_row() -> PreferenceItem {
    let mut row = text("Clear cookies", None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(clear_cookies));
    }
    row
}

/// The WebView data clear (the cache, cookies and local storage of the WebKit view).
pub fn clear_webview_row() -> PreferenceItem {
    let mut row = text("Clear WebView data", None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(crate::settings::clear_webview_data));
    }
    row
}

/// Clears every manga's reader viewer flags (reading mode and orientation) and says so in a toast.
pub fn reset_reader_settings() {
    let repo = MangaRepo::new(crate::data::app().db(EntryKind::Manga));
    if kuro_domain::entries::viewer_flags::reset_viewer_flags_manga(&repo) {
        crate::toast::notify("All reader settings reset");
    } else {
        crate::toast::notify("Couldn't reset reader settings");
    }
}

/// The "Reset per-series reader settings" row. The anime equivalent is not offered (Android parity).
pub fn reset_reader_row() -> PreferenceItem {
    let mut row = text("Reset per-series reader settings", Some("Resets reading mode and orientation of all series"), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(reset_reader_settings));
    }
    row
}

/// The typed rows of Advanced settings, in their groups. `changed` redraws the page after a reset.
pub fn page_rows(prefs: &Prefs, changed: Rc<dyn Fn()>) -> Vec<Preference> {
    let revoke = revoke_trust_row(prefs, Rc::clone(&changed));
    vec![
        item(share_crash_logs_row()),
        item(verbose_row(prefs)),
        item(onboarding_guide_row(prefs)),
        group("Data", vec![reindex_downloads_row()]),
        group(
            "Networking",
            [clear_cookies_row(), super::cookie_import::cookie_import_row(), clear_webview_row(), doh_row(prefs, Rc::clone(&changed))]
                .into_iter()
                .chain(doh_hint_row(prefs))
                .chain([user_agent_row(prefs), reset_user_agent_row(prefs, changed)])
                .collect(),
        ),
        group("Library", vec![refresh_covers_row(), reset_reader_row()]),
        group("Extensions", vec![revoke]),
    ]
}

/// Onboarding guide (task T-47a-071): pushes the onboarding flow. The flow writes its completion flag only when its last
/// step finishes, and a user who reaches this row has normally finished it, so the flag keeps its value.
pub fn onboarding_guide_row(prefs: &Prefs) -> PreferenceItem {
    let store = prefs.store().clone();
    let mut row = text("Onboarding guide", None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(move || open_onboarding(&store)));
    }
    row
}

fn open_onboarding(store: &kuro_prefs::store::PrefStore) {
    let flow = crate::shell::onboarding::page(
        store,
        crate::ui::pop,
        || crate::ui::open(crate::pref_model::onboarding::DATA_AND_STORAGE),
    );
    crate::ui::push(&flow);
}

/// The Verbose logging subtitle (task T-47a-070).
pub const VERBOSE_SUBTITLE: &str = "Print verbose logs to system log (reduces app performance)";

thread_local! {
    /// The live logging handle `main` owns. Only a weak link is kept here, so the log still flushes when `main` ends.
    static LOGGING: std::cell::RefCell<std::rc::Weak<kuro_core::logging::Logging>> = const { std::cell::RefCell::new(std::rc::Weak::new()) };
}

/// Called by `main` once logging is up: the handle stays with the caller, and Verbose logging reaches it from here.
pub fn share_logging(log: kuro_core::logging::Logging) -> Rc<kuro_core::logging::Logging> {
    let log = Rc::new(log);
    LOGGING.with(|slot| *slot.borrow_mut() = Rc::downgrade(&log));
    log
}

/// Changes the tracing filter now. Without a logging handle (file logging is off) there is nothing to change.
fn apply_verbose(on: bool) -> Result<(), String> {
    match LOGGING.with(|slot| slot.borrow().upgrade()) {
        Some(log) => log.set_verbose(on).map_err(|err| err.to_string()),
        None => Ok(()),
    }
}

/// The Verbose logging switch: the filter changes as the switch is flipped, the preference follows.
pub fn verbose_row(prefs: &Prefs) -> PreferenceItem {
    let veto = bind::veto(|on: bool| {
        let applied = apply_verbose(on);
        async move { applied.map(|()| true) }
    });
    switch_with("Verbose logging", Some(VERBOSE_SUBTITLE), &prefs.network.verbose_logging, true, veto)
}
/// How long the Share crash logs row shows its success and failure status before the subtitle comes back.
const COPIED_STATUS: std::time::Duration = std::time::Duration::from_secs(4);
const FAILED_STATUS: std::time::Duration = std::time::Duration::from_secs(2);
const SHARE_SUBTITLE: &str = "Saves error logs to a file for sharing with the developers";
const COPIED_TEXT: &str = "Crash logs copied to the clipboard";
const FAILED_TEXT: &str = "Failed to get logs";

/// The debug block of the crash dump. Components this build has not loaded say `not loaded`, as the crash report does.
fn dump_debug_info() -> kuro_core::crash_dump::DebugInfo {
    let info = &*kuro_core::app_info::APP_INFO;
    let facts: std::collections::BTreeMap<_, _> = kuro_platform::system::facts().into_iter().collect();
    let fact = |key: &str| facts.get(key).cloned().unwrap_or_else(|| "unknown".to_owned());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    kuro_core::crash_dump::DebugInfo {
        app_id: info.app_id.to_owned(),
        app_version: format!("{} ({}, {}, {})", info.version_name, info.git_sha, info.version_code, info.build_time),
        os: fact("os"),
        kernel: fact("kernel"),
        desktop: fact("desktop"),
        gtk: format!("{}.{}.{}", gtk::major_version(), gtk::minor_version(), gtk::micro_version()),
        webview: "not loaded".to_owned(),
        current_time: kuro_core::utc::rfc3339(now),
        mpv: "not loaded".to_owned(),
        ffmpeg: "not loaded".to_owned(),
        sqlite: rusqlite::version().to_owned(),
        locale: fact("locale"),
    }
}

/// Composes the dump from the debug block, the extension lists and the log file in `log_dir`, writes it under
/// `<cache_dir>/crash/` and returns the file and its text.
fn write_crash_dump(
    cache_dir: &std::path::Path,
    log_dir: &std::path::Path,
    debug: &kuro_core::crash_dump::DebugInfo,
    manga: &[kuro_core::crash_dump::ExtensionStatus],
    anime: &[kuro_core::crash_dump::ExtensionStatus],
) -> std::io::Result<(std::path::PathBuf, String)> {
    use kuro_core::crash_dump::{compose, write_dump, DumpInputs};
    let log = std::fs::read(log_dir.join(kuro_core::logging::LOG_FILE_NAME)).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    let text = compose(&DumpInputs { debug, manga_extensions: manga, anime_extensions: anime, exception: None, log: log.as_deref() });
    write_dump(cache_dir, &text).map(|path| (path, text))
}

/// What a tap on Share crash logs does once the dump is built: copy the text and reveal the file.
fn reveal_dump(path: &std::path::Path, text: &str) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(text);
    }
    gtk::FileLauncher::new(Some(&gtk::gio::File::for_path(path))).open_containing_folder(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
        if let Err(err) = result {
            tracing::warn!(%err, "the crash log folder did not open");
        }
    });
}

/// Builds the dump off the GTK thread. The row is insensitive until it is done, then it shows `Crash logs copied to the
/// clipboard` for 4 s or `Failed to get logs` for 2 s in place of its subtitle.
fn share_crash_logs(row: &gtk::Widget, subtitle: &gtk::Label) {
    row.set_sensitive(false);
    let debug = dump_debug_info();
    let (manga, anime) = crate::ext_host::crash_log_extensions();
    let dirs = kuro_platform::linux::LinuxPaths::system().map(|p| (kuro_platform::paths::Paths::cache_dir(&p), kuro_platform::paths::Paths::log_dir(&p)));
    let (row, subtitle) = (row.clone(), subtitle.clone());
    gtk::glib::spawn_future_local(async move {
        let done = match dirs {
            Ok((cache, logs)) => gtk::gio::spawn_blocking(move || write_crash_dump(&cache, &logs, &debug, &manga, &anime)).await.map_err(|_| "the dump task panicked".to_owned()).and_then(|r| r.map_err(|e| e.to_string())),
            Err(err) => Err(err.to_string()),
        };
        let (text, shown) = match done {
            Ok((path, text)) => {
                reveal_dump(&path, &text);
                (COPIED_TEXT, COPIED_STATUS)
            }
            Err(err) => {
                tracing::warn!(%err, "the crash logs could not be written");
                (FAILED_TEXT, FAILED_STATUS)
            }
        };
        crate::pref_model::widgets::row::set_subtitle(&subtitle, Some(text));
        row.set_sensitive(true);
        gtk::glib::timeout_add_local_once(shown, move || crate::pref_model::widgets::row::set_subtitle(&subtitle, Some(SHARE_SUBTITLE)));
    });
}

/// The Share crash logs row: a tap builds `<cache>/crash/kurobayomi_crash_logs.txt`, copies the text and reveals the file.
pub fn share_crash_logs_row() -> PreferenceItem {
    use crate::pref_model::widgets::row::{self, RowSpec};
    PreferenceItem::Custom {
        title: "Share crash logs".to_owned(),
        build: Rc::new(|| {
            let built = row::build(RowSpec { title: "Share crash logs", subtitle: Some(SHARE_SUBTITLE), icon: None, trailing: None, on_click: None });
            let content = built.row.child().and_downcast::<gtk::Box>().expect("the row content");
            built.row.set_child(None::<&gtk::Widget>);
            content.set_focusable(true);
            content.set_cursor_from_name(Some("pointer"));
            let (target, subtitle) = (content.clone().upcast::<gtk::Widget>(), built.subtitle.clone());
            let tap = {
                let (target, subtitle) = (target.clone(), subtitle.clone());
                Rc::new(move || if target.is_sensitive() { share_crash_logs(&target, &subtitle) })
            };
            let click = gtk::GestureClick::new();
            let on_click = Rc::clone(&tap);
            click.connect_released(move |_, _, _, _| on_click());
            content.add_controller(click);
            let keys = gtk::EventControllerKey::new();
            keys.connect_key_pressed(move |_, key, _, _| {
                if crate::pref_model::widgets::dialog::is_enter(key) || key == gtk::gdk::Key::space {
                    tap();
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            });
            content.add_controller(keys);
            content.upcast()
        }),
    }
}

/// Invalidates the shell's download index (the one the banners and filters read): its next renewal rescans the download
/// folders, and the snapshot file on disk is gone. The one index serves manga and anime alike.
fn reindex_downloads() {
    match crate::shell::host::Shell::get() {
        Some(shell) => {
            shell.downloads().invalidate();
            crate::toast::notify("Downloads index invalidated");
        }
        None => crate::toast::notify("Failed to reindex downloads"),
    }
}

/// The Reindex downloads row (task T-47a-074).
pub fn reindex_downloads_row() -> PreferenceItem {
    let mut row = text("Reindex downloads", Some("Force app to recheck downloaded chapters and episodes"), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(reindex_downloads));
    }
    row
}

/// Starts the manga and anime library update and the manga and anime metadata update (task T-47a-079). A job already
/// running is left alone, and nothing is shown.
pub fn refresh_covers() {
    for kind in [EntryKind::Manga, EntryKind::Anime] {
        crate::updates::library_trigger::start(kind);
    }
    crate::misc::runner::refresh_library_covers();
}

/// The Refresh library covers row (task T-47a-079).
pub fn refresh_covers_row() -> PreferenceItem {
    let mut row = text("Refresh library covers", None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(refresh_covers));
    }
    row
}

/// Revoke trusted unknown extensions (task T-47a-084): the trusted set of manga and anime extensions is cleared and the
/// page is redrawn. The extension lists read the set when they are shown, so nothing waits for a restart.
pub fn revoke_trust_row(prefs: &Prefs, changed: Rc<dyn Fn()>) -> PreferenceItem {
    let trusted = prefs.sources.trusted_extensions.clone();
    let mut row = text("Revoke trusted unknown extensions", None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(move || match trusted.set(std::collections::BTreeSet::new()) {
            Ok(()) => changed(),
            Err(err) => tracing::warn!(%err, "the trusted extensions were not cleared"),
        }));
    }
    row
}

/// Advanced settings: the spec page with the rows of [`page_rows`] in place of their spec twins.
pub fn page() -> Option<Built> {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_vexpand(true);
    fill(&holder);
    built(holder, toolbar("settings_advanced"))
}

fn fill(holder: &gtk::Box) {
    while let Some(child) = holder.first_child() {
        holder.remove(&child);
    }
    let prefs = app_prefs();
    let weak = holder.downgrade();
    let changed: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(holder) = weak.upgrade() {
            fill(&holder);
        }
    });
    let tree = merge(spec_tree("settings_advanced", prefs.store()), page_rows(&prefs, changed));
    holder.append(&page::body(&tree).widget);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_guide_is_a_top_level_row_right_after_verbose_logging() {
        let rows = page_rows(&Prefs::in_memory(), Rc::new(|| {}));
        let titles: Vec<&str> = rows
            .iter()
            .filter_map(|entry| match entry {
                Preference::Item(item) => Some(item.title()),
                Preference::Group(_) => None,
            })
            .collect();
        let at = titles.iter().position(|t| *t == "Onboarding guide").expect("the row is there");
        assert_eq!(titles[at - 1], "Verbose logging");
    }

    #[test]
    fn the_doh_list_has_disabled_and_the_thirteen_providers_in_order() {
        assert_eq!(DOH_PROVIDERS.len(), 14);
        assert_eq!(DOH_PROVIDERS[0], (-1, "Disabled"));
        assert_eq!(DOH_PROVIDERS[13], (13, "LibreDNS"));
        let ids: Vec<i32> = DOH_PROVIDERS.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, [-1, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]);
    }

    #[test]
    fn clear_cookies_empties_the_stored_jar_and_reports_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(COOKIE_FILE);
        let jar = kuro_net::cookies::CookieJar::load(file.clone());
        let url = reqwest::Url::parse("https://example.org/").unwrap();
        let (n, v) = ("k", "v");
        reqwest::cookie::CookieStore::set_cookies(
            &*jar.provider(),
            &mut std::iter::once(&reqwest::header::HeaderValue::from_str(&format!("{n}={v}; Max-Age=3600")).unwrap()),
            &url,
        );
        jar.flush().unwrap();
        assert_eq!(jar.get(&url).len(), 1);
        assert_eq!(clear_cookies_at(file.clone()).unwrap(), "Cookies cleared");
        assert!(kuro_net::cookies::CookieJar::load(file).get(&url).is_empty());
    }

    #[test]
    fn a_stored_provider_is_the_list_value_and_an_unknown_one_shows_disabled() {
        let prefs = Prefs::in_memory();
        assert_eq!(value_of(&doh_row(&prefs, Rc::new(|| {}))), "-1");
        prefs.network.doh_provider.set(4).unwrap();
        assert_eq!(value_of(&doh_row(&prefs, Rc::new(|| {}))), "4");
    }

    #[test]
    fn a_doh_change_writes_the_provider_and_redraws_the_page_for_the_restart_toast() {
        let prefs = Prefs::in_memory();
        let redraws = Rc::new(std::cell::Cell::new(0));
        let counter = Rc::clone(&redraws);
        let PreferenceItem::BasicList { on_change, .. } = doh_row(&prefs, Rc::new(move || counter.set(counter.get() + 1))) else {
            panic!("a list")
        };
        let mut future = on_change("4".to_owned());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert_eq!(future.as_mut().poll(&mut context), std::task::Poll::Ready(Ok(true)));
        assert_eq!((prefs.network.doh_provider.get(), redraws.get()), (4, 1));
        assert_eq!(REQUIRES_RESTART, "Requires app restart to take effect");
    }

    #[test]
    fn the_local_name_hint_shows_under_the_doh_row_only_while_doh_is_on() {
        let prefs = Prefs::in_memory();
        let networking = |prefs: &Prefs| {
            page_rows(prefs, Rc::new(|| {})).into_iter().find_map(|entry| match entry {
                Preference::Group(g) if g.title == "Networking" => Some(g.items),
                _ => None,
            })
        };
        let hints = |items: &[PreferenceItem]| items.iter().filter(|i| matches!(i, PreferenceItem::Info { .. })).count();
        assert!(doh_hint_row(&prefs).is_none());
        assert_eq!(hints(&networking(&prefs).unwrap()), 0);
        prefs.network.doh_provider.set(1).unwrap();
        let items = networking(&prefs).unwrap();
        let at = items.iter().position(|i| matches!(i, PreferenceItem::Info { .. })).unwrap();
        assert_eq!(items[at - 1].title(), "DNS over HTTPS (DoH)");
        let PreferenceItem::Info { text, .. } = &items[at] else { panic!("info") };
        assert_eq!(text, "Local names such as localhost are not resolved while DoH is on.");
    }

    fn value_of(row: &PreferenceItem) -> String {
        let PreferenceItem::BasicList { value, .. } = row else { panic!("list") };
        value.clone()
    }

    #[test]
    fn user_agent_rejects_line_breaks_and_control_characters_but_not_a_tab_or_plain_text() {
        assert_eq!(validate_user_agent("Mozilla/5.0 (X11; Linux x86_64)"), Ok(()));
        assert_eq!(validate_user_agent("with\ttab"), Ok(()));
        for bad in ["a\rb", "a\nb", "a\u{7}b", "a\u{85}b"] {
            assert_eq!(validate_user_agent(bad), Err(INVALID_USER_AGENT.to_owned()), "{bad:?}");
        }
    }

    #[test]
    fn the_reset_row_is_enabled_only_when_the_user_agent_differs_from_the_default() {
        let prefs = Prefs::in_memory();
        assert!(!user_agent_is_custom(&prefs));
        let PreferenceItem::Text { common, .. } = reset_user_agent_row(&prefs, Rc::new(|| {})) else { panic!("text row") };
        assert!(!common.enabled);
        prefs.network.default_user_agent.set("kuro/1.0".into()).unwrap();
        assert!(user_agent_is_custom(&prefs));
        let PreferenceItem::Text { common, on_click, .. } = reset_user_agent_row(&prefs, Rc::new(|| {})) else { panic!("text row") };
        assert!(common.enabled);
        on_click.expect("tapping resets")();
        assert_eq!(prefs.network.default_user_agent.get(), prefs.network.default_user_agent.default_value());
    }

    #[test]
    fn revoke_clears_the_trusted_set_and_redraws_the_page() {
        use std::cell::Cell;
        let prefs = Prefs::in_memory();
        prefs.sources.trusted_extensions.set(std::collections::BTreeSet::from(["pkg.a".to_owned()])).unwrap();
        let redrawn = Rc::new(Cell::new(0));
        let counter = Rc::clone(&redrawn);
        let row = revoke_trust_row(&prefs, Rc::new(move || counter.set(counter.get() + 1)));
        let PreferenceItem::Text { on_click, common } = &row else { panic!("an action row") };
        assert_eq!(common.title, "Revoke trusted unknown extensions");
        (on_click.as_ref().expect("a tap action"))();
        assert!(prefs.sources.trusted_extensions.get().is_empty());
        assert_eq!(redrawn.get(), 1);
    }

    #[test]
    fn the_user_agent_is_a_single_line_row_that_refuses_a_blank_value() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::EditText { can_be_blank, validate, error_message, .. } = user_agent_row(&prefs) else {
            panic!("edit row")
        };
        assert!(!can_be_blank);
        assert!(!validate("a\nb"));
        assert_eq!(error_message.map(|msg| msg("a\nb")), Some(INVALID_USER_AGENT.to_owned()));
    }

    #[test]
    fn the_dump_goes_to_the_crash_folder_with_the_extension_blocks_and_the_last_500_error_lines() {
        use kuro_core::crash_dump::{dump_path, DebugInfo, ExtensionStatus};
        let (cache, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let log: String = (0..600).map(|i| format!("2026-10-10T10:00:00Z ERROR boom {i}\n2026-10-10T10:00:00Z INFO fine {i}\n")).collect();
        std::fs::write(logs.path().join(kuro_core::logging::LOG_FILE_NAME), log).unwrap();
        let old = ExtensionStatus { name: "Old".into(), installed_name: "1".into(), installed_code: 1, available: Some(("2".into(), 2)), obsolete: false };
        let (path, text) = write_crash_dump(cache.path(), logs.path(), &DebugInfo::default(), &[old], &[]).unwrap();
        assert_eq!(path, dump_path(cache.path()));
        assert!(path.ends_with("crash/kurobayomi_crash_logs.txt"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert!(text.contains("Problematic manga extensions:") && !text.contains("Problematic anime extensions:"));
        assert_eq!(text.lines().filter(|l| l.contains("ERROR")).count(), 500);
        assert!(text.contains("boom 599") && !text.contains("boom 99\n"), "the newest lines win");
    }

    #[test]
    fn a_missing_log_file_still_gives_a_dump() {
        let (cache, logs) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (_, text) = write_crash_dump(cache.path(), logs.path(), &Default::default(), &[], &[]).unwrap();
        assert!(text.ends_with("(no log file)\n"));
    }

    #[test]
    fn the_verbose_switch_writes_the_network_preference() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs, Rc::new(|| {}));
        let Some(Preference::Item(PreferenceItem::Switch { common, pref, .. })) = rows.iter().find(|entry| matches!(entry, Preference::Item(PreferenceItem::Switch { .. }))) else {
            panic!("a switch at the top level")
        };
        assert_eq!(common.title, "Verbose logging");
        assert_eq!(common.subtitle.as_deref(), Some(VERBOSE_SUBTITLE));
        assert_eq!(pref.key(), "verbose_logging");
    }

    #[test]
    fn the_verbose_switch_accepts_a_value_when_no_logging_handle_is_shared() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::Switch { veto, .. } = verbose_row(&prefs) else { panic!("a switch") };
        let mut future = veto(true);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert_eq!(future.as_mut().poll(&mut context), std::task::Poll::Ready(Ok(true)));
        assert_eq!(apply_verbose(false), Ok(()));
    }

    #[test]
    fn the_refresh_covers_row_sits_in_the_library_group_and_is_tappable() {
        let PreferenceItem::Text { common, on_click, .. } = refresh_covers_row() else { panic!("a text row") };
        assert_eq!(common.title, "Refresh library covers");
        assert!(common.enabled && on_click.is_some());
        let rows = page_rows(&Prefs::in_memory(), Rc::new(|| {}));
        let library = rows.iter().find_map(|entry| match entry {
            Preference::Group(g) if g.title == "Library" => Some(g),
            _ => None,
        });
        assert_eq!(library.map(|g| g.items.len()), Some(2));
    }

    #[test]
    fn the_crash_log_row_and_the_reindex_row_are_tappable() {
        let PreferenceItem::Custom { title, .. } = share_crash_logs_row() else { panic!("a custom row") };
        assert_eq!(title, "Share crash logs");
        let PreferenceItem::Text { common, on_click, .. } = reindex_downloads_row() else { panic!("a text row") };
        assert_eq!(common.subtitle.as_deref(), Some("Force app to recheck downloaded chapters and episodes"));
        assert!(on_click.is_some());
    }

    #[test]
    fn the_reader_reset_row_has_its_spec_subtitle() {
        let PreferenceItem::Text { common, on_click, .. } = reset_reader_row() else { panic!("text row") };
        assert_eq!(common.subtitle.as_deref(), Some("Resets reading mode and orientation of all series"));
        assert!(on_click.is_some());
    }
}
