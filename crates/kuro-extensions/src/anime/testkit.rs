//! Test helpers: a valid anime manifest, zip builders, fixed test keys, signed `.kuroext` fixtures and store documents.
//! Keys come from fixed seeds, so every run builds the same bytes. The seeds are for tests only.

use std::io::{Cursor, Write};

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use ed25519_dalek::{Signer, SigningKey};
use kuro_core::hash::sha256_hex;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

pub const PKG: &str = "eu.example.animeext.en.demo";
pub const SOURCE_ID: i64 = 1_234_567_890_123_456_789;

/// A manifest that passes every rule, for version 1.0.7 with code 7.
pub const MANIFEST: &str = r#"{
    "schema": 1, "kind": "anime", "pkg": "eu.example.animeext.en.demo",
    "name": "Demo", "versionName": "1.0.7", "versionCode": 7, "abi": "1.0",
    "lang": "en", "nsfw": false,
    "sources": [ { "id": 1234567890123456789, "lang": "en", "name": "Demo", "baseUrl": "https://example.org" } ]
}"#;

/// The manifest with another version code, for upgrade and downgrade cases.
pub fn manifest_version(version_name: &str, version_code: i64) -> String {
    MANIFEST
        .replace("\"versionName\": \"1.0.7\"", &format!("\"versionName\": \"{version_name}\""))
        .replace("\"versionCode\": 7", &format!("\"versionCode\": {version_code}"))
}

/// Builds a zip with `entries`, stored without compression so tests are exact.
pub fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in entries {
        writer.start_file(*name, options).expect("start entry");
        writer.write_all(bytes).expect("write entry");
    }
    writer.finish().expect("finish zip").into_inner()
}

/// The key that signs the valid fixtures. Its digest is the store signing key.
pub fn fixture_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

/// A second key, for wrong-signer cases.
pub fn other_key() -> SigningKey {
    SigningKey::from_bytes(&[9u8; 32])
}

/// Lowercase hex SHA-256 of the key's public bytes: the store's `signingKey` and the package signer.
pub fn signer_hex(key: &SigningKey) -> String {
    sha256_hex(&key.verifying_key().to_bytes())
}

/// A signed `.kuroext` file and the values a store entry needs to describe it.
pub struct Fixture {
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
    pub sha256_hex: String,
    pub ed25519_b64: String,
    pub public_key_b64: String,
    pub signer_hex: String,
}

/// Builds a package with the given manifest and component, and signs the raw digest with `key`.
pub fn signed_package(key: &SigningKey, manifest: &str, wasm: &[u8]) -> Fixture {
    let bytes = build_zip(&[("manifest.json", manifest.as_bytes()), ("source.wasm", wasm)]);
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    let signature = key.sign(&sha256);
    Fixture {
        sha256_hex: hex(&sha256),
        ed25519_b64: BASE64_STANDARD.encode(signature.to_bytes()),
        public_key_b64: BASE64_STANDARD.encode(key.verifying_key().to_bytes()),
        signer_hex: signer_hex(key),
        sha256,
        bytes,
    }
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The `resources.kuroext` block for `fixture`, served at `url`.
pub fn kuroext_block(fixture: &Fixture, url: &str) -> Value {
    json!({
        "url": url,
        "sha256": fixture.sha256_hex,
        "ed25519": fixture.ed25519_b64,
        "publicKey": fixture.public_key_b64,
    })
}

/// One store extension entry for `pkg` at `version_code`. `kuroext` is `None` for an APK-only listing.
pub fn store_extension(pkg: &str, version_code: i64, kuroext: Option<Value>) -> Value {
    let mut resources = json!({
        "apkUrl": "https://example.org/apk/demo.apk",
        "iconUrl": "https://example.org/icon/demo.png",
    });
    if let Some(block) = kuroext {
        resources["kuroext"] = block;
    }
    json!({
        "name": "Aniyomi: Demo",
        "packageName": pkg,
        "resources": resources,
        "extensionLib": "16",
        "versionCode": version_code,
        "versionName": "1.0.7",
        "contentWarning": "SAFE",
        "isTorrent": false,
        "sources": [ { "id": SOURCE_ID, "name": "Demo", "language": "en", "homeUrl": "https://example.org" } ],
    })
}

/// A non-legacy store document with an inline extension list.
pub fn store_document(name: &str, signing_key: &str, extensions: Vec<Value>) -> Value {
    json!({
        "name": name,
        "badgeLabel": "EX",
        "signingKey": signing_key,
        "contact": { "website": "https://example.org", "discord": null },
        "extensionList": { "extensions": extensions },
        "extensionListUrl": null,
    })
}

/// The bytes of a store document, as a server would send them.
pub fn store_bytes(document: &Value) -> Vec<u8> {
    serde_json::to_vec(document).expect("store json")
}

/// A network client over in-memory preferences, a throw-away cookie file and no WebView. Its cookie jar and user agent are
/// the ones a store client shares.
pub fn test_net() -> kuro_net::NetworkClient {
    test_net_at(std::env::temp_dir().join("kuro-extensions-test-cookies.json"))
}

/// As [`test_net`], with the cookie file at `cookie_file`. The file is written only when a cookie was set.
pub fn test_net_at(cookie_file: std::path::PathBuf) -> kuro_net::NetworkClient {
    let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    kuro_net::NetworkClient::new(&prefs, cookie_file, std::sync::Arc::new(kuro_platform::webview::NoWebView), toaster)
        .expect("the test network client builds")
}

/// The store HTTP client of the tests: `http_client` over [`test_net`].
pub fn test_client() -> reqwest::Client {
    crate::anime::store::http_client(&test_net()).expect("the test store client builds")
}

/// A trust store over an in-memory preference, holding `entries` as the trusted set.
pub fn trust_with_entries(entries: &[String]) -> crate::anime::trust::TrustStore {
    let pref = kuro_prefs::store::PrefStore::in_memory()
        .pref("__APP_STATE_trusted_extensions", std::collections::BTreeSet::new());
    pref.extend(entries.iter().cloned());
    crate::anime::trust::TrustStore::new(pref)
}
