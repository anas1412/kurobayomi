//! Category, source and repo creators (T-21a-015..018).

use std::collections::{BTreeMap, HashSet};

use kuro_data::category::Category;
use kuro_data::extension_repo::ExtensionRepo;

use crate::creator::SourceRegistry;
use crate::schema::{BackupAnime, BackupAnimeSource, BackupCategory, BackupExtensionRepos, BackupManga, BackupSource};

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
    fn repos_map_fields() {
        let repos = vec![ExtensionRepo { base_url: "b".into(), name: "n".into(), short_name: None, website: "w".into(), signing_key_fingerprint: "f".into() }];
        let got = manga_repos(&repos);
        assert_eq!(got[0].signing_key_fingerprint, "f");
        assert_eq!(got[0].short_name, None);
    }
}
