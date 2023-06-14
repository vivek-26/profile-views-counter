use anyhow::Error;
use axum::async_trait;

#[async_trait]
pub trait Operations {
    async fn get_views(&self) -> Result<i64, Error>;
    async fn update_views(&self, views: i64) -> Result<(), Error>;
    async fn close_connection(&self);
}
