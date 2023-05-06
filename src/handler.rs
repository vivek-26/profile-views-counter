use super::datastore::PostgresDB;
use super::fetcher::{BadgeFetcher, Fetcher};
use super::state::State;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Extension,
};
use std::sync::Arc;

pub async fn health_check_handler() -> Response {
    StatusCode::OK.into_response()
}

pub async fn profile_views_handler(
    state: Extension<Arc<State<PostgresDB>>>,
    badge_fetcher: Extension<Arc<BadgeFetcher>>,
) -> Response {
    let views = state.update().await;

    match badge_fetcher.get_badge(views.to_string()).await {
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
