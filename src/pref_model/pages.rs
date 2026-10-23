//! The settings screens this area builds for the app (T-47b-058). `settings::body` hands each id here; an id this area
//! does not own gives `None` and the caller keeps its own builder.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gtk::glib;
use kuro_prefs::pref::Pref;
use kuro_prefs::source::SourcePrefs;

use super::spec_convert::Registry;
use super::stores::RepoLink;
use super::stores_backend::RepoOps;
use super::stores_page::{self, Texts};
use super::{backup_pages, debug_info, language, page, shell};
use crate::screens::Built;

/// The screens of this area: the settings shell, the language picker, the one repository list, the two backup pages,
/// Debug info, and Data and storage (built from the spec so the empty library's restore can highlight its backup rows).
pub const IDS: [&str; 7] = [
    "settings",
    "settings_app_language",
    "settings_extension_repos",
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
        "settings_extension_repos" => extension_repos(),
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

/// The one repository list of both kinds of source. A `kurobayomi://add-repo` link parks its address for this page
/// (T-47b-017) and opens the add dialog with it; the user still confirms the add.
fn extension_repos() -> Option<Built> {
    let links = SourcePrefs::new(crate::data::app().base.store().clone()).manga_extension_repos;
    let ops = Arc::new(RepoOps::new(links.clone()));
    let (built, page) = stores_page::build(ops, Texts::repos(), move |push| follow_links(links, push));
    if let Some(url) = crate::shell::uri::take_repo_url() {
        page.open_deep_link(&url);
    } else if crate::browse::mount::take_add_request() {
        page.open_add();
    }
    Some(built)
}

/// Pushes the saved repo links now, and again each time they change. The preference is polled on the main loop, as
/// `bind` does for widgets.
fn follow_links(links: Pref<BTreeSet<String>>, push: Box<dyn Fn(Vec<RepoLink>)>) {
    let watch = links.watch();
    push(repo_rows(watch.latest()));
    glib::timeout_add_local(Duration::from_millis(250), move || {
        if let Some(set) = watch.try_changed() {
            push(repo_rows(set));
        }
        glib::ControlFlow::Continue
    });
}

fn repo_rows(set: BTreeSet<String>) -> Vec<RepoLink> {
    set.into_iter().map(RepoLink).collect()
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
        assert_eq!(titles("settings_extension_repos"), "Repositories");
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
