use std::{collections::HashMap, time::Duration};

use anyhow::Error;
use axum::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use tokio::sync::RwLock;

#[async_trait]
pub trait ShieldsIoFetcher {
    async fn fetch(&self, params: &ShieldsIoParams, views: u64) -> Result<String, Error>;
}

#[derive(Deserialize)]
pub struct ShieldsIoParams {
    label: String,
    color: String,
    style: String,
}

impl ShieldsIoParams {
    fn label(&self) -> &str {
        self.label.as_ref()
    }

    fn color(&self) -> &str {
        self.color.as_ref()
    }

    fn style(&self) -> &str {
        self.style.as_ref()
    }

    fn to_query_string_template(&self, views: u64) -> (String, String) {
        let padding = views.to_string().chars().map(|_| '*').collect::<String>();
        let query_string_template = format!(
            "label={}&color={}&style={}&message={}",
            self.label(),
            self.color(),
            self.style(),
            padding,
        );

        (query_string_template, padding)
    }
}

impl std::fmt::Display for ShieldsIoParams {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{{ label: {}, color: {}, style: {} }}",
            self.label(),
            self.color(),
            self.style()
        )
    }
}

pub struct Shields {
    client: reqwest::Client,
    service_url: String,
    cache: RwLock<HashMap<String, String>>,
}

impl Shields {
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

        Ok(Shields {
            client,
            service_url: "https://shields.io/static/v1".to_string(),
            cache: RwLock::new(HashMap::new()),
        })
    }

    async fn update_cache(&self, key: String, value: String) {
        let mut cache_writer = self.cache.write().await;

        // delete the old key if present; the old key will be having one less padding character than the new key
        let old_key: &str = &key.as_str()[..key.len() - 1];
        cache_writer.remove(old_key);
        tracing::info!("removed old key: {}", old_key);

        // insert the new key
        tracing::info!("inserting key: {}", key);
        cache_writer.insert(key, value);
    }
}

#[async_trait]
impl ShieldsIoFetcher for Shields {
    async fn fetch(&self, params: &ShieldsIoParams, views: u64) -> Result<String, Error> {
        let (query_params, padding) = params.to_query_string_template(views);

        let cache_reader = self.cache.read().await;
        if let Some(badge) = cache_reader.get(&query_params) {
            tracing::info!("cache hit, params: {}, views: {}", params, views);
            return Ok(badge.replace(&padding, views.to_string().as_str()));
        }

        drop(cache_reader); // dropping the read lock

        tracing::info!(
            "cache miss, fetching badge, params: {}, views: {}",
            params,
            views
        );
        let url = format!("{}?{}", self.service_url, query_params);
        let badge_template = self.client.get(url).send().await?.text().await?;

        let badge = badge_template.replace(&padding, views.to_string().as_str());
        self.update_cache(query_params, badge_template).await;

        Ok(badge)
    }
}
