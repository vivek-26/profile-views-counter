use std::time::Duration;

use anyhow::Error;
use axum::async_trait;
use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Response, StatusCode,
};
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::{DatastoreError, DatastoreOperations};

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

    async fn handle_unexpected_error(&self, response: Response) -> DatastoreError {
        let status_code = response.status();
        let server_error_msg = response.text().await.unwrap_or_else(|_| "none".to_string());
        DatastoreError::Unexpected(format!(
            "status code: {}, server error message: {}",
            status_code, server_error_msg
        ))
    }
}

#[derive(Clone)]
pub(crate) enum OperationType {
    Update,
    Insert,
}

struct TransactionMetadata<'txn> {
    table: &'txn str,
    user_name: &'txn str,
    op_type: OperationType,
}

struct UserViewsOperation<'txn> {
    metadata: TransactionMetadata<'txn>,
}

impl<'txn> Serialize for UserViewsOperation<'txn> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut operations = serializer.serialize_map(None)?;
        operations.serialize_entry("table", &self.metadata.table)?;
        match self.metadata.op_type {
            OperationType::Update => {
                operations.serialize_entry("id", &self.metadata.user_name)?;
                operations.serialize_entry(
                    "fields",
                    &serde_json::json!({ "count": { "$increment": 1 } }),
                )?;
            }
            OperationType::Insert => {
                operations.serialize_entry(
                    "record",
                    &serde_json::json!({ "id": &self.metadata.user_name, "count": 1 }),
                )?;
                operations.serialize_entry("createOnly", &true)?;
            }
        }
        operations.serialize_entry("columns", &serde_json::json!(["count"]))?;
        operations.end()
    }
}

#[derive(Serialize)]
enum Operations<'txn> {
    #[serde(rename = "update")]
    Update(UserViewsOperation<'txn>),

    #[serde(rename = "insert")]
    Insert(UserViewsOperation<'txn>),
}

#[derive(Serialize)]
pub(crate) struct XataTransaction<'txn> {
    operations: [Operations<'txn>; 1],
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
            .get(0)
            .and_then(|result| result["columns"].get("count"))
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                serde::de::Error::custom(format_args!(
                    "failed to deserialize server response: {}",
                    value
                ))
            })?;

        Ok(ProfileViews { count })
    }
}

#[derive(Debug, Deserialize)]
struct TransactionError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct XataTransactionError {
    errors: [TransactionError; 1],
}

#[async_trait]
impl DatastoreOperations for Xata {
    async fn get_latest_views(&self, user_name: &str) -> Result<u64, DatastoreError> {
        let metadata = TransactionMetadata {
            table: self.table_name.as_str(),
            user_name,
            op_type: OperationType::Update,
        };

        let transaction = XataTransaction {
            operations: [Operations::Update(UserViewsOperation { metadata })],
        };

        let update_txn_resp = self
            .client
            .post(self.db_endpoint.as_str())
            .json(&transaction)
            .send()
            .await
            .map_err(DatastoreError::Client)?;

        // xata returns 400 if transaction fails with some error.
        // reference - https://xata.io/docs/api-reference/db/db_branch_name/transaction#execute-a-transaction-on-a-branch
        match update_txn_resp.status() {
            StatusCode::OK => {
                let count = update_txn_resp
                    .json::<ProfileViews>()
                    .await
                    .map_err(DatastoreError::Client)?
                    .count;

                Ok(count)
            }
            StatusCode::BAD_REQUEST => {
                let txn_error_resp = update_txn_resp
                    .json::<XataTransactionError>()
                    .await
                    .map_err(DatastoreError::Client)?;

                let txn_error = txn_error_resp
                    .errors
                    .iter()
                    .find(|err| {
                        err.message.contains(user_name) && err.message.contains("not found")
                    })
                    .map(|_| Err(DatastoreError::UserNotFound(user_name.to_string())))
                    .unwrap_or_else(|| {
                        Err(DatastoreError::Unexpected(format!(
                            "failed to update count for user: `{}`, error: {:?}",
                            user_name, txn_error_resp
                        )))
                    });

                txn_error
            }
            _ => Err(self.handle_unexpected_error(update_txn_resp).await),
        }
    }

    async fn onboard_user(&self, user_name: &str) -> Result<u64, DatastoreError> {
        let metadata = TransactionMetadata {
            table: self.table_name.as_str(),
            user_name,
            op_type: OperationType::Insert,
        };

        let transaction = XataTransaction {
            operations: [Operations::Insert(UserViewsOperation { metadata })],
        };

        let insert_txn_resp = self
            .client
            .post(self.db_endpoint.as_str())
            .json(&transaction)
            .send()
            .await
            .map_err(DatastoreError::Client)?;

        match insert_txn_resp.status() {
            StatusCode::OK => {
                let count = insert_txn_resp
                    .json::<ProfileViews>()
                    .await
                    .map_err(DatastoreError::Client)?
                    .count;

                Ok(count)
            }
            _ => Err(self.handle_unexpected_error(insert_txn_resp).await),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_serialize_update_user_views_operation() {
        let serialized =
            serde_json::to_string(&test_helper::user_views_transaction(OperationType::Update));
        assert!(serialized.is_ok());

        let expected = r#"{"operations":[{"update":{"table":"profile_views","id":"test_user","fields":{"count":{"$increment":1}},"columns":["count"]}}]}"#;
        assert_eq!(serialized.unwrap(), expected);
    }

    #[test]
    fn test_serialize_insert_user_views_operation() {
        let serialized =
            serde_json::to_string(&test_helper::user_views_transaction(OperationType::Insert));
        assert!(serialized.is_ok());

        let expected = r#"{"operations":[{"insert":{"table":"profile_views","record":{"count":1,"id":"test_user"},"createOnly":true,"columns":["count"]}}]}"#;
        assert_eq!(serialized.unwrap(), expected);
    }
}

#[cfg(test)]
mod test_helper {
    use super::*;

    pub(crate) fn user_views_transaction(op: OperationType) -> XataTransaction<'static> {
        let metadata = TransactionMetadata {
            table: "profile_views",
            user_name: "test_user",
            op_type: op.clone(),
        };

        match op {
            OperationType::Update => XataTransaction {
                operations: [Operations::Update(UserViewsOperation { metadata })],
            },
            OperationType::Insert => XataTransaction {
                operations: [Operations::Insert(UserViewsOperation { metadata })],
            },
        }
    }
}
