//! Linux-specific integration: XDG directories, the Secret Service, disk space and the launcher badge.

pub mod appearance;
pub mod device;
pub mod launcher;
pub mod paths;
pub mod secrets;
pub mod secrets_file;
pub mod storage;

pub use paths::LinuxPaths;
pub use secrets::SecretService;
pub use secrets_file::EncryptedFileStore;
