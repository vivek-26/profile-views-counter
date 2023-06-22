use std::time::Duration;

use anyhow::Error;
use axum::async_trait;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::DatastoreOperations;

pub struct Xata {
    client: reqwest::Client,
    db_endpoint: String,
    table_name: String,
}

impl Xata {
    pub fn new() -> Result<Xata, Error> {
        let db_endpoint = std::env::var("XATA_DB_ENDPOINT")
            .expect("missing XATA_TABLE_ENDPOINT environment variable");

        let auth_token =
            std::env::var("XATA_AUTH_TOKEN").expect("missing XATA_AUTH_TOKEN environment variable");

        let table_name =
            std::env::var("XATA_TABLE_NAME").expect("missing XATA_TABLE_NAME environment variable");

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
            db_endpoint,
            table_name,
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

struct TransactionMetadata<'txn> {
    table: &'txn str,
    user_name: &'txn str,
}

struct UpdateCountOperation<'txn> {
    metadata: &'txn TransactionMetadata<'txn>,
}

impl<'txn> Serialize for UpdateCountOperation<'txn> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut operations = serializer.serialize_map(None)?;
        operations.serialize_entry("table", &self.metadata.table)?;
        operations.serialize_entry("id", &self.metadata.user_name)?;
        operations.serialize_entry(
            "fields",
            &serde_json::json!({ "count": { "$increment": 1 } }),
        )?;
        operations.end()
    }
}

struct GetCountOperation<'txn> {
    metadata: &'txn TransactionMetadata<'txn>,
}

impl<'txn> Serialize for GetCountOperation<'txn> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut operations = serializer.serialize_map(None)?;
        operations.serialize_entry("table", &self.metadata.table)?;
        operations.serialize_entry("id", &self.metadata.user_name)?;
        operations.serialize_entry("columns", &serde_json::json!(["count"]))?;
        operations.end()
    }
}

#[derive(Serialize)]
enum Operations<'txn> {
    #[serde(rename = "update")]
    Update(&'txn UpdateCountOperation<'txn>),

    #[serde(rename = "get")]
    Get(&'txn GetCountOperation<'txn>),
}

#[derive(Serialize)]
struct XataTransaction<'txn> {
    operations: Vec<&'txn Operations<'txn>>,
}

struct ProfileViews {
    count: u64,
}

impl<'de> Deserialize<'de> for ProfileViews {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;

        let count = value["results"]
            .get(1)
            .and_then(|result| result["columns"].get("count"))
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                tracing::error!("failed to deserialize server response: {}", value);
                serde::de::Error::custom("failed to deserialize server response")
            })?;

        Ok(ProfileViews { count })
    }
}

#[async_trait]
impl DatastoreOperations for Xata {
    async fn get_latest_views(&self, user_name: &str) -> Result<u64, Error> {
        let metadata = TransactionMetadata {
            table: self.table_name.as_str(),
            user_name,
        };

        let update_operation = UpdateCountOperation {
            metadata: &metadata,
        };
        let get_operation = GetCountOperation {
            metadata: &metadata,
        };

        let update = Operations::Update(&update_operation);
        let get = Operations::Get(&get_operation);

        let transaction = XataTransaction {
            operations: vec![&update, &get],
        };

        let response = self
            .client
            .post(self.db_endpoint.as_str())
            .json(&transaction)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(response).await);
        }

        let count = response.json::<ProfileViews>().await?.count;
        Ok(count)
    }
}
