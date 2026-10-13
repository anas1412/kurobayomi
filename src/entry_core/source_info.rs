//! The source line of the info header (`SourceName` and the stub warning), for either kind.

/// The source line of the info header.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceInfo {
    pub name: String,
    /// True when the extension is not installed: the header shows a warning icon in the error color.
    pub stub: bool,
}
