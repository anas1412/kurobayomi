//! `cargo run -p kuro-source-scrape --example check_recipe -- <recipe.toml> <fixtures dir> [--live]`
//!
//! Prints what every operation of the recipe returned from the fixture pages its `[tests]` table names. With `--live`
//! it runs the operations against the real site instead: one request at a time, one second apart, with a plain user
//! agent. Exits non-zero when an operation errors or returns nothing.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use kuro_net::NetworkClient;
use kuro_source_scrape::Recipe;
use kuro_source_scrape::check::{FixtureFeed, LiveFeed, polite, run};
use kuro_source_scrape::fetch::Fetcher;

fn client(dir: &std::path::Path) -> Result<NetworkClient, String> {
    let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    NetworkClient::new(&prefs, dir.join("cookies.json"), Arc::new(kuro_platform::webview::NoWebView), toaster).map_err(|e| e.to_string())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let live = args.iter().any(|a| a == "--live");
    let paths: Vec<&String> = args.iter().filter(|a| *a != "--live").collect();
    let [recipe_path, fixtures] = paths[..] else {
        eprintln!("usage: check_recipe <recipe.toml> <fixtures dir> [--live]");
        return ExitCode::from(2);
    };
    let recipe = match Recipe::load(&PathBuf::from(recipe_path)) {
        Ok(recipe) => recipe,
        Err(e) => {
            eprintln!("FAIL recipe: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("recipe {} ({:?}, {}): {}", recipe.id, recipe.kind, recipe.lang, if live { "live" } else { "fixtures" });
    let report = if live {
        let scratch = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("FAIL cannot make a scratch folder: {e}");
                return ExitCode::FAILURE;
            }
        };
        let client = match client(scratch.path()) {
            Ok(client) => client,
            Err(e) => {
                eprintln!("FAIL network client: {e}");
                return ExitCode::FAILURE;
            }
        };
        let mut feed = LiveFeed(Fetcher::new(client, Arc::new(polite(recipe.clone()))));
        run(&recipe, &mut feed).await
    } else {
        run(&recipe, &mut FixtureFeed::new(&recipe, PathBuf::from(fixtures))).await
    };
    print!("{report}");
    if report.failed() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
