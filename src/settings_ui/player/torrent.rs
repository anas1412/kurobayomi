//! The Torrent settings page (tasks T-48a-032 and T-48a-034): enable with the one-time legal notice, port, trackers,
//! reset trackers, proxy mode and proxy URL. The rules are in `kuro_prefs::torrent`; the server itself is in the
//! torrent area. Every write goes through its row's veto, so a refused value is not stored.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Box as GBox, Label, Orientation};
use kuro_i18n::{trf, tr, Arg, MsgKey};
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;
use kuro_prefs::torrent::{is_reset_enabled, validate_port, validate_proxy_url, ProxyMode, ProxyUrlError};

use crate::pref_model::bind;
use crate::pref_model::model::{ItemCommon, PreferenceItem};
use crate::pref_model::strings;
use crate::pref_model::widgets::dialog::{self, Dialog};
use crate::settings_ui::player::conf_edit;
use crate::settings_ui::rows::{enum_choice, switch_with, text, text_veto};

/// The port rule as an error message: the port must be a number from 0 to 65535 (E-TS-02).
fn port_error(port: &str) -> Option<String> {
    validate_port(port).is_err().then(|| tr(MsgKey::pref_player_torrents_port_error))
}

fn validate_port_text(port: &str) -> Result<(), String> {
    port_error(port).map_or(Ok(()), Err)
}

/// The proxy URL rule (E-TS-03). An empty URL is allowed only with mode None; anything else needs a scheme and a host
/// from the supported list. `None` means the value is valid.
pub fn proxy_error(mode: ProxyMode, url: &str) -> Option<String> {
    if url.is_empty() {
        return (mode != ProxyMode::None).then(|| tr(MsgKey::pref_player_torrents_proxy_url_invalid_uri));
    }
    match validate_proxy_url(url) {
        Ok(()) => None,
        Err(ProxyUrlError::InvalidUri) => Some(tr(MsgKey::pref_player_torrents_proxy_url_invalid_uri)),
        Err(ProxyUrlError::UnsupportedProtocol(scheme)) => Some(trf(
            MsgKey::pref_player_torrents_proxy_url_invalid_protocol,
            &[("arg1", Arg::Str(scheme.as_str()))],
        )),
    }
}

/// The proxy modes in list order, labelled with the Kotlin strings.
fn proxy_modes() -> [(ProxyMode, String); 4] {
    [
        (ProxyMode::None, tr(MsgKey::pref_player_torrents_proxy_mode_none)),
        (ProxyMode::Tracker, tr(MsgKey::pref_player_torrents_proxy_mode_tracker)),
        (ProxyMode::Peers, tr(MsgKey::pref_player_torrents_proxy_mode_peers)),
        (ProxyMode::Full, tr(MsgKey::pref_player_torrents_proxy_mode_full)),
    ]
}

/// The rows of the Torrent page, loose (the spec page has no group title).
pub fn page_rows(prefs: &Prefs) -> Vec<PreferenceItem> {
    let t = &prefs.torrent;
    let modes = proxy_modes();
    let options: Vec<(ProxyMode, &str)> = modes.iter().map(|(mode, label)| (*mode, label.as_str())).collect();
    vec![
        enable_row(prefs),
        port_row(&t.torr_server_port),
        conf_edit::multi_line_row(&tr(MsgKey::pref_player_torrents_trackers), &t.torr_server_trackers, true),
        reset_trackers_row(prefs),
        enum_choice(&tr(MsgKey::pref_player_torrents_proxy_mode), &t.torr_server_proxy_mode, &options, true),
        proxy_url_row(prefs),
    ]
}

/// The enable switch. The first time torrents are turned on, the legal notice shows once (E-TS-01).
fn enable_row(prefs: &Prefs) -> PreferenceItem {
    let torrent = prefs.torrent.clone();
    let veto = bind::veto(move |on: bool| {
        let notice = torrent.enable_changed(on);
        let result = torrent.torr_server_enable.set(on).map(|()| true).map_err(|err| err.to_string());
        if notice && result.is_ok() {
            show_legal_notice();
        }
        async move { result }
    });
    switch_with(&tr(MsgKey::pref_player_torrents_enable), None, &prefs.torrent.torr_server_enable, true, veto)
}

fn show_legal_notice() {
    let body = GBox::new(Orientation::Vertical, 8);
    body.append(&Label::builder().label(tr(MsgKey::pref_player_torrents_notice_text)).wrap(true).xalign(0.0).build());
    body.append(&Label::builder().label(tr(MsgKey::pref_player_torrents_notice_footer)).wrap(true).xalign(0.0).build());
    dialog::show(
        &crate::ui::window(),
        &tr(MsgKey::pref_player_torrents_notice),
        &body,
        Some(&strings::ok()),
        |dialog: Dialog| dialog.finish(),
        || {},
    );
}

/// The port: a dialog with the summary hint, OK only for a valid port (E-TS-02).
fn port_row(pref: &Pref<String>) -> PreferenceItem {
    PreferenceItem::EditTextInfo {
        common: ItemCommon::bound(tr(MsgKey::pref_player_torrents_port)),
        pref: pref.clone(),
        dialog_subtitle: Some(tr(MsgKey::pref_player_torrents_port_summary)),
        can_be_blank: false,
        validate: Rc::new(|port: &str| validate_port(port).is_ok()),
        error_message: Some(Rc::new(|port: &str| port_error(port).unwrap_or_default())),
        veto: text_veto(pref, false, validate_port_text),
    }
}

/// Resets the trackers string to the default. The row is enabled only while torrents are on and the value differs
/// from the default (E-TS-09).
fn reset_trackers_row(prefs: &Prefs) -> PreferenceItem {
    let t = &prefs.torrent;
    let enabled = is_reset_enabled(t.torr_server_enable.get(), &t.torr_server_trackers.get());
    let mut row = text(&tr(MsgKey::pref_player_torrents_trackers_reset), None, enabled);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        let trackers = t.clone();
        *on_click = Some(Rc::new(move || {
            if let Err(err) = trackers.reset_trackers() {
                tracing::warn!(%err, "could not reset the torrent trackers");
                crate::toast::notify(strings::COULD_NOT_SAVE);
            }
        }));
    }
    row
}

/// The proxy URL dialog. Its validity depends on the proxy mode stored at the time of the check.
fn proxy_url_row(prefs: &Prefs) -> PreferenceItem {
    let (url, mode) = (prefs.torrent.torr_server_proxy_url.clone(), prefs.torrent.torr_server_proxy_mode.clone());
    let (valid_mode, message_mode, write_mode) = (mode.clone(), mode.clone(), mode);
    let writer = url.clone();
    PreferenceItem::EditTextInfo {
        common: ItemCommon::bound(tr(MsgKey::pref_player_torrents_proxy_url)),
        pref: url,
        dialog_subtitle: Some(tr(MsgKey::pref_player_torrents_proxy_url_dialog)),
        can_be_blank: true,
        validate: Rc::new(move |text: &str| proxy_error(valid_mode.get(), text).is_none()),
        error_message: Some(Rc::new(move |text: &str| proxy_error(message_mode.get(), text).unwrap_or_default())),
        veto: bind::veto(move |text: String| {
            let result = match proxy_error(write_mode.get(), &text) {
                Some(message) => Err(message),
                None => writer.set(text).map(|()| true).map_err(|err| err.to_string()),
            };
            async move { result }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_must_be_a_number_from_0_to_65535() {
        assert_eq!(port_error("8090"), None);
        assert_eq!(port_error("0"), None);
        assert_eq!(port_error("65535"), None);
        assert_eq!(port_error("65536").as_deref(), Some("Port must be a number between 0 and 65535"));
        assert_eq!(port_error("").as_deref(), Some("Port must be a number between 0 and 65535"));
        assert!(port_error("-1").is_some());
    }

    #[test]
    fn proxy_url_rules_depend_on_the_mode() {
        assert_eq!(proxy_error(ProxyMode::None, ""), None);
        assert_eq!(proxy_error(ProxyMode::Peers, "").as_deref(), Some("Invalid uri"));
        assert_eq!(proxy_error(ProxyMode::Peers, "socks5h://10.0.0.1:1080"), None);
        assert_eq!(proxy_error(ProxyMode::Peers, "ftp://host").as_deref(), Some("Unsupported protocol: \"ftp\""));
        assert_eq!(proxy_error(ProxyMode::None, "not a url").as_deref(), Some("Invalid uri"));
    }

    #[test]
    fn the_page_offers_the_torrent_rows_in_order_with_the_stored_defaults() {
        let prefs = Prefs::in_memory();
        let titles: Vec<String> = page_rows(&prefs).iter().map(|i| i.title().to_owned()).collect();
        assert_eq!(
            titles,
            ["Enable TorrServer", "TorrServer port", "Torrent trackers", "Reset default torrent trackers string", "Proxy mode", "Proxy url"]
        );
        assert!(!prefs.torrent.torr_server_enable.get());
        assert_eq!(prefs.torrent.torr_server_port.get(), "8090");
    }

    #[test]
    fn reset_is_offered_only_for_a_changed_value_while_torrents_are_on() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let reset = |rows: &[PreferenceItem]| match &rows[3] {
            PreferenceItem::Text { common, .. } => common.enabled,
            _ => panic!("reset row"),
        };
        assert!(!reset(&rows));
        prefs.torrent.torr_server_enable.set(true).unwrap();
        assert!(!reset(&page_rows(&prefs)));
        prefs.torrent.torr_server_trackers.set("udp://x".to_owned()).unwrap();
        assert!(reset(&page_rows(&prefs)));
    }
}
