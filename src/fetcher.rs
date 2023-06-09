use std::time::Duration;

use anyhow::Error;
use axum::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};

#[async_trait]
pub trait SheildsIO {
    async fn get_badge(&self, query_params: String) -> Result<String, Error>;
}

pub struct BadgeFetcher {
    client: reqwest::Client,
    badge_url: String,
}

impl BadgeFetcher {
    pub fn new() -> Result<Self, Error> {
        // default headers
        let mut headers = HeaderMap::new();
        headers.insert(
            "Cache-Control",
            HeaderValue::from_static("max-age=0, no-cache, no-store, must-revalidate"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .pool_max_idle_per_host(5)
            .pool_idle_timeout(Duration::from_secs(120))
            .timeout(Duration::from_secs(5))
            .build()?;

        Ok(BadgeFetcher {
            client,
            badge_url: "https://shields.io/static/v1".to_string(),
        })
    }
}

#[async_trait]
impl SheildsIO for BadgeFetcher {
    async fn get_badge(&self, query_params: String) -> Result<String, Error> {
        let url = format!("{}?{}", self.badge_url, query_params);

        tracing::info!("fetching badge, params: {}", query_params);
        let response = self.client.get(url).send().await?.text().await?;

        Ok(response)
    }
}
