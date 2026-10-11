//! Extension packages. `anime` holds the area-19 `.kuroext` container, store protocol, trust rule and install pipeline.
//! The top-level modules are the area-18/20 `manifest.toml` directory loader. A verified package is instantiated through the
//! WASM host (`kuro-source-wasm`) with the `wasm` feature, which is on by default (`instantiate`).

pub mod anime;
pub mod error;
pub mod fuzz;
pub mod icon;
pub mod install;
pub mod loader;
pub mod manifest;
pub mod notice;
pub mod registry;
pub mod repo;
pub mod watch;
#[cfg(feature = "wasm")]
pub mod instantiate;

pub use error::LoadError;
pub use install::{InstallError, InstallStep, install_package};
pub use loader::{COMPONENT_FILE, LoadResult, MANIFEST_FILE, discover, load_package};
pub use manifest::Manifest;
pub use registry::{Available, ExtensionRegistry, Installed, Snapshot, Untrusted};
