//! Android-only domain pieces this crate does not port. Each row says what the user loses (nothing, unless noted).
//!
//! * Release install (T-07a-037). No in-place install: the package manager owns files on Android, and on Linux the
//!   user installs from the release page. The user loses one-tap self-update and the update notification. The
//!   replacement is a "new version available" banner with an "open release page" button. Only the host's architecture
//!   is read (`std::env::consts::ARCH`), so no ABI list is kept, and no code path downloads a binary.
//! * Release check (T-09a-046, T-07a-042). The result has two outcomes, `NoNewUpdate` and `NewUpdate`. The removed
//!   "OS too old" outcome was never produced upstream.
//! * Dead anime helper (T-07a-042). `NetworkToLocalAnime` does not take the unused source manager.
//! * Compose and DI annotations (T-07a-038, T-08a-042, T-09a-054). Compose stability hints, Java serialization and
//!   the Android dependency-injection factories have no Rust counterpart. Entries are plain `Clone` structs shared as
//!   `Arc`. Each former factory line has a replacement: a free function taking the repository, or a method of
//!   [`crate::app::Domain`]. Track models derive `serde` where a backup needs them. The user loses nothing.
//! * Logging (T-07a-039, T-08a-042). Errors go to `tracing` with their chain. Background work that must outlive the
//!   dialog that started it runs as a detached task (`app::rx`), so closing the dialog does not stop an apply-to-all.
//! * Widgets (T-08a-038). No launcher widget reads the updates feed. The Updates tab and the library-update
//!   notification show the same rows. A future tray or Plasma widget can use `updates::window::three_months_ago`
//!   with `UpdatesRepo::with_read(false, ...)`.
//! * Storage (T-09a-050). Content URIs and remote locations are refused with `StorageUnavailable`. The media-scanner
//!   marker only stops Android's media scanner, which Linux desktop indexers ignore, so it is not written.
//! * Source flag (T-09a-056). A source cannot be excluded from the image data saver. Backups from forks that carry
//!   that flag are read and the field is ignored.
//! * Android construct removal (T-10a-035). The legacy chapter and episode conversions, the background-work and
//!   package-info calls, and the Compose image type are not in this crate. Delayed tracking survives a restart through
//!   `Domain::start`.
