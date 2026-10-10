//! Enhanced trackers: the shared framework (`enhanced.rs`, `http.rs`, `sources.rs`) and the Jellyfin tracker. Kavita,
//! Komga and Suwayomi are separate modules (area 33 tasks T-33a-040 onward).

pub mod enhanced;
pub mod http;
pub mod jellyfin;
pub mod kavita;
pub mod komga;
pub mod registry;
pub mod secret_migration;
pub mod sources;

// MangaUpdates and Simkl live at the crate root (`crate::mangaupdates`, `crate::simkl`) like the other services;
// these re-exports give them the `trackers::` paths of the area 32 layout without moving the modules.
pub use crate::{mangaupdates, simkl};
pub use registry::{Started, all_trackers, start_trackers};

/// Splits the enhanced services by whether one of their accepted identities is installed
/// (`SettingsTrackingScreen.kt:116-135`). Returns the available services and the names of the missing ones, in order.
/// `installed` holds the source identities `"{name}/{lang}"` the app found.
pub fn split_enhanced_services<'a>(
    services: &'a [EnhancedService],
    installed: &[&str],
) -> (Vec<&'a EnhancedService>, Vec<&'static str>) {
    let mut available = Vec::new();
    let mut missing = Vec::new();
    for s in services {
        if s.accepted.iter().any(|id| installed.contains(id)) {
            available.push(s);
        } else {
            missing.push(s.name);
        }
    }
    (available, missing)
}

/// One enhanced tracker as the Settings group lists it.
#[derive(Debug, Clone, Copy)]
pub struct EnhancedService {
    pub name: &'static str,
    pub accepted: &'static [&'static str],
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVICES: [EnhancedService; 2] = [
        EnhancedService {
            name: "Kavita",
            accepted: &["kavita/all"],
        },
        EnhancedService {
            name: "Jellyfin",
            accepted: &["jellyfin/all"],
        },
    ];

    #[test]
    fn t_33a_009_e99_a_service_without_its_source_is_listed_as_missing() {
        let (available, missing) = split_enhanced_services(&SERVICES, &["jellyfin/all"]);
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].name, "Jellyfin");
        assert_eq!(missing, vec!["Kavita"]);
    }

    #[test]
    fn t_33a_009_e100_all_installed_leaves_no_missing_names() {
        let (available, missing) =
            split_enhanced_services(&SERVICES, &["kavita/all", "jellyfin/all"]);
        assert_eq!(available.len(), 2);
        assert!(missing.is_empty());
    }
}
