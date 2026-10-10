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
use crate::settings_ui::clipboard::copy_to_clipboard;
use crate::settings_ui::rows::{item, tap_item, text, TapRow};

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
    let base = crate::data::app().base.clone();
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map(Http)
            .map_err(|e| ReleaseError::Transport(e.to_string()))
            .and_then(|http| {
                let service = GithubReleases::new(http, std::env::consts::ARCH);
                app_update::check(&service, app_update::UPDATE_REPOSITORY, env!("CARGO_PKG_VERSION"), &base, chrono::Utc::now().timestamp_millis(), true)
            });
        let _ = tx.send_blocking(result);
    });
    gtk::glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            done(result);
        }
    });
}

/// "New version available!" with the notes, and Open on GitHub or Not now. There is no in-place install.
fn show_update(release: &Release) {
    let dialog = gtk::AlertDialog::builder()
        .message(format!("{} {}", tr(MsgKey::update_check_notification_update_available), release.version))
        .detail(&release.info)
        .buttons([tr(MsgKey::update_check_open), tr(MsgKey::action_not_now)])
        .cancel_button(1)
        .default_button(0)
        .build();
    let link = release.release_link.clone();
    dialog.choose(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, move |choice| {
        if choice == Ok(0) {
            gtk::UriLauncher::new(&link).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
                if let Err(e) = result {
                    tracing::warn!(error = %e, "could not open the release page");
                }
            });
        }
    });
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
    tap_item(app_update::CHECK_ROW_TITLE, None, true, Some(spinner), Rc::new(check_tapped))
}

/// The typed rows of the About page, merged over the spec rows of the same titles.
pub fn page_rows() -> Vec<Preference> {
    vec![item(version_row()), item(check_row())]
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
