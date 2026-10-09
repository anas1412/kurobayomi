//! Source domain (area 09 section 1): the source model, pins, uninstalled-source stubs, browse routing, the registry,
//! the ports of the source layer, and the non-library listings. Area 17 adds the identity adapter, the language groups,
//! the migration list, the pure toggles and the remote entry update.

pub mod browse;
pub mod db_repo;
#[cfg(test)]
pub mod fakes;
pub mod identity;
pub mod incognito;
pub mod languages;
pub mod migration;
pub mod model;
pub mod non_library;
pub mod paging;
pub mod pin;
pub mod registry;
pub mod remote_update;
pub mod repo;
pub mod stub;
pub mod toggles;
