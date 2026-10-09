//! The anime package and store path end to end (T-19a-069), run headless against a local HTTP server.

use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::anime::install::{Begin, InstallDirs, InstallTracker, install};
use crate::anime::loader::{LoadContext, PackageDir};
use crate::anime::manager::{ExtensionState, SourceHandle};
use crate::anime::model::{InstallStep, LoadResult};
use crate::anime::testkit::test_client;
use crate::anime::store::refresh::fetch_all;
use crate::anime::testkit::{
    MANIFEST, PKG, SOURCE_ID, fixture_key, kuroext_block, other_key, signed_package, signer_hex, store_bytes,
    store_document, store_extension, trust_with_entries,
};

async fn serve_json(server: &MockServer, route: &str, body: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path(route))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_package_installs_loads_after_a_restart_and_goes_obsolete_when_its_store_is_removed() {
    let server = MockServer::start().await;
    let store_a_key = signer_hex(&fixture_key());
    let store_b_key = signer_hex(&other_key());
    let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
    let block = kuroext_block(&fixture, &format!("{}/pkg/demo.kuroext", server.uri()));
    let store_a = store_document("Store A", &store_a_key, vec![store_extension(PKG, 7, Some(block))]);
    let store_b =
        store_document("Store B", &store_b_key, vec![store_extension("eu.example.animeext.en.other", 1, None)]);
    serve_json(&server, "/a.json", store_bytes(&store_a)).await;
    serve_json(&server, "/b.json", store_bytes(&store_b)).await;
    serve_json(&server, "/pkg/demo.kuroext", fixture.bytes.clone()).await;
    let url_a = format!("{}/a.json", server.uri());
    let url_b = format!("{}/b.json", server.uri());
    let client = test_client();
    let cancel = CancellationToken::new();

    // First run: refresh both stores, install the package.
    let aggregate = fetch_all(&client, &[url_a.clone(), url_b.clone()], &cancel).await.unwrap();
    assert_eq!(aggregate.stores.len(), 2);
    assert!(aggregate.failures.is_empty());
    let listed = aggregate.available.iter().find(|ext| ext.pkg == PKG).expect("listed").clone();
    let store_key = aggregate.stores.iter().find(|row| row.index_url == url_a).unwrap().signing_key.clone();
    let work = tempfile::tempdir().unwrap();
    let package_dir = work.path().join("extensions").join("anime");
    let download_dir = work.path().join("cache").join("ext-download");
    let lock = tokio::sync::Mutex::new(());
    let mut tracker = InstallTracker::new();
    let ticket = match tracker.begin(PKG) {
        Begin::Started(ticket) => ticket,
        other => panic!("{other:?}"),
    };
    let mut steps = Vec::new();
    let dirs = InstallDirs { package_dir: &package_dir, download_dir: &download_dir };
    let placed = install(&client, dirs, &listed, &store_key, &lock, &ticket.cancel, &mut |step| {
        steps.push(step);
        tracker.set_step(&ticket, step);
    })
    .await;
    assert!(placed.is_ok(), "{placed:?}");
    assert_eq!(steps.last(), Some(&InstallStep::Installed));

    // Restart: a fresh state loads the package from disk, trusted by store A's key.
    let trust = trust_with_entries(&[]);
    let stores = aggregate.stores.clone();
    let ctx = LoadContext { trust: &trust, stores: &stores, nsfw_allowed: true };
    let mut state = ExtensionState::new();
    state.apply_loaded(PackageDir::new(&package_dir).scan(&ctx));
    state.apply_available(aggregate.available.clone());
    let loaded = state.installed().get(PKG).expect("loads as installed");
    assert_eq!(loaded.version_code, 7);
    assert_eq!(loaded.store.as_ref().map(|store| store.index_url.as_str()), Some(url_a.as_str()));
    assert!(!loaded.is_obsolete);
    match state.source_for_id(SOURCE_ID) {
        Some(handle @ SourceHandle::Installed { .. }) => assert_eq!(handle.installed().unwrap().0, PKG),
        other => panic!("source should resolve to the installed handle, got {other:?}"),
    }

    // Store A is removed: the next refresh marks the package obsolete, and its trust is gone with the store.
    let after = fetch_all(&client, std::slice::from_ref(&url_b), &cancel).await.unwrap();
    state.apply_available(after.available.clone());
    assert!(state.installed()[PKG].is_obsolete, "no store lists it any more");
    let ctx_without_a = LoadContext { trust: &trust, stores: &after.stores, nsfw_allowed: true };
    let rescanned = PackageDir::new(&package_dir).scan(&ctx_without_a);
    assert!(matches!(rescanned.as_slice(), [LoadResult::Untrusted(_)]), "{rescanned:?}");
}
