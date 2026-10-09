//! Extension lists by type, extension languages and the source rows of one extension (`GetMangaExtensionsByType.kt`,
//! `GetMangaExtensionLanguages.kt`, `GetExtensionSources.kt`; T-10a-023). Pure functions over plain rows: the caller
//! fills them from the extension manager, so this module does not depend on the extension crates.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use kuro_prefs::source::SourcePrefs;
use tokio::sync::watch;

use crate::app::rx::{combine3, pref_stream};
use crate::source::languages::lang_cmp;

/// One source a package declares (`MangaSource` of an extension).
#[derive(Clone, Debug, PartialEq)]
pub struct ExtSource {
    pub id: i64,
    pub name: String,
    pub lang: String,
}

/// One extension as the lists show it. Installed, untrusted and available entries share this shape.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtRow {
    pub pkg: String,
    pub name: String,
    pub lang: String,
    pub nsfw: bool,
    /// Installed only: the version is older than the platform allows.
    pub obsolete: bool,
    /// Installed only: a newer version is available.
    pub has_update: bool,
    pub sources: Vec<ExtSource>,
}

/// The four lists of the extension screen.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionsByType {
    pub updates: Vec<ExtRow>,
    pub installed: Vec<ExtRow>,
    pub untrusted: Vec<ExtRow>,
    pub available: Vec<ExtRow>,
}

/// What [`compute_extensions_by_type`] reads.
pub struct ByTypeInput<'a> {
    pub installed: &'a [ExtRow],
    pub untrusted: &'a [ExtRow],
    pub available: &'a [ExtRow],
    pub enabled_languages: &'a BTreeSet<String>,
    pub show_nsfw: bool,
    /// Manga keeps an available extension with no sources when its own language is enabled. Anime drops it.
    pub keep_sourceless: bool,
}

/// `String.CASE_INSENSITIVE_ORDER` for the names, approximated by the lower-case forms.
fn name_cmp(a: &str, b: &str) -> Ordering {
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// Splits the lists as the screen shows them (`GetMangaExtensionsByType.subscribe`). Installed entries are filtered by
/// NSFW, sorted obsolete-first then by name, and split into the updates and the rest. Untrusted entries are sorted by
/// name only. An available entry is dropped when its package is installed or untrusted, or when it is NSFW and NSFW
/// sources are hidden. Every enabled-language source of an available entry becomes its own row, keyed `<pkg>-<id>`.
pub fn compute_extensions_by_type(input: &ByTypeInput<'_>) -> ExtensionsByType {
    let mut installed: Vec<ExtRow> =
        input.installed.iter().filter(|e| input.show_nsfw || !e.nsfw).cloned().collect();
    installed.sort_by(|a, b| (!a.obsolete).cmp(&!b.obsolete).then_with(|| name_cmp(&a.name, &b.name)));
    let (updates, installed): (Vec<ExtRow>, Vec<ExtRow>) = installed.into_iter().partition(|e| e.has_update);

    let mut untrusted = input.untrusted.to_vec();
    untrusted.sort_by(|a, b| name_cmp(&a.name, &b.name));

    let mut available: Vec<ExtRow> = Vec::new();
    for ext in input.available.iter().filter(|ext| {
        !input.installed.iter().any(|i| i.pkg == ext.pkg)
            && !input.untrusted.iter().any(|u| u.pkg == ext.pkg)
            && (input.show_nsfw || !ext.nsfw)
    }) {
        if ext.sources.is_empty() {
            if input.keep_sourceless && input.enabled_languages.contains(&ext.lang) {
                available.push(ext.clone());
            }
            continue;
        }
        for src in ext.sources.iter().filter(|s| input.enabled_languages.contains(&s.lang)) {
            available.push(ExtRow {
                name: src.name.clone(),
                lang: src.lang.clone(),
                pkg: format!("{}-{}", ext.pkg, src.id),
                sources: vec![src.clone()],
                ..ext.clone()
            });
        }
    }
    available.sort_by(|a, b| name_cmp(&a.name, &b.name));

    ExtensionsByType { updates, installed, untrusted, available }
}

/// The distinct languages of the available extensions: each source's language, or the extension's own when it has no
/// sources. Enabled languages come first, then [`lang_cmp`] with the given display names.
pub fn extension_languages(
    available: &[ExtRow],
    enabled: &BTreeSet<String>,
    display: &dyn Fn(&str) -> String,
) -> Vec<String> {
    let mut langs: Vec<String> = Vec::new();
    for ext in available {
        let found: Vec<String> = if ext.sources.is_empty() {
            vec![ext.lang.clone()]
        } else {
            ext.sources.iter().map(|s| s.lang.clone()).collect()
        };
        for lang in found {
            if !langs.contains(&lang) {
                langs.push(lang);
            }
        }
    }
    langs.sort_by(|a, b| (!enabled.contains(a)).cmp(&!enabled.contains(b)).then_with(|| lang_cmp(a, b, display)));
    langs
}

/// Whether an extension's source rows show their own name as the label (`labelAsName`): the package has more than one
/// source, and they do not all share one name.
pub fn label_as_name(sources: &[ExtSource]) -> bool {
    let multi = sources.len() > 1;
    let single_name = multi && sources.iter().map(|s| s.name.as_str()).collect::<BTreeSet<_>>().len() == 1;
    multi && !single_name
}

/// A source is shown unless its id is in the hidden set (stored as text, as the preference stores it).
pub fn source_enabled(id: i64, hidden: &BTreeSet<String>) -> bool {
    !hidden.contains(&id.to_string())
}

/// One source row of an extension's detail screen (`GetExtensionSources`): the source, whether it is shown, and whether
/// the row labels itself by name.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionSourceRow {
    pub source: ExtSource,
    pub enabled: bool,
    pub label_as_name: bool,
}

/// The rows of `sources` against the current hidden set.
pub fn extension_source_rows(sources: &[ExtSource], hidden: &BTreeSet<String>) -> Vec<ExtensionSourceRow> {
    let label = label_as_name(sources);
    sources
        .iter()
        .map(|source| ExtensionSourceRow { source: source.clone(), enabled: source_enabled(source.id, hidden), label_as_name: label })
        .collect()
}

/// Recomputes `f` whenever `input` changes and publishes the result only when it differs from the last one.
fn derive<I, O>(mut input: watch::Receiver<I>, f: impl Fn(&I) -> O + Send + 'static) -> watch::Receiver<O>
where
    I: Send + Sync + 'static,
    O: PartialEq + Send + Sync + 'static,
{
    let (tx, rx) = watch::channel(f(&input.borrow_and_update()));
    tokio::spawn(async move {
        while input.changed().await.is_ok() {
            let value = f(&input.borrow_and_update());
            if tx.is_closed() {
                break;
            }
            tx.send_if_modified(|current| {
                if *current == value {
                    false
                } else {
                    *current = value;
                    true
                }
            });
        }
    });
    rx
}

/// The source rows of one extension as a stream: they are recomputed, and published when different, each time the hidden
/// set changes, so `enabled` re-emits (`GetExtensionSources.subscribe`). `anime` picks the anime or manga hidden set.
/// Must run inside a tokio runtime.
pub fn subscribe_extension_sources(
    prefs: &SourcePrefs,
    anime: bool,
    sources: Vec<ExtSource>,
) -> watch::Receiver<Vec<ExtensionSourceRow>> {
    let hidden = if anime { pref_stream(&prefs.disabled_anime_sources) } else { pref_stream(&prefs.disabled_manga_sources) };
    derive(hidden, move |hidden| extension_source_rows(&sources, hidden))
}

/// The four extension lists as a stream. `show_nsfw_source` and the enabled languages are watched, so toggling NSFW
/// re-filters at once (the one-time read of the Kotlin original is the documented improvement). The lists are the
/// caller's streams. Must run inside a tokio runtime.
pub fn subscribe_extensions_by_type(
    prefs: &SourcePrefs,
    installed: watch::Receiver<Vec<ExtRow>>,
    untrusted: watch::Receiver<Vec<ExtRow>>,
    available: watch::Receiver<Vec<ExtRow>>,
    keep_sourceless: bool,
) -> watch::Receiver<ExtensionsByType> {
    let lists = combine3(installed, untrusted, available);
    let settings = combine3(lists, pref_stream(&prefs.show_nsfw_source), pref_stream(&prefs.enabled_languages));
    derive(settings, move |((installed, untrusted, available), show_nsfw, enabled)| {
        compute_extensions_by_type(&ByTypeInput {
            installed,
            untrusted,
            available,
            enabled_languages: enabled,
            show_nsfw: *show_nsfw,
            keep_sourceless,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    fn src(id: i64, name: &str, lang: &str) -> ExtSource {
        ExtSource { id, name: name.into(), lang: lang.into() }
    }

    fn ext(pkg: &str, name: &str, lang: &str, sources: Vec<ExtSource>) -> ExtRow {
        ExtRow { pkg: pkg.into(), name: name.into(), lang: lang.into(), nsfw: false, obsolete: false, has_update: false, sources }
    }

    fn input<'a>(
        installed: &'a [ExtRow],
        untrusted: &'a [ExtRow],
        available: &'a [ExtRow],
        enabled: &'a BTreeSet<String>,
        keep_sourceless: bool,
    ) -> ByTypeInput<'a> {
        ByTypeInput { installed, untrusted, available, enabled_languages: enabled, show_nsfw: false, keep_sourceless }
    }

    #[test]
    fn installed_sort_obsolete_first_then_name_and_split_out_the_updates() {
        let mut old = ext("a", "Zed", "en", vec![]);
        old.obsolete = true;
        let mut upd = ext("b", "beta", "en", vec![]);
        upd.has_update = true;
        let plain = ext("c", "Alpha", "en", vec![]);
        let installed = [plain, upd, old];
        let enabled = set(&["en"]);
        let out = compute_extensions_by_type(&input(&installed, &[], &[], &enabled, true));
        assert_eq!(out.updates.iter().map(|e| e.pkg.as_str()).collect::<Vec<_>>(), ["b"]);
        assert_eq!(out.installed.iter().map(|e| e.pkg.as_str()).collect::<Vec<_>>(), ["a", "c"], "obsolete first");
    }

    #[test]
    fn untrusted_is_sorted_by_name_and_ignores_nsfw() {
        let mut nsfw = ext("n", "Bravo", "en", vec![]);
        nsfw.nsfw = true;
        let untrusted = [ext("x", "charlie", "en", vec![]), nsfw, ext("y", "Alpha", "en", vec![])];
        let enabled = set(&["en"]);
        let out = compute_extensions_by_type(&input(&[], &untrusted, &[], &enabled, true));
        assert_eq!(out.untrusted.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["Alpha", "Bravo", "charlie"]);
    }

    /// EC-37, EC-38: installed and untrusted packages are removed from available, and each enabled-language source
    /// becomes its own row keyed `<pkg>-<id>`.
    #[test]
    fn available_drops_installed_and_untrusted_and_expands_sources_by_language() {
        let installed = [ext("inst", "Installed", "en", vec![])];
        let untrusted = [ext("untr", "Untrusted", "en", vec![])];
        let available = [
            ext("inst", "Installed", "en", vec![]),
            ext("untr", "Untrusted", "en", vec![]),
            ext("multi", "Multi", "all", vec![src(10, "Jp Source", "ja"), src(11, "En Source", "en"), src(12, "Fr", "fr")]),
        ];
        let enabled = set(&["en", "ja"]);
        let out = compute_extensions_by_type(&input(&installed, &untrusted, &available, &enabled, true));
        assert_eq!(out.available.iter().map(|e| e.pkg.as_str()).collect::<Vec<_>>(), ["multi-11", "multi-10"], "sorted by name");
        assert_eq!(out.available[0].sources, [src(11, "En Source", "en")]);
    }

    #[test]
    fn a_sourceless_extension_is_kept_for_manga_and_dropped_for_anime_when_its_language_is_enabled() {
        let available = [ext("bare", "Bare", "en", vec![]), ext("bare-fr", "Fr Bare", "fr", vec![])];
        let enabled = set(&["en"]);
        let manga = compute_extensions_by_type(&input(&[], &[], &available, &enabled, true));
        assert_eq!(manga.available.iter().map(|e| e.pkg.as_str()).collect::<Vec<_>>(), ["bare"]);
        let anime = compute_extensions_by_type(&input(&[], &[], &available, &enabled, false));
        assert!(anime.available.is_empty());
    }

    #[test]
    fn languages_list_enabled_first_then_by_display_name() {
        let available = [
            ext("a", "A", "de", vec![src(1, "x", "de")]),
            ext("b", "B", "en", vec![src(2, "y", "en"), src(3, "z", "fr")]),
            ext("c", "C", "es", vec![]),
        ];
        let enabled = set(&["fr"]);
        let display = |tag: &str| tag.to_owned();
        assert_eq!(extension_languages(&available, &enabled, &display), ["fr", "de", "en", "es"]);
    }

    /// The truth table of `labelAsName` in `GetExtensionSources.kt`.
    #[test]
    fn the_label_truth_table_follows_the_multi_source_rules() {
        assert!(!label_as_name(&[src(1, "A", "en")]), "one source: no label");
        assert!(!label_as_name(&[src(1, "Same", "en"), src(2, "Same", "ja")]), "one name in several languages");
        assert!(label_as_name(&[src(1, "One", "en"), src(2, "Two", "en")]), "several names");
    }

    #[test]
    fn a_source_is_enabled_unless_its_id_is_hidden() {
        let hidden = set(&["7"]);
        assert!(!source_enabled(7, &hidden));
        assert!(source_enabled(8, &hidden));
    }

    #[test]
    fn the_rows_carry_enabled_and_label_as_name() {
        let sources = [src(1, "One", "en"), src(2, "Two", "en")];
        let rows = extension_source_rows(&sources, &set(&["2"]));
        assert_eq!(rows.iter().map(|r| (r.source.id, r.enabled, r.label_as_name)).collect::<Vec<_>>(), [(1, true, true), (2, false, true)]);
    }

    #[tokio::test]
    async fn the_source_rows_re_emit_when_the_hidden_set_changes() {
        let prefs = SourcePrefs::new(kuro_prefs::store::PrefStore::in_memory());
        let mut rows = subscribe_extension_sources(&prefs, false, vec![src(5, "Five", "en")]);
        assert!(rows.borrow_and_update()[0].enabled);
        prefs.disabled_manga_sources.set(set(&["5"]));
        tokio::time::timeout(crate::app::rx::TEST_WAIT, rows.changed()).await.expect("waited").expect("open");
        assert!(!rows.borrow_and_update()[0].enabled, "hidden source is not enabled");
        prefs.disabled_anime_sources.set(set(&["5"]));
        let waited = tokio::time::timeout(std::time::Duration::from_millis(200), rows.changed()).await;
        assert!(waited.is_err(), "the anime hidden set does not touch the manga rows");
    }

    #[tokio::test]
    async fn show_nsfw_source_is_observed_reactively() {
        let prefs = SourcePrefs::new(kuro_prefs::store::PrefStore::in_memory());
        prefs.show_nsfw_source.set(false);
        prefs.enabled_languages.set(set(&["en"]));
        let mut nsfw = ext("n", "Adult", "en", vec![]);
        nsfw.nsfw = true;
        let (_i, installed) = watch::channel(vec![nsfw, ext("s", "Safe", "en", vec![])]);
        let (_u, untrusted) = watch::channel(Vec::new());
        let (_a, available) = watch::channel(Vec::new());
        let mut out = subscribe_extensions_by_type(&prefs, installed, untrusted, available, true);
        assert_eq!(out.borrow_and_update().installed.len(), 1, "NSFW hidden at the start");
        prefs.show_nsfw_source.set(true);
        tokio::time::timeout(crate::app::rx::TEST_WAIT, out.changed()).await.expect("waited").expect("open");
        assert_eq!(out.borrow_and_update().installed.len(), 2, "the toggle re-filters without a restart");
    }
}
