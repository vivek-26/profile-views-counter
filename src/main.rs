mod datastore;
mod handler;
mod keepalive;
mod state;

use axum::{routing::get, Router};
use datastore::PostgresDB;
use dotenv::dotenv;
use state::State;
use std::{net::SocketAddr, sync::Arc};
use tokio::task;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    if std::env::var("PRODUCTION").is_err() {
        dotenv().ok();

        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .pretty()
                .with_env_filter(EnvFilter::from_default_env())
                .finish(),
        )
        .expect("failed to set global default subscriber");
    } else {
        tracing::subscriber::set_global_default(
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(EnvFilter::from_default_env())
                .with_target(false)
                .finish(),
        )
        .expect("failed to set global default subscriber");
    }

    let db_connection_str =
        std::env::var("DATABASE_URL").expect("missing env variable DATABASE_URL");

    // setup database connection pool
    let db = PostgresDB::new(&db_connection_str).await;

    // initialize state
    let state = Arc::new(State::initialize(db).await.unwrap());
    let state_clone = state.clone();

    // async thread to update profile views in database at regular intervals
    let _update_loop_handle = task::spawn(async move {
        state_clone.update_loop().await;
    });

    // build our application with some routes
    let app = Router::new()
        .route("/healthz", get(handler::health_check_handler))
        .route("/count.svg", get(handler::profile_views_handler))
        .with_state(state);

    let port = std::env::var("PORT")?
        .parse::<u16>()
        .expect("missing env variable PORT");
    let keep_alive_interval = std::env::var("KEEPALIVE_INTERVAL_SECS")?
        .parse::<u64>()
        .unwrap_or(600);
    let server_keep_alive = keepalive::KeepAlive::new(port, keep_alive_interval);

    // async thread to keep server alive by hitting health check route at regular intervals
    let _server_keep_alive_loop_handle = task::spawn(async move {
        server_keep_alive.health_check_loop().await;
    });

    // run it with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
