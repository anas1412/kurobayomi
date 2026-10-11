//! The anime side of the source directory (T-41a-027, T-41a-031, T-41a-036, T-41b-014). The shared pages ask a
//! `SourceDirectory` for the installed sources and their loaders; this one answers for the anime packages. The list of
//! sources is real: it is what the extension host published. A source's runnable instance is not: the app has no anime
//! source registry yet, so the instances come through [`AnimeInstances`], which the registry installs once it exists.
//! Until then every anime source is listed and none can be asked, and the pages say so the way they do for a stub.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use kuro_data::anime::AnimeRepo;
use kuro_domain::source::model::Source;
use kuro_extensions::anime::model::Buckets;
use kuro_source::SourcePref;
use kuro_source::anime::AnimeSource;
use kuro_source::filter_list::FilterList;

use super::loader::{AnimeLoader, Lookup, repo_lookup};
use crate::browse::directory::{DirectorySource, SourceDirectory};
use crate::browse::listing::PageLoader;

/// The runnable anime sources the app holds.
pub trait AnimeInstances: Send + Sync + 'static {
    /// The source with this id, `None` when no instance of it is loaded.
    fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>>;
    /// The settings rows of a configurable source (`ConfigurableSource::preferences`); `None` for any other.
    fn preferences(&self, _id: i64) -> Option<Vec<SourcePref>> {
        None
    }
}

static INSTANCES: Mutex<Option<Arc<dyn AnimeInstances>>> = Mutex::new(None);

/// Installs the instances the pages read. A second call replaces the first.
pub fn install_instances(instances: Arc<dyn AnimeInstances>) {
    *INSTANCES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(instances);
}

/// The installed instances, if the app has any.
pub fn instances() -> Option<Arc<dyn AnimeInstances>> {
    INSTANCES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

/// The sources of the installed anime packages, each with the package it came from and its site. A source id that two
/// packages declare is listed once.
pub fn sources_of(buckets: &Buckets) -> Vec<DirectorySource> {
    let mut seen = BTreeSet::new();
    buckets
        .updates
        .iter()
        .chain(&buckets.installed)
        .flat_map(|ext| ext.sources.iter().map(move |source| (ext, source)))
        .filter(|(_, source)| seen.insert(source.id))
        .map(|(ext, source)| DirectorySource {
            source: Source::new(source.id, &source.name, &source.lang),
            package: ext.pkg.clone(),
            home_url: Some(source.base_url.clone()).filter(|url| !url.is_empty()),
        })
        .collect()
}

/// A lookup for the library rows of one source's listings.
pub type LookupFor = Arc<dyn Fn(i64) -> Lookup + Send + Sync>;

/// The installed anime sources as a `SourceDirectory`. It is a snapshot taken when a page is built.
pub struct AnimeDirectory {
    sources: Vec<DirectorySource>,
    instances: Option<Arc<dyn AnimeInstances>>,
    lookup: LookupFor,
}

impl AnimeDirectory {
    pub fn new(sources: Vec<DirectorySource>, instances: Option<Arc<dyn AnimeInstances>>, lookup: LookupFor) -> Self {
        Self { sources, instances, lookup }
    }

    /// The directory of the app now: what the extension host published, the installed instances, and `repo` as the library.
    pub fn current(repo: AnimeRepo) -> Self {
        let mut sources = sources_of(&crate::ext_host::anime_lists());
        sources.extend(crate::recipe_sources::anime_rows());
        sources.extend(crate::browse::builtin::anime_rows());
        Self::new(sources, crate::browse::builtin::with_anime(instances()), Arc::new(move |id| repo_lookup(repo.clone(), id)))
    }

    /// The runnable source of an id, for a page that needs more of it than its loader (its latest flag).
    pub fn instance(&self, source_id: i64) -> Option<Arc<dyn AnimeSource>> {
        self.instances.as_ref().and_then(|instances| instances.source(source_id))
    }
}

impl SourceDirectory for AnimeDirectory {
    fn sources(&self) -> Vec<DirectorySource> {
        self.sources.clone()
    }

    fn loader(&self, source_id: i64) -> Option<Arc<dyn PageLoader>> {
        let source = self.instance(source_id)?;
        Some(Arc::new(AnimeLoader::new(source, (self.lookup)(source_id))))
    }

    fn filters(&self, source_id: i64) -> Option<FilterList> {
        self.instance(source_id).map(|source| source.filter_list())
    }

    fn preferences(&self, source_id: i64) -> Option<Vec<SourcePref>> {
        self.instances.as_ref().and_then(|instances| instances.preferences(source_id)).filter(|rows| !rows.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use kuro_extensions::anime::abi::AbiVersion;
    use kuro_extensions::anime::model::{AnimeSourceRef, Installed, SignerDigest};

    use super::*;
    use crate::browse::anime::fixtures::{FakeSource, anime, lookup, page};

    fn installed(pkg: &str, sources: Vec<AnimeSourceRef>) -> Installed {
        Installed {
            name: pkg.into(),
            pkg: pkg.into(),
            version_name: "1".into(),
            version_code: 1,
            abi: AbiVersion { major: 1, minor: 0 },
            lang: "en".into(),
            nsfw: false,
            torrent: false,
            sources,
            icon: Some(PathBuf::from("/icons/a.png")),
            signer: SignerDigest(String::new()),
            has_update: false,
            is_obsolete: false,
            store: None,
        }
    }

    fn source_ref(id: i64, name: &str, base: &str) -> AnimeSourceRef {
        AnimeSourceRef { id, name: name.into(), lang: "en".into(), base_url: base.into() }
    }

    struct Held(Arc<FakeSource>, Vec<SourcePref>);

    impl AnimeInstances for Held {
        fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>> {
            (id == self.0.id()).then(|| Arc::clone(&self.0) as Arc<dyn AnimeSource>)
        }
        fn preferences(&self, id: i64) -> Option<Vec<SourcePref>> {
            (id == self.0.id()).then(|| self.1.clone())
        }
    }

    #[test]
    fn the_sources_of_updates_and_installed_packages_are_listed_once_with_their_package_and_site() {
        let buckets = Buckets {
            updates: vec![installed("eu.up", vec![source_ref(1, "One", "https://one.test")])],
            installed: vec![installed("eu.in", vec![source_ref(1, "One again", ""), source_ref(2, "Two", "")])],
            ..Buckets::default()
        };
        let listed = sources_of(&buckets);
        assert_eq!(listed.iter().map(|s| (s.source.id, s.package.as_str())).collect::<Vec<_>>(), [(1, "eu.up"), (2, "eu.in")], "a repeated id is listed once");
        assert_eq!(listed[0].home_url.as_deref(), Some("https://one.test"));
        assert_eq!(listed[1].home_url, None, "an empty site is no site");
        assert!(sources_of(&Buckets::default()).is_empty());
    }

    #[test]
    fn without_instances_every_source_is_listed_and_none_can_be_asked() {
        let (lookup, _) = lookup(&[]);
        let directory = AnimeDirectory::new(sources_of(&Buckets { installed: vec![installed("eu.in", vec![source_ref(2, "Two", "")])], ..Buckets::default() }), None, Arc::new(move |_| Arc::clone(&lookup)));
        assert_eq!(directory.sources().len(), 1);
        assert!(directory.loader(2).is_none() && directory.filters(2).is_none() && directory.preferences(2).is_none());
    }

    #[test]
    fn an_installed_instance_gives_its_loader_filters_and_settings_rows() {
        let source = FakeSource::new(7, vec![Ok(page(vec![anime("/a", "A")], false))]);
        let rows = vec![SourcePref::Switch { key: "hd".into(), title: "HD".into(), summary: None, default: false }];
        let (lookup, _) = lookup(&[]);
        let directory = AnimeDirectory::new(Vec::new(), Some(Arc::new(Held(Arc::clone(&source), rows.clone()))), Arc::new(move |_| Arc::clone(&lookup)));
        assert!(directory.loader(7).is_some() && directory.loader(8).is_none(), "only the held instance has a loader");
        assert!(directory.filters(7).is_some());
        assert_eq!(directory.preferences(7), Some(rows));
        assert_eq!(directory.preferences(8), None);
        let empty = AnimeDirectory::new(Vec::new(), Some(Arc::new(Held(source, Vec::new()))), Arc::new(|_| unreachable!("no lookup is made")));
        assert_eq!(empty.preferences(7), None, "a source with no rows is not configurable");
    }

    #[test]
    fn the_installed_instances_are_read_back_and_replaced() {
        let source = FakeSource::new(7, Vec::new());
        install_instances(Arc::new(Held(Arc::clone(&source), Vec::new())));
        assert!(instances().is_some_and(|held| held.source(7).is_some() && held.source(8).is_none()));
        *INSTANCES.lock().expect("lock") = None;
        assert!(instances().is_none());
    }
}
