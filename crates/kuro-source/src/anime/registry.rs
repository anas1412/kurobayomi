//! The builtin anime source registry (T-16a-038). It is the only source of anime sources in this build: APK loading, the
//! classloader and the `createSources` factory are dropped (the third-party mechanism is decided in area 20).

use std::sync::Arc;

use super::AnimeSource;

/// Number of builtin modules that add sources. Adding a source means adding its module here and bumping this count.
pub const BUILTIN_MODULE_COUNT: usize = 0;

/// The anime sources this build ships. Empty until a module adds one.
pub fn builtin_anime_sources() -> Vec<Arc<dyn AnimeSource>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_lists_one_entry_per_registered_module() {
        assert_eq!(builtin_anime_sources().len(), BUILTIN_MODULE_COUNT);
    }
}
