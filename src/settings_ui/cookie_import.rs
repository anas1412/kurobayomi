//! The manual Cloudflare cookie import (T-12a-033): paste the `cf_clearance` value and the user agent of the browser
//! that earned it. The cookie is stored for the site's domain and the user agent becomes the default `User-Agent`
//! for that domain, since a clearance only works with the agent that earned it. Like Clear cookies, it works on the
//! jar file in the platform data directory.
use std::path::Path;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{tr, MsgKey};
use kuro_net::cookies::{ClearanceImportError, CookieJar};

use crate::pref_model::model::PreferenceItem;
use crate::pref_model::widgets::dialog;
use crate::settings_ui::rows::text;

/// Why an import was refused, as the dialog words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    /// The site is blank or has no host.
    Site,
    Clearance(ClearanceImportError),
    /// The jar file could not be written.
    Save,
}

impl ImportError {
    pub fn message(self) -> &'static str {
        match self {
            Self::Site => "Enter the address of the site",
            Self::Clearance(ClearanceImportError::Empty) => "Enter the cf_clearance value",
            Self::Clearance(ClearanceImportError::Malformed) => "The value must not contain spaces or semicolons",
            Self::Clearance(ClearanceImportError::InvalidUserAgent) => "Enter the browser's user agent",
            Self::Save => "The cookie could not be saved",
        }
    }
}

/// The site as a URL: a bare host such as `site.example` is read as `https://site.example`.
fn site_url(site: &str) -> Option<reqwest::Url> {
    let site = site.trim();
    let text = if site.contains("://") { site.to_owned() } else { format!("https://{site}") };
    reqwest::Url::parse(&text).ok().filter(|url| url.host_str().is_some())
}

/// Stores the pasted clearance for `site` in the jar file `file` with `user_agent` as the domain default.
pub fn import_at(file: &Path, site: &str, value: &str, user_agent: &str) -> Result<(), ImportError> {
    let url = site_url(site).ok_or(ImportError::Site)?;
    let jar = CookieJar::load(file.to_path_buf());
    jar.import_clearance(&url, value.trim(), user_agent).map_err(ImportError::Clearance)?;
    jar.flush().map_err(|err| {
        tracing::warn!(%err, "the imported cookie could not be saved");
        ImportError::Save
    })
}

fn entry(placeholder: &str) -> gtk::Entry {
    gtk::Entry::builder().placeholder_text(placeholder).activates_default(false).build()
}

/// Opens the import dialog over `parent`. OK is enabled once all three fields have text; a refused import keeps the
/// dialog open and says why.
pub fn show(parent: &impl IsA<gtk::Window>) {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let hint = gtk::Label::new(Some(&tr(MsgKey::cookie_import_hint)));
    hint.set_wrap(true);
    hint.set_xalign(0.0);
    body.append(&hint);
    let (site, value, agent) = (entry("Site address"), entry("cf_clearance"), entry("User agent"));
    for field in [&site, &value, &agent] {
        body.append(field);
    }
    let filled = {
        let fields = [site.clone(), value.clone(), agent.clone()];
        move || fields.iter().all(|f| !f.text().trim().is_empty())
    };
    let slot: Rc<std::cell::RefCell<Option<dialog::Dialog>>> = Rc::default();
    let refresh = {
        let (slot, filled) = (slot.clone(), filled.clone());
        move || {
            if let Some(dialog) = slot.borrow().as_ref() {
                dialog.set_confirm_enabled(filled());
            }
        }
    };
    for field in [&site, &value, &agent] {
        let refresh = refresh.clone();
        field.connect_changed(move |_| refresh());
    }
    let confirm = {
        let (site, value, agent) = (site.clone(), value.clone(), agent.clone());
        move |dialog: dialog::Dialog| {
            let result = crate::browse::cookies::jar_file()
                .ok_or(ImportError::Save)
                .and_then(|file| import_at(&file, &site.text(), &value.text(), &agent.text()));
            match result {
                Ok(()) => dialog.finish(),
                Err(err) => dialog.set_error(Some(err.message())),
            }
        }
    };
    let shown = dialog::show(parent, &tr(MsgKey::cookie_import_title), &body, Some(&tr(MsgKey::action_ok)), confirm, || {});
    *slot.borrow_mut() = Some(shown);
    refresh();
}

/// The Networking row that opens the dialog.
pub fn cookie_import_row() -> PreferenceItem {
    let mut row = text(&tr(MsgKey::cookie_import_title), None, true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(|| {
            let parent = gtk::Window::list_toplevels().into_iter().find_map(|w| w.downcast::<gtk::Window>().ok());
            if let Some(parent) = parent {
                show(&parent);
            }
        }));
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) Firefox/136.0";

    #[test]
    fn a_pasted_clearance_and_agent_are_stored_for_the_domain() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cookies.json");
        import_at(&file, "site.test", " abc123 ", UA).unwrap();
        let jar = CookieJar::load(file);
        let url = reqwest::Url::parse("https://www.site.test/x").unwrap();
        assert!(jar.get(&url).iter().any(|c| c.name() == "cf_clearance" && c.value() == "abc123"));
        assert_eq!(jar.domain_user_agent(&url).as_deref(), Some(UA));
    }

    #[test]
    fn a_refused_import_names_the_reason_and_stores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cookies.json");
        assert_eq!(import_at(&file, "", "abc", UA), Err(ImportError::Site));
        assert_eq!(import_at(&file, "site.test", "", UA), Err(ImportError::Clearance(ClearanceImportError::Empty)));
        assert_eq!(import_at(&file, "site.test", "a;b", UA), Err(ImportError::Clearance(ClearanceImportError::Malformed)));
        assert_eq!(import_at(&file, "site.test", "a b", UA), Err(ImportError::Clearance(ClearanceImportError::Malformed)));
        assert_eq!(import_at(&file, "site.test", "abc", " "), Err(ImportError::Clearance(ClearanceImportError::InvalidUserAgent)));
        assert!(!file.exists());
        assert!(ImportError::Site.message().contains("site"));
    }

    #[test]
    fn the_row_carries_the_catalogue_title_and_is_tappable() {
        let PreferenceItem::Text { common, on_click, .. } = cookie_import_row() else { panic!("a text row") };
        assert_eq!(common.title, "Import Cloudflare cookie");
        assert!(common.enabled && on_click.is_some());
        assert!(tr(MsgKey::cookie_import_hint).contains("cf_clearance"));
    }
}
