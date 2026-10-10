//! The anime extension update check and its notice (T-19a-052, T-19a-053). The last check is stored as wall-clock Unix
//! seconds under `anime_ext_last_check`. The wait until the next scheduled check is a monotonic sleep of `next_delay`,
//! so the clock is read only to decide that wait.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_channel::Sender;
use kuro_platform::notifier::{APP_NAME, Notifier, send};
use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;
use tokio_util::sync::CancellationToken;

use crate::anime::manager::Event;
use crate::anime::store::StoreError;
use crate::anime::store::refresh::{Aggregate, fetch_all};
use crate::notice::{GENERIC_TEXT, update_title};

/// The pref key of the last check, in Unix seconds (TST-19-102).
pub const LAST_CHECK_KEY: &str = "anime_ext_last_check";
/// The notice id. It differs from the manga notice, so the two never replace each other (E-UPD-04).
pub const NOTICE_ID: &str = "anime-ext-updates";
/// The minimum gap between two scheduled checks (TST-19-100).
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// How long to wait before the next check. Zero when no check is stored or the stored one is a day old or more. A
/// stored time in the future counts as a full day. `now` and `last_check` are Unix seconds.
pub fn next_delay(last_check: Option<i64>, now: i64) -> Duration {
    let Some(last) = last_check else {
        return Duration::ZERO;
    };
    match u64::try_from(now.saturating_sub(last)) {
        Ok(elapsed) => CHECK_INTERVAL.saturating_sub(Duration::from_secs(elapsed)),
        Err(_) => CHECK_INTERVAL,
    }
}

/// The handle of the last-check pref: Unix seconds under [`LAST_CHECK_KEY`], unset until the first successful check.
pub fn last_check_pref(store: &PrefStore) -> Pref<i64> {
    store.pref(LAST_CHECK_KEY, 0)
}

/// The wall clock in Unix seconds. It is read only to decide the wait and to stamp a finished check.
pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
}

/// One refresh: every store document is fetched once, and the same documents give the list (E-UPD-02). A refresh
/// where no store could be read counts as failed, so the previous list stays. Returns true when the list was sent.
/// This is the refresh the Refresh button runs too, so a manual refresh updates Browse as well.
pub async fn refresh_once(
    client: &reqwest::Client,
    index_urls: &[String],
    cancel: &CancellationToken,
    events: &Sender<Event>,
    is_latest: &(dyn Fn() -> bool + Sync),
) -> bool {
    let aggregate: Aggregate = match fetch_all(client, index_urls, cancel).await {
        Ok(aggregate) => aggregate,
        Err(StoreError::Cancelled) => return false,
        Err(err) => {
            tracing::error!("the extension refresh failed: {err}");
            return false;
        }
    };
    if aggregate.stores.is_empty() && !aggregate.failures.is_empty() {
        tracing::error!("no store could be read, so the previous extension list stays");
        return false;
    }
    if !is_latest() {
        tracing::debug!("a newer extension refresh started, so this result is dropped");
        return false;
    }
    events.send(Event::Refreshed(aggregate)).await.is_ok()
}

/// The scheduled check (T-19a-052). It waits `next_delay` on the monotonic timer, runs [`refresh_once`], and on success
/// stores `now()` as wall-clock seconds. After a failed refresh it waits a full [`CHECK_INTERVAL`] before it tries
/// again, so a store that is down is not hit in a loop. `index_urls` is read at each check, so stores added later are
/// included. It returns when `cancel` fires or the receiver is gone.
pub async fn run_checks(
    client: &reqwest::Client,
    index_urls: &(dyn Fn() -> Vec<String> + Sync),
    last_check: &Pref<i64>,
    now: &(dyn Fn() -> i64 + Sync),
    events: &Sender<Event>,
    cancel: &CancellationToken,
) {
    let mut failed = false;
    loop {
        let delay = if failed {
            CHECK_INTERVAL
        } else {
            next_delay(last_check.is_set().then(|| last_check.get()), now())
        };
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return,
            _ = tokio::time::sleep(delay) => {}
        }
        if events.is_closed() {
            return;
        }
        if refresh_once(client, &index_urls(), cancel, events, &|| true).await {
            failed = false;
            if let Err(err) = last_check.set(now()) {
                tracing::warn!("could not save {LAST_CHECK_KEY}: {err}");
            }
            // The next check is a full interval away, from the stamp just written.
        } else if cancel.is_cancelled() {
            return;
        } else {
            failed = true;
        }
    }
}

/// The update notice. It shows when the set of packages with an update changes and is withdrawn when none is left
/// (T-19a-053). The count itself is `ExtensionState::update_count`, which is never stored.
#[derive(Debug, Default)]
pub struct UpdateNotice {
    shown: Vec<String>,
}

impl UpdateNotice {
    /// `names` are the names of the packages with an update. Same names as the last call change nothing. The body is
    /// the names joined with ", ", and with `hide_content` it goes out as a sensitive notice (app name, generic body).
    pub fn sync(&mut self, names: &[String], hide_content: bool, notifier: &dyn Notifier) {
        if names.is_empty() {
            notifier.withdraw(NOTICE_ID);
            self.shown.clear();
            return;
        }
        if self.shown == names {
            return;
        }
        send(notifier, hide_content, true, NOTICE_ID, APP_NAME, &update_title(names.len()), &names.join(", "), GENERIC_TEXT);
        self.shown = names.to_vec();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        calls: Mutex<Vec<String>>,
    }

    impl Notifier for Recorder {
        fn show(&self, id: &str, title: &str, body: Option<&str>) {
            self.calls.lock().unwrap().push(format!("show {id} | {title} | {}", body.unwrap_or("-")));
        }
        fn withdraw(&self, id: &str) {
            self.calls.lock().unwrap().push(format!("withdraw {id}"));
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_check_is_due_at_once_when_none_is_stored_and_after_a_day() {
        let now = 1_000_000;
        assert_eq!(next_delay(None, now), Duration::ZERO);
        assert_eq!(next_delay(Some(now - 86_400), now), Duration::ZERO);
        assert_eq!(next_delay(Some(now - 3_600), now), Duration::from_secs(86_400 - 3_600));
        assert_eq!(next_delay(Some(now + 10), now), CHECK_INTERVAL, "a future stamp waits a full day");
    }

    #[test]
    fn the_notice_is_shown_only_when_the_names_change_and_withdrawn_at_zero() {
        let rec = Recorder::default();
        let mut notice = UpdateNotice::default();
        notice.sync(&names(&["Alpha"]), false, &rec);
        notice.sync(&names(&["Alpha"]), false, &rec);
        notice.sync(&names(&["Alpha", "Beta"]), false, &rec);
        notice.sync(&[], false, &rec);
        assert_eq!(
            *rec.calls.lock().unwrap(),
            vec![
                "show anime-ext-updates | Extension update available | Alpha".to_owned(),
                "show anime-ext-updates | 2 extension updates available | Alpha, Beta".to_owned(),
                "withdraw anime-ext-updates".to_owned(),
            ]
        );
    }

    #[test]
    fn a_hidden_content_setting_replaces_the_title_and_body() {
        let rec = Recorder::default();
        UpdateNotice::default().sync(&names(&["Alpha", "Beta"]), true, &rec);
        assert_eq!(*rec.calls.lock().unwrap(), vec![format!("show anime-ext-updates | {APP_NAME} | {GENERIC_TEXT}")]);
    }

    use crate::anime::testkit::{store_bytes, store_document, store_extension, test_client};
    use kuro_prefs::store::PrefStore;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn serve_one_store(server: &MockServer, status: u16) {
        let body = store_bytes(&store_document("S", "k", vec![store_extension("p.a", 1, None)]));
        Mock::given(method("GET"))
            .and(path("/index.json"))
            .respond_with(ResponseTemplate::new(status).set_body_bytes(body))
            .mount(server)
            .await;
    }

    fn urls_of(server: &MockServer) -> Vec<String> {
        vec![format!("{}/index.json", server.uri())]
    }

    #[test]
    fn tst_19_102_the_stored_key_is_anime_ext_last_check_and_the_manga_key_is_untouched() {
        let store = PrefStore::in_memory();
        last_check_pref(&store).set(123).unwrap();
        assert_eq!(store.get_raw("anime_ext_last_check"), Some(kuro_prefs::values::PrefValue::Long(123)));
        assert_eq!(store.get_raw("last_ext_check"), None);
    }

    #[tokio::test]
    async fn tst_19_101_one_refresh_fetches_each_store_once_and_sends_the_list() {
        let server = MockServer::start().await;
        serve_one_store(&server, 200).await;
        let (events, inbox) = async_channel::unbounded();
        assert!(refresh_once(&test_client(), &urls_of(&server), &CancellationToken::new(), &events, &|| true).await);
        assert!(matches!(inbox.try_recv(), Ok(Event::Refreshed(agg)) if agg.available.len() == 1));
        assert_eq!(server.received_requests().await.unwrap().len(), 1, "one request per store");
    }

    #[tokio::test]
    async fn a_superseded_refresh_sends_nothing_so_the_newer_result_is_not_overwritten() {
        let server = MockServer::start().await;
        serve_one_store(&server, 200).await;
        let (events, inbox) = async_channel::unbounded();
        assert!(!refresh_once(&test_client(), &urls_of(&server), &CancellationToken::new(), &events, &|| false).await);
        assert!(inbox.try_recv().is_err(), "a dropped result sends no event");
    }

    #[tokio::test]
    async fn a_failed_refresh_sends_nothing_so_the_old_list_stays() {
        let server = MockServer::start().await;
        serve_one_store(&server, 500).await;
        let (events, inbox) = async_channel::unbounded();
        assert!(!refresh_once(&test_client(), &urls_of(&server), &CancellationToken::new(), &events, &|| true).await);
        assert!(inbox.try_recv().is_err());
    }

    #[tokio::test]
    async fn tst_19_060_a_refresh_with_one_bad_store_logs_the_store_failure_once() {
        let server = MockServer::start().await;
        serve_one_store(&server, 500).await;
        let (events, _inbox) = async_channel::unbounded();
        let recording = crate::anime::testkit::start_capture();
        refresh_once(&test_client(), &urls_of(&server), &CancellationToken::new(), &events, &|| true).await;
        let logged = recording.events();
        let store_failures = logged.iter().filter(|event| event.message.starts_with("Failed to ")).count();
        assert_eq!(store_failures, 1, "{logged:?}");
    }

    /// Lets the scheduler run for a short real time. It never ends on its own, so a timeout is the normal outcome.
    async fn let_run(task: &mut (impl std::future::Future<Output = ()> + Unpin)) {
        assert!(tokio::time::timeout(Duration::from_millis(400), task).await.is_err(), "ends only on cancel");
    }

    #[tokio::test]
    async fn tst_19_100_a_check_within_a_day_does_not_fetch() {
        let server = MockServer::start().await;
        serve_one_store(&server, 200).await;
        let store = PrefStore::in_memory();
        let last = last_check_pref(&store);
        last.set(2_000_000 - 3_600).unwrap();
        let (events, inbox) = async_channel::unbounded();
        let cancel = CancellationToken::new();
        let (urls, client) = (urls_of(&server), test_client());
        let (list, now) = (|| urls.clone(), || 2_000_000);
        let mut task = Box::pin(run_checks(&client, &list, &last, &now, &events, &cancel));
        let_run(&mut task).await;
        assert_eq!(server.received_requests().await.unwrap().len(), 0, "the second start within 24 h does not fetch");
        assert!(inbox.try_recv().is_err());
        assert_eq!(last.get(), 2_000_000 - 3_600, "the stamp is unchanged");
        cancel.cancel();
        task.await;
    }

    #[tokio::test]
    async fn a_due_check_fetches_once_sends_the_list_and_stamps_the_wall_clock_time() {
        let server = MockServer::start().await;
        serve_one_store(&server, 200).await;
        let store = PrefStore::in_memory();
        let last = last_check_pref(&store);
        last.set(2_000_000 - 86_400).unwrap();
        let (events, inbox) = async_channel::unbounded();
        let cancel = CancellationToken::new();
        let (urls, client) = (urls_of(&server), test_client());
        let (list, now) = (|| urls.clone(), || 2_000_000);
        let mut task = Box::pin(run_checks(&client, &list, &last, &now, &events, &cancel));
        let_run(&mut task).await;
        assert!(matches!(inbox.try_recv(), Ok(Event::Refreshed(_))));
        assert_eq!(last.get(), 2_000_000);
        assert_eq!(server.received_requests().await.unwrap().len(), 1, "one fetch, then a full day of waiting");
        cancel.cancel();
        task.await;
    }

    #[tokio::test]
    async fn a_failed_scheduled_check_keeps_the_stamp_and_does_not_retry_in_a_loop() {
        let server = MockServer::start().await;
        serve_one_store(&server, 500).await;
        let store = PrefStore::in_memory();
        let last = last_check_pref(&store);
        let (events, inbox) = async_channel::unbounded();
        let cancel = CancellationToken::new();
        let (urls, client) = (urls_of(&server), test_client());
        let (list, now) = (|| urls.clone(), || 5_000_000);
        let mut task = Box::pin(run_checks(&client, &list, &last, &now, &events, &cancel));
        let_run(&mut task).await;
        assert_eq!(server.received_requests().await.unwrap().len(), 1, "no stamp, so the first check ran at once");
        assert!(!last.is_set(), "a failed refresh does not stamp");
        assert!(inbox.try_recv().is_err(), "the old list is kept");
        cancel.cancel();
        task.await;
    }
}
