//! Anime extension store domain (area 09 section 1, T-09a-030 to T-09a-034): the store model, the wire format and its
//! sniffing, the fetch with its one-step `indexV2` follow, and the operations over the `kuro-data` repository.
//!
//! Keys: a store is keyed by the URL the user entered (Q-STORE-1). The document its data came from is kept in a separate
//! column, `resolved_index_url` (the `indexV2` target, or `repo.json` for a legacy list); the list is read from it. The
//! HTTP client is `kuro-net` (area 12), not in the workspace yet, so the fetch takes the [`StoreHttp`] port and tests use
//! a fake.

pub mod available;
pub mod fetch;
pub mod model;
pub mod ops;
pub mod wire;

pub use fetch::{ListSource, StoreHttp, fetch_store, legacy_base, legacy_extensions, list_source};
pub use model::{AnimeExtensionStore, Contact, NO_SIGNING_KEY};
pub use ops::{add_store, fetch_extensions, insert_from_preference, refresh_all, subscribe_all, subscribe_count};
pub use wire::{StoreWire, gunzip_if_needed, sniff};
