//! Launch intents (area 36, section 4): the pure route table of the requests that the argv, URI and GApplication code
//! parse, the notification dismiss, the extension search, the share and save flows, and the recorded drops of Android
//! features this port does not carry.
//!
//! The route table is pure and tested here. The window applies the steps it returns; the share flow is the GTK part.

pub mod drops;
pub mod route;
pub mod share;
