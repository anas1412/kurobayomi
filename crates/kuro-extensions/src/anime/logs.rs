//! The log texts of area 19 (T-19a-060, `p1-data-and-config.md` section 11). Each text is kept verbatim as the event
//! message. The events carry the fields `pkg`, `store` and `version_code` where they apply.
//!
//! The Android-only texts (installer dispatch, Shizuku, the package receiver, the APK lookup, class loading and the
//! repo migration) have no function here, because the code they describe is not ported.

/// Installing a package that is not signed (`LDR:86`).
pub const NOT_SIGNED_FOR_INSTALL: &str = "Extension to be installed is not signed.";
/// An upgrade whose version code is lower than the installed one (`LDR:80`).
pub const DOWNGRADE: &str = "Installed extension version is higher. Downgrading is not allowed.";
/// An upgrade signed by another key than the installed one (`LDR:91`).
pub const SIGNATURE_NOT_MATCHED: &str = "Installed extension signature is not matched.";
/// Copying the verified package into the package folder failed (`LDR:110`).
pub const COPY_FAILED: &str = "Failed to copy extension file.";
/// The downloaded file could not be read for verification (`INST:193`).
pub const READ_DOWNLOADED_FAILED: &str = "Failed to read downloaded extension file.";

/// A package file that the loader cannot find (`LDR:188`).
pub fn package_not_found(pkg: &str) -> String {
    format!("Extension package is not found ({pkg})")
}

/// The version name is empty (`LDR:255`).
pub fn missing_version_name(name: &str) -> String {
    format!("Missing versionName for extension {name}")
}

/// The package's library version is not one we support (`LDR:267`). `supported` is the list of supported versions.
pub fn lib_version(found: &str, supported: &[String]) -> String {
    format!(
        "Lib version is {found}, while only version(s) {} are supported",
        supported.join(", ")
    )
}

/// The package has no signature (`LDR:274`).
pub fn not_signed(pkg: &str) -> String {
    format!("Package {pkg} isn't signed")
}

/// The package is signed but not trusted (`LDR:285`).
pub fn untrusted(pkg: &str) -> String {
    format!("Extension {pkg} isn't trusted")
}

/// An NSFW package while NSFW sources are off (`LDR:292`).
pub fn nsfw_not_allowed(pkg: &str) -> String {
    format!("NSFW extension {pkg} not allowed")
}

/// A package failed to load (`LDR:301`, `LDR:339`).
pub fn load_error(name: &str, pkg: &str) -> String {
    format!("Extension load error: {name} ({pkg})")
}

/// Adding a store failed (`SSVC:63-65`).
pub fn add_store_failed(index_url: &str) -> String {
    format!("Failed to add extension store '{index_url}'")
}

/// Fetching a store's extensions failed (`SREPO:75-77`). A store whose document cannot be read has no name yet, so
/// the caller passes the index URL for `name` too.
pub fn fetch_store_failed(name: &str, index_url: &str) -> String {
    format!("Failed to fetch extensions for store '{name} ({index_url})'")
}

/// Refreshing a store failed (`SREPO:45-46`). Kurobayomi fetches the index and the extension list in one request, so
/// the fetch text above is the one logged for a failed store, and this text has no emitter yet.
pub fn refresh_store_failed(name: &str, index_url: &str) -> String {
    format!("Failed to refresh extension store '{name} ({index_url})'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixed_texts_match_the_kotlin_logs() {
        assert_eq!(
            NOT_SIGNED_FOR_INSTALL,
            "Extension to be installed is not signed."
        );
        assert_eq!(
            DOWNGRADE,
            "Installed extension version is higher. Downgrading is not allowed."
        );
        assert_eq!(
            SIGNATURE_NOT_MATCHED,
            "Installed extension signature is not matched."
        );
        assert_eq!(COPY_FAILED, "Failed to copy extension file.");
        assert_eq!(
            READ_DOWNLOADED_FAILED,
            "Failed to read downloaded extension file."
        );
    }

    #[test]
    fn the_templates_fill_in_the_kotlin_placeholders() {
        assert_eq!(
            package_not_found("eu.a"),
            "Extension package is not found (eu.a)"
        );
        assert_eq!(
            missing_version_name("eu.a"),
            "Missing versionName for extension eu.a"
        );
        assert_eq!(
            lib_version("15.0", &["14.0".into(), "16.0".into()]),
            "Lib version is 15.0, while only version(s) 14.0, 16.0 are supported"
        );
        assert_eq!(not_signed("eu.a"), "Package eu.a isn't signed");
        assert_eq!(untrusted("eu.a"), "Extension eu.a isn't trusted");
        assert_eq!(nsfw_not_allowed("eu.a"), "NSFW extension eu.a not allowed");
        assert_eq!(load_error("A", "eu.a"), "Extension load error: A (eu.a)");
        assert_eq!(
            add_store_failed("https://x/i.json"),
            "Failed to add extension store 'https://x/i.json'"
        );
        assert_eq!(
            fetch_store_failed("S", "https://x/i.json"),
            "Failed to fetch extensions for store 'S (https://x/i.json)'"
        );
        assert_eq!(
            refresh_store_failed("S", "https://x/i.json"),
            "Failed to refresh extension store 'S (https://x/i.json)'"
        );
    }
}
