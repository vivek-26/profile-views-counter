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
        let db_endpoint = std::env::var("XATA_DB_ENDPOINT")?;
        let api_key = std::env::var("XATA_API_KEY")?;
        let table_name = std::env::var("XATA_TABLE_NAME")?;

        // all request to xata.io will use bearer auth token
        let mut auth_header = HeaderMap::new();
        let mut auth_header_value = HeaderValue::from_str(&format!("Bearer {}", api_key))?;
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
    use pretty_assertions::assert_eq;
    use serde_json;
    use serial_test::serial;

    #[test]
    fn test_serialize_update_user_views_operation() {
        let serialized =
            serde_json::to_string(&test_helpers::user_views_transaction(OperationType::Update))
                .unwrap();

        let expected = format!(
            r#"{{"operations":[{{"update":{{"table":"{}","id":"{}","fields":{{"count":{{"$increment":1}}}},"columns":["count"]}}}}]}}"#,
            test_helpers::TEST_TABLE_NAME,
            test_helpers::TEST_USER_NAME
        );
        assert_eq!(serialized, expected);
    }

    #[test]
    fn test_serialize_insert_user_views_operation() {
        let serialized =
            serde_json::to_string(&test_helpers::user_views_transaction(OperationType::Insert))
                .unwrap();

        let expected = format!(
            r#"{{"operations":[{{"insert":{{"table":"{}","record":{{"count":1,"id":"{}"}},"createOnly":true,"columns":["count"]}}}}]}}"#,
            test_helpers::TEST_TABLE_NAME,
            test_helpers::TEST_USER_NAME
        );
        assert_eq!(serialized, expected);
    }

    #[tokio::test]
    #[serial]
    async fn it_gets_latest_views_for_onboarded_user() {
        let expected_count = 998 as u64;

        let mock = test_helpers::mock_xata_server()
            .match_body(
                format!(
                    r#"{{"operations":[{{"update":{{"table":"{}","id":"{}","fields":{{"count":{{"$increment":1}}}},"columns":["count"]}}}}]}}"#,
                    test_helpers::TEST_TABLE_NAME,
                    test_helpers::TEST_USER_NAME
                ).as_str(),
            )
            .with_status(200)
            .with_body(
                format!(r#"{{"results":[{{"columns":{{"count":{}}},"id":"{}","operation":"update","rows":1}}]}}"#, expected_count, test_helpers::TEST_USER_NAME
                ).as_str())
            .create_async().await;

        let count = Xata::new()
            .unwrap()
            .get_latest_views(test_helpers::TEST_USER_NAME)
            .await;

        mock.assert_async().await;
        assert_eq!(count.unwrap(), 998);
    }

    #[tokio::test]
    #[serial]
    async fn it_returns_user_not_found_error_for_non_onboarded_user() {
        let mock = test_helpers::mock_xata_server()
            .match_body(
                format!(
                    r#"{{"operations":[{{"update":{{"table":"{}","id":"{}","fields":{{"count":{{"$increment":1}}}},"columns":["count"]}}}}]}}"#,
                    test_helpers::TEST_TABLE_NAME,
                    test_helpers::TEST_USER_NAME
                ).as_str(),
            )
            .with_status(400)
            .with_body(
                format!(r#"{{"errors":[{{"index":0,"message":"table [{}]: record [{}] not found"}}]}}"#,
                        test_helpers::TEST_TABLE_NAME,
                        test_helpers::TEST_USER_NAME
                ).as_str(),
            )
            .create_async().await;

        let count = Xata::new()
            .unwrap()
            .get_latest_views(test_helpers::TEST_USER_NAME)
            .await;

        mock.assert_async().await;
        assert_eq!(
            count.unwrap_err().to_string(),
            DatastoreError::UserNotFound(test_helpers::TEST_USER_NAME.to_string()).to_string()
        );
    }

    #[tokio::test]
    #[serial]
    async fn it_handles_unexpected_error_while_fetching_latest_views() {
        let mock = test_helpers::mock_xata_server()
            .match_body(
                format!(
                    r#"{{"operations":[{{"update":{{"table":"{}","id":"{}","fields":{{"count":{{"$increment":1}}}},"columns":["count"]}}}}]}}"#,
                    test_helpers::TEST_TABLE_NAME,
                    test_helpers::TEST_USER_NAME
                ).as_str(),
            )
            .with_status(500)
            .with_body(r#"unavailable"#)
            .create_async().await;

        let count = Xata::new()
            .unwrap()
            .get_latest_views(test_helpers::TEST_USER_NAME)
            .await;

        mock.assert_async().await;
        assert_eq!(
            count.unwrap_err().to_string(),
            DatastoreError::Unexpected(
                r#"status code: 500 Internal Server Error, server error message: unavailable"#
                    .to_string()
            )
            .to_string()
        );
    }

    #[tokio::test]
    #[serial]
    async fn it_onboards_user_successfully() {
        let mock = test_helpers::mock_xata_server()
            .match_body(
                format!(
                    r#"{{"operations":[{{"insert":{{"table":"{}","record":{{"count":1,"id":"{}"}},"createOnly":true,"columns":["count"]}}}}]}}"#,
                    test_helpers::TEST_TABLE_NAME,
                    test_helpers::TEST_USER_NAME
                ).as_str(),
            )
            .with_status(200)
            .with_body(
                format!(r#"{{"results":[{{"columns":{{"count":1}},"id":"{}","operation":"insert","rows":1}}]}}"#, test_helpers::TEST_USER_NAME
                ).as_str())
            .create_async().await;

        let count = Xata::new()
            .unwrap()
            .onboard_user(test_helpers::TEST_USER_NAME)
            .await;

        mock.assert_async().await;
        assert_eq!(count.unwrap(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn it_handles_unexpected_error_while_onboarding_user() {
        let mock = test_helpers::mock_xata_server()
            .match_body(
                format!(
                    r#"{{"operations":[{{"insert":{{"table":"{}","record":{{"count":1,"id":"{}"}},"createOnly":true,"columns":["count"]}}}}]}}"#,
                    test_helpers::TEST_TABLE_NAME,
                    test_helpers::TEST_USER_NAME
                ).as_str(),
            )
            .with_status(500)
            .with_body(r#"unavailable"#)
            .create_async().await;

        let count = Xata::new()
            .unwrap()
            .onboard_user(test_helpers::TEST_USER_NAME)
            .await;

        mock.assert_async().await;
        assert_eq!(
            count.unwrap_err().to_string(),
            DatastoreError::Unexpected(
                r#"status code: 500 Internal Server Error, server error message: unavailable"#
                    .to_string()
            )
            .to_string()
        );
    }
}

#[cfg(test)]
mod test_helpers {
    use super::*;

    pub(crate) static TEST_TABLE_NAME: &str = "profile_views";
    pub(crate) static TEST_USER_NAME: &str = "test_user";
    pub(crate) static TEST_API_KEY: &str = "test_api_key";
    pub(crate) static TEST_DB_ENDPOINT_PATH: &str = "/v1/branch/test_branch/transaction";

    pub(crate) fn user_views_transaction(op: OperationType) -> XataTransaction<'static> {
        let metadata = TransactionMetadata {
            table: TEST_TABLE_NAME,
            user_name: TEST_USER_NAME,
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

    pub(crate) fn set_env_variables(db_endpoint: String) {
        std::env::set_var("XATA_API_KEY", TEST_API_KEY);
        std::env::set_var("XATA_TABLE_NAME", TEST_TABLE_NAME);
        std::env::set_var("XATA_DB_ENDPOINT", db_endpoint);
    }

    pub(crate) fn mock_xata_server() -> mockito::Mock {
        let mut server = mockito::Server::new();
        let url = format!("{}{}", server.url(), TEST_DB_ENDPOINT_PATH);
        set_env_variables(url.clone());

        let mock = server
            .mock("POST", TEST_DB_ENDPOINT_PATH)
            .match_header("Authorization", &*format!("Bearer {}", TEST_API_KEY));

        mock
    }
}
