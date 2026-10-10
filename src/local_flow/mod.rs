//! Local manga folders (plan areas 14, 43 and 45). `scan` reads a folder with kuro-local, `sync` stores it under the
//! built-in local source id, `reader` opens its chapters, and `ui` holds the window actions. Scan and sync take no GTK
//! types, so their tests run without a display.

pub mod cover;
pub mod open;
pub mod reader;
pub mod scan;
pub mod sync;
pub mod ui;

#[cfg(test)]
mod integration;
#[cfg(test)]
mod tests;
