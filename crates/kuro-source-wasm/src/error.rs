//! Why a package did not load into sources. The loader keeps the display text. `Instantiate` is
//! `LoadError::Instantiate`; `Manifest` and `NsfwDisabled` are the `LoadError` variants of the same names (design 12).

/// A failure to load one package. The display text is the message the loader keeps.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WasmError {
    #[error("source.wasm is {size} bytes, the limit is {limit}")]
    TooLarge { size: usize, limit: usize },
    #[error("wasm engine: {0}")]
    Engine(String),
    #[error("io: {0}")]
    Io(String),
    #[error("component does not compile: {0}")]
    Compile(String),
    /// A missing export, a failed link or a failed `create-sources` or `info` call. The display is
    /// design 12's `Extension load error: {cause}`; the data 13 form `Extension load error: $name ($pkg)`
    /// is the startup log line only (`startup.rs`), which knows the name and package.
    #[error("Extension load error: {0}")]
    Instantiate(String),
    /// The package's source list is not valid, for example a duplicate source id (T-18a-007).
    #[error("Invalid manifest: {0}")]
    Manifest(String),
    /// The package has no signer key (data 13: `Package $pkgName isn't signed`).
    #[error("Package {0} isn't signed")]
    NotSigned(String),
    /// The package's lib version is outside 1.4 to 1.5 (data 13).
    #[error("Lib version is {0}, while only versions 1.4 to 1.5 are allowed")]
    LibVersion(String),
    /// The file is not a `.kuroext` archive.
    #[error("Not an extension package")]
    NotAnExtension,
    /// A trusted NSFW package while the NSFW setting is off (T-18a-010).
    #[error("NSFW extension {0} not allowed")]
    NsfwDisabled(String),
}
