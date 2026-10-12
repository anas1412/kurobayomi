//! Trust of installed extensions (`TrustMangaExtension`, `TrustAnimeExtension`; T-10a-024). The signing keys come from
//! the extension repositories. The anime extension stores were cut, so there is no anime key source here. The stored triple `<package>:<versionCode>:<hash>` keeps the Aniyomi format,
//! so a backup of `trusted_extensions` restores unchanged.

use std::collections::BTreeSet;

use kuro_data::extension_repo::ExtensionRepoRepo;
use kuro_prefs::pref::Pref;

/// The identity of an installed extension package. Area 20 supplies it. `fingerprints` holds the signing
/// fingerprints in order, and its last entry is the one the stored triple uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageIdentity {
    pub name: String,
    pub version_code: i64,
    pub fingerprints: Vec<String>,
}

/// The signing keys of the repositories (manga) or stores (anime) the user added.
pub trait SigningKeys: Send + Sync {
    fn signing_keys(&self) -> Vec<String>;
}

/// Manga: the signing keys of the extension repositories.
pub struct MangaRepoKeys(pub ExtensionRepoRepo);

impl SigningKeys for MangaRepoKeys {
    fn signing_keys(&self) -> Vec<String> {
        match self.0.all() {
            Ok(repos) => repos.into_iter().map(|r| r.signing_key_fingerprint).collect(),
            Err(e) => {
                tracing::warn!("cannot read extension repos for trust: {e:?}");
                Vec::new()
            }
        }
    }
}

/// `<package>:<versionCode>:<hash>`.
pub fn triple(name: &str, version_code: i64, hash: &str) -> String {
    format!("{name}:{version_code}:{hash}")
}

/// Trust decisions for one kind of extension, over the `trusted_extensions` preference.
pub struct TrustExtension<K: SigningKeys> {
    trusted: Pref<BTreeSet<String>>,
    keys: K,
}

impl<K: SigningKeys> TrustExtension<K> {
    pub fn new(trusted: Pref<BTreeSet<String>>, keys: K) -> Self {
        Self { trusted, keys }
    }

    /// `isTrusted`: trusted when a repository or store key is among the fingerprints, or when this exact version
    /// was trusted by hash. A package with no fingerprints is not trusted (EC-39).
    pub fn is_trusted(&self, identity: &PackageIdentity) -> bool {
        let Some(last) = identity.fingerprints.last() else {
            tracing::warn!("extension {} has no signing fingerprint; not trusted", identity.name);
            return false;
        };
        if self
            .keys
            .signing_keys()
            .iter()
            .any(|key| identity.fingerprints.contains(key))
        {
            return true;
        }
        self.trusted
            .get()
            .contains(&triple(&identity.name, identity.version_code, last))
    }

    /// `trust(name, versionCode, hash)`: drops every older entry of the package, then adds this one (EC-40).
    pub fn trust(&self, name: &str, version_code: i64, hash: &str) {
        let prefix = format!("{name}:");
        let mut set: BTreeSet<String> = self
            .trusted
            .get()
            .into_iter()
            .filter(|entry| !entry.starts_with(&prefix))
            .collect();
        set.insert(triple(name, version_code, hash));
        self.trusted.set(set);
    }

    /// `revokeAll`: deletes the preference. Repository and store keys are not touched.
    pub fn revoke_all(&self) {
        self.trusted.delete();
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::source::SourcePrefs;
    use kuro_prefs::store::PrefStore;

    use super::*;

    struct Keys(Vec<String>);

    impl SigningKeys for Keys {
        fn signing_keys(&self) -> Vec<String> {
            self.0.clone()
        }
    }

    fn trust_with(keys: &[&str]) -> (TrustExtension<Keys>, Pref<BTreeSet<String>>) {
        let store = PrefStore::in_memory();
        let trusted = SourcePrefs::new(store).trusted_extensions.clone();
        let keys = Keys(keys.iter().map(|k| k.to_string()).collect());
        (TrustExtension::new(trusted.clone(), keys), trusted)
    }

    fn package(name: &str, version: i64, fingerprints: &[&str]) -> PackageIdentity {
        PackageIdentity {
            name: name.to_owned(),
            version_code: version,
            fingerprints: fingerprints.iter().map(|f| f.to_string()).collect(),
        }
    }

    #[test]
    fn a_repository_key_among_the_fingerprints_trusts_the_package() {
        let (trust, _) = trust_with(&["repo-key"]);
        assert!(trust.is_trusted(&package("pkg", 1, &["other", "repo-key"])));
        assert!(!trust.is_trusted(&package("pkg", 1, &["other"])));
    }

    #[test]
    fn a_stored_triple_trusts_only_that_exact_version() {
        let (trust, _) = trust_with(&[]);
        trust.trust("pkg", 3, "h");
        assert!(trust.is_trusted(&package("pkg", 3, &["x", "h"])));
        assert!(!trust.is_trusted(&package("pkg", 4, &["x", "h"])));
    }

    #[test]
    fn an_empty_fingerprint_list_is_not_trusted_and_does_not_panic() {
        let (trust, _) = trust_with(&["repo-key"]);
        trust.trust("pkg", 1, "");
        assert!(!trust.is_trusted(&package("pkg", 1, &[])));
    }

    #[test]
    fn trust_replaces_the_older_version_of_the_same_package_only() {
        let (trust, stored) = trust_with(&[]);
        trust.trust("a.b", 1, "h1");
        trust.trust("a.bc", 1, "x");
        trust.trust("a.b", 2, "h2");
        let entries: Vec<String> = stored.get().into_iter().collect();
        assert_eq!(entries, vec!["a.b:2:h2".to_owned(), "a.bc:1:x".to_owned()]);
    }

    #[test]
    fn the_stored_triple_keeps_the_aniyomi_format() {
        let (trust, stored) = trust_with(&[]);
        trust.trust("eu.pkg", 12, "abc");
        assert!(stored.get().contains("eu.pkg:12:abc"));
    }

    #[test]
    fn revoke_all_clears_the_triples_and_keeps_repository_keys_trusted() {
        let (trust, stored) = trust_with(&["repo-key"]);
        trust.trust("pkg", 1, "h");
        trust.revoke_all();
        assert!(stored.get().is_empty());
        assert!(trust.is_trusted(&package("pkg", 9, &["repo-key"])));
        assert!(!trust.is_trusted(&package("pkg", 1, &["h"])));
    }
}
