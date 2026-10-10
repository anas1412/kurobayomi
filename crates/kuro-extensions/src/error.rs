//! Why one extension package cannot be used (`p1-edge-cases-and-tests.md` E1.12, E1.13 and the manifest rules).

/// A package directory that the loader cannot use. The directory stays listed so the UI can show the error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadError {
    #[error("manifest.toml is not valid: {0}")]
    Manifest(String),
    #[error("the api version {0} is not supported; only 1.x is")]
    UnsupportedApi(String),
    #[error("component.wasm is missing")]
    MissingComponent,
    #[error("the component does not instantiate: {0}")]
    Instantiate(String),
    #[error("could not read {0}")]
    Io(String),
}
