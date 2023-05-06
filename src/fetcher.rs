use anyhow::Error;
use axum::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use std::time::Duration;

#[async_trait]
pub trait Fetcher {
    async fn get_badge(&self, message: String) -> Result<String, Error>;
}

pub struct BadgeFetcher {
    client: reqwest::Client,
    badge_url: String,
    label: String,
    color: String,
}

impl BadgeFetcher {
    pub fn new() -> Self {
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
            .build()
            .expect("failed to create http client for fetching badges");

        BadgeFetcher {
            client,
            badge_url: "https://shields.io/static/v1".to_string(),
            label: "Profile%20Views".to_string(),
            color: "brightgreen".to_string(),
        }
    }
}

#[async_trait]
impl Fetcher for BadgeFetcher {
    async fn get_badge(&self, message: String) -> Result<String, Error> {
        let url = format!(
            "{}?label={}&message={}&color={}",
            self.badge_url, self.label, message, self.color
        );

        tracing::info!("fetching badge, message: {}", message);
        let response = self.client.get(url).send().await?.text().await?;

        Ok(response)
    }
}
