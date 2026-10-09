//! Process-restart snapshot of the playing episode (area 45 `PlayerViewModel` saved state; T-45b-030).
//! The file holds the episode id, the position in milliseconds and the quality index. It lives in
//! the state directory that the platform layer provides (`kuro_platform::paths::Paths::state_dir`).
//! The caller saves through a `WriteThrottle` of `SAVE_DEBOUNCE_MS`, and restores only when the
//! episode id matches.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::progress::WriteThrottle;

/// File name inside the state directory.
pub const FILE_NAME: &str = "player-session.json";
/// Debounce interval for snapshot writes.
pub const SAVE_DEBOUNCE_MS: i64 = 5000;
/// Deadline for the final progress save on window close (E-29).
pub const FINAL_SAVE_DEADLINE_MS: u64 = 2000;
/// Runtime shutdown timeout after the final save (E-29).
pub const SHUTDOWN_TIMEOUT_MS: u64 = 3000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub episode_id: i64,
    pub episode_position_ms: i64,
    pub quality_index: i64,
}

pub fn path(state_dir: &Path) -> PathBuf {
    state_dir.join(FILE_NAME)
}

pub fn encode(snapshot: &Snapshot) -> String {
    json!({
        "episode_id": snapshot.episode_id,
        "episode_position_ms": snapshot.episode_position_ms,
        "quality_index": snapshot.quality_index,
    })
    .to_string()
}

/// Reads a snapshot. Missing fields, wrong types and invalid JSON give `None`.
pub fn decode(text: &str) -> Option<Snapshot> {
    let value: Value = serde_json::from_str(text).ok()?;
    Some(Snapshot {
        episode_id: value.get("episode_id")?.as_i64()?,
        episode_position_ms: value.get("episode_position_ms")?.as_i64()?,
        quality_index: value.get("quality_index")?.as_i64()?,
    })
}

/// Writes snapshots through a `WriteThrottle` of `SAVE_DEBOUNCE_MS`. The window owns one and calls
/// `save` on every progress tick. A forced call (pause, close) always writes.
#[derive(Clone, Copy, Debug)]
pub struct SnapshotWriter {
    throttle: WriteThrottle,
}

impl Default for SnapshotWriter {
    fn default() -> Self {
        Self {
            throttle: WriteThrottle::new(SAVE_DEBOUNCE_MS),
        }
    }
}

impl SnapshotWriter {
    /// Writes the snapshot when the interval has passed or `forced`. Returns whether it wrote.
    pub fn save(
        &mut self,
        state_dir: &Path,
        snapshot: &Snapshot,
        now_ms: i64,
        forced: bool,
    ) -> io::Result<bool> {
        if !self.throttle.due(now_ms, forced) {
            return Ok(false);
        }
        save(state_dir, snapshot)?;
        Ok(true)
    }
}

pub fn save(state_dir: &Path, snapshot: &Snapshot) -> io::Result<()> {
    fs::create_dir_all(state_dir)?;
    fs::write(path(state_dir), encode(snapshot))
}

/// The saved snapshot, or `None` when the file is absent or unreadable.
pub fn load(state_dir: &Path) -> Option<Snapshot> {
    decode(&fs::read_to_string(path(state_dir)).ok()?)
}

/// `(position_ms, quality_index)` to restore for `episode_id`. Only a snapshot for that same
/// episode restores anything.
pub fn restore_for(snapshot: &Snapshot, episode_id: i64) -> Option<(i64, i64)> {
    (snapshot.episode_id == episode_id)
        .then_some((snapshot.episode_position_ms, snapshot.quality_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Snapshot {
        Snapshot {
            episode_id: 42,
            episode_position_ms: 615_000,
            quality_index: 2,
        }
    }

    #[test]
    fn snapshot_round_trips_through_the_state_directory() {
        let dir = tempfile::tempdir().unwrap();
        save(dir.path(), &sample()).unwrap();
        assert_eq!(load(dir.path()), Some(sample()));
    }

    #[test]
    fn writer_coalesces_to_five_seconds_and_forced_writes_always_land() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer = SnapshotWriter::default();
        let at = |position_ms| Snapshot {
            episode_position_ms: position_ms,
            ..sample()
        };
        assert!(writer.save(dir.path(), &at(1), 0, false).unwrap());
        assert!(!writer.save(dir.path(), &at(2), 4999, false).unwrap());
        assert_eq!(load(dir.path()).unwrap().episode_position_ms, 1);
        assert!(writer.save(dir.path(), &at(3), 5000, false).unwrap());
        assert!(writer.save(dir.path(), &at(4), 5001, true).unwrap());
        assert_eq!(load(dir.path()).unwrap().episode_position_ms, 4);
    }

    #[test]
    fn restore_only_for_the_same_episode() {
        assert_eq!(restore_for(&sample(), 42), Some((615_000, 2)));
        assert_eq!(restore_for(&sample(), 7), None);
    }

    #[test]
    fn bad_or_missing_fields_give_none() {
        assert_eq!(decode("not json"), None);
        assert_eq!(decode(r#"{"episode_id": 1}"#), None);
        assert_eq!(
            decode(r#"{"episode_id": "1", "episode_position_ms": 0, "quality_index": 0}"#),
            None
        );
    }

    #[test]
    fn missing_file_loads_as_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), None);
    }
}
