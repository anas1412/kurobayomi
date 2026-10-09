//! Download policy (area 09 section 1, `download/`): the typed preferences, the new-item auto-download filter, and the
//! remove-after-read and speed-limit arithmetic. Downloading itself is `kuro-downloads` (areas 23 to 25).

pub mod filter;
pub mod policy;
pub mod prefs;
