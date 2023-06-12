use std::sync::Arc;

use axum::{
    extract::{Query, State as StateExtractor},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::badge::{Fetcher, ShieldsIOParams};
use super::datastore::Datastore;
use super::state::AppState;

pub async fn health_check_handler() -> Response {
    StatusCode::OK.into_response()
}

pub async fn profile_views_handler(
    StateExtractor(state): StateExtractor<Arc<AppState<impl Datastore, impl Fetcher>>>,
    query: Query<ShieldsIOParams>,
) -> Response {
    let views = state.update().await;
    let badge_params = format!(
        "label={}&message={}&color={}&style={}",
        query.label(),
        views,
        query.color(),
        query.style(),
    );

    match state.badge_fetcher.fetch(badge_params).await {
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
