#![allow(dead_code)]
use std::path::PathBuf;
use std::sync::Arc;

use kuro_net::NetworkClient;
use kuro_source_scrape::Recipe;

pub const MANGA: &str = include_str!("../fixtures/manga/recipe.toml");
pub const ANIME: &str = include_str!("../fixtures/anime/recipe.toml");

pub fn fixture(kind: &str, name: &str) -> String {
    std::fs::read_to_string(dir(kind).join(name)).expect("fixture")
}

pub fn dir(kind: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(kind)
}

/// The recipe text with a replaced piece, parsed.
pub fn with(text: &str, from: &str, to: &str) -> Result<Recipe, String> {
    assert!(text.contains(from), "{from:?} is not in the recipe");
    Recipe::parse(&text.replacen(from, to, 1)).map_err(|e| e.to_string())
}

/// A client with an in-memory preference store and a temporary cookie file. No request is made with it.
pub fn client() -> (NetworkClient, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    let client = NetworkClient::new(&prefs, dir.path().join("cookies.json"), Arc::new(kuro_platform::webview::NoWebView), toaster).expect("client");
    (client, dir)
}
