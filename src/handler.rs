use super::datastore::PostgresDB;
use super::state::State;

use anyhow::Error;
use axum::{
    body::Bytes,
    extract::State as StateExtractor,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use reqwest::header::{HeaderMap, HeaderValue};

pub async fn profile_views_handler(
    StateExtractor(state): StateExtractor<State<PostgresDB>>,
) -> Response {
    let views = state.update().await;
    let url = format!(
        "https://shields.io/static/v1?label=Profile%20Views&message={}&color=brightgreen",
        views
    );

    match fetch_badge(&url).await {
        Ok(badge) => (
            // docs - https://docs.rs/axum/latest/axum/response/index.html
            StatusCode::OK,
            [
                (
                    "Cache-Control",
                    "max-age=0, no-cache, no-store, must-revalidate",
                ),
                ("Content-Type", "image/svg+xml"),
            ],
            badge,
        )
            .into_response(),
        Err(e) => {
            tracing::error!("failed to fetch badge from shields.io, reason: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn fetch_badge(url: &str) -> Result<Bytes, Error> {
    tracing::info!("badge url: {}", url);
    let mut headers = HeaderMap::new();
    headers.insert(
        "Cache-Control",
        HeaderValue::from_static("max-age=0, no-cache, no-store, must-revalidate"),
    );

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await?
        .bytes()
        .await?;

    Ok(response)
}
