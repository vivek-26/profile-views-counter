use std::sync::Arc;

use axum::{
    extract::State as StateExtractor,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::datastore::Datastore;
use super::fetcher::Fetcher;
use super::state::State;

pub async fn health_check_handler() -> Response {
    StatusCode::OK.into_response()
}

pub async fn profile_views_handler(
    StateExtractor(state): StateExtractor<Arc<State<impl Datastore, impl Fetcher>>>,
) -> Response {
    let views = state.update().await;

    match state.badge_fetcher.get_badge(views.to_string()).await {
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
