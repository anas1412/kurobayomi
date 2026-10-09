//! Category, source, extension store and repo and custom button creators (T-21a-015..018).

use std::collections::{BTreeMap, HashSet};

use kuro_data::category::Category;
use kuro_data::extension_repo::ExtensionRepo;
use kuro_db::statements_anime::{CustomButton, ExtensionStore};

use crate::creator::SourceRegistry;
use crate::schema::{BackupAnime, BackupAnimeSource, BackupCategory, BackupCustomButtons, BackupExtensionRepos, BackupExtensionStore, BackupManga, BackupSource};

/// Categories other than the system category (id 0), with name, order, id and flags. Duplicate orders are logged
/// and still written (E-15).
pub fn categories(cats: &[Category]) -> Vec<BackupCategory> {
    let mut seen: BTreeMap<i64, usize> = BTreeMap::new();
    for c in cats.iter().filter(|c| c.id != 0) {
        *seen.entry(c.order).or_default() += 1;
    }
    for (order, n) in seen.iter().filter(|(_, n)| **n > 1) {
        tracing::warn!(order, count = n, "duplicate category order in backup");
    }
    cats.iter()
        .filter(|c| c.id != 0)
        .map(|c| BackupCategory { name: c.name.clone(), order: c.order, id: c.id, flags: c.flags })
        .collect()
}

/// Distinct source ids in first-seen order (E-42).
fn distinct(ids: impl Iterator<Item = i64>) -> Vec<i64> {
    let mut seen = HashSet::new();
    ids.filter(|id| seen.insert(*id)).collect()
}

/// Manga sources referenced by the written manga entries (B-08d).
pub fn manga_sources(entries: &[BackupManga], reg: &dyn SourceRegistry) -> Vec<BackupSource> {
    distinct(entries.iter().map(|e| e.source))
        .into_iter()
        .map(|id| BackupSource { name: reg.name_of(id), source_id: id })
        .collect()
}

/// Anime sources referenced by the written anime entries (B-08d).
pub fn anime_sources(entries: &[BackupAnime], reg: &dyn SourceRegistry) -> Vec<BackupAnimeSource> {
    distinct(entries.iter().map(|e| e.source))
        .into_iter()
        .map(|id| BackupAnimeSource { name: reg.name_of(id), source_id: id })
        .collect()
}

pub fn manga_repos(repos: &[ExtensionRepo]) -> Vec<BackupExtensionRepos> {
    repos
        .iter()
        .map(|r| BackupExtensionRepos {
            base_url: r.base_url.clone(),
            name: r.name.clone(),
            short_name: r.short_name.clone(),
            website: r.website.clone(),
            signing_key_fingerprint: r.signing_key_fingerprint.clone(),
        })
        .collect()
}

pub fn anime_stores(stores: &[ExtensionStore]) -> Vec<BackupExtensionStore> {
    stores
        .iter()
        .map(|s| BackupExtensionStore {
            index_url: s.index_url.clone(),
            name: s.name.clone(),
            badge_label: Some(s.badge_label.clone()),
            contact_website: s.contact_website.clone(),
            signing_key: s.signing_key.clone(),
            contact_discord: s.contact_discord.clone(),
            is_legacy: Some(s.is_legacy),
            extension_list_url: s.extension_list_url.clone(),
        })
        .collect()
}

/// Custom buttons without the row id (restore assigns new ids, B-08f).
pub fn custom_buttons(buttons: &[CustomButton]) -> Vec<BackupCustomButtons> {
    buttons
        .iter()
        .map(|b| BackupCustomButtons {
            name: b.name.clone(),
            is_favorite: b.is_favorite,
            sort_index: b.sort_index,
            content: b.content.clone(),
            long_press_content: b.long_press_content.clone(),
            on_startup: b.on_startup.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Reg;
    impl SourceRegistry for Reg {
        fn is_installed(&self, id: i64) -> bool {
            id == 1
        }
        fn name_of(&self, id: i64) -> String {
            match id {
                1 => "Installed".into(),
                2 => "Stub".into(),
                _ => String::new(),
            }
        }
        fn stub_label(&self, id: i64) -> String {
            id.to_string()
        }
    }

    fn manga(source: i64) -> BackupManga {
        BackupManga { source, url: "u".into(), ..Default::default() }
    }

    #[test]
    fn source_list_distinct_first_seen() {
        let entries = vec![manga(2), manga(1), manga(2)];
        let got: Vec<i64> = manga_sources(&entries, &Reg).iter().map(|s| s.source_id).collect();
        assert_eq!(got, vec![2, 1]);
    }

    #[test]
    fn stub_name_empty_when_unknown_and_installed_uses_display_name() {
        let entries = vec![manga(1), manga(9)];
        let got = manga_sources(&entries, &Reg);
        assert_eq!(got[0].name, "Installed");
        assert_eq!(got[1].name, "", "unknown source id is written with an empty name (E-17)");
    }

    #[test]
    fn categories_skip_system_and_keep_duplicates() {
        let cats = vec![
            Category { id: 0, name: "Default".into(), order: 0, flags: 0, hidden: false },
            Category { id: 3, name: "A".into(), order: 1, flags: 4, hidden: false },
            Category { id: 4, name: "B".into(), order: 1, flags: 0, hidden: false },
        ];
        let got = categories(&cats);
        assert_eq!(got.len(), 2, "system category excluded, duplicate order still written");
        assert_eq!(got[0], BackupCategory { name: "A".into(), order: 1, id: 3, flags: 4 });
        assert!(categories(&[]).is_empty());
    }

    #[test]
    fn repos_and_stores_map_fields() {
        let repos = vec![ExtensionRepo { base_url: "b".into(), name: "n".into(), short_name: None, website: "w".into(), signing_key_fingerprint: "f".into() }];
        let got = manga_repos(&repos);
        assert_eq!(got[0].signing_key_fingerprint, "f");
        assert_eq!(got[0].short_name, None);
        let stores = vec![ExtensionStore { index_url: "i".into(), name: "n".into(), badge_label: "B".into(), signing_key: "k".into(), contact_website: "w".into(), contact_discord: None, is_legacy: false, extension_list_url: Some("x".into()), resolved_index_url: "i".into() }];
        let got = anime_stores(&stores);
        assert_eq!(got[0].is_legacy, Some(false));
        assert_eq!(got[0].extension_list_url.as_deref(), Some("x"));
        assert_eq!(got[0].contact_discord, None);
    }

    #[test]
    fn custom_buttons_roundtrip_without_id() {
        let buttons = vec![
            CustomButton { id: 9, name: "a".into(), is_favorite: true, sort_index: 1, content: "c".into(), long_press_content: "l".into(), on_startup: String::new() },
        ];
        let got = custom_buttons(&buttons);
        assert_eq!(got[0].on_startup, "");
        assert_eq!(got[0].sort_index, 1);
    }
}
