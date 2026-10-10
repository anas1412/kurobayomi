//! The app release check on the GTK side (T-06a-070, T-07a-036, T-09a-042): the reqwest transport for the GitHub
//! client, the opt-in startup check that stays silent on failure, and the manual "Check for updates" action that
//! reports the error text. The throttle and the version rule are `kuro_domain::release::check`; this file reads and
//! writes `__APP_STATE_last_app_check` around that call and decides when it may run at all.

use std::time::Duration;

use gtk::glib;
use kuro_domain::release::check::{Outcome, check_for_update};
use kuro_domain::release::github::{GithubReleases, ReleaseHttp};
use kuro_domain::release::{Release, ReleaseError, ReleaseService};
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::base::BasePrefs;

/// The GitHub `owner/name` that releases are read from (`UPDATE_REPOSITORY`). Empty turns the check off. A build
/// sets it with the `KB_UPDATE_REPOSITORY` environment variable.
pub const UPDATE_REPOSITORY: &str = match option_env!("KB_UPDATE_REPOSITORY") {
    Some(repository) => repository,
    None => "",
};

/// The title of the About row that starts a manual check (`spec/settings.json`, `settings_about`).
pub const CHECK_ROW_TITLE: &str = "Check for updates";

const TIMEOUT: Duration = Duration::from_secs(30);

/// Whether the check exists in this build: the `updater` feature is on and a repository is set.
pub fn enabled(updater_feature: bool, repository: &str) -> bool {
    updater_feature && !repository.is_empty()
}

fn available() -> bool {
    enabled(kuro_core::app_info::APP_INFO.updater_enabled, UPDATE_REPOSITORY)
}

/// The About row is only listed when the check exists (`notes` of `settings_about`).
pub fn hides_row(title: &str) -> bool {
    title == CHECK_ROW_TITLE && !available()
}

/// The GET behind [`GithubReleases`], over a blocking reqwest client. Built on the worker thread that runs the check.
struct Http(reqwest::blocking::Client);

impl Http {
    fn new() -> Result<Self, String> {
        reqwest::blocking::Client::builder().timeout(TIMEOUT).build().map(Self).map_err(|e| e.to_string())
    }
}

impl ReleaseHttp for Http {
    fn get(&self, url: &str, user_agent: &str) -> Result<(u16, Vec<u8>), String> {
        let response = self.0.get(url).header(reqwest::header::USER_AGENT, user_agent).send().map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let body = response.bytes().map_err(|e| e.to_string())?;
        Ok((status, body.to_vec()))
    }
}

/// One check at `now` (milliseconds): the stored `last_app_check` goes in as the throttle, and the check's
/// `checked_at` is stored only when the lookup returned a release (an error or no asset leaves it alone).
pub fn check(
    service: &dyn ReleaseService,
    repository: &str,
    current: &str,
    base: &BasePrefs,
    now: i64,
    force: bool,
) -> Result<Outcome, ReleaseError> {
    let done = check_for_update(service, repository, current, base.last_app_check.get(), now, force)?;
    if let Some(at) = done.checked_at {
        if let Err(e) = base.last_app_check.set(at) {
            tracing::warn!(error = %e, "could not store the last release check");
        }
    }
    Ok(done.outcome)
}

/// Runs [`check`] over the real transport on a worker thread and hands the result to `deliver` on the main loop.
fn spawn_check(base: BasePrefs, force: bool, deliver: impl FnOnce(Result<Outcome, ReleaseError>) + 'static) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let result = Http::new().map_err(ReleaseError::Transport).and_then(|http| {
            let service = GithubReleases::new(http, std::env::consts::ARCH);
            check(&service, UPDATE_REPOSITORY, env!("CARGO_PKG_VERSION"), &base, chrono::Utc::now().timestamp_millis(), force)
        });
        let _ = tx.send_blocking(result);
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            deliver(result);
        }
    });
}

/// The startup check: only when the build has the check and the user opted in. It is throttled, and any failure is
/// logged and otherwise silent. A newer release shows one toast.
pub fn startup(base: &BasePrefs) {
    if !available() || !base.check_app_updates.get() {
        return;
    }
    spawn_check(base.clone(), false, |result| match result {
        Ok(Outcome::NewUpdate(release)) => crate::toast::notify(update_text(&release)),
        Ok(Outcome::NoNewUpdate) => {}
        Err(e) => tracing::debug!(error = %e, "the startup release check failed"),
    });
}

/// The manual check of the About row: it ignores the opt-in and the throttle, and says what happened, with the error
/// text when the lookup failed. A newer release opens a dialog with a button to its page.
pub fn manual(base: &BasePrefs) {
    spawn_check(base.clone(), true, |result| match result {
        Ok(Outcome::NoNewUpdate) => crate::toast::notify(tr(MsgKey::update_check_no_new_updates)),
        Ok(Outcome::NewUpdate(release)) => show_update(&release),
        Err(e) => crate::toast::notify(e.to_string()),
    });
}

/// Runs the About row's action. False for every other row.
pub fn on_action(screen: &str, title: &str) -> bool {
    if screen != "settings_about" || title != CHECK_ROW_TITLE || !available() {
        return false;
    }
    manual(&crate::data::app().base);
    true
}

fn update_text(release: &Release) -> String {
    format!("{} {}", tr(MsgKey::update_check_notification_update_available), release.version)
}

/// "New version available!" with the notes, and Open on GitHub or Not now. There is no in-place install, the package
/// manager owns the files (T-07a-037).
fn show_update(release: &Release) {
    let dialog = gtk::AlertDialog::builder()
        .message(update_text(release))
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

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use kuro_prefs::store::PrefStore;

    use super::*;

    const DAY: i64 = 24 * 60 * 60 * 1000;

    struct Fixed {
        calls: Cell<u32>,
        reply: Result<Option<Release>, ReleaseError>,
    }

    impl ReleaseService for Fixed {
        fn latest(&self, _repository: &str) -> Result<Option<Release>, ReleaseError> {
            self.calls.set(self.calls.get() + 1);
            self.reply.clone()
        }
    }

    fn fixed(reply: Result<Option<Release>, ReleaseError>) -> Fixed {
        Fixed { calls: Cell::new(0), reply }
    }

    fn release(version: &str) -> Release {
        Release { version: version.into(), info: String::new(), release_link: "https://example.test/r".into(), download_link: "https://example.test/d".into() }
    }

    fn base() -> BasePrefs {
        BasePrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn the_check_needs_the_updater_feature_and_a_repository() {
        assert!(enabled(true, "o/r"));
        assert!(!enabled(true, ""));
        assert!(!enabled(false, "o/r"));
    }

    #[test]
    fn the_startup_setting_is_off_until_the_user_opts_in() {
        assert!(!base().check_app_updates.get());
    }

    #[test]
    fn the_about_row_is_listed_only_when_the_check_exists() {
        assert_eq!(hides_row(CHECK_ROW_TITLE), !available());
        assert!(!hides_row("Version"));
    }

    #[test]
    fn a_returned_release_stores_the_check_time_and_a_second_call_inside_three_days_is_skipped() {
        let (base, service, now) = (base(), fixed(Ok(Some(release("v2.0.0")))), 100 * DAY);
        let current = "1.0.0";
        let first = check(&service, "o/r", current, &base, now, false).unwrap();
        assert_eq!(first, Outcome::NewUpdate(release("v2.0.0")));
        assert_eq!(base.last_app_check.get(), now);
        check(&service, "o/r", current, &base, now + DAY, false).unwrap();
        assert_eq!(service.calls.get(), 1, "the throttle read the stored time");
        check(&service, "o/r", current, &base, now + DAY, true).unwrap();
        assert_eq!(service.calls.get(), 2, "force bypasses it");
        assert_eq!(base.last_app_check.get(), now + DAY);
    }

    #[test]
    fn an_error_or_a_missing_asset_leaves_the_stored_time_alone() {
        let base = base();
        let current = "1.0.0";
        let failing = fixed(Err(ReleaseError::Http(403)));
        assert_eq!(check(&failing, "o/r", current, &base, 50 * DAY, true), Err(ReleaseError::Http(403)));
        let no_asset = fixed(Ok(None));
        assert_eq!(check(&no_asset, "o/r", current, &base, 50 * DAY, true), Ok(Outcome::NoNewUpdate));
        assert_eq!(base.last_app_check.get(), 0);
    }

    #[test]
    fn an_empty_repository_makes_no_call() {
        let (base, service) = (base(), fixed(Ok(Some(release("v2.0.0")))));
        let got = check(&service, "", "1.0.0", &base, 100 * DAY, true);
        assert_eq!(got, Ok(Outcome::NoNewUpdate));
        assert_eq!(service.calls.get(), 0);
    }

    #[test]
    fn the_transport_returns_the_status_and_the_body_and_sends_the_user_agent() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/releases/latest", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap();
            let body = "{\"x\":1}";
            let reply = format!("HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            stream.write_all(reply.as_bytes()).unwrap();
            String::from_utf8_lossy(&buf[..n]).to_ascii_lowercase()
        });
        let (status, body) = Http::new().unwrap().get(&url, "Kurobayomi").unwrap();
        assert_eq!((status, body), (403, b"{\"x\":1}".to_vec()));
        assert!(server.join().unwrap().contains("user-agent: kurobayomi"));
    }
}
