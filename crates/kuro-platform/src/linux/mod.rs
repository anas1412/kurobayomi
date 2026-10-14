//! Linux-specific integration: XDG directories, the Secret Service, MPRIS, the screen-saver inhibit, disk space and the launcher badge.

pub mod appearance;
pub mod device;
pub mod idle;
pub mod launcher;
pub mod media;
pub mod paths;
pub mod power_network;
pub mod secrets;
pub mod secrets_file;
pub mod storage;

pub use idle::ScreenSaver;
pub use media::Mpris;
pub use paths::LinuxPaths;
pub use secrets::SecretService;
pub use secrets_file::EncryptedFileStore;
