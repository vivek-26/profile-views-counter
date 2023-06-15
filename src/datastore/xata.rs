use std::time::Duration;

use anyhow::Error;
use axum::async_trait;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

use super::DatastoreOperations;

pub struct Xata {
    client: reqwest::Client,
    table_endpoint: String,
    record_id: String,
}

macro_rules! noop {
    () => {};
}

#[derive(Serialize, Deserialize)]
struct ProfileViews {
    count: i64,
}

impl Xata {
    pub fn new() -> Result<Xata, Error> {
        let table_endpoint = std::env::var("XATA_TABLE_ENDPOINT")
            .expect("missing XATA_TABLE_ENDPOINT environment variable");

        let record_id =
            std::env::var("XATA_RECORD_ID").expect("missing XATA_RECORD_ID environment variable");

        let auth_token =
            std::env::var("XATA_AUTH_TOKEN").expect("missing XATA_AUTH_TOKEN environment variable");

        // all request to xata.io will use bearer auth token
        let mut auth_header = HeaderMap::new();
        let mut auth_header_value = HeaderValue::from_str(&format!("Bearer {}", auth_token))?;
        auth_header_value.set_sensitive(true);
        auth_header.insert(header::AUTHORIZATION, auth_header_value);

        let client = reqwest::Client::builder()
            .default_headers(auth_header)
            .pool_max_idle_per_host(5)
            .pool_idle_timeout(Duration::from_secs(120))
            .timeout(Duration::from_secs(5))
            .build()?;

        Ok(Xata {
            client,
            table_endpoint,
            record_id,
        })
    }

    async fn handle_error_response(&self, response: reqwest::Response) -> Error {
        let status_code = response.status();
        let server_error_msg = response.text().await.unwrap_or_else(|_| "none".to_string());
        Error::msg(format!(
            "status code -> {}, server error message -> {}",
            status_code, server_error_msg
        ))
    }
}

#[async_trait]
impl DatastoreOperations for Xata {
    async fn get_views(&self) -> Result<i64, Error> {
        let url = format!("{}/data/{}", self.table_endpoint, self.record_id);
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let count = response.json::<ProfileViews>().await?.count;
        Ok(count)
    }

    async fn update_views(&self, views: i64) -> Result<(), Error> {
        let url = format!("{}/data/{}", self.table_endpoint, self.record_id);
        let response = self
            .client
            .patch(url)
            .json(&ProfileViews { count: views })
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        Ok(())
    }

    async fn close_connection(&self) {
        noop!();
    }
}
