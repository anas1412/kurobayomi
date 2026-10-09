//! Localisation for Kurobayomi: the Fluent catalogue, locale resolution, and typed message keys.
//!
//! Plan 51-i18n. `tr`, `trf` and `tr_plural` return text for a [`MsgKey`]. The catalogue is
//! built once (see [`init`]). Before `init` runs, English is used. GTK is not a dependency here.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use fluent_bundle::{FluentArgs, FluentValue};
use tracing::{debug, warn};

pub mod catalog;
pub mod date;
pub mod import;
pub mod locale;
pub mod names;
pub mod notice;
pub mod picker;
pub mod source_names;
pub mod time;

mod keys {
    include!(concat!(env!("OUT_DIR"), "/keys.rs"));
}

mod sources {
    include!(concat!(env!("OUT_DIR"), "/locales.rs"));
}

pub use keys::{MSG_COUNT, MsgKey};
pub use locale::LocaleInfo;

use catalog::Catalog;

/// A message argument. `Str` is for `%s` and `%c`, `Int` for `%d`, `Dec` for `%.Nf`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arg<'a> {
    Str(&'a str),
    Int(i64),
    Dec(f64),
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();
static WARNED: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);

/// Sets the language for the whole process. `pref` is the `app_language` preference (empty for
/// Default). `system` is the list from `glib::language_names()`, in priority order. A second call,
/// or a call after the first `tr`, is ignored with a warning.
pub fn init(pref: &str, system: &[String]) {
    let offered = offered_locales();
    let chain = locale::resolve(pref, system, &offered);
    install(&CATALOG, &chain, sources::LOCALE_SOURCES);
}

/// The language tags the catalogue ships, for the language picker (`picker::rows`,
/// `picker::settings_subtitle`) and for [`init`].
pub fn offered_locales() -> Vec<&'static str> {
    sources::LOCALE_SOURCES
        .iter()
        .map(|(tag, _)| *tag)
        .collect()
}

fn install(slot: &OnceLock<Catalog>, chain: &[&'static str], sources: &[(&str, &str)]) -> bool {
    if slot.get().is_some() {
        warn!("[i18n] init ignored: the catalogue is already in use");
        return false;
    }
    if slot.set(Catalog::build(chain, sources)).is_err() {
        warn!("[i18n] init ignored: the catalogue was set concurrently");
        return false;
    }
    true
}

fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| Catalog::build(&["en"], sources::LOCALE_SOURCES))
}

/// The active locale. English before `init`.
pub fn current_locale() -> LocaleInfo {
    catalog().info()
}

/// Plain message (`<string>`).
///
/// An unknown key does not compile, because [`MsgKey`] is generated from the English file:
///
/// ```compile_fail
/// let _ = kuro_i18n::tr(kuro_i18n::MsgKey::no_such_message);
/// ```
pub fn tr(key: MsgKey) -> String {
    format_key(key, None)
}

/// Message with named arguments `arg1`, `arg2`, ... (`<string>` with `%s`, `%d`, `%.Nf`).
pub fn trf(key: MsgKey, args: &[(&str, Arg<'_>)]) -> String {
    format_key(key, Some(&to_args(args, None)))
}

/// Plural message (`<plurals>`). `count` selects the CLDR category and is available as `$count`.
/// Other arguments are `arg1`, `arg2`, ... as for [`trf`].
pub fn tr_plural(key: MsgKey, count: i64, args: &[(&str, Arg<'_>)]) -> String {
    format_key(key, Some(&to_args(args, Some(count))))
}

fn to_args<'a>(args: &[(&str, Arg<'a>)], count: Option<i64>) -> FluentArgs<'a> {
    let mut out = FluentArgs::with_capacity(args.len() + 1);
    if let Some(count) = count {
        out.set("count", FluentValue::from(count));
    }
    for (name, arg) in args {
        let value = match *arg {
            Arg::Str(s) => FluentValue::from(s),
            Arg::Int(i) => FluentValue::from(i),
            Arg::Dec(d) => FluentValue::from(d),
        };
        out.set(name.to_string(), value);
    }
    out
}

fn format_key(key: MsgKey, args: Option<&FluentArgs>) -> String {
    let id = key.id();
    match catalog().format(id, args) {
        Some((text, errors)) => {
            for error in errors {
                debug!("[i18n] {id}: {error}");
            }
            text
        }
        None => {
            if warn_once(id) {
                warn!("[i18n] missing message {id}");
            }
            id.to_string()
        }
    }
}

/// True the first time `id` is reported.
fn warn_once(id: &'static str) -> bool {
    let mut guard = WARNED
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.get_or_insert_with(HashSet::new).insert(id)
}

#[cfg(test)]
mod import_tests;
#[cfg(test)]
mod tests;
