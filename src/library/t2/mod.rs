//! The unit tests of `p1-edge-cases-and-tests.md` section T-2, one `t2_<area>_<nn>` test per id (test-only).
//! They run with `cargo test -p kurobayomi --bin kurobayomi library::`:
//! the package is a binary, so there is no `--lib` target.

mod bulk;
mod catscr;
mod category;
mod codec;
mod filter;
mod pipeline;
mod search;
mod selection;
mod sort;
mod ui;
