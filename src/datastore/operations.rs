use anyhow::Error;
use axum::async_trait;

#[async_trait]
pub trait Operations {
    async fn get_latest_views(&self, user_name: &str) -> Result<u64, Error>;
}
