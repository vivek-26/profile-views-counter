use axum::async_trait;

#[async_trait]
pub trait Operations {
    async fn get_latest_views(&self, user_name: &str) -> Result<u64, Error>;
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Client(#[from] reqwest::Error),

    #[error("user `{0}` not found")]
    UserNotFound(String),

    #[error("unexpected error: {0}")]
    Unexpected(String),
}
