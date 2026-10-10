//! The torrent server of the player (area 45 `TorrentServerService.kt`, `TorrentServerApi.kt`, `TorrentServerUtils.kt`,
//! T-45a-072 to T-45a-078). TorrServer runs as a child process on loopback (C-18, area 13 section 4). The pure parts
//! (routing, links, trackers, proxy modes) need no server; `api` and `server` talk to it.

pub mod api;
pub mod links;
pub mod proxy;
pub mod route;
pub mod server;
pub mod service;
pub mod trackers;
