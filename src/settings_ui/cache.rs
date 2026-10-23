//! The cache rows of Data and storage: the one size limit of every cache and Clear cache (`crate::cache_limit`), and the
//! messages. Library covers and downloads are outside the caches, so a clear never touches them.
use kuro_i18n::{Arg, MsgKey, tr, trf};
use std::io;
use std::rc::Rc;

use kuro_prefs::Prefs;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::{self, text};
use crate::settings_ui::usage::si_size;

/// The toast after a clear: `Cache cleared, <n> files deleted`, or the error text.
pub fn cleared_message(result: &io::Result<usize>) -> String {
    match result {
        Ok(deleted) => trf(MsgKey::cache_deleted, &[("arg1", Arg::Int(i64::try_from(*deleted).unwrap_or(i64::MAX)))]),
        Err(_) => tr(MsgKey::cache_delete_error).to_owned(),
    }
}

/// The size limit row: the choices of `cache_size_mb`, each labelled with its size. The background trim reads the new
/// value within seconds.
pub fn size_row(prefs: &Prefs) -> PreferenceItem {
    rows::ranged_choice(&tr(MsgKey::pref_cache_size), &prefs.storage.cache_size_mb, |mb| si_size(crate::cache_limit::mb_to_bytes(mb)), true)
}

/// The "Clear cache" row, subtitled `Used: <size>`. Tapping empties every cache, shows the toast and runs `changed` so
/// the page shows the new size.
pub fn clear_row(changed: Rc<dyn Fn()>) -> PreferenceItem {
    let dirs = crate::cache_limit::app_dirs();
    let used = crate::cache_limit::used(&dirs);
    let mut row = text(&tr(MsgKey::pref_clear_cache), Some(&trf(MsgKey::used_cache, &[("arg1", Arg::Str(&si_size(used)))])), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(move || {
            let result = if dirs.is_empty() { Err(io::Error::other("no cache directory")) } else { crate::cache_limit::clear(&dirs) };
            if let Err(err) = &result {
                tracing::warn!(%err, "clearing the cache failed");
            }
            crate::toast::notify(cleared_message(&result));
            changed();
        }));
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_toast_reports_the_count_or_the_error() {
        assert_eq!(cleared_message(&Ok(3)), "Cache cleared, 3 files deleted");
        assert_eq!(cleared_message(&Err(io::Error::other("denied"))), "Error occurred while clearing");
    }

    #[test]
    fn the_rows_show_the_used_size_and_the_limit_choices() {
        let row = clear_row(Rc::new(|| {}));
        assert_eq!(row.title(), "Clear cache");
        let PreferenceItem::Text { common, on_click, .. } = row else { panic!("text row") };
        assert!(common.subtitle.as_deref().is_some_and(|s| s.starts_with("Used: ")));
        assert!(on_click.is_some());
        let size = size_row(&Prefs::with_store(kuro_prefs::store::PrefStore::in_memory(), std::sync::Arc::new(kuro_prefs::memory::MemorySecrets::default())));
        assert_eq!(size.title(), "Cache size limit");
    }
}
