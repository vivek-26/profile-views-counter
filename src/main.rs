mod datastore;
mod state;

use anyhow::Error;
use axum::{
    body::Bytes,
    extract::State as StateExtractor,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use datastore::PostgresDB;
use dotenv::dotenv;
use reqwest::header::{HeaderMap, HeaderValue};
use state::State;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // load environment variables from .env file if we are running locally
    if std::env::var("PRODUCTION").is_err() {
        dotenv().ok();
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "example_tokio_postgres=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db_connection_str = std::env::var("DATABASE_URL").unwrap();

    // setup database connection pool
    let db = PostgresDB::new(&db_connection_str).await;

    // initialize state
    let state = State::initialize(db).await.unwrap();

    // build our application with some routes
    let app = Router::new()
        .route("/count.svg", get(handler))
        .with_state(state);

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn handler(StateExtractor(state): StateExtractor<State<PostgresDB>>) -> Response {
    let views = state.update();
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
