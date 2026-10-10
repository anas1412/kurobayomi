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
use crate::settings_ui::rows::{edit_text, group, int_choice, text};
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

/// The DNS-over-HTTPS list. The new provider applies to the next request, so there is no restart note.
pub fn doh_row(prefs: &Prefs) -> PreferenceItem {
    int_choice("DNS over HTTPS (DoH)", &prefs.network.doh_provider, &DOH_PROVIDERS, true)
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
        group(
            "Networking",
            vec![clear_cookies_row(), clear_webview_row(), doh_row(prefs), user_agent_row(prefs), reset_user_agent_row(prefs, changed)],
        ),
        group("Library", vec![reset_reader_row()]),
        group("Extensions", vec![revoke]),
    ]
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
        assert_eq!(value_of(&doh_row(&prefs)), "-1");
        prefs.network.doh_provider.set(4).unwrap();
        assert_eq!(value_of(&doh_row(&prefs)), "4");
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
    fn the_reader_reset_row_has_its_spec_subtitle() {
        let PreferenceItem::Text { common, on_click, .. } = reset_reader_row() else { panic!("text row") };
        assert_eq!(common.subtitle.as_deref(), Some("Resets reading mode and orientation of all series"));
        assert!(on_click.is_some());
    }
}
