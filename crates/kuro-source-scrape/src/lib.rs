//! Declarative sources: a TOML recipe of URL templates and CSS selectors becomes a manga or anime source, with no code.
//! `docs/source-recipes.md` describes the format. The pieces:
//!
//! - [`recipe`]: the format and its checks (unknown keys, hosts, selectors, regexes).
//! - [`engine`]: each operation as a pure function over an HTML string, so tests use saved pages.
//! - [`fetch`]: the only network path; the hosts of `base_urls`, one request at a time, the recipe's rate limit.
//! - [`manga`] and [`anime`]: the source traits over the two.
//! - [`check`]: what `examples/check_recipe.rs` runs.

pub mod anime;
pub mod check;
pub mod engine;
pub mod fetch;
pub mod manga;
pub mod recipe;

pub use anime::ScrapeAnime;
pub use manga::ScrapeManga;
pub use recipe::{Recipe, RecipeError};

use std::path::Path;

/// The recipes of a folder: every `*.toml` file, in name order. A file that is not a valid recipe is reported with its
/// reason and skipped, and so is a second recipe with an id already seen.
pub fn load_dir(dir: &Path) -> (Vec<Recipe>, Vec<(String, String)>) {
    let mut names: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect(),
        Err(_) => return (Vec::new(), Vec::new()),
    };
    names.sort();
    let (mut recipes, mut problems) = (Vec::<Recipe>::new(), Vec::new());
    for path in names {
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match Recipe::load(&path) {
            Ok(recipe) if recipes.iter().any(|r| r.id == recipe.id) => problems.push((file, format!("id {:?} is already used by another recipe", recipe.id))),
            Ok(recipe) => recipes.push(recipe),
            Err(e) => problems.push((file, e.to_string())),
        }
    }
    (recipes, problems)
}
