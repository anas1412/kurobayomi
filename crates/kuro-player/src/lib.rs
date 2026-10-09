//! Player core (areas 45, 46, 48): libmpv2 wrapper, startup option table, event observer,
//! URL and load helpers, key maps and the UI state model. No GTK here. The GL area, windows and
//! MPRIS sit in the GTK and platform layers. Pure modules are tested without libmpv; `mpv` tests
//! need a system libmpv.

pub mod aspect;
pub mod assets;
pub mod config;
pub mod controls;
pub mod enums;
pub mod episodes;
pub mod errors;
pub mod hosters;
pub mod keys;
pub mod load;
pub mod mpv;
pub mod observer;
pub mod options;
pub mod progress;
pub mod source_url;
pub mod state;
pub mod time;
pub mod tracks;
pub mod transport;
pub mod types;
pub mod window;
