//! The one place a recipe touches the network: requests go to the hosts of `base_urls` only, one at a time, with the
//! recipe's headers and at least `rate_limit_ms` between them.

use std::sync::Arc;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue};
use kuro_net::NetworkClient;
use kuro_source::SourceError;
use kuro_source_http::http_source::{HttpCore, default_headers};
use kuro_source_http::response::HttpResponse;
use kuro_source_http::{map_net_error, request};
use tokio::sync::Mutex;
use tokio::time::Instant;
use url::Url;

use crate::recipe::{Recipe, RecipeError};

pub(crate) fn outside(err: RecipeError) -> SourceError {
    SourceError::Url(err.to_string())
}

pub struct Fetcher {
    recipe: Arc<Recipe>,
    core: HttpCore,
    headers: HeaderMap,
    interval: Duration,
    last_done: Mutex<Option<Instant>>,
}

impl Fetcher {
    /// The headers are the app's default ones with the recipe's on top, so a recipe can replace the user agent.
    pub fn new(client: NetworkClient, recipe: Arc<Recipe>) -> Self {
        let core = HttpCore::new(client);
        let mut headers = default_headers(&core);
        for (name, value) in &recipe.headers {
            if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
                headers.insert(name, value);
            }
        }
        let interval = Duration::from_millis(recipe.rate_limit_ms);
        Self { recipe, core, headers, interval, last_done: Mutex::new(None) }
    }

    pub fn recipe(&self) -> &Recipe {
        &self.recipe
    }

    /// The headers a player or an image loader should send with the files of this site: the recipe's own, without the
    /// user agent.
    pub fn file_headers(&self) -> Vec<(String, String)> {
        self.recipe.headers.iter().filter(|(name, _)| !name.eq_ignore_ascii_case("user-agent")).map(|(n, v)| (n.clone(), v.clone())).collect()
    }

    /// The id of the source (name, language and version, as `generate_id` makes it).
    pub fn id(&self) -> i64 {
        self.core.id(&self.recipe.name, &self.recipe.lang, self.recipe.version)
    }

    /// Fetches a page. The address is checked before the request and the final address after it (a redirect to another
    /// host is an error, though the request has then been made). Requests of one source never overlap.
    pub async fn get(&self, url: &Url) -> Result<(String, Url), SourceError> {
        self.recipe.check_host(url).map_err(outside)?;
        let request = request::get(url.as_str(), &self.headers)?;
        let mut last = self.last_done.lock().await;
        if let Some(done) = *last {
            tokio::time::sleep_until(done + self.interval).await;
        }
        let outcome = self.core.client().execute(request).await.map_err(map_net_error);
        let response = match outcome {
            Ok(response) => HttpResponse::read(response).await,
            Err(err) => Err(err),
        };
        *last = Some(Instant::now());
        drop(last);
        let response = response?;
        self.recipe.check_host(&response.url).map_err(outside)?;
        Ok((response.text(), response.url))
    }
}
