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
    pref.extend(entries.iter().cloned()).expect("the in-memory store accepts the trust entries");
    crate::anime::trust::TrustStore::new(pref)
}

/// One event a [`capture`] call recorded: its level, its message and its fields as text.
#[derive(Debug, Clone, PartialEq)]
pub struct Logged {
    pub level: tracing::Level,
    pub message: String,
    pub fields: Vec<(String, String)>,
}

#[derive(Clone, Default)]
struct Recorder(std::sync::Arc<std::sync::Mutex<Vec<Logged>>>);

#[derive(Default)]
struct Fields {
    message: String,
    fields: Vec<(String, String)>,
}

impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{value:?}");
        } else {
            self.fields.push((field.name().to_owned(), format!("{value:?}")));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_owned();
        } else {
            self.fields.push((field.name().to_owned(), value.to_owned()));
        }
    }
}

thread_local! {
    /// The recorder of the capture that is active on this thread, if any.
    static SINK: std::cell::RefCell<Option<Recorder>> = const { std::cell::RefCell::new(None) };
}

/// The one global layer of the test binary. It hands each event to the capture of the logging thread, so a callsite hit
/// first by a test with no capture is never cached as disabled for the tests that do capture.
struct Router;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for Router {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        SINK.with(|sink| {
            if let Some(recorder) = &*sink.borrow() {
                let mut fields = Fields::default();
                event.record(&mut fields);
                let logged =
                    Logged { level: *event.metadata().level(), message: fields.message, fields: fields.fields };
                recorder.0.lock().expect("the recorder lock").push(logged);
            }
        });
    }
}

/// Records the events logged on this thread until it is dropped. Use it in async tests, where the logs happen across
/// awaits on the same thread.
pub struct Capture {
    recorder: Recorder,
}

impl Capture {
    /// Every event recorded so far.
    pub fn events(&self) -> Vec<Logged> {
        self.recorder.0.lock().expect("the recorder lock").clone()
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        SINK.with(|sink| *sink.borrow_mut() = None);
    }
}

/// Starts recording the events logged on this thread.
pub fn start_capture() -> Capture {
    use tracing_subscriber::layer::SubscriberExt;
    static INSTALL: std::sync::Once = std::sync::Once::new();
    INSTALL.call_once(|| {
        let _ = tracing::subscriber::set_global_default(tracing_subscriber::registry().with(Router));
    });
    let recorder = Recorder::default();
    SINK.with(|sink| *sink.borrow_mut() = Some(recorder.clone()));
    Capture { recorder }
}

/// Runs `f` with a recording subscriber on this thread and returns its result with every event `f` logged.
pub fn capture<T>(f: impl FnOnce() -> T) -> (T, Vec<Logged>) {
    let recording = start_capture();
    let result = f();
    (result, recording.events())
}

/// A one-connection HTTP server for a download that stalls mid-body. It sends the headers and `first`, then waits.
/// Sending on the returned handle releases `rest`; dropping it leaves the body unfinished and closes the connection.
/// The URL is `http://127.0.0.1:<port>/pkg.kuroext`.
pub async fn stalled_server(first: Vec<u8>, rest: Vec<u8>) -> (String, tokio::sync::oneshot::Sender<()>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("the test server binds");
    let url = format!("http://{}/pkg.kuroext", listener.local_addr().expect("the test server has an address"));
    let (release, released) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut request = [0u8; 4096];
        let _ = socket.read(&mut request).await;
        let head = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", first.len() + rest.len());
        if socket.write_all(head.as_bytes()).await.is_err() || socket.write_all(&first).await.is_err() {
            return;
        }
        if released.await.is_ok() {
            let _ = socket.write_all(&rest).await;
        }
    });
    (url, release)
}

/// Polls `condition` every 5 ms and panics after five seconds, so a test that waits for a state cannot hang.
pub async fn wait_until(mut condition: impl FnMut() -> bool) {
    for _ in 0..1000 {
        if condition() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("the awaited state never came");
}

/// True when `dir` holds a file with at least one byte.
pub fn has_nonempty_file(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| entries.filter_map(Result::ok).any(|entry| entry.metadata().is_ok_and(|meta| meta.len() > 0)))
        .unwrap_or(false)
}
