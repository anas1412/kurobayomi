//! Builders shared by the integration tests of `kuro-extensions` (T-18a-044): a valid manifest, zip packages signed with
//! fixed test keys, store entries, and the model values the pipeline works on. The seeds are for tests only.
#![allow(dead_code)]

use std::io::{Cursor, Write};

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use ed25519_dalek::{Signer, SigningKey};
use kuro_core::hash::sha256_hex;
use kuro_extensions::anime::abi::AbiVersion;
use kuro_extensions::anime::model::{AnimeSourceRef, Available, Installed, KuroextRef, SignerDigest, StoreRef};
use kuro_extensions::anime::store::map;
use kuro_extensions::anime::store::wire::ExtensionListJson;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

pub const PKG: &str = "eu.example.animeext.en.demo";
pub const INDEX_URL: &str = "https://example.org/store/index.json";

/// A manifest for `PKG` that passes every rule.
pub fn manifest(version_name: &str, version_code: i64) -> String {
    format!(
        r#"{{ "schema": 1, "kind": "anime", "pkg": "{PKG}", "name": "Demo", "versionName": "{version_name}",
            "versionCode": {version_code}, "abi": "1.0", "lang": "en", "nsfw": false,
            "sources": [ {{ "id": 1234567890123456789, "lang": "en", "name": "Demo", "baseUrl": "https://example.org" }} ] }}"#
    )
}

pub fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in entries {
        writer.start_file(*name, options).expect("start entry");
        writer.write_all(bytes).expect("write entry");
    }
    writer.finish().expect("finish zip").into_inner()
}

/// A signing key from a fixed seed.
pub fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

/// The signer digest of a key: lowercase hex SHA-256 of its public bytes. It is also a store's `signingKey`.
pub fn signer_hex(key: &SigningKey) -> String {
    sha256_hex(&key.verifying_key().to_bytes())
}

pub struct Signed {
    pub bytes: Vec<u8>,
    pub sha256: [u8; 32],
    pub ed25519: Vec<u8>,
    pub public_key: Vec<u8>,
}

impl Signed {
    pub fn reference(&self) -> KuroextRef {
        KuroextRef {
            url: "https://example.org/demo.kuroext".into(),
            sha256: self.sha256,
            ed25519: self.ed25519.clone(),
            public_key: self.public_key.clone(),
        }
    }
}

/// A `.kuroext` with `manifest` and `wasm`, its digest signed with `key`.
pub fn signed(key: &SigningKey, manifest: &str, wasm: &[u8]) -> Signed {
    let bytes = build_zip(&[("manifest.json", manifest.as_bytes()), ("source.wasm", wasm)]);
    let sha256: [u8; 32] = Sha256::digest(&bytes).into();
    Signed {
        ed25519: key.sign(&sha256).to_bytes().to_vec(),
        public_key: key.verifying_key().to_bytes().to_vec(),
        sha256,
        bytes,
    }
}

pub fn store_ref() -> StoreRef {
    StoreRef { index_url: INDEX_URL.into(), name: "Example".into() }
}

/// One store extension entry as the JSON store lists it. `kuroext` is the `resources.kuroext` block, if any.
pub fn entry(pkg: &str, name: &str, version_code: i64, langs: &[(&str, i64)], kuroext: Option<Value>) -> Value {
    let mut resources = json!({ "apkUrl": "https://example.org/a.apk", "iconUrl": "https://example.org/i.png" });
    if let Some(block) = kuroext {
        resources["kuroext"] = block;
    }
    let sources: Vec<Value> = langs
        .iter()
        .map(|(lang, id)| json!({ "id": id, "name": format!("{name} {lang}"), "language": lang, "homeUrl": "https://example.org" }))
        .collect();
    json!({
        "name": name, "packageName": pkg, "resources": resources, "extensionLib": "16",
        "versionCode": version_code, "versionName": "1.0.0", "contentWarning": "SAFE", "isTorrent": false,
        "sources": sources,
    })
}

/// A valid `resources.kuroext` block for a package signed with `key`.
pub fn block(key: &SigningKey) -> Value {
    let signed = signed(key, &manifest("1.0.0", 1), b"\0asm");
    json!({
        "url": "https://example.org/demo.kuroext",
        "sha256": signed.sha256.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        "ed25519": BASE64_STANDARD.encode(&signed.ed25519),
        "publicKey": BASE64_STANDARD.encode(&signed.public_key),
    })
}

/// The entries of a store document, mapped as the store fetch maps them.
pub fn parse_list(entries: Vec<Value>) -> Result<Vec<Available>, String> {
    let list: ExtensionListJson = serde_json::from_value(json!({ "extensions": entries })).map_err(|e| e.to_string())?;
    list.extensions.iter().map(|ext| map::available(ext, &store_ref()).map_err(|e| e.to_string())).collect()
}

/// One runnable listing of `pkg` at `version_code` with one English source.
pub fn listed(pkg: &str, name: &str, version_code: i64) -> Available {
    parse_list(vec![entry(pkg, name, version_code, &[("en", 1)], Some(block(&key(7))))]).expect("a valid entry").remove(0)
}

/// An installed package.
pub fn installed(pkg: &str, name: &str, version_code: i64) -> Installed {
    Installed {
        name: name.into(),
        pkg: pkg.into(),
        version_name: "1.0.0".into(),
        version_code,
        abi: AbiVersion { major: 1, minor: 0 },
        lang: "en".into(),
        nsfw: false,
        torrent: false,
        sources: vec![AnimeSourceRef { id: 1, name: name.into(), lang: "en".into(), base_url: String::new() }],
        icon: None,
        signer: SignerDigest(signer_hex(&key(7))),
        has_update: false,
        is_obsolete: false,
        store: None,
    }
}
