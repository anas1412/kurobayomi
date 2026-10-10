//! The registered migration steps.
//!
//! Empty on purpose. The 41 legacy Aniyomi migrations (versions 7 to 130) are not ported: each one
//! rewrites a key of Aniyomi's Android SharedPreferences file (renames, value codecs, features that
//! do not exist on Linux). Kurobayomi creates `config.toml` and `state.toml` fresh on first run, so
//! there is no old file for them to rewrite, and backup import (area 21) maps old keys itself.
//! The three always-migrations (scheduler set-up) are not preference rewrites. They live in
//! `kuro_core::migrate::list`, next to the framework that runs them.

use super::Migration;

pub const MIGRATIONS: &[Migration] = &[];
