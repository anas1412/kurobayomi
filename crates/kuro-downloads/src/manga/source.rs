//! The source port of a manga download and the page types its signatures use. For now these are the definitions of
//! `crate::source` and `crate::model`, so the app's `LiveChapterSource` serves both stacks; phase D moves the
//! definitions here.

pub use crate::model::{ChapterRef, Page, PageState};
pub use crate::source::{ChapterSource, FolderOnlySource, ImageResponse, ImageStream};
