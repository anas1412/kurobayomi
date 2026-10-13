//! The WASM host side of a repository plug-in: the compile cache and the instantiate step of a `.wasm` fetched from a
//! repository link (`repo`). Only the `wasm` feature builds this module, because wasmtime is large.

use kuro_platform::kind::Kind;
use kuro_source_wasm::HostEnv;

/// Where compiled plug-ins are cached between starts (see `kuro_source_wasm::cache`). Call once at start.
pub fn set_compile_cache_dir(dir: std::path::PathBuf) {
    kuro_source_wasm::cache::set_dir(dir);
}

/// The sources of one plug-in of a repository link, by the kind its index entry names.
pub enum RepoPlugin {
    Manga(Vec<std::sync::Arc<dyn kuro_source::CatalogueSource>>),
    Anime(Vec<std::sync::Arc<dyn kuro_source::anime::AnimeSource>>),
}

/// Instantiates a `.wasm` plug-in fetched from a repository link. It is a factory, as the packages are. `hosts` is the
/// allow-list its index entry names; the plug-in cannot reach any other host.
pub async fn instantiate_repo_plugin(
    bytes: Vec<u8>,
    hosts: Vec<String>,
    kind: Kind,
    client: kuro_net::NetworkClient,
    prefs: kuro_prefs::store::PrefStore,
) -> Result<RepoPlugin, String> {
    let env = HostEnv { client, prefs };
    let loaded = if kind.is_anime() {
        kuro_source_wasm::instantiate_anime(bytes, hosts, true, env).await.map(RepoPlugin::Anime)
    } else {
        kuro_source_wasm::instantiate(bytes, hosts, true, env).await.map(RepoPlugin::Manga)
    };
    loaded.map_err(|error| error.to_string())
}
