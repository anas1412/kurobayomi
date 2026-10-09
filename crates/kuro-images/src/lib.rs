//! Covers, image decode, disk and memory caches, and cover file layout (area:34).

pub mod art;
pub mod buffered;
pub mod chapter_cache;
pub mod cover_file;
pub mod cover_image;
pub mod cover_paths;
pub mod decode;
pub mod disk_image_cache;
pub mod error;
pub mod exception;
pub mod fetch;
pub mod format;
pub mod http;
pub mod invalidate;
pub mod keys;
pub mod loader;
pub mod memory_cache;
pub mod options;
pub mod platform;
pub mod resource;
pub mod sample;
pub mod save;
pub mod share;
pub mod sniff;
#[cfg(test)]
mod testing;
pub mod texture;

pub use art::{ArtKind, ArtRef, Owner};
pub use decode::{DecodeOptions, Decoded};
pub use error::ImageError;
pub use fetch::{
    DataSource, Fetched, ImageService, LoadRequest, LoadedImage, Payload, SourceClient, SourceHttp,
};
pub use options::{CachePolicy, LoadOptions, RequestSize, Scale};
