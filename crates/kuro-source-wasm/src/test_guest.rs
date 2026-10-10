//! WAT guests for the host tests. Each one is a component whose `invoke` does one thing: return text, return an error,
//! trap, spin, or grow memory. They are text, so the tests need no toolchain and no binary fixtures.

use std::fmt::Write;

/// What the guest's `invoke` does.
pub enum Behaviour {
    /// Returns `Ok(text)` for every op.
    Returns(String),
    /// Returns `Err(text)` for every op.
    Fails(String),
    /// Returns its own `input-json` as the `Ok` text.
    Echoes,
    /// Traps at once.
    Traps,
    /// Loops forever, so only the epoch deadline stops it.
    Spins,
    /// Grows memory until the store refuses, then traps.
    Grows,
}

/// The component bytes (WAT text, which `Component::new` accepts with the `wat` feature).
pub fn guest(behaviour: Behaviour) -> Vec<u8> {
    let (payload, body) = match behaviour {
        Behaviour::Returns(text) => (text, returns(0)),
        Behaviour::Fails(text) => (text, returns(1)),
        Behaviour::Echoes => (String::new(), echoes()),
        Behaviour::Traps => (String::new(), "(unreachable)".to_owned()),
        Behaviour::Spins => (String::new(), "(loop $spin (br $spin))".to_owned()),
        Behaviour::Grows => (String::new(), grows()),
    };
    template(&payload, &body).into_bytes()
}

/// A component that exports nothing the host calls. Loading it must fail with the missing-export error.
pub fn without_invoke() -> Vec<u8> {
    b"(component (core module $m (memory (export \"memory\") 1)))".to_vec()
}

/// Writes `Ok` (`discriminant` 0) or `Err` (1) with the payload at offset 0 into the return area.
fn returns(discriminant: u8) -> String {
    format!(
        "(i32.store (i32.const {RET_AREA}) (i32.const {discriminant}))\n\
         (i32.store offset=4 (i32.const {RET_AREA}) (i32.const 0))\n\
         (i32.store offset=8 (i32.const {RET_AREA}) (i32.const PAYLOAD_LEN))"
    )
}

/// Returns the input string as the `Ok` text. The lifted string points into memory the host wrote.
fn echoes() -> String {
    format!(
        "(i32.store (i32.const {RET_AREA}) (i32.const 0))\n\
         (i32.store offset=4 (i32.const {RET_AREA}) (local.get 2))\n\
         (i32.store offset=8 (i32.const {RET_AREA}) (local.get 3))"
    )
}

fn grows() -> String {
    "(block $done
       (loop $grow
         (br_if $done (i32.eq (memory.grow (i32.const 16)) (i32.const -1)))
         (br $grow)))
     (unreachable)"
        .to_owned()
}

/// Where the guest writes its `result<string, string>`. The payload sits at offset 0, and the host's allocations start
/// at 128 KiB, so this area lies between them.
const RET_AREA: u32 = 65536;

/// The component around one `invoke` body. A lifted export with a multi-value result returns a pointer to that result.
fn template(payload: &str, body: &str) -> String {
    let len = payload.len();
    let body = body.replace("PAYLOAD_LEN", &len.to_string());
    let mut data = String::new();
    for byte in payload.bytes() {
        write!(data, "\\{byte:02x}").expect("writing to a String");
    }
    format!(
        r#"(component
  (core module $m
    (memory (export "memory") 4)
    (global $heap (mut i32) (i32.const 131072))
    (data (i32.const 0) "{data}")
    (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
      (local $p i32)
      (local.set $p (global.get $heap))
      (global.set $heap (i32.add (local.get $p) (local.get 3)))
      (local.get $p))
    (func (export "invoke") (param i32 i32 i32 i32) (result i32)
      {body}
      (i32.const {RET_AREA})))
  (core instance $i (instantiate $m))
  (alias core export $i "memory" (core memory $mem))
  (alias core export $i "cabi_realloc" (core func $realloc))
  (alias core export $i "invoke" (core func $invoke))
  (func (export "invoke") (param "op" string) (param "input-json" string) (result (result string (error string)))
    (canon lift (core func $invoke) (memory $mem) (realloc (func $realloc))))
)
"#
    )
}

/// The environment every test guest gets: an in-memory preference store, and a client that makes no request on its own.
/// The directory holds the cookie file, so the caller keeps it alive for the test.
pub fn env() -> (kuro_source_wasm::source::HostEnv, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("temp dir");
    let prefs = kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory());
    let (toaster, _feed) = kuro_platform::toast::toast_channel();
    let client = kuro_net::NetworkClient::new(
        &prefs,
        dir.path().join("cookies.json"),
        std::sync::Arc::new(kuro_platform::webview::NoWebView),
        toaster,
    )
    .expect("client");
    let host_env = kuro_source_wasm::source::HostEnv {
        client,
        prefs: kuro_prefs::store::PrefStore::in_memory(),
    };
    (host_env, dir)
}
