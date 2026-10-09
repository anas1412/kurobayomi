//! The fixed user-visible text of each error (T-19a-060, `p1-ui-parity.md` section 10 and `p1-data-and-config.md`
//! section 12). The UI shows these strings only. The raw error text goes to the log, never to the page.
//!
//! Every error enum is matched without a wildcard, so a new variant does not compile until it has a text here.

use crate::anime::install::download::DownloadError;
use crate::anime::install::place::PlaceError;
use crate::anime::model::LoadError;
use crate::anime::package::verify::VerifyError;
use crate::anime::store::{LEGACY_URL_MESSAGE, StoreError};

/// The generic store failure. Network, status, and cancel errors all show this.
pub const ADD_STORE_FAILED: &str = "Failed to add extension store";
/// Kept verbatim from Aniyomi (K-3).
pub const LEGACY_URL_NOT_VALID: &str = LEGACY_URL_MESSAGE;
pub const ALREADY_EXISTS: &str = "This extension store already exists";
pub const REDIRECT_LOOP: &str = "This extension store points to itself";
pub const STORE_TOO_LARGE: &str = "The extension store index is too large";
pub const STORE_UNREADABLE: &str = "This extension store could not be read";

pub const DOWNLOAD_FAILED: &str = "Failed to download extension";
pub const DOWNLOAD_TOO_LARGE: &str = "The extension is too large to install";
pub const DOWNLOAD_WRITE_FAILED: &str = "The extension could not be saved";

pub const DIGEST_MISMATCH: &str = "The download does not match the store listing";
pub const BAD_SIGNATURE: &str = "The extension signature is not valid";
pub const UNTRUSTED_SIGNER: &str = "The extension is not signed by this store";
pub const BAD_KEY: &str = "The extension signing key is not valid";
pub const VERIFY_READ_FAILED: &str = "The downloaded extension could not be read";

pub const DOWNGRADE: &str = "Installed extension version is higher. Downgrading is not allowed.";
pub const SIGNATURE_NOT_MATCHED: &str = "Installed extension signature is not matched.";
pub const INSTALLED_UNREADABLE: &str = "The installed extension cannot be read";
pub const PKG_MISMATCH: &str = "The download is not the extension the store lists";
pub const LANG_MISMATCH: &str = "The extension language does not match its sources";
pub const PLACE_SIGNER_NOT_STORE: &str = "The extension is not signed by this store";
pub const PLACE_INVALID: &str = "This extension is not valid";
pub const PLACE_WRITE_FAILED: &str = "The extension could not be installed";

pub const LOAD_NOT_FOUND: &str = "This extension was not found";
pub const LOAD_NO_VERSION: &str = "This extension has no version name";
pub const LOAD_UNSUPPORTED_ABI: &str = "This extension needs a newer version of Kurobayomi";
pub const LOAD_NOT_SIGNED: &str = "This extension is not signed";
pub const LOAD_NSFW_HIDDEN: &str = "NSFW sources are turned off";
pub const LOAD_FAILED: &str = "This extension could not be loaded";

/// The text for a failed store add or refresh.
pub fn store_error(error: &StoreError) -> &'static str {
    match error {
        StoreError::Network(_) | StoreError::Status(_) | StoreError::Cancelled => ADD_STORE_FAILED,
        StoreError::TooLarge => STORE_TOO_LARGE,
        StoreError::Decode(_) => STORE_UNREADABLE,
        StoreError::Invalid(message) if message == LEGACY_URL_MESSAGE => LEGACY_URL_NOT_VALID,
        StoreError::Invalid(_) => ADD_STORE_FAILED,
        StoreError::RedirectLoop => REDIRECT_LOOP,
        StoreError::AlreadyExists => ALREADY_EXISTS,
    }
}

/// The text for a failed download.
pub fn download_error(error: &DownloadError) -> &'static str {
    match error {
        DownloadError::Http(_) | DownloadError::Network(_) | DownloadError::Cancelled => {
            DOWNLOAD_FAILED
        }
        DownloadError::TooLarge => DOWNLOAD_TOO_LARGE,
        DownloadError::Io(_) => DOWNLOAD_WRITE_FAILED,
    }
}

/// The text for a package that fails the digest or signature check.
pub fn verify_error(error: &VerifyError) -> &'static str {
    match error {
        VerifyError::DigestMismatch => DIGEST_MISMATCH,
        VerifyError::BadSignature => BAD_SIGNATURE,
        VerifyError::UntrustedSigner(_) => UNTRUSTED_SIGNER,
        VerifyError::BadKey => BAD_KEY,
        VerifyError::Io(_) => VERIFY_READ_FAILED,
    }
}

/// The text for a package that cannot be placed in the package folder.
pub fn place_error(error: &PlaceError) -> &'static str {
    match error {
        PlaceError::Downgrade => DOWNGRADE,
        PlaceError::SignerChanged => SIGNATURE_NOT_MATCHED,
        PlaceError::InstalledUnreadable => INSTALLED_UNREADABLE,
        PlaceError::PkgMismatch { .. } => PKG_MISMATCH,
        PlaceError::LangMismatch => LANG_MISMATCH,
        PlaceError::SignerNotStore => PLACE_SIGNER_NOT_STORE,
        PlaceError::Package(_) => PLACE_INVALID,
        PlaceError::Io(_) => PLACE_WRITE_FAILED,
    }
}

/// The text for a package that loads with an error.
pub fn load_error(error: &LoadError) -> &'static str {
    match error {
        LoadError::NotFound(_) => LOAD_NOT_FOUND,
        LoadError::MissingVersionName(_) => LOAD_NO_VERSION,
        LoadError::UnsupportedAbi(_) => LOAD_UNSUPPORTED_ABI,
        LoadError::NotSigned(_) => LOAD_NOT_SIGNED,
        LoadError::NsfwNotAllowed(_) => LOAD_NSFW_HIDDEN,
        LoadError::Manifest(_)
        | LoadError::Container(_)
        | LoadError::Io(_)
        | LoadError::Load { .. } => LOAD_FAILED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_store_error_has_a_fixed_text() {
        let legacy = StoreError::Invalid(LEGACY_URL_MESSAGE.into());
        assert_eq!(store_error(&legacy), LEGACY_URL_NOT_VALID);
        let cases = [
            (StoreError::Network("x".into()), ADD_STORE_FAILED),
            (StoreError::Status(500), ADD_STORE_FAILED),
            (StoreError::TooLarge, STORE_TOO_LARGE),
            (StoreError::Decode("x".into()), STORE_UNREADABLE),
            (
                StoreError::Invalid("extensionList is missing".into()),
                ADD_STORE_FAILED,
            ),
            (StoreError::RedirectLoop, REDIRECT_LOOP),
            (StoreError::Cancelled, ADD_STORE_FAILED),
            (StoreError::AlreadyExists, ALREADY_EXISTS),
        ];
        for (error, text) in cases {
            assert_eq!(store_error(&error), text, "{error:?}");
        }
    }

    #[test]
    fn every_download_error_has_a_fixed_text() {
        assert_eq!(download_error(&DownloadError::Http(404)), DOWNLOAD_FAILED);
        assert_eq!(
            download_error(&DownloadError::Network("x".into())),
            DOWNLOAD_FAILED
        );
        assert_eq!(download_error(&DownloadError::Cancelled), DOWNLOAD_FAILED);
        assert_eq!(download_error(&DownloadError::TooLarge), DOWNLOAD_TOO_LARGE);
        assert_eq!(
            download_error(&DownloadError::Io("disk full".into())),
            DOWNLOAD_WRITE_FAILED
        );
    }

    #[test]
    fn every_verify_error_has_a_fixed_text() {
        let cases = [
            (VerifyError::DigestMismatch, DIGEST_MISMATCH),
            (VerifyError::BadSignature, BAD_SIGNATURE),
            (VerifyError::UntrustedSigner("ab".into()), UNTRUSTED_SIGNER),
            (VerifyError::BadKey, BAD_KEY),
            (VerifyError::Io("x".into()), VERIFY_READ_FAILED),
        ];
        for (error, text) in cases {
            assert_eq!(verify_error(&error), text, "{error:?}");
        }
    }

    #[test]
    fn every_place_error_has_a_fixed_text() {
        let cases = [
            (PlaceError::Downgrade, DOWNGRADE),
            (PlaceError::SignerChanged, SIGNATURE_NOT_MATCHED),
            (PlaceError::InstalledUnreadable, INSTALLED_UNREADABLE),
            (
                PlaceError::PkgMismatch {
                    expected: "a".into(),
                    found: "b".into(),
                },
                PKG_MISMATCH,
            ),
            (PlaceError::LangMismatch, LANG_MISMATCH),
            (PlaceError::SignerNotStore, PLACE_SIGNER_NOT_STORE),
            (
                PlaceError::Package(LoadError::Container("x".into())),
                PLACE_INVALID,
            ),
            (PlaceError::Io("x".into()), PLACE_WRITE_FAILED),
        ];
        for (error, text) in cases {
            assert_eq!(place_error(&error), text, "{error:?}");
        }
    }

    #[test]
    fn every_load_error_has_a_fixed_text() {
        let cases = [
            (LoadError::NotFound("a".into()), LOAD_NOT_FOUND),
            (LoadError::MissingVersionName("a".into()), LOAD_NO_VERSION),
            (
                LoadError::UnsupportedAbi("2.0".into()),
                LOAD_UNSUPPORTED_ABI,
            ),
            (LoadError::NotSigned("a".into()), LOAD_NOT_SIGNED),
            (LoadError::NsfwNotAllowed("a".into()), LOAD_NSFW_HIDDEN),
            (LoadError::Manifest("x".into()), LOAD_FAILED),
            (LoadError::Container("x".into()), LOAD_FAILED),
            (LoadError::Io("x".into()), LOAD_FAILED),
            (
                LoadError::Load {
                    pkg: "a".into(),
                    reason: "x".into(),
                },
                LOAD_FAILED,
            ),
        ];
        for (error, text) in cases {
            assert_eq!(load_error(&error), text, "{error:?}");
        }
    }

    #[test]
    fn no_text_is_a_raw_error_message() {
        let raw = StoreError::Network("connection reset by peer".into());
        assert!(!store_error(&raw).contains("connection reset"));
    }
}
