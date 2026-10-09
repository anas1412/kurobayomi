//! Why a package did not load into sources. The loader stores the text as `LoadError::Instantiate`.

/// A failure to load one package. The display text is the message the loader keeps.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WasmError {
    #[error("source.wasm is {size} bytes, the limit is {limit}")]
    TooLarge { size: usize, limit: usize },
    #[error("wasm engine: {0}")]
    Engine(String),
    #[error("component does not compile: {0}")]
    Compile(String),
    /// A missing export, a failed link or a failed `create-sources` or `info` call.
    #[error("{0}")]
    Instantiate(String),
}
