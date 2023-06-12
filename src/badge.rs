use std::time::Duration;

use anyhow::Error;
use axum::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;

#[async_trait]
pub trait Fetcher {
    async fn fetch(&self, query_params: String) -> Result<String, Error>;
}

pub struct ShieldsIO {
    client: reqwest::Client,
    service_url: String,
}

#[derive(Deserialize)]
pub struct ShieldsIOParams {
    label: String,
    color: String,
    style: String,
}

impl ShieldsIOParams {
    pub fn label(&self) -> &str {
        self.label.as_ref()
    }

    pub fn color(&self) -> &str {
        self.color.as_ref()
    }

    pub fn style(&self) -> &str {
        self.style.as_ref()
    }
}

impl ShieldsIO {
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

        Ok(ShieldsIO {
            client,
            service_url: "https://shields.io/static/v1".to_string(),
        })
    }
}

#[async_trait]
impl Fetcher for ShieldsIO {
    async fn fetch(&self, query_params: String) -> Result<String, Error> {
        let url = format!("{}?{}", self.service_url, query_params);

        tracing::info!("fetching badge, params: {}", query_params);
        let response = self.client.get(url).send().await?.text().await?;

        Ok(response)
    }
}
