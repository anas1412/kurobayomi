//! Android plumbing and sync features this crate does not port (T-05a-027, T-05a-029, T-06a-006, T-06a-007).
//!
//! * No paging source. Aniyomi's Paging3 wrapper over `limit`/`offset` queries has no caller outside its
//!   handlers. Lists load in full and the UI virtualises them (`gtk::ListView`). User-visible loss: none, since
//!   library sizes fit in memory.
//! * No coroutine transaction context and no thread-local transaction element. The replacement is `Db::write`,
//!   a single writer that runs a synchronous closure inside one transaction. The debug-only database inspector
//!   switch is dropped because it exists only in Android Studio. `rusqlite` with bundled SQLite is used in every
//!   build.
//! * No sync-state column write path. The `is_syncing` column stays in the schema for compatibility, and every
//!   update statement still writes `is_syncing = 0`. The reset statements get no Rust method. The SyncYomi-style
//!   sync that would set the flag is not ported, so the user loses cross-device sync. Backups remain the transfer
//!   mechanism.
//!
//! The tests below keep those decisions true: a later change that adds one of these pieces fails them.

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Every `.rs` file of this crate's `src` directory, as text.
    fn sources() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect()
    }

    #[test]
    fn no_paging_source_exists() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        assert!(!dir.join("paging.rs").exists());
        let needle = concat!("Paging", "Source");
        assert!(sources().iter().all(|s| !s.contains(needle)));
    }

    #[test]
    fn no_sync_state_write_path_exists() {
        for text in sources() {
            assert!(!text.contains(concat!("reset_is", "_syncing")));
            assert!(!text.contains(concat!("resetIs", "Syncing")));
        }
    }

    #[test]
    fn no_coroutine_transaction_context_exists() {
        for text in sources() {
            assert!(!text.contains(concat!("Transaction", "Element")));
        }
    }
}
