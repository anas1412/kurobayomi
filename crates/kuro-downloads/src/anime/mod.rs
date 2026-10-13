//! Anime download engine (area 24) on top of the shared queue pieces (area 25): `DownloadState`, the per-source
//! scheduler, the retry policy and the reader-facing error type. Folder names follow `AnimeDownloadProvider`.

pub mod cache;
pub mod delete_rules;
pub mod downloader;
pub mod enqueue;
pub mod ffmpeg;
pub mod layout;
pub mod lifecycle;
pub mod local_server;
pub mod manager;
pub mod model;
pub mod naming;
pub mod network;
pub mod notify;
pub mod offline;
pub mod pending;
pub mod progress;
pub mod resolve;
pub mod runner;
pub mod store;
pub mod torrent;
