//! The Tracking screen rows this module binds (tasks T-47a-053 to T-47a-059): the three switches and the mark-as-read
//! choice, the seven tracker rows with their login and logout dialogs, and the enhanced trackers section.
//!
//! The rows are driven by `kuro_track::login_rows` over the services of [`tracker_host`]: the check mark follows the
//! stored login, a browser row opens the authorise URL and shows a pending state until the redirect arrives, a
//! credentials row opens the login dialog, and a logged-in row asks to log out. The services are built off the GTK
//! thread, so the page shows logged-out rows first and redraws when they are ready.
//!
//! Decisions: a failed credentials login keeps the old session and the dialog open (the rule of
//! `kuro_track::dialog`, which does not port Aniyomi's logout on failure); an enhanced tracker's source is installed
//! when an installed extension (manga) or source (anime) has the accepted source's name or the tracker's own name.
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use kuro_prefs::prefs::Prefs;
use kuro_prefs::tracking::AutoTrackState;
use kuro_track::dialog::{login_title, logout_title, run_login, run_logout, username_label, LoginForm};
use kuro_track::login_rows::{login_row_views, row_subtitle, tap_row, LoginRow, LoginRowView, RowTap, LOGIN_ROWS};
use kuro_track::messages::{english, MsgKey};
use kuro_track::oauth::ClientSecrets;
use kuro_track::tracker::Tracker;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::pref_model::widgets::dialog;
use crate::screens::{built, Built};
use crate::settings_ui::rows::{enum_choice, group, info, item, switch, tap_item, TapRow};
use crate::settings_ui::tracker_host::{self, now_ms, Host};

/// Shown when a row is tapped before the tracker services are built, or when they could not be.
pub const NOT_READY: &str = "Tracker services are not available yet";
/// The subtitle of a browser login that waits for its redirect.
pub const WAITING: &str = "Waiting for the browser login…";
/// The info text under the tracker rows (`tracking_info`).
pub const TRACKING_INFO: &str = "One-way sync to update the chapter progress in external tracker services. Set up tracking for individual entries from their tracking button.";
/// The info text under the enhanced trackers (`enhanced_tracking_info`).
pub const ENHANCED_INFO: &str = "Provides enhanced features for specific sources. Entries are automatically tracked when added to your library.";

/// One enhanced tracker: its name, the source identities it accepts, and whether it serves anime (else manga).
#[derive(Debug, Clone, Copy)]
pub struct Enhanced {
    pub name: &'static str,
    pub accepted: &'static [&'static str],
    pub anime: bool,
}

/// The enhanced manga trackers, then the enhanced anime tracker, in the order the spec lists them.
pub const ENHANCED: [Enhanced; 4] = [
    Enhanced { name: "Komga", accepted: kuro_track::trackers::komga::ACCEPTED_SOURCES, anime: false },
    Enhanced { name: "Kavita", accepted: kuro_track::trackers::kavita::ACCEPTED_SOURCES, anime: false },
    Enhanced { name: "Suwayomi", accepted: kuro_track::trackers::suwayomi::ACCEPTED_SOURCES, anime: false },
    Enhanced { name: "Jellyfin", accepted: kuro_track::trackers::jellyfin::ACCEPTED_SOURCES, anime: true },
];

/// The choices of "Update progress when marked as read/watched" (`AutoTrackState`).
pub const MARK_READ_OPTIONS: [(AutoTrackState, &str); 3] =
    [(AutoTrackState::ALWAYS, "Always"), (AutoTrackState::ASK, "Always ask"), (AutoTrackState::NEVER, "Never")];

/// Whether the login button is enabled: both fields have text and no login is in flight.
pub fn login_enabled(username: &str, password: &str, in_flight: bool) -> bool {
    LoginForm { username: username.to_owned(), password: password.to_owned(), processing: in_flight, input_error: false }.can_submit()
}

/// The enhanced section's info text: the base text, then the trackers whose source is not installed, after two newlines.
pub fn enhanced_info(not_installed: &[&str]) -> String {
    if not_installed.is_empty() {
        ENHANCED_INFO.to_owned()
    } else {
        format!("{ENHANCED_INFO}\n\nAvailable but source not installed: {}", not_installed.join(", "))
    }
}

/// Splits the enhanced trackers into those with an installed source (shown) and those without (listed in the info text).
pub fn split_enhanced<'a>(trackers: &'a [Enhanced], installed: impl Fn(&Enhanced) -> bool) -> (Vec<&'a Enhanced>, Vec<&'a Enhanced>) {
    trackers.iter().partition(|tracker| installed(tracker))
}

/// Whether a source of `tracker` is installed: the manga list serves the manga trackers and the anime list the anime
/// one, and a name matches when it has the accepted source's name or the tracker's own name in it (any case).
pub fn is_installed(tracker: &Enhanced, manga: &[String], anime: &[String]) -> bool {
    let names = if tracker.anime { anime } else { manga };
    let own = tracker.name.to_lowercase();
    names.iter().any(|name| {
        let name = name.to_lowercase();
        name.contains(&own) || tracker.accepted.iter().any(|id| name.contains(id.split('/').next().unwrap_or(id)))
    })
}

/// The names of the installed manga extensions and the installed anime sources.
fn installed_names() -> (Vec<String>, Vec<String>) {
    let manga = crate::ext_host::registry().map(|registry| registry.snapshot().installed.values().map(|ext| ext.name.clone()).collect()).unwrap_or_default();
    let anime = crate::ext_host::anime_sources().0.into_iter().map(|source| source.name).collect();
    (manga, anime)
}

/// The warning rows: one while the secrets live in the fallback file because no keyring is reachable.
pub fn warning_rows(fallback: bool) -> Vec<Preference> {
    tracker_host::keyring_warning(if fallback { kuro_platform::secrets::SecretsMode::EncryptedFile } else { kuro_platform::secrets::SecretsMode::Primary })
        .map(|text| item(info(text)))
        .into_iter()
        .collect()
}

fn launch(url: &str) {
    gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
        if let Err(err) = result {
            tracing::warn!(%err, "the login page did not open");
            crate::toast::notify("Could not open the browser");
        }
    });
}

/// The login dialog (Kitsu and MangaUpdates): the username or email field, the password field with its show toggle, and
/// the Login button, which stays disabled until both fields have text and shows `Loading…` while the login runs.
fn open_login_dialog(tracker: Arc<dyn Tracker>, changed: Rc<dyn Fn()>) {
    let name = tracker.name().to_owned();
    let form = LoginForm::for_tracker(tracker.as_ref());
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let user = gtk::Entry::builder().placeholder_text(username_label(tracker.id())).text(&form.username).build();
    let pass = gtk::PasswordEntry::builder().show_peek_icon(true).placeholder_text("Password").text(&form.password).build();
    column.append(&user);
    column.append(&pass);
    let shown: Rc<std::cell::RefCell<Option<dialog::Dialog>>> = Rc::default();
    let refresh: Rc<dyn Fn()> = {
        let (user, pass, shown) = (user.clone(), pass.clone(), Rc::clone(&shown));
        Rc::new(move || {
            if let Some(dialog) = shown.borrow().as_ref() {
                dialog.set_confirm_enabled(login_enabled(&user.text(), &pass.text(), false));
            }
        })
    };
    for changed_field in [user.clone().upcast::<gtk::Editable>(), pass.clone().upcast::<gtk::Editable>()] {
        let refresh = Rc::clone(&refresh);
        changed_field.connect_changed(move |_| refresh());
    }
    let (user_text, pass_text) = (user.clone(), pass.clone());
    let dialog = dialog::show(
        &crate::ui::window(),
        &login_title(&name),
        &column,
        Some(LoginForm::for_tracker(tracker.as_ref()).button_label()),
        move |dialog| {
            let (username, password) = (user_text.text().to_string(), pass_text.text().to_string());
            let busy = LoginForm { username: username.clone(), password: password.clone(), processing: true, input_error: false };
            dialog.set_error(None);
            dialog.set_confirm_enabled(false);
            if let Some(button) = &dialog.confirm {
                button.set_label(busy.button_label());
            }
            let (tracker, changed) = (Arc::clone(&tracker), Rc::clone(&changed));
            crate::source_bridge::call_on_main(
                {
                    let tracker = Arc::clone(&tracker);
                    async move { run_login(tracker.as_ref(), &username, &password).await }
                },
                move |result| match result {
                    Ok(key) => {
                        crate::toast::notify(english(&key));
                        dialog.finish();
                        changed();
                    }
                    Err(key) => {
                        dialog.set_error(Some(&english(&key)));
                        dialog.set_confirm_enabled(true);
                        if let Some(button) = &dialog.confirm {
                            button.set_label(LoginForm { username: String::new(), password: String::new(), processing: false, input_error: true }.button_label());
                        }
                    }
                },
            );
        },
        || {},
    );
    dialog.set_confirm_enabled(login_enabled(&user.text(), &pass.text(), false));
    *shown.borrow_mut() = Some(dialog);
}

/// The logout dialog: the title names the tracker, Log out clears the credentials and says `You are now logged out`.
fn open_logout_dialog(tracker: Arc<dyn Tracker>, changed: Rc<dyn Fn()>) {
    let title = logout_title(tracker.name());
    let body = gtk::Label::builder().label(&title).wrap(true).build();
    dialog::show(
        &crate::ui::window(),
        &title,
        &body,
        Some("Log out"),
        move |dialog| match run_logout(tracker.as_ref()) {
            Ok(key) => {
                crate::toast::notify(english(&key));
                dialog.finish();
                changed();
            }
            Err(key) => dialog.set_error(Some(&english(&key))),
        },
        || {},
    );
}

/// The MyAnimeList login: the loopback listener starts on the runtime, the authorise URL opens in the browser, and the
/// redirect completes the login. The paste field of Aniyomi's dialog has no row here; a busy port ends the attempt.
fn start_mal_login(host: &Rc<Host>, changed: Rc<dyn Fn()>) {
    use kuro_track::mal::login::LoginDialog;
    use kuro_track::mal::redirect::DEFAULT_PORT;
    let Some(shell) = crate::shell::host::Shell::get() else { return };
    let (tx, rx) = async_channel::unbounded::<Result<String, Option<String>>>();
    let mal = Arc::clone(&host.mal);
    shell.runtime().spawn(async move {
        let mut login = match LoginDialog::open(&mal, DEFAULT_PORT).await {
            Ok(login) => login,
            Err(err) => {
                let _ = tx.send(Err(Some(err.to_string()))).await;
                return;
            }
        };
        let _ = tx.send(Ok(login.authorize_url().to_owned())).await;
        let done = match login.wait_redirect(&mal).await {
            Some(Ok(())) => None,
            Some(Err(err)) => Some(err.to_string()),
            None => Some("The login needs a free port for the redirect".to_owned()),
        };
        let _ = tx.send(Err(done)).await;
    });
    host.begin_waiting(kuro_track::model::TrackerId::MYANIMELIST, now_ms());
    let host = Rc::clone(host);
    gtk::glib::spawn_future_local(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                Ok(url) => launch(&url),
                Err(outcome) => {
                    host.stop_waiting(kuro_track::model::TrackerId::MYANIMELIST);
                    crate::toast::notify(match outcome {
                        None => english(&MsgKey::TrackerLoggedIn("MyAnimeList".to_owned())),
                        Some(text) => text,
                    });
                    changed();
                    break;
                }
            }
        }
    });
}

/// What a tap on a login row does: the decision is `kuro_track::login_rows::tap_row`, this carries it out.
fn tapped(index: usize, changed: &Rc<dyn Fn()>) {
    let Some(host) = tracker_host::host() else {
        crate::toast::notify(NOT_READY);
        return;
    };
    let Some(view) = login_row_views(&host.manager).get(index).copied() else { return };
    let id = view.row.tracker;
    let tap = tap_row(&view, &mut host.pending.borrow_mut(), &ClientSecrets::default(), &host.manager, now_ms());
    let Some(tracker) = host.manager.get(id) else { return };
    match tap {
        Some(RowTap::ConfirmLogout) => open_logout_dialog(tracker, Rc::clone(changed)),
        Some(RowTap::OpenCredentials) => open_login_dialog(tracker, Rc::clone(changed)),
        Some(RowTap::OpenBrowser(url) | RowTap::StartSimklLogin(url)) => {
            host.begin_waiting(id, now_ms());
            launch(&url);
            changed();
        }
        Some(RowTap::StartMalLogin) => {
            start_mal_login(&host, Rc::clone(changed));
            changed();
        }
        Some(RowTap::Toast(key)) => crate::toast::notify(english(&key)),
        None => {
            if let Some(hint) = tracker.setup_message() {
                crate::toast::notify(english(&hint));
            }
        }
    }
}

/// The check mark of a logged-in row.
fn check_mark() -> gtk::Widget {
    let label = crate::pref_model::strings::logged_in();
    let image = gtk::Image::from_icon_name("object-select-symbolic");
    crate::pref_model::style::install();
    image.add_css_class("tracker-check");
    image.set_pixel_size(crate::pref_model::widgets::TRACKER_CHECK_SIZE);
    image.set_tooltip_text(Some(&label));
    image.set_accessible_role(gtk::AccessibleRole::Img);
    image.update_property(&[gtk::accessible::Property::Label(&label)]);
    image.upcast()
}

/// The login state of every row, read from the services (logged out while they are not built).
fn views(host: Option<&Host>) -> Vec<LoginRowView> {
    match host {
        Some(host) => login_row_views(&host.manager),
        None => LOGIN_ROWS.iter().map(|&row| LoginRowView { row, logged_in: false, expired: false, configured: true }).collect(),
    }
}

/// The subtitle of a login row: the tracker's own state text, or the pending text while its browser login waits.
fn subtitle(host: Option<&Host>, row: LoginRow) -> Option<String> {
    let host = host?;
    let tracker = host.manager.get(row.tracker)?;
    let waiting = host.is_waiting(row.tracker, now_ms()) && !tracker.is_logged_in();
    row_subtitle(tracker.as_ref(), waiting || host.mal.login_in_progress() && row.tracker == kuro_track::model::TrackerId::MYANIMELIST)
        .map(|key| english(&key))
        .or_else(|| waiting.then(|| WAITING.to_owned()))
}

/// The tracker row for one login row: a check when logged in, a subtitle for pending or expired states, and a tap that
/// runs the row's action.
fn tracker_item(index: usize, view: LoginRowView, host: Option<&Host>, changed: &Rc<dyn Fn()>) -> PreferenceItem {
    let trailing: Option<Rc<dyn Fn() -> gtk::Widget>> = view.logged_in.then(|| Rc::new(check_mark) as Rc<dyn Fn() -> gtk::Widget>);
    let changed = Rc::clone(changed);
    let on_tap: Rc<dyn Fn(&TapRow)> = Rc::new(move |_| tapped(index, &changed));
    let logo = match kuro_track::logo::logo_asset(view.row.tracker) {
        Some(kuro_track::tracker::LogoRef::Embedded(bytes)) => Some(bytes),
        _ => None,
    };
    // Only AniList is switched on until the other trackers are fixed; their rows stay visible but cannot be tapped.
    let enabled = tracker_enabled(view.row.tracker);
    let sub = if enabled { subtitle(host, view.row) } else { Some(NOT_AVAILABLE.to_owned()) };
    tap_item(view.row.name, sub.as_deref(), enabled, trailing, logo, on_tap)
}

const NOT_AVAILABLE: &str = "Not available yet";

/// The trackers that can log in today.
pub fn tracker_enabled(tracker: kuro_track::model::TrackerId) -> bool {
    tracker == kuro_track::model::TrackerId::ANILIST
}

/// The enhanced trackers group: a row for each tracker whose source is installed (a tap is a no-op login), a disabled
/// placeholder for the others (so the spec row is dropped), then the info text and the not-installed list.
fn enhanced_group(host: Option<&Host>, changed: &Rc<dyn Fn()>) -> Preference {
    let (manga, anime) = installed_names();
    let (present, absent) = split_enhanced(&ENHANCED, |tracker| is_installed(tracker, &manga, &anime));
    let mut items: Vec<PreferenceItem> = ENHANCED
        .iter()
        .map(|tracker| {
            let shown = present.iter().any(|p| p.name == tracker.name);
            let live = host.and_then(|host| host.manager.get(enhanced_id(tracker.name)));
            let logged_in = shown && live.as_ref().is_some_and(|t| t.is_logged_in());
            let on_logout: Rc<dyn Fn()> = {
                let (changed, live) = (Rc::clone(changed), live.clone());
                Rc::new(move || {
                    if let Some(tracker) = &live {
                        open_logout_dialog(Arc::clone(tracker), Rc::clone(&changed));
                    }
                })
            };
            if shown {
                PreferenceItem::Tracker { name: tracker.name.to_owned(), logged_in, on_login: Rc::new(login_noop), on_logout }
            } else {
                crate::settings_ui::rows::text(tracker.name, None, false)
            }
        })
        .collect();
    let missing: Vec<&str> = absent.iter().map(|tracker| tracker.name).collect();
    if missing.is_empty() {
        items.push(info(ENHANCED_INFO));
    } else {
        // the spec info row is replaced by the full text: a disabled twin drops it, the full text is a row of its own
        items.push(PreferenceItem::Info { text: ENHANCED_INFO.to_owned(), enabled: false });
        items.push(info(&enhanced_info(&missing)));
    }
    group("Enhanced trackers", items)
}

/// An enhanced tracker has no credentials to enter: its login is a no-op (`loginNoop`).
fn login_noop() {}

fn enhanced_id(name: &str) -> kuro_track::model::TrackerId {
    use kuro_track::model::TrackerId;
    match name {
        "Komga" => TrackerId::KOMGA,
        "Kavita" => TrackerId::KAVITA,
        "Suwayomi" => TrackerId::SUWAYOMI,
        _ => TrackerId::JELLYFIN,
    }
}

/// The typed rows of the Tracking screen. `changed` redraws the page after a login, a logout or a pending state.
pub fn page_rows(prefs: &Prefs, changed: Rc<dyn Fn()>) -> Vec<Preference> {
    let tracking = kuro_prefs::tracking::TrackPrefs::new(prefs.store().clone());
    let host = tracker_host::host();
    let views = views(host.as_deref());
    let mut trackers: Vec<PreferenceItem> = views.iter().enumerate().map(|(index, view)| tracker_item(index, *view, host.as_deref(), &changed)).collect();
    trackers.push(info(TRACKING_INFO));
    let mut rows = warning_rows(host.as_ref().is_some_and(|host| host.fallback));
    rows.extend([
        item(switch("Update progress after reading/watching", &tracking.auto_update_track, true)),
        item(switch("Open track menu on adding to library", &tracking.track_on_adding_to_library, true)),
        item(switch("Show next episode's airing time", &tracking.show_next_episode_airing_time, true)),
        item(enum_choice("Update progress when marked as read/watched", &tracking.auto_update_track_on_mark_read, &MARK_READ_OPTIONS, true)),
        group("Trackers", trackers),
        enhanced_group(host.as_deref(), &changed),
    ]);
    rows
}

/// Fills `holder` with the Tracking page, and starts the tracker services if they are not up. The page redraws itself
/// when they are, and after every login change.
pub fn fill(holder: &gtk::Box) {
    while let Some(child) = holder.first_child() {
        holder.remove(&child);
    }
    let prefs = crate::settings_ui::route::prefs();
    let weak = holder.downgrade();
    let changed: Rc<dyn Fn()> = Rc::new(move || {
        if let Some(holder) = weak.upgrade() {
            fill(&holder);
        }
    });
    tracker_host::set_refresh(Rc::clone(&changed));
    tracker_host::ensure();
    let tree = crate::settings_ui::merge::merge(crate::settings_ui::route::spec_tree("settings_tracking", prefs.store()), page_rows(&prefs, changed));
    holder.append(&crate::pref_model::page::body(&tree).widget);
}

/// The Tracking page body for `toolbar`.
pub fn page(toolbar: Vec<gtk::Widget>) -> Option<Built> {
    let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
    holder.set_vexpand(true);
    fill(&holder);
    built(holder, toolbar)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn login_needs_both_fields_and_no_login_in_flight() {
        assert!(login_enabled("me@example.org", "secret", false));
        assert!(!login_enabled("  ", "secret", false));
        assert!(!login_enabled("me", "", false));
        assert!(!login_enabled("me", "secret", true));
    }

    #[test]
    fn the_enhanced_info_lists_the_missing_trackers_after_two_newlines() {
        assert_eq!(enhanced_info(&[]), ENHANCED_INFO);
        assert_eq!(
            enhanced_info(&["Komga", "Kavita"]),
            format!("{ENHANCED_INFO}\n\nAvailable but source not installed: Komga, Kavita")
        );
    }

    #[test]
    fn manga_and_anime_trackers_are_told_apart_by_the_kind_of_the_installed_source() {
        let kinds: Vec<(&str, bool)> = ENHANCED.iter().map(|t| (t.name, t.anime)).collect();
        assert_eq!(kinds, [("Komga", false), ("Kavita", false), ("Suwayomi", false), ("Jellyfin", true)]);
        let komga = &ENHANCED[0];
        let jellyfin = &ENHANCED[3];
        assert!(is_installed(komga, &names(&["Tachiyomi: Komga"]), &[]));
        assert!(!is_installed(komga, &[], &names(&["Komga"])), "an anime source does not serve a manga tracker");
        assert!(is_installed(jellyfin, &[], &names(&["Jellyfin"])));
        assert!(!is_installed(jellyfin, &names(&["Jellyfin"]), &[]));
        assert!(is_installed(&ENHANCED[2], &names(&["Tachidesk"]), &[]), "the accepted source name counts");
    }

    #[test]
    fn an_enhanced_tracker_is_shown_only_when_its_source_is_installed() {
        let manga = names(&["Kavita"]);
        let (shown, missing) = split_enhanced(&ENHANCED, |tracker| is_installed(tracker, &manga, &[]));
        assert_eq!(shown.iter().map(|t| t.name).collect::<Vec<_>>(), ["Kavita"]);
        assert_eq!(missing.iter().map(|t| t.name).collect::<Vec<_>>(), ["Komga", "Suwayomi", "Jellyfin"]);
        let (none, all) = split_enhanced(&ENHANCED, |_| false);
        assert!(none.is_empty());
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn the_mark_read_choices_are_always_always_ask_and_never() {
        let labels: Vec<&str> = MARK_READ_OPTIONS.iter().map(|(_, label)| *label).collect();
        assert_eq!(labels, ["Always", "Always ask", "Never"]);
    }

    #[test]
    fn the_seven_tracker_rows_follow_the_login_row_order() {
        let names: Vec<&str> = LOGIN_ROWS.iter().map(|row| row.name).collect();
        assert_eq!(names, ["MyAnimeList", "AniList", "Kitsu", "MangaUpdates", "Shikimori", "Simkl", "Bangumi"]);
    }

    #[test]
    fn before_the_services_exist_every_row_is_logged_out_and_unconfigured_state_is_not_claimed() {
        let rows = views(None);
        assert_eq!(rows.len(), 7);
        assert!(rows.iter().all(|view| !view.logged_in && !view.expired && view.configured));
        assert_eq!(subtitle(None, LOGIN_ROWS[0]), None);
    }

    #[test]
    fn the_warning_row_appears_only_in_fallback_mode() {
        assert!(warning_rows(false).is_empty());
        let rows = warning_rows(true);
        assert!(matches!(rows.as_slice(), [Preference::Item(PreferenceItem::Info { text, .. })] if text == "Tokens stored unencrypted"));
    }

    #[test]
    fn the_switches_and_the_tracker_group_bind_the_tracking_preferences() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs, Rc::new(|| {}));
        let titles: Vec<&str> = rows.iter().filter_map(|entry| match entry {
            Preference::Item(item) => Some(item.title()),
            Preference::Group(_) => None,
        }).collect();
        assert_eq!(titles[0], "Update progress after reading/watching");
        assert!(rows.iter().any(|entry| matches!(entry, Preference::Group(g) if g.title == "Trackers" && g.items.len() == 8)));
        assert!(rows.iter().any(|entry| matches!(entry, Preference::Group(g) if g.title == "Enhanced trackers")));
    }

    #[test]
    fn only_anilist_can_log_in_until_the_others_are_fixed() {
        use kuro_track::model::TrackerId;
        assert!(tracker_enabled(TrackerId::ANILIST));
        for other in [TrackerId::MYANIMELIST, TrackerId::KITSU, TrackerId::SIMKL] {
            assert!(!tracker_enabled(other));
        }
    }

    #[test]
    fn a_tracker_row_is_a_tappable_row_titled_with_the_tracker() {
        let changed: Rc<dyn Fn()> = Rc::new(|| {});
        let view = views(None)[2];
        let row = tracker_item(2, view, None, &changed);
        assert!(matches!(row, PreferenceItem::Custom { ref title, .. } if title == "Kitsu"));
    }
}
