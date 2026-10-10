//! The guest sandbox seen from outside the crate (T-18a-044, edge cases 12.9): a guest that traps, spins or grows
//! memory fails its own load with an error and stops the host from nothing else, an unknown export fails the load, and a
//! host outside the `hosts` allow-list is refused. The guests are the WAT components of `src/test_guest.rs`, compiled
//! into this file, so there is no guest toolchain and no binary fixture. The call that traps after a good load needs the
//! crate's private source type, so that case (`a_trapping_call_is_a_source_error_and_the_source_still_answers`) stays
//! in `src/source.rs`.
//!
//! | 12.9 case | here |
//! |---|---|
//! | `trap_becomes_source_error` (W1) | `a_trapping_guest_is_an_error_not_a_panic` (at load); the call-time case is in `src/source.rs` |
//! | `spin_hits_epoch_deadline` (W2) | `a_spinning_guest_is_stopped_by_the_epoch_deadline` (5 ticks) |
//! | `memory_limit_denies_growth` (W3) | `a_guest_that_grows_past_its_memory_limit_traps` |
//! | `disallowed_host_is_rejected` (W4) | `a_host_outside_the_allow_list_is_refused_and_named` |
//! | `unknown_export_fails_load` (W7) | `a_package_without_invoke_fails_with_the_missing_export_text` |

#[path = "../src/test_guest.rs"]
mod test_guest;

use std::time::{Duration, Instant};

use kuro_source_wasm::WasmError;
use kuro_source_wasm::bridge::check_host;
use kuro_source_wasm::source::{instantiate, instantiate_with_deadline};
use test_guest::{Behaviour, env, guest, without_invoke};
use url::Url;

fn hosts(list: &[&str]) -> Vec<String> {
    list.iter().map(|h| (*h).to_owned()).collect()
}

#[test]
fn a_host_outside_the_allow_list_is_refused_and_named() {
    let allowed = hosts(&["api.example.test"]);
    let url = Url::parse("https://evil.test/feed").unwrap();
    assert_eq!(
        check_host(&allowed, &url),
        Err("host not allowed: evil.test".to_owned())
    );
}

#[test]
fn an_allowed_host_passes_and_a_wildcard_covers_its_subdomains_only() {
    let allowed = hosts(&["api.example.test", "*.cdn.example.test"]);
    assert!(check_host(&allowed, &Url::parse("https://api.example.test/x").unwrap()).is_ok());
    assert!(
        check_host(
            &allowed,
            &Url::parse("https://img.cdn.example.test/x").unwrap()
        )
        .is_ok()
    );
    assert!(
        check_host(
            &allowed,
            &Url::parse("https://cdn.example.test.evil/x").unwrap()
        )
        .is_err()
    );
}

#[test]
fn an_empty_allow_list_means_no_network_at_all() {
    let url = Url::parse("https://api.example.test/").unwrap();
    assert_eq!(
        check_host(&[], &url),
        Err("host not allowed: api.example.test".to_owned())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_trapping_guest_is_an_error_not_a_panic() {
    let (host_env, _dir) = env();
    let outcome = instantiate(guest(Behaviour::Traps), Vec::new(), false, host_env).await.map(|sources| sources.len());
    match outcome {
        Err(WasmError::Instantiate(text)) => assert!(text.contains("unreachable"), "{text}"),
        other => panic!("expected the trap as an error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_spinning_guest_is_stopped_by_the_epoch_deadline() {
    let started = Instant::now();
    let (host_env, _dir) = env();
    let outcome = instantiate_with_deadline(guest(Behaviour::Spins), Vec::new(), false, host_env, 5)
        .await
        .map(|sources| sources.len());
    match outcome {
        Err(WasmError::Instantiate(text)) => assert!(text.contains("interrupt"), "the epoch deadline stops it: {text}"),
        other => panic!("expected the deadline to stop the guest, got {other:?}"),
    }
    assert!(started.elapsed() < Duration::from_secs(20), "stopped by the deadline, not by the test");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_guest_that_grows_past_its_memory_limit_traps() {
    let (host_env, _dir) = env();
    let outcome = instantiate(guest(Behaviour::Grows), Vec::new(), false, host_env).await.map(|sources| sources.len());
    match outcome {
        Err(WasmError::Instantiate(text)) => assert!(text.contains("unreachable"), "the grow refusal traps: {text}"),
        other => panic!("expected a trap, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_package_without_invoke_fails_with_the_missing_export_text() {
    let (host_env, _dir) = env();
    let outcome = instantiate(without_invoke(), Vec::new(), false, host_env).await.map(|sources| sources.len());
    assert_eq!(outcome.err(), Some(WasmError::Instantiate("missing export invoke".into())));
}
