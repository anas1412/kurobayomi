//! The settings screens this area builds for the app (T-47b-058). `settings::body` hands each id here; an id this area
//! does not own gives `None` and the caller keeps its own builder.

use std::rc::Rc;
use std::sync::Arc;

use kuro_data::extension_repo::ExtensionRepoRepo;
use kuro_data::extension_store::ExtensionStoreRepo;

use super::net::BlockingHttp;
use super::spec_convert::Registry;
use super::stores_backend::{RepoOps, StoreOps};
use super::stores_page::{self, Texts};
use super::{backup_pages, debug_info, language, page, shell};
use crate::screens::Built;
use crate::watch::bind_watch;

/// The screens of this area: the settings shell, the language picker, the two extension lists, the two backup pages,
/// Debug info, and Data and storage (built from the spec so the onboarding restore can highlight its backup rows).
pub const IDS: [&str; 8] = [
    "settings",
    "settings_app_language",
    "settings_anime_extension_stores",
    "settings_manga_extension_repos",
    "settings_create_backup",
    "settings_restore_backup",
    "settings_debug_info",
    "settings_data_storage",
];

/// The screen for `id`, or `None` when this area does not build it.
pub fn body(id: &str) -> Option<Built> {
    match id {
        "settings" => shell::body(),
        "settings_app_language" => language::page(language::pref(crate::data::app().base.store())),
        "settings_anime_extension_stores" => anime_extension_stores(),
        "settings_manga_extension_repos" => manga_extension_repos(),
        "settings_create_backup" => backup_pages::create_page(),
        "settings_restore_backup" => backup_pages::restore_page(),
        "settings_debug_info" => debug_info::page(),
        "settings_data_storage" => spec_page(id).map(with_storage_rows),
        _ => None,
    }
}

/// A page made from the spec's groups through the preference model, so its rows take the highlight request.
pub fn spec_page(id: &str) -> Option<Built> {
    let registry = crate::ui::with_spec(Registry::new);
    let store = crate::data::app().base.store().clone();
    let navigate: Rc<dyn Fn(&str)> = Rc::new(|target| crate::ui::open(target));
    let tree = registry.build(id, &store, &navigate)?;
    let body = page::body(&tree);
    Some(Built { content: body.widget, end: crate::ui::toolbar(&crate::ui::screen(id)) })
}

/// Data and storage with its usage row replaced by one real row per mounted storage root (T-22a-035).
fn with_storage_rows(built: Built) -> Built {
    super::storage_rows::replace_usage_row(&built.content, &super::storage_rows::rows());
    built
}

fn anime_extension_stores() -> Option<Built> {
    let db = crate::data::app().anime_db.clone();
    let ops = Arc::new(StoreOps::new(ExtensionStoreRepo::new(db.clone()), BlockingHttp::shared()));
    let (built, _page) = stores_page::build(ops, Texts::stores(), move |push| {
        bind_watch(kuro_domain::extension_store::subscribe_all(&db), move |published| match published {
            Ok(stores) => push(stores.as_ref().clone()),
            Err(err) => tracing::warn!(%err, "the extension store list could not be read"),
        });
    });
    Some(built)
}

fn manga_extension_repos() -> Option<Built> {
    let repos = ExtensionRepoRepo::new(crate::data::app().manga_db.clone());
    let ops = Arc::new(RepoOps::new(repos.clone(), BlockingHttp::shared()));
    let (built, _page) = stores_page::build(ops, Texts::repos(), move |push| {
        bind_watch(repos.watch_all(), move |published| match published {
            Ok(list) => push(list.as_ref().clone()),
            Err(err) => tracing::warn!(%err, "the extension repo list could not be read"),
        });
    });
    Some(built)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_47b_058_every_id_this_area_builds_is_a_spec_screen() {
        crate::ui::with_spec(|spec| {
            for id in IDS {
                assert!(spec.screen(id).is_some(), "{id}");
            }
        });
    }

    #[test]
    fn t_47b_058_the_pages_have_the_titles_of_their_spec_screens() {
        let titles = |id: &str| crate::ui::screen(id).title;
        assert_eq!(titles("settings_anime_extension_stores"), "Extension stores");
        assert_eq!(titles("settings_manga_extension_repos"), "Extension repos");
        assert_eq!(titles("settings_app_language"), "App language");
        assert_eq!(titles("settings_create_backup"), "Create backup");
        assert_eq!(titles("settings_restore_backup"), "Restore backup");
        assert_eq!(titles("settings_debug_info"), "Debug info");
    }

    #[test]
    fn t_47b_058_the_spec_page_registry_knows_data_and_storage() {
        let registry = crate::ui::with_spec(Registry::new);
        assert!(registry.contains("settings_data_storage"));
        assert!(registry.contains("settings_debug_info"));
    }
}
