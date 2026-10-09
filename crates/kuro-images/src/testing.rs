//! Fixture helpers for image tests (test builds only).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use http::HeaderMap;
use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;
use kuro_platform::toast::toast_channel;
use kuro_platform::webview::NoWebView;
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::store::PrefStore;

use crate::fetch::{SourceClient, SourceHttp};

/// The first bytes of a JPEG file. Enough for `sniff`.
pub const JPEG: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00,
];

/// Paths under one root, laid out like `LinuxPaths` (`data`, `cache`, ...).
pub struct TestPaths {
    root: PathBuf,
}

impl TestPaths {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }
}

impl Paths for TestPaths {
    fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    fn log_dir(&self) -> PathBuf {
        self.root.join("state/logs")
    }

    fn temp_dir(&self) -> PathBuf {
        self.root.join("cache/tmp")
    }

    fn crash_dir(&self) -> PathBuf {
        self.root.join("state/crashes")
    }

    fn default_downloads_dir(&self) -> PathBuf {
        self.root.join("downloads")
    }

    fn default_backups_dir(&self) -> PathBuf {
        self.root.join("data/backups")
    }

    fn covers_dir(&self) -> PathBuf {
        self.root.join("cache/covers")
    }

    fn plugin_dir(&self) -> PathBuf {
        self.root.join("data/plugins")
    }
}

/// A network client with the default transport, in-memory preferences and a cookie file under `dir`.
pub fn test_client(dir: &Path) -> NetworkClient {
    let prefs = NetworkPrefs::new(PrefStore::in_memory());
    NetworkClient::new(
        &prefs,
        dir.join("cookies.json"),
        Arc::new(NoWebView),
        toast_channel().0,
    )
    .expect("test network client")
}

/// No source has an HTTP client, so every request uses the shared client.
pub struct NoSources;

impl SourceHttp for NoSources {
    fn http_source(&self, _source_id: i64) -> Option<SourceClient> {
        None
    }
}

/// One HTTP source with its own client and headers.
pub struct FixedSource {
    pub id: i64,
    pub client: NetworkClient,
    pub headers: HeaderMap,
}

impl SourceHttp for FixedSource {
    fn http_source(&self, source_id: i64) -> Option<SourceClient> {
        (source_id == self.id).then(|| SourceClient {
            client: self.client.clone(),
            headers: self.headers.clone(),
        })
    }
}

/// A `w` x `h` RGBA gradient with a varying alpha, for encode and decode round trips.
pub fn gradient(w: u32, h: u32) -> image::RgbaImage {
    image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([
            (x * 255 / w.max(1)) as u8,
            (y * 255 / h.max(1)) as u8,
            90,
            200,
        ])
    })
}

/// The image encoded in `format`. JPEG has no alpha, so it is encoded as RGB.
pub fn encode(img: &image::RgbaImage, format: image::ImageFormat) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    let dynamic = image::DynamicImage::ImageRgba8(img.clone());
    let dynamic = if format == image::ImageFormat::Jpeg {
        image::DynamicImage::ImageRgb8(dynamic.to_rgb8())
    } else {
        dynamic
    };
    dynamic
        .write_to(&mut out, format)
        .expect("encode test image");
    out.into_inner()
}

/// A fixture from `tests/fixtures`.
pub fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .expect("fixture file")
}
