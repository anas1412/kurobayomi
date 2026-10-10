//! The Upcoming page logic (T-42b-025 to T-42b-045): the entry query and grouping ([`model`], [`query`]), the
//! calendar math ([`calendar`]), the layout numbers ([`layout`]) and the row text ([`rows`]), under the widgets of the
//! calendar ([`calendar_view`]) and the page ([`page`]).
//!
//! The spec places this code in `src/upcoming/`; this build keeps it under `src/updates/` (the GUI lane owns only that
//! tree), so the page can mount it from `src/updates`.

pub mod calendar;
pub mod calendar_view;
pub mod layout;
pub mod model;
pub mod page;
pub mod query;
pub mod rows;
