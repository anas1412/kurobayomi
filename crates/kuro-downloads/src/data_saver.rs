//! Data-saver proxy for page downloads (T-23a-017; `aniyomi/util/DataSaver.kt`).
//!
//! Two services are kept: Bandwidth Hero and wsrv.nl. resmush.it is dropped (decision D-DL-09): it is plain HTTP and
//! the Kotlin code reads its JSON by string search. The service defaults to `None`, so the downloader fetches the original
//! image URL unless the user chose a service. The cache key stays the original URL; only the request changes.

use std::collections::BTreeSet;

use url::form_urlencoded;

/// The proxy service (`SourcePreferences.DataSaver`, without resmush.it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Service {
    #[default]
    None,
    BandwidthHero,
    WsrvNl,
}

/// The data-saver preferences the downloader reads. The defaults are the Kotlin defaults.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataSaver {
    /// `data_saver`.
    pub service: Service,
    /// `data_saver_downloader`: when off, the downloader never uses the service.
    pub downloader_enabled: bool,
    /// `data_saver_excluded`: source ids that never use the service.
    pub excluded_sources: BTreeSet<i64>,
    /// `data_saver_server`: the Bandwidth Hero base URL. Trailing slashes are removed.
    pub server: String,
    /// `data_saver_image_quality`.
    pub quality: i64,
    /// `data_saver_ignore_jpeg`: leave JPEG pages unchanged.
    pub ignore_jpeg: bool,
    /// `data_saver_ignore_gif`: leave GIF pages unchanged.
    pub ignore_gif: bool,
    /// `data_saver_image_format_jpeg`: convert to JPEG instead of WebP.
    pub format_jpeg: bool,
    /// `data_saver_color_bw`: Bandwidth Hero only.
    pub color_bw: bool,
}

impl Default for DataSaver {
    fn default() -> Self {
        Self {
            service: Service::None,
            downloader_enabled: true,
            excluded_sources: BTreeSet::new(),
            server: String::new(),
            quality: 80,
            ignore_jpeg: false,
            ignore_gif: true,
            format_jpeg: false,
            color_bw: false,
        }
    }
}

impl DataSaver {
    /// `true` when the downloader uses the service for this source (`DataSaver(source, prefs)` with the downloader
    /// switch applied). The service is `None` by default, so this is `false` by default.
    pub fn is_active_for(&self, source_id: i64) -> bool {
        self.downloader_enabled && self.service != Service::None && !self.excluded_sources.contains(&source_id)
    }

    /// The URL to request for `image_url`. It is `image_url` itself when the service does not apply.
    pub fn request_url(&self, source_id: i64, image_url: &str) -> String {
        if !self.is_active_for(source_id) {
            return image_url.to_owned();
        }
        match self.service {
            Service::None => image_url.to_owned(),
            Service::BandwidthHero => self.bandwidth_hero_url(image_url),
            Service::WsrvNl => self.wsrv_url(image_url),
        }
    }

    fn bandwidth_hero_url(&self, image_url: &str) -> String {
        let server = self.server.trim_end_matches('/');
        if server.trim().is_empty() || image_url.contains(server) {
            return image_url.to_owned();
        }
        if is_jpeg(image_url) {
            if self.ignore_jpeg {
                return image_url.to_owned();
            }
        } else if is_gif(image_url) && self.ignore_gif {
            return image_url.to_owned();
        }
        // `URLEncoder.encode(url, "utf-8")` leaves the same bytes as `byte_serialize`.
        let escaped: String = form_urlencoded::byte_serialize(image_url.as_bytes()).collect();
        format!(
            "{server}/?jpg={}&l={}&bw={}&url={escaped}",
            flag(self.format_jpeg),
            self.quality,
            flag(self.color_bw)
        )
    }

    fn wsrv_url(&self, image_url: &str) -> String {
        if is_jpeg(image_url) {
            if self.ignore_jpeg {
                return image_url.to_owned();
            }
        } else if is_gif(image_url) && self.ignore_gif {
            return image_url.to_owned();
        }
        // The raw URL goes into the query, as the Kotlin code does.
        let quality = self.quality;
        let suffix = if image_url.to_ascii_lowercase().contains(".webp") || is_gif(image_url) {
            // Animated images keep their format unless JPEG was asked for.
            if self.format_jpeg { format!("&output=jpg&q={quality}&n=-1") } else { format!("&q={quality}&n=-1") }
        } else if self.format_jpeg {
            format!("&output=jpg&q={quality}")
        } else {
            format!("&output=webp&q={quality}")
        };
        format!("https://wsrv.nl/?url={image_url}{suffix}")
    }
}

fn is_jpeg(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains(".jpeg") || lower.contains(".jpg")
}

fn is_gif(url: &str) -> bool {
    url.to_ascii_lowercase().contains(".gif")
}

fn flag(on: bool) -> u8 {
    u8::from(on)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(service: Service) -> DataSaver {
        DataSaver { service, ..DataSaver::default() }
    }

    #[test]
    fn the_default_is_off_so_the_original_url_is_fetched() {
        let saver = DataSaver::default();
        assert_eq!(saver.service, Service::None, "data_saver defaults to NONE");
        assert!(saver.downloader_enabled, "data_saver_downloader defaults to true");
        assert_eq!(saver.request_url(1, "https://cdn.example/1.jpg"), "https://cdn.example/1.jpg");
    }

    #[test]
    fn the_downloader_switch_excluded_sources_and_none_each_turn_the_service_off() {
        let url = "https://cdn.example/1.png";
        let mut saver = active(Service::WsrvNl);
        assert_ne!(saver.request_url(1, url), url, "a chosen service is used");
        saver.downloader_enabled = false;
        assert_eq!(saver.request_url(1, url), url, "data_saver_downloader off");
        saver.downloader_enabled = true;
        saver.excluded_sources.insert(7);
        assert_eq!(saver.request_url(7, url), url, "an excluded source is fetched directly");
        assert_ne!(saver.request_url(8, url), url, "other sources still use it");
        assert_eq!(active(Service::None).request_url(1, url), url);
    }

    #[test]
    fn wsrv_png_defaults_to_webp_with_the_quality() {
        assert_eq!(
            active(Service::WsrvNl).request_url(1, "https://cdn.example/1.png"),
            "https://wsrv.nl/?url=https://cdn.example/1.png&output=webp&q=80"
        );
    }

    #[test]
    fn wsrv_jpeg_is_converted_only_when_ignore_jpeg_is_off_and_format_is_jpeg() {
        let mut saver = active(Service::WsrvNl);
        saver.format_jpeg = true;
        assert_eq!(
            saver.request_url(1, "https://cdn.example/1.JPG"),
            "https://wsrv.nl/?url=https://cdn.example/1.JPG&output=jpg&q=80"
        );
        saver.ignore_jpeg = true;
        assert_eq!(saver.request_url(1, "https://cdn.example/1.jpg"), "https://cdn.example/1.jpg");
    }

    #[test]
    fn wsrv_animated_images_keep_their_format_and_add_n_minus_one() {
        let mut saver = active(Service::WsrvNl);
        assert_eq!(
            saver.request_url(1, "https://cdn.example/1.webp"),
            "https://wsrv.nl/?url=https://cdn.example/1.webp&q=80&n=-1"
        );
        saver.format_jpeg = true;
        assert_eq!(
            saver.request_url(1, "https://cdn.example/1.webp"),
            "https://wsrv.nl/?url=https://cdn.example/1.webp&output=jpg&q=80&n=-1"
        );
        saver.ignore_gif = false;
        saver.format_jpeg = false;
        assert_eq!(
            saver.request_url(1, "https://cdn.example/2.gif"),
            "https://wsrv.nl/?url=https://cdn.example/2.gif&q=80&n=-1",
            "a GIF is compressed when ignore_gif is off"
        );
    }

    #[test]
    fn bandwidth_hero_encodes_the_url_and_sends_the_flags() {
        let mut saver = active(Service::BandwidthHero);
        saver.server = "https://bw.example/".into();
        assert_eq!(
            saver.request_url(1, "https://cdn.example/1.png"),
            "https://bw.example/?jpg=0&l=80&bw=0&url=https%3A%2F%2Fcdn.example%2F1.png"
        );
        saver.format_jpeg = true;
        saver.color_bw = true;
        saver.quality = 50;
        assert_eq!(
            saver.request_url(1, "https://cdn.example/a b.png"),
            "https://bw.example/?jpg=1&l=50&bw=1&url=https%3A%2F%2Fcdn.example%2Fa+b.png",
            "a space is '+' as URLEncoder writes it"
        );
    }

    #[test]
    fn bandwidth_hero_leaves_urls_alone_without_a_server_or_when_already_proxied() {
        let mut saver = active(Service::BandwidthHero);
        assert_eq!(saver.request_url(1, "https://cdn.example/1.png"), "https://cdn.example/1.png", "blank server");
        saver.server = "https://bw.example".into();
        assert_eq!(
            saver.request_url(1, "https://bw.example/?url=x.png"),
            "https://bw.example/?url=x.png",
            "a URL that already goes through the server is kept"
        );
        assert_eq!(saver.request_url(1, "https://cdn.example/1.gif"), "https://cdn.example/1.gif", "ignore_gif is on");
        saver.ignore_jpeg = true;
        assert_eq!(saver.request_url(1, "https://cdn.example/1.jpg"), "https://cdn.example/1.jpg");
    }
}
