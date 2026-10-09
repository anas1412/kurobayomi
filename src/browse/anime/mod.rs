//! The anime Browse tab (area 41, part 2). The anime sources list and its tab state, the browse page's rules, the anime
//! extension list model and the anime page loader. The kind-agnostic pieces (grouping, listing state, content plan,
//! toasts, favourites, nav rules, languages) are the parent module's and are reused unchanged. Nothing here builds a GTK
//! view or registers a screen yet, so the models have no caller outside their tests.
#![allow(dead_code)] // no view registers these yet, see the module note above

pub mod browse;
pub mod extensions;
pub mod loader;
pub mod sources;

#[cfg(test)]
mod fixtures;
