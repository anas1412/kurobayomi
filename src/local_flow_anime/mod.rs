//! Local anime folders (wire-local-anime). The chosen root holds one folder per series, and each series folder holds its
//! video files as episodes. A scan reads them with kuro-local and writes the anime and episode rows under the local
//! source id, so the library and the entry page list them like any other entry.
//!
//! Pure rules: `plan` (which episode rows to add, update, remove) and `scan` (series folders and their rows). Database
//! work: `sync` (one scan written through kuro-data) and `open` (which episode a title and name refer to). The header
//! button is `button`.

pub mod button;
pub mod open;
pub mod plan;
pub mod scan;
pub mod sync;

pub use button::header_actions;
pub use open::{anime_id_for_title, open_episode, open_local_episode};
