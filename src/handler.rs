use std::sync::Arc;

use axum::{
    extract::{Path, Query, State as StateExtractor},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use super::badge::{ShieldsIoFetcher, ShieldsIoParams};
use super::datastore::DatastoreOperations;
use super::state::AppState;

#[derive(Deserialize)]
pub struct PathParams {
    user_name: String,
}

pub async fn health_check_handler() -> Response {
    StatusCode::OK.into_response()
}

pub async fn profile_views_handler(
    StateExtractor(state): StateExtractor<
        Arc<AppState<impl DatastoreOperations, impl ShieldsIoFetcher>>,
    >,
    query: Query<ShieldsIoParams>,
    path_params: Path<PathParams>,
) -> Response {
    let views = match state.db.get_latest_views(&path_params.user_name).await {
        Ok(views) => views,
        Err(e) => {
            tracing::error!("failed to fetch views from database, reason: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match state.badge.fetch(&query, views).await {
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
