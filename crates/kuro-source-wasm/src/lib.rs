//! WASM source host (areas 18, 20): runs extension guests behind the `kuro-source` traits. P2, feature-gated.
//!
//! A package is one `source.wasm` component that implements the `kuro:source@1.4.0` world (`wit/kuro-source.wit`).
//! [`instantiate`] compiles it, asks it for its sources, and returns each one as a `CatalogueSource`. Every guest call
//! runs on a blocking thread, behind a per-source mutex, under an epoch deadline and a memory limit (see [`limits`]).

// the shared WAT guests are also compiled into tests/sandbox.rs, where they name this crate by its own name
extern crate self as kuro_source_wasm;

pub mod bridge;
pub mod error;
pub mod install;
pub mod limits;
pub mod manifest;
pub mod package;
pub mod source;
pub mod startup;
pub mod verify;
pub mod wire;

#[cfg(test)]
mod test_guest;

mod bindings {
    wasmtime::component::bindgen!({ path: "wit", world: "source" });
}

pub use error::WasmError;
pub use source::{HostEnv, WasmSource, instantiate};
