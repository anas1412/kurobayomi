//! The About page rows this module binds (tasks T-47a-093 and T-47a-094): the Version row, whose subtitle carries the
//! build and which copies the debug information when tapped, and the Check for updates row, which runs the manual
//! release check of `app_update`. The row is hidden when the updater is off in this build. The other rows of the spec
//! page stay as they are.
//!
//! Decisions: the build date is the raw build time string (the Android date-format fallback), and the debug block uses
//! the labels of the debug-info page's own groups. The Check for updates row runs its own copy of the manual check so it
//! knows when the check ends: a spinner shows while it runs and a second tap is ignored (T-47a-094). Aniyomi's "no
//! longer supported" toast belongs to an Android API level; Linux has no such rule, so the row has no EOL branch.
use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_core::app_info::APP_INFO;
use kuro_domain::release::check::Outcome;
use kuro_domain::release::github::{GithubReleases, ReleaseHttp};
use kuro_domain::release::{Release, ReleaseError};
use kuro_i18n::{tr, MsgKey};

use crate::app_update;
use crate::pref_model::debug_info::{app_rows, device_rows, model_text, version_text};
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::changelog;
use crate::settings_ui::clipboard::copy_to_clipboard;
use crate::settings_ui::rows::{item, tap_item, text, TapRow};
use crate::ui::{DialogButton, Role};

pub const VERSION_TITLE: &str = "Version";
pub const BUILD_TIME_TITLE: &str = "Build time";
pub const MODEL_TITLE: &str = "Device model";

/// The Version subtitle: `Stable <version> (<build time>)` in a release, `Debug <sha> (<build time>)` in a debug build.
pub fn version_subtitle() -> String {
    let info = &*APP_INFO;
    format!("{} ({})", version_text(info.build_type, &info.version_name, info.git_sha), info.build_time)
}

/// The debug information block the Version row copies: one `Title: value` line per row, app rows first.
pub fn debug_block(app: &[(String, String)], device: &[(String, String)]) -> String {
    app.iter().chain(device).map(|(title, value)| format!("{title}: {value}")).collect::<Vec<_>>().join("\n")
}

/// The debug information as the About page copies it, read now.
pub fn current_debug_block() -> String {
    let read = |path: &str| std::fs::read_to_string(path).ok();
    let model = model_text(read("/sys/class/dmi/id/sys_vendor").as_deref(), read("/sys/class/dmi/id/product_name").as_deref());
    let gtk_version = format!("{}.{}.{}", gtk::major_version(), gtk::minor_version(), gtk::micro_version());
    let app = app_rows(VERSION_TITLE, BUILD_TIME_TITLE);
    let device = device_rows(MODEL_TITLE, model, &kuro_platform::system::facts(), gtk_version);
    debug_block(&app, &device)
}

/// The Version row: the build subtitle, and a tap copies the debug information.
pub fn version_row() -> PreferenceItem {
    let mut row = text(VERSION_TITLE, Some(&version_subtitle()), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(std::rc::Rc::new(|| copy_to_clipboard(&current_debug_block())));
    }
    row
}

/// Whether the release check exists in this build (the updater feature and a repository).
pub fn check_available() -> bool {
    app_update::enabled(APP_INFO.updater_enabled, app_update::UPDATE_REPOSITORY)
}

/// What a finished check shows.
#[derive(Debug, PartialEq, Eq)]
pub enum Report {
    /// A toast with this text.
    Toast(String),
    /// The new-version dialog for this release.
    Dialog(Release),
}

/// The message of a finished manual check: the error text when the lookup failed, the dialog for a newer release.
pub fn report(result: Result<Outcome, ReleaseError>) -> Report {
    match result {
        Ok(Outcome::NoNewUpdate) => Report::Toast(tr(MsgKey::update_check_no_new_updates)),
        Ok(Outcome::NewUpdate(release)) => Report::Dialog(release),
        Err(e) => Report::Toast(e.to_string()),
    }
}

/// One check at a time: a second tap while a check runs does nothing.
#[derive(Default)]
pub struct InFlight(Cell<bool>);

impl InFlight {
    /// True when the caller may start a check (and now holds the flag).
    pub fn begin(&self) -> bool {
        !self.0.replace(true)
    }

    pub fn end(&self) {
        self.0.set(false);
    }
}

thread_local! {
    static CHECKING: InFlight = InFlight::default();
}

/// The GET behind [`GithubReleases`], over a blocking client built on the worker thread.
struct Http(reqwest::blocking::Client);

impl ReleaseHttp for Http {
    fn get(&self, url: &str, user_agent: &str) -> Result<(u16, Vec<u8>), String> {
        let response = self.0.get(url).header(reqwest::header::USER_AGENT, user_agent).send().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body = response.bytes().map_err(|e| e.to_string())?;
        Ok((status, body.to_vec()))
    }
}

/// The forced check, off the UI thread: `done` runs on the main context with the result.
fn run_check(done: impl FnOnce(Result<Outcome, ReleaseError>) + 'static) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map(Http)
            .map_err(|e| ReleaseError::Transport(e.to_string()))
            .and_then(|http| {
                let service = GithubReleases::new(http, std::env::consts::ARCH);
                app_update::check(&service, app_update::UPDATE_REPOSITORY, env!("CARGO_PKG_VERSION"), chrono::Utc::now().timestamp_millis(), true)
            });
        let _ = tx.send_blocking(result);
    });
    gtk::glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            done(result);
        }
    });
}

/// Opens `url` with the desktop's handler; `failed` runs when it cannot be opened.
fn launch(url: &str, failed: impl FnOnce() + 'static) {
    gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, move |result| {
        if let Err(e) = result {
            tracing::warn!(error = %e, "could not open the link");
            failed();
        }
    });
}

/// The update page (T-47ba-030, ui-parity 3.5): a 48 px icon, the heading, the version, the notes in a scrolling body
/// and the pinned Download and Not now buttons. Enter is Download, Escape is Not now.
///
/// Decision (T-47ba-032): there is no in-place install (T-35a-056 drops it). Download opens the release's file link
/// with the desktop's handler and closes the page; the button is disabled on the first click. Offline, the handler
/// fails and a toast says so.
fn show_update(release: &Release) {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let icon = gtk::Image::builder().icon_name("software-update-available-symbolic").pixel_size(48).halign(gtk::Align::Center).build();
    icon.set_accessible_role(gtk::AccessibleRole::Presentation);
    column.append(&icon);
    column.append(&gtk::Label::builder().label(tr(MsgKey::update_check_notification_update_available)).css_classes(["title-1"]).build());
    column.append(&gtk::Label::builder().label(&release.version).css_classes(["dim-label"]).build());

    let notes = gtk::Label::builder().use_markup(true).wrap(true).xalign(0.0).yalign(0.0).selectable(true).build();
    notes.set_markup(&changelog::to_pango(&release.info));
    notes.connect_activate_link(|_, url| {
        launch(url, || {});
        gtk::glib::Propagation::Stop
    });
    let scroll = gtk::ScrolledWindow::builder().child(&notes).propagate_natural_height(true).max_content_height(360).hscrollbar_policy(gtk::PolicyType::Never).build();
    column.append(&scroll);

    if changelog::link_visible(&release.release_link) {
        let open = gtk::Button::builder().label(tr(MsgKey::update_check_open)).icon_name("web-browser-symbolic").halign(gtk::Align::Start).build();
        open.add_css_class("flat");
        let link = release.release_link.clone();
        open.connect_clicked(move |_| launch(&link, || {}));
        column.append(&open);
    }

    let file = release.download_link.clone();
    let download = DialogButton::new(tr(MsgKey::update_check_confirm), Role::Suggested, move || {
        launch(&file, || crate::toast::notify(tr(MsgKey::update_check_notification_download_error)));
    });
    let title = tr(MsgKey::update_check_notification_update_available);
    crate::ui::dialog(&title, &column, 520, vec![DialogButton::cancel(tr(MsgKey::action_not_now)), download]).present();
}

/// A tap on the row: starts a check unless one runs, shows the spinner meanwhile, then the report.
fn check_tapped(row: &TapRow) {
    if !CHECKING.with(InFlight::begin) {
        return;
    }
    let spinner = row.trailing.clone().and_downcast::<gtk::Spinner>();
    if let Some(spinner) = &spinner {
        spinner.set_visible(true);
        spinner.start();
    }
    run_check(move |result| {
        if let Err(e) = &result {
            tracing::warn!(error = %e, "the update check failed");
        }
        match report(result) {
            Report::Toast(text) => crate::toast::notify(text),
            Report::Dialog(release) => show_update(&release),
        }
        if let Some(spinner) = &spinner {
            spinner.stop();
            spinner.set_visible(false);
        }
        CHECKING.with(InFlight::end);
    });
}

/// The Check for updates row. Where the build has no updater the row is a disabled twin, which drops the spec row.
pub fn check_row() -> PreferenceItem {
    if !check_available() {
        return text(app_update::CHECK_ROW_TITLE, None, false);
    }
    let spinner: Rc<dyn Fn() -> gtk::Widget> = Rc::new(|| gtk::Spinner::builder().visible(false).build().upcast());
    tap_item(app_update::CHECK_ROW_TITLE, None, true, Some(spinner), None, Rc::new(check_tapped))
}

/// This app's project page. Aniyomi's website, Discord and privacy page describe the Android app, so none is linked.
pub const PROJECT_URL: &str = "https://github.com/anas1412/kurobayomi";
/// Where bugs are reported.
pub const ISSUES_URL: &str = "https://github.com/anas1412/kurobayomi/issues";
/// The release tag of a build: `v<version>` for a release, `r<commit count>` for a preview.
pub fn release_tag(build_type: &str, version_name: &str, commit_count: u32) -> String {
    if build_type == "preview" { format!("r{commit_count}") } else { format!("v{version_name}") }
}

/// The GitHub release page of a tag.
pub fn release_page_url(repo: &str, tag: &str) -> String {
    format!("https://github.com/{repo}/releases/tag/{tag}")
}

/// Opens a link in the desktop's browser; a failure is logged only.
fn open_url(url: &str) {
    gtk::UriLauncher::new(url).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
        if let Err(err) = result {
            tracing::warn!(%err, "the link did not open");
        }
    });
}

/// A row that opens `url` when tapped.
fn link_row(title: &str, url: &str, enabled: bool) -> PreferenceItem {
    let mut row = text(title, None, enabled);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        let url = url.to_owned();
        *on_click = Some(Rc::new(move || open_url(&url)));
    }
    row
}

/// What's new: the release page of this build. Hidden in debug builds and when no release repository is set.
fn whats_new_row() -> PreferenceItem {
    let info = &*APP_INFO;
    let repo = crate::app_update::UPDATE_REPOSITORY;
    let tag = release_tag(info.build_type, &info.version_name, info.commit_count);
    link_row("What's new", &release_page_url(repo, &tag), info.build_type != "debug" && !repo.is_empty())
}

/// The bottom row of links: the project page and the issue tracker.
fn links_row() -> PreferenceItem {
    let build: Rc<dyn Fn() -> gtk::Widget> = Rc::new(|| {
        let bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        bar.set_halign(gtk::Align::Center);
        for (label, url) in [("GitHub", PROJECT_URL), ("Report an issue", ISSUES_URL)] {
            let button = gtk::Button::with_label(label);
            button.add_css_class("flat");
            button.connect_clicked(move |_| open_url(url));
            bar.append(&button);
        }
        bar.upcast()
    });
    PreferenceItem::Custom { title: "Links".to_owned(), build }
}

/// The typed rows of the About page, merged over the spec rows of the same titles. The links row has no spec twin and
/// is kept at the end.
pub fn page_rows() -> Vec<Preference> {
    vec![
        item(version_row()),
        item(check_row()),
        item(whats_new_row()),
        item(crate::settings_ui::maintenance::share_crash_logs_row()),
        item(links_row()),
    ]
}

#[cfg(test)]
mod link_tests {
    use super::*;

    #[test]
    fn a_preview_build_tags_its_commit_count_and_a_release_its_version() {
        assert_eq!(release_tag("preview", "0.1.0", 42), "r42");
        assert_eq!(release_tag("release", "0.1.0", 42), "v0.1.0");
    }

    #[test]
    fn the_release_page_url_joins_repo_and_tag() {
        assert_eq!(release_page_url("owner/name", "v1.2.3"), "https://github.com/owner/name/releases/tag/v1.2.3");
    }

    #[test]
    fn the_links_lead_to_this_apps_project_and_never_to_aniyomi() {
        let urls = [PROJECT_URL, ISSUES_URL];
        for url in urls {
            assert!(url.starts_with("https://github.com/anas1412/kurobayomi"), "{url}");
        }
        assert!(!urls.iter().any(|u| u.contains("aniyomi")));
    }

    #[test]
    fn the_page_has_no_privacy_row_and_the_link_bar_is_the_last_row() {
        let rows = page_rows();
        let titles: Vec<&str> = rows.iter().map(|r| match r { Preference::Item(i) => i.title(), Preference::Group(g) => g.title.as_str() }).collect();
        assert!(!titles.contains(&"Privacy policy"));
        assert_eq!(titles.last(), Some(&"Links"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_debug_block_lists_the_app_rows_then_the_device_rows() {
        let app = vec![("Version".to_owned(), "Stable 1.0".to_owned())];
        let device = vec![("GTK".to_owned(), "4.16.0".to_owned())];
        assert_eq!(debug_block(&app, &device), "Version: Stable 1.0\nGTK: 4.16.0");
    }

    #[test]
    fn the_version_subtitle_ends_with_the_build_time_in_brackets() {
        let subtitle = version_subtitle();
        assert!(subtitle.ends_with(&format!("({})", APP_INFO.build_time)), "{subtitle}");
    }

    #[test]
    fn the_check_row_is_a_dropped_twin_where_the_updater_is_not_built() {
        let row = check_row();
        assert_eq!(row.title(), app_update::CHECK_ROW_TITLE);
        assert_eq!(matches!(row, PreferenceItem::Custom { .. }), check_available());
        assert!(row.enabled() || !check_available());
    }

    #[test]
    fn a_second_tap_while_a_check_runs_is_refused_until_it_ends() {
        let flight = InFlight::default();
        assert!(flight.begin());
        assert!(!flight.begin());
        flight.end();
        assert!(flight.begin());
    }

    #[test]
    fn a_finished_check_reports_the_error_text_the_no_update_toast_or_the_release() {
        assert_eq!(report(Ok(Outcome::NoNewUpdate)), Report::Toast(tr(MsgKey::update_check_no_new_updates)));
        let release = Release { version: "v2".into(), info: "notes".into(), release_link: "https://x".into(), download_link: "https://y".into() };
        assert_eq!(report(Ok(Outcome::NewUpdate(release.clone()))), Report::Dialog(release));
        let err = ReleaseError::Transport("offline".into());
        assert_eq!(report(Err(err.clone())), Report::Toast(err.to_string()));
    }

    #[test]
    fn the_version_row_carries_the_build_subtitle_and_copies_on_tap() {
        let PreferenceItem::Text { common, on_click, .. } = version_row() else { panic!("a text row") };
        assert_eq!(common.title, VERSION_TITLE);
        assert_eq!(common.subtitle.as_deref(), Some(version_subtitle().as_str()));
        assert!(on_click.is_some());
    }
}
