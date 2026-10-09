//! Extension packages. `anime` holds the area-19 `.kuroext` container, store protocol, trust rule and install pipeline.
//! The top-level modules are the area-18/20 `manifest.toml` directory loader. The WASM host is `kuro-source-wasm` (P2),
//! so a verified package is not instantiated here.

pub mod anime;
pub mod error;
pub mod icon;
pub mod install;
pub mod loader;
pub mod manifest;
pub mod notice;
pub mod registry;

pub use error::LoadError;
pub use install::{InstallError, InstallStep, install_package};
pub use loader::{COMPONENT_FILE, LoadResult, MANIFEST_FILE, discover, load_package};
pub use manifest::Manifest;
pub use registry::{Available, ExtensionRegistry, Installed, Snapshot, Untrusted};
