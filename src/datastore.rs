use std::time::Duration;

use axum::async_trait;
use sqlx::{postgres::PgPoolOptions, PgPool};

#[async_trait]
pub trait Datastore {
    async fn get_views(&self) -> Result<i64, DataAccessError>;
    async fn update_views(&self, views: i64) -> Result<(), DataAccessError>;
    async fn close_connection(&self);
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct DataAccessError(#[from] sqlx::Error);

#[derive(Clone)]
pub struct PostgresDB(PgPool);

impl PostgresDB {
    pub async fn new(conn_str: &str) -> PostgresDB {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect(conn_str)
            .await
            .expect("cannot connect to database");

        tracing::info!("connected to database");

        PostgresDB(pool)
    }
}

#[async_trait]
impl Datastore for PostgresDB {
    async fn get_views(&self) -> Result<i64, DataAccessError> {
        let count: i64 = sqlx::query_scalar("SELECT count FROM profile_views")
            .fetch_one(&self.0)
            .await
            .map_err(DataAccessError::from)?;

        Ok(count)
    }

    async fn update_views(&self, views: i64) -> Result<(), DataAccessError> {
        sqlx::query("UPDATE profile_views SET count = $1 WHERE id = 1")
            .bind(views)
            .execute(&self.0)
            .await
            .map_err(DataAccessError::from)?;

        Ok(())
    }

    async fn close_connection(&self) {
        self.0.close().await;
    }
}
