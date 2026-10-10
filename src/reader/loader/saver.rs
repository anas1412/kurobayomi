//! The data saver of the reader's HTTP loader (T-43a-017, `HttpPageLoader.kt:51`). The loader asks for its images through
//! the service the user chose, once per loader, and the URL formats are those of `kuro_downloads::data_saver`, which the
//! downloader uses too. The cache key stays the original image URL; only the request changes.
//!
//! Decision on the plan conflict: `RISKS-AND-DECISIONS.md` section 4 drops all three services, while T-43a-017 and
//! `p1-data-and-config.md` section 5.1 list all three, and `kuro-downloads` already keeps two of them. Resolved as follows.
//! Bandwidth Hero and wsrv.nl stay: both rewrite the request URL with no extra round trip, and Bandwidth Hero is the
//! user's own server. resmush.it stays dropped (D-DL-09, `NOT-PORTING.md` O-4): it is plain HTTP, it needs a blocking
//! request per image before the real one, and Kotlin reads its JSON by string search. A stored `RESMUSH_IT` choice is
//! kept so settings and backups round-trip, and the reader treats it as no service. The default is no service, so no
//! image URL goes to a third party unless the user picked one.

use std::collections::BTreeSet;

use kuro_downloads::data_saver::{DataSaver, Service};
use kuro_prefs::source::{DataSaver as Choice, SourcePrefs};

/// The service a stored choice maps to. `RESMUSH_IT` is no service (see the module notes).
pub fn service_of(choice: Choice) -> Service {
    match choice {
        Choice::BANDWIDTH_HERO => Service::BandwidthHero,
        Choice::WSRV_NL => Service::WsrvNl,
        Choice::NONE | Choice::RESMUSH_IT => Service::None,
    }
}

/// The excluded sources the preference stores as decimal strings. Entries that are not numbers are skipped.
pub fn excluded_ids(stored: &BTreeSet<String>) -> BTreeSet<i64> {
    stored.iter().filter_map(|s| s.parse().ok()).collect()
}

/// The data saver the reader uses, from the stored settings. `data_saver_downloader` concerns downloads only, so the
/// reader always honours the chosen service.
pub fn from_prefs(prefs: &SourcePrefs) -> DataSaver {
    DataSaver {
        service: service_of(prefs.data_saver.get()),
        downloader_enabled: true,
        excluded_sources: excluded_ids(&prefs.data_saver_excluded_sources.get()),
        server: prefs.data_saver_server.get().trim_end_matches('/').to_owned(),
        quality: i64::from(prefs.data_saver_image_quality.get()),
        ignore_jpeg: prefs.data_saver_ignore_jpeg.get(),
        ignore_gif: prefs.data_saver_ignore_gif.get(),
        format_jpeg: prefs.data_saver_image_format_jpeg.get(),
        color_bw: prefs.data_saver_color_bw.get(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    fn prefs() -> SourcePrefs {
        SourcePrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn the_default_is_no_service_so_the_url_is_unchanged() {
        let saver = from_prefs(&prefs());
        assert_eq!(saver.request_url(7, "https://img/a.png"), "https://img/a.png");
    }

    #[test]
    fn bandwidth_hero_uses_the_users_server_in_the_exact_format() {
        let p = prefs();
        p.data_saver.set(Choice::BANDWIDTH_HERO).unwrap();
        p.data_saver_server.set("https://hero.example/".into()).unwrap();
        p.data_saver_color_bw.set(true).unwrap();
        let saver = from_prefs(&p);
        assert_eq!(
            saver.request_url(7, "https://img/a b.png"),
            "https://hero.example/?jpg=0&l=80&bw=1&url=https%3A%2F%2Fimg%2Fa+b.png"
        );
        assert_eq!(saver.request_url(7, "https://img/a.gif"), "https://img/a.gif", "gif is ignored by default");
        assert_eq!(saver.request_url(7, "https://hero.example/x.png"), "https://hero.example/x.png", "never twice");
    }

    #[test]
    fn wsrv_uses_webp_unless_jpeg_is_asked_for_and_keeps_animated_formats() {
        let p = prefs();
        p.data_saver.set(Choice::WSRV_NL).unwrap();
        p.data_saver_image_quality.set(60).unwrap();
        assert_eq!(from_prefs(&p).request_url(7, "https://img/a.png"), "https://wsrv.nl/?url=https://img/a.png&output=webp&q=60");
        assert_eq!(from_prefs(&p).request_url(7, "https://img/a.webp"), "https://wsrv.nl/?url=https://img/a.webp&q=60&n=-1");
        p.data_saver_image_format_jpeg.set(true).unwrap();
        assert_eq!(from_prefs(&p).request_url(7, "https://img/a.png"), "https://wsrv.nl/?url=https://img/a.png&output=jpg&q=60");
        p.data_saver_ignore_jpeg.set(true).unwrap();
        assert_eq!(from_prefs(&p).request_url(7, "https://img/a.JPG"), "https://img/a.JPG");
    }

    #[test]
    fn an_excluded_source_and_the_dropped_resmush_choice_are_left_alone() {
        let p = prefs();
        p.data_saver.set(Choice::WSRV_NL).unwrap();
        p.data_saver_excluded_sources.set(["7".to_owned(), "not a number".to_owned()].into()).unwrap();
        let saver = from_prefs(&p);
        assert_eq!(saver.request_url(7, "https://img/a.png"), "https://img/a.png");
        assert_ne!(saver.request_url(8, "https://img/a.png"), "https://img/a.png");
        p.data_saver.set(Choice::RESMUSH_IT).unwrap();
        assert_eq!(service_of(p.data_saver.get()), Service::None);
        assert_eq!(from_prefs(&p).request_url(8, "https://img/a.png"), "https://img/a.png");
    }
}
