//! TST-19-204 (T-19a-068): decoding a 16 MiB protobuf store stays under 128 MiB of resident memory.
//! This is its own test binary with a single test, so no other test's allocations move the process peak.

use kuro_extensions::anime::store::wire::{decode_store_pb, PbContact, PbExtension, PbExtensionList, PbResources, PbSource, PbStore};
use prost::Message;

const MIB: usize = 1024 * 1024;

/// A `Vm*` line of `/proc/self/status` in KiB.
fn status_kib(field: &str) -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("read /proc/self/status");
    let line = status.lines().find(|line| line.starts_with(field)).unwrap_or_else(|| panic!("no {field} line"));
    line.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or_else(|| panic!("bad {field} line: {line}"))
}

fn store_of_at_least(bytes: usize) -> Vec<u8> {
    let mut extensions = Vec::new();
    let mut size = 0;
    while size < bytes {
        let n = extensions.len();
        let ext = PbExtension {
            name: format!("Aniyomi: Extension number {n}"),
            package_name: format!("eu.kanade.tachiyomi.animeextension.lang{n}"),
            resources: Some(PbResources {
                apk_url: format!("https://example.org/apk/extension-{n}.apk"),
                icon_url: format!("https://example.org/icon/extension-{n}.png"),
            }),
            extension_lib: "16".into(),
            version_code: i64::try_from(n).unwrap_or(0),
            version_name: "16.1".into(),
            sources: vec![PbSource { id: i64::try_from(n).unwrap_or(0), name: format!("Source {n}"), language: "en".into(), home_url: "https://example.org".into(), ..PbSource::default() }],
            ..PbExtension::default()
        };
        size += ext.encoded_len();
        extensions.push(ext);
    }
    PbStore {
        name: "Big".into(),
        badge_label: "B".into(),
        signing_key: "k".into(),
        contact: Some(PbContact { website: "https://example.org".into(), discord: None }),
        extension_list: Some(PbExtensionList { extensions }),
        extension_list_url: None,
    }
    .encode_to_vec()
}

#[test]
#[ignore = "slow: run on demand"]
fn decoding_a_16_mib_store_stays_under_128_mib_of_rss() {
    let body = store_of_at_least(16 * MIB);
    assert!(body.len() >= 16 * MIB, "the fixture is {} bytes", body.len());
    // Reset the peak counter to the current RSS (Linux: writing 5 to clear_refs). If that is refused the peak still
    // includes the fixture build, which only makes the bound stricter.
    let _ = std::fs::write("/proc/self/clear_refs", "5");
    let before_kib = status_kib("VmRSS:");
    let store = decode_store_pb(&body).expect("the store decodes");
    let peak_kib = status_kib("VmHWM:");
    assert_eq!(store.extension_list.map(|list| list.extensions.len() > 1000), Some(true));
    let grown_mib = peak_kib.saturating_sub(before_kib) / 1024;
    eprintln!("decode grew RSS by {grown_mib} MiB");
    assert!(grown_mib < 128, "decoding grew the resident set by {grown_mib} MiB");
}
