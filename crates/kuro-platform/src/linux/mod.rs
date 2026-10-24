//! Linux-specific integration: MPRIS, the screen-saver inhibit, disk space, the launcher badge and the hidden web pages.

pub mod appearance;
pub mod device;
pub mod hidden_pages;
pub mod idle;
pub mod launcher;
pub mod media;
pub mod power_network;
pub mod storage;

pub use idle::ScreenSaver;
pub use media::Mpris;
